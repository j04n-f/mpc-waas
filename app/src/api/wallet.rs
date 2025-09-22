use crate::db::models::{Chain, TransactionActiveModel, WalletActiveModel, WalletModel};
use crate::db::repositories::{TransactionRepository, WalletRepository};
use crate::utils::request::request_user_id;
use actix_web::{
    HttpRequest, HttpResponse, Result,
    error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound},
    web,
};
use alloy::primitives::{Address, U256, Uint};
use alloy::providers::Provider;
use alloy_rlp::{Encodable, RlpDecodable, RlpEncodable};
use futures::future::join_all;
use proto::mpc::participant_client::ParticipantClient;
use proto::mpc::{CreateWalletMessage, DeleteWalletMessage, SignMessage};
use sea_orm::{DatabaseConnection, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use tonic::transport::Channel;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateWalletRequest {
    pub name: String,
    pub chain: Chain,
}

#[derive(Deserialize)]
pub struct TransactionRequest {
    pub to: Address,
    pub value: Uint<256, 4>,
}

#[derive(Serialize)]
#[allow(dead_code)]
pub struct WalletResponse {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub chain: Chain,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub id: i32,
    pub hash: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl From<WalletModel> for WalletResponse {
    fn from(val: WalletModel) -> Self {
        WalletResponse {
            id: val.id,
            user_id: val.user_id,
            name: val.name,
            chain: val.chain,
        }
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("").route(web::post().to(create_wallet)))
        .service(web::resource("/{id}").route(web::delete().to(delete_wallet)))
        .service(web::resource("/{id}/tx").route(web::post().to(send_tx)));
}

pub async fn create_wallet(
    req: HttpRequest,
    data: web::Json<CreateWalletRequest>,
    db: web::Data<DatabaseConnection>,
    participants: web::Data<Vec<Channel>>,
) -> Result<HttpResponse> {
    let user_id = request_user_id(&req)?;

    // Revert transaction on keygen failure
    // TODO: Add a clean up mechanism for partially created wallets
    let txn = db
        .begin()
        .await
        .map_err(|_| ErrorInternalServerError("Failed to create wallet"))?;

    let repository = WalletRepository::new_with_transaction(&txn);

    let wallet = repository
        .create(WalletActiveModel {
            user_id: Set(user_id),
            name: Set(data.name.clone()),
            chain: Set(data.chain.clone()),
            ..Default::default()
        })
        .await
        .map_err(|_| ErrorInternalServerError("Failed to create wallet"))?;

    // Must be unique for all participants
    let execution_id = Uuid::new_v4();

    let futures = participants.iter().map(|p| {
        let mut client = ParticipantClient::new(p.clone());
        let request_clone = tonic::Request::new(CreateWalletMessage {
            wallet_id: wallet.id,
            chain: data.chain.clone().into(),
            execution_id: execution_id.as_bytes().to_vec(),
        });

        async move {
            client.new_wallet(request_clone).await.map_err(|err| {
                log::error!("Failed to create wallet on participant: {err}");
                ErrorInternalServerError("Failed to create wallet")
            })
        }
    });

    let is_created = join_all(futures).await.iter().all(|res| res.is_ok());

    if is_created {
        txn.commit()
            .await
            .map_err(|_| ErrorInternalServerError("Failed to create wallet"))?;

        Ok(HttpResponse::Created().json(wallet))
    } else {
        txn.rollback()
            .await
            .map_err(|_| ErrorInternalServerError("Failed to create wallet"))?;

        Ok(HttpResponse::InternalServerError().finish())
    }
}

pub async fn delete_wallet(
    req: HttpRequest,
    path: web::Path<i32>,
    db: web::Data<DatabaseConnection>,
    participants: web::Data<Vec<Channel>>,
) -> Result<HttpResponse> {
    let user_id = request_user_id(&req)?;

    let wallet_id = path.into_inner();

    // Revert transaction on keygen failure
    // to be sure no dangling wallets exist
    let txn = db
        .begin()
        .await
        .map_err(|_| ErrorInternalServerError("Failed to delete wallet"))?;

    let repository = WalletRepository::new_with_transaction(&txn);

    let wallet = repository
        .find_by_id(wallet_id)
        .await
        .map_err(|_| ErrorInternalServerError("Failed to delete wallet"))?;

    let wallet = match wallet {
        Some(w) if w.user_id == user_id => Ok(w),
        _ => Err(ErrorNotFound("Wallet not found")),
    }?;

    let futures = participants.iter().map(|p| {
        let mut client = ParticipantClient::new(p.clone());
        let request_clone = tonic::Request::new(DeleteWalletMessage {
            wallet_id: wallet.id,
        });

        async move { client.delete_wallet(request_clone).await }
    });

    let is_deleted = join_all(futures).await.iter().all(|res| res.is_ok());

    if is_deleted {
        repository
            .delete(wallet_id)
            .await
            .map_err(|_| ErrorInternalServerError("Failed to delete wallet"))?;

        txn.commit()
            .await
            .map_err(|_| ErrorInternalServerError("Failed to delete wallet"))?;

        Ok(HttpResponse::NoContent().finish())
    } else {
        txn.rollback()
            .await
            .map_err(|_| ErrorInternalServerError("Failed to delete wallet"))?;

        Ok(HttpResponse::InternalServerError().finish())
    }
}

#[derive(Debug, RlpEncodable, RlpDecodable)]
struct RawTransaction {
    nonce: u64,
    gas_price: u64,
    gas_limit: u64,
    to: Address,
    value: U256,
    data: Vec<u8>,
}

#[derive(Debug, RlpEncodable, RlpDecodable)]
struct SignedTransaction {
    nonce: u64,
    gas_price: u64,
    gas_limit: u64,
    to: Address,
    value: U256,
    data: Vec<u8>,
    v: u32,
    r: U256,
    s: U256,
}

pub async fn send_tx(
    req: HttpRequest,
    data: web::Json<TransactionRequest>,
    db: web::Data<DatabaseConnection>,
    provider: web::Data<dyn Provider + Send + Sync>,
    participants: web::Data<Vec<Channel>>,
    path: web::Path<i32>,
) -> Result<HttpResponse> {
    let user_id = request_user_id(&req)?;
    let wallet_id = path.into_inner();

    let txn = db.begin().await.map_err(|_| ErrorInternalServerError(""))?;

    let wallet_repository = WalletRepository::new_with_transaction(&txn);
    let transaction_repository = TransactionRepository::new_with_transaction(&txn);

    let wallet = wallet_repository
        .find_by_id(wallet_id)
        .await
        .map_err(|_| ErrorInternalServerError("Failed to retrive the wallet"))?;

    let wallet = match wallet {
        Some(w) if w.user_id == user_id => Ok(w),
        _ => Err(ErrorNotFound("Wallet not found")),
    }?;

    let transaction_model = transaction_repository
        .create(TransactionActiveModel {
            user_id: Set(user_id),
            wallet_id: Set(wallet_id),
            ..Default::default()
        })
        .await
        .map_err(|_| ErrorInternalServerError(""))?;

    let tx_data = match wallet.chain {
        Chain::Ethereum => {
            // TODO: Fetch nonce from provider to avoid replay attacks
            // TODO: Allow custom gas price, gas limit, data
            let unsigned_tx = RawTransaction {
                nonce: 10,
                gas_price: 1000000000u64,
                gas_limit: 21000u64,
                to: data.to,
                value: U256::from(data.value),
                data: Vec::new(),
            };

            let mut rlp_buf = Vec::new();

            unsigned_tx.encode(&mut rlp_buf);

            Ok(rlp_buf)
        }
        _ => Err(ErrorBadRequest("Chain not supported")),
    }?;

    // Must be unique for all participants
    let execution_id = Uuid::new_v4();

    // Threshold equal to 2 participants for now
    let futures = participants.iter().take(2).map(|p| {
        let mut client = ParticipantClient::new(p.clone());
        let request_clone = tonic::Request::new(SignMessage {
            tx_id: transaction_model.id,
            wallet_id,
            execution_id: execution_id.as_bytes().to_vec(),
            chain: wallet.chain.clone().into(),
            data: tx_data.clone(),
        });

        async move { client.sign_tx(request_clone).await }
    });

    let results = join_all(futures).await;

    let is_signed = results.iter().all(|res| res.is_ok());

    let mut signature = None;

    if is_signed && let Some(Ok(response)) = results.first() {
        let s = response.get_ref();
        signature = Some((s.r.clone(), s.s.clone(), s.v));
    }

    if let Some((r, s, v)) = signature {
        txn.commit()
            .await
            .map_err(|_| ErrorInternalServerError("Failed to sign transaction"))?;

        let tx_hash = match wallet.chain {
            Chain::Ethereum => {
                // TODO: Fetch nonce from provider to avoid replay attacks
                // TODO: Allow custom gas price, gas limit, data
                let signed_tx = SignedTransaction {
                    nonce: 10,
                    gas_price: 1000000000u64,
                    gas_limit: 21000u64,
                    to: data.to,
                    value: U256::from(data.value),
                    data: Vec::new(),
                    v,
                    r: U256::from_be_slice(&r),
                    s: U256::from_be_slice(&s),
                };

                let mut rlp_buf = Vec::new();

                signed_tx.encode(&mut rlp_buf);

                let tx = provider
                    .send_raw_transaction(&rlp_buf)
                    .await
                    .map_err(|err| {
                        log::error!("{err}");
                        ErrorInternalServerError("Failed to send transaction")
                    })?;

                let res = tx.get_receipt().await.map_err(|err| {
                    log::error!("{err}");
                    ErrorInternalServerError("Failed to send transaction")
                })?;

                Ok(res.transaction_hash)
            }
            _ => Err(ErrorBadRequest("Chain not supported")),
        }?;

        Ok(HttpResponse::Ok().json(TransactionResponse {
            id: transaction_model.id,
            hash: tx_hash.to_string(),
        }))
    } else {
        txn.rollback()
            .await
            .map_err(|_| ErrorInternalServerError(""))?;

        Ok(HttpResponse::InternalServerError().json(ErrorResponse {
            error: "Failed to sign transaction".to_string(),
        }))
    }
}
