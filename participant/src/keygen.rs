use crate::client::{Client, Room};
use generic_ec::Curve;

use anyhow::Result;
use cggmp21::ExecutionId;
use cggmp21::KeyShare;
use cggmp21::key_refresh::AuxOnlyMsg;
use cggmp21::key_share::{DirtyAuxInfo, DirtyIncompleteKeyShare, Valid};
use cggmp21::keygen::ThresholdMsg;
use cggmp21::security_level::SecurityLevel128;
use log::info;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::error::Error;

static TOTAL_PARTIES: u16 = 3;
static THRESHOLD: u16 = 2;

#[derive(Deserialize, Serialize)]
pub struct ShareSecret {
    index: u16,
    share: String,
}

pub struct Keygen {
    aux_room: Room,
    keygen_room: Room,
}

impl Keygen {
    pub fn new(client: &Client, id: i32) -> Self {
        Self {
            aux_room: client.room(format!("aux_{id}").as_str()),
            keygen_room: client.room(format!("keygen_{id}").as_str()),
        }
    }

    async fn compute_keygen<T: Curve>(
        &self,
        index: u16,
        eid: ExecutionId<'_>,
    ) -> Result<Valid<DirtyIncompleteKeyShare<T>>> {
        let (_, incoming, outgoing) = self
            .keygen_room
            .clone()
            .join_room::<ThresholdMsg<T, SecurityLevel128, Sha256>>(index)
            .await?;

        let party = cggmp21::round_based::MpcParty::connected((incoming, outgoing));

        info!(
            "Starting keygen phase with index: {}, total parties: {}, threshold: {}",
            index, TOTAL_PARTIES, THRESHOLD
        );

        // TODO: Use HD Wallets
        let key_share = cggmp21::keygen::<T>(eid, index, TOTAL_PARTIES)
            .set_threshold(THRESHOLD)
            .hd_wallet(false)
            .start(&mut rand::rngs::OsRng, party)
            .await?;

        Ok(key_share)
    }

    async fn compute_aux_info(
        &self,
        index: u16,
        eid: ExecutionId<'_>,
    ) -> Result<Valid<DirtyAuxInfo>> {
        let (_, incoming, outgoing) = self
            .aux_room
            .clone()
            .join_room::<AuxOnlyMsg<Sha256, SecurityLevel128>>(index)
            .await?;

        info!("Starting Aux info phase with index: {}", index);

        let pregenerated_primes = cggmp21::PregeneratedPrimes::generate(&mut rand::rngs::OsRng);

        let party = cggmp21::round_based::MpcParty::connected((incoming, outgoing));

        let aux_info = cggmp21::aux_info_gen(eid, index, TOTAL_PARTIES, pregenerated_primes)
            .start(&mut rand::rngs::OsRng, party)
            .await?;

        Ok(aux_info)
    }

    pub async fn compute_share<T: Curve>(
        self,
        index: u16,
        execution_id: &[u8],
    ) -> Result<KeyShare<T, SecurityLevel128>> {
        let eid = ExecutionId::new(execution_id);

        let (keygen_result, aux_result) = futures::future::join(
            self.compute_keygen::<T>(index, eid),
            self.compute_aux_info(index, eid),
        )
        .await;

        let keygen = keygen_result.map_err(|err| {
            log::error!("Keygen phase failed: {err}");
            if let Some(source) = err.source() {
                log::error!("Caused by: {}", source);
            }
            err
        })?;

        let aux_info = aux_result.map_err(|err| {
            log::error!("Aux info phase failed: {err}");
            if let Some(source) = err.source() {
                log::error!("Caused by: {}", source);
            }
            err
        })?;

        let share = KeyShare::from_parts((keygen, aux_info)).map_err(|err| {
            log::error!("Key share phase failed: {err}");
            if let Some(source) = err.source() {
                log::error!("Caused by: {}", source);
            }
            err
        })?;

        Ok(share)
    }
}
