use crate::client::{Client, Room};
use alloy::signers::k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use anyhow::Result;
use cggmp21::DataToSign;
use cggmp21::ExecutionId;
use cggmp21::KeyShare;
use generic_ec::{Curve, Point, coords::HasAffineX};
use proto::mpc::Chain;

use cggmp21::hd_wallet::slip10::SupportedCurve;
use cggmp21::round_based::MpcParty;
use cggmp21::security_level::SecurityLevel128;
use cggmp21::signing::msg::Msg;
use sha2::Sha256;
use std::error::Error;

pub struct Signing {
    room: Room,
}

impl Signing {
    pub fn new(client: &Client, id: i32) -> Self {
        Self {
            room: client.room(format!("signing_{id}").as_str()),
        }
    }

    pub async fn sign_tx<T>(
        self,
        index: u16,
        execution_id: &[u8],
        tx: &[u8],
        key_share: KeyShare<T, SecurityLevel128>,
        chain: Chain,
    ) -> Result<(Vec<u8>, Vec<u8>, u32)>
    where
        T: Curve + SupportedCurve,
        Point<T>: HasAffineX<T>,
    {
        let eid = ExecutionId::new(execution_id);

        let (_, incoming, outgoing) = self.room.join_room::<Msg<T, Sha256>>(index).await?;

        let party = MpcParty::connected((incoming, outgoing));

        let data = match chain {
            Chain::Ethereum => DataToSign::digest::<Sha256>(tx),
            Chain::Bitcoin => DataToSign::digest::<Sha256>(tx),
        };

        // TODO: Harcoded parties_indexes_at_keygen. Participants has a harcoded index.
        // Indexes must be issued on room creation and stored in DB.
        let signature = cggmp21::signing(eid, index, &[0, 1], &key_share)
            .sign(&mut rand::rngs::OsRng, party, data)
            .await
            .map_err(|err| {
                log::error!("Signin phase failed: {err}");
                if let Some(source) = err.source() {
                    log::error!("Caused by: {}", source);
                }
                err
            })?;

        let r = signature.r.into_inner().to_be_bytes();
        let r_bytes = r.as_bytes();
        let s = signature.s.into_inner().to_be_bytes();
        let s_bytes = s.as_bytes();

        let v = match chain {
            Chain::Ethereum => {
                let pub_key = key_share.shared_public_key.into_inner().to_bytes(false);
                let v_key = VerifyingKey::from_sec1_bytes(&pub_key).map_err(|err| {
                    log::error!("Verifying key failed: {err}");
                    if let Some(source) = err.source() {
                        log::error!("Caused by: {}", source);
                    }
                    err
                })?;
                let s = Signature::from_slice(&[r_bytes, s_bytes].concat()).map_err(|err| {
                    log::error!("Signature failed: {err}");
                    if let Some(source) = err.source() {
                        log::error!("Caused by: {}", source);
                    }
                    err
                })?;

                let reid = RecoveryId::trial_recovery_from_msg(
                    &v_key,
                    &data.to_scalar().to_be_bytes(),
                    &s,
                );

                // TODO: Harcoded! Use an input from the request or a config value
                let chain_id = 1;

                // https://medium.com/@LucasJennings/a-step-by-step-guide-to-generating-raw-ethereum-transactions-c3292ad36ab4
                match reid {
                    Err(_) => {
                        if r.last().unwrap() % 2 == 0 {
                            37
                        } else {
                            38
                        }
                    }
                    Ok(id) => chain_id * 2 + 35 + id.to_byte(),
                }
            }
            Chain::Bitcoin => 0,
        };

        Ok((r_bytes.to_vec(), s_bytes.to_vec(), v.into()))
    }
}
