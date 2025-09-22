mod client;
mod config;
mod keygen;
mod signing;

use log::info;

use cggmp21::KeyShare;
use cggmp21::security_level::SecurityLevel128;
use cggmp21::supported_curves::Secp256k1;
use proto::mpc::participant_server::{Participant, ParticipantServer};
use proto::mpc::{
    Chain, CreateWalletMessage, DeleteWalletMessage, Empty, SignMessage, SignatureMessage,
};
use tonic::{Request, Response, Status, transport::Server};
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};
use vaultrs::kv2;

use client::Client;
use config::AppConfig;
use keygen::Keygen;
use signing::Signing;

pub struct ParticipantHandler {
    client: Client,
    vault: VaultClient,
    index: u16,
}

impl ParticipantHandler {
    pub fn new(client: Client, vault: VaultClient, index: u16) -> Self {
        Self {
            client,
            vault,
            index,
        }
    }
}

#[tonic::async_trait]
impl Participant for ParticipantHandler {
    async fn new_wallet(
        &self,
        request: Request<CreateWalletMessage>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();

        let wallet_id = req.wallet_id;
        let execution_id = req.execution_id;
        let chain = Chain::try_from(req.chain).map_err(|_| Status::internal("Invalid chain"))?;

        let keygen = Keygen::new(&self.client, wallet_id);

        let share = match chain {
            Chain::Ethereum => keygen.compute_share::<Secp256k1>(self.index, &execution_id),
            Chain::Bitcoin => keygen.compute_share::<Secp256k1>(self.index, &execution_id),
        }
        .await
        .map_err(|err| {
            log::error!("Share computation failed: {err}");
            Status::internal("Failed to create new wallet")
        })?;

        kv2::set(&self.vault, "secret", &wallet_id.to_string(), &share)
            .await
            .map_err(|_| Status::internal("Failed to store new wallet"))?;

        Ok(Response::new(Empty {}))
    }

    async fn delete_wallet(
        &self,
        request: Request<DeleteWalletMessage>,
    ) -> Result<Response<Empty>, Status> {
        let wallet_id = request.into_inner().wallet_id;

        info!("Deleting wallet - wallet_id: {}", wallet_id);

        kv2::delete_metadata(&self.vault, "secret", &wallet_id.to_string())
            .await
            .map_err(|_| Status::internal("Failed to delete wallet"))?;

        info!("Wallet deleted successfully - wallet_id: {}", wallet_id);

        Ok(Response::new(Empty {}))
    }

    async fn sign_tx(
        &self,
        request: Request<SignMessage>,
    ) -> Result<Response<SignatureMessage>, Status> {
        let req = request.into_inner();

        let tx_id = req.tx_id;
        let wallet_id = req.wallet_id.to_string();
        let execution_id = req.execution_id;
        let chain = Chain::try_from(req.chain).map_err(|_| Status::internal("Invalid chain"))?;
        let tx = req.data;

        let signign = Signing::new(&self.client, tx_id);

        let key = match chain {
            Chain::Ethereum => kv2::read::<KeyShare<Secp256k1, SecurityLevel128>>(
                &self.vault,
                "secret",
                &wallet_id,
            ),
            Chain::Bitcoin => kv2::read::<KeyShare<Secp256k1, SecurityLevel128>>(
                &self.vault,
                "secret",
                &wallet_id,
            ),
        }
        .await
        .map_err(|_| Status::internal("Wallet not found"))?;

        let (r, s, v) = signign
            .sign_tx(self.index, &execution_id, &tx, key, chain)
            .await
            .map_err(|_| Status::internal("Transaction signing failed"))?;

        Ok(Response::new(SignatureMessage { r, s, v }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    info!("Starting MPC participant service");

    let config = AppConfig::from_env()?;

    let server_url = surf::Url::parse(&config.sse_url())?;

    let client = Client::new(server_url)?;

    info!("Connecting to Vault at: {}", config.vault.address);

    let vault = VaultClient::new(
        VaultClientSettingsBuilder::default()
            .address(&config.vault.address)
            .token(&config.vault.token)
            .build()?,
    )?;

    info!("Successfully connected to Vault");

    let addr = config.participant_addr().parse()?;

    let p = ParticipantHandler::new(client, vault, config.participant.index);

    info!("Starting gRPC server on address: {}", addr);

    Server::builder()
        .add_service(ParticipantServer::new(p))
        .serve(addr)
        .await?;

    info!("MPC participant service stopped");

    Ok(())
}
