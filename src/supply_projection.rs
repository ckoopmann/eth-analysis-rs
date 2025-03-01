use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, Decode};
use tracing::{debug, info};

use crate::{
    beacon_chain,
    caching::{self, CacheKey},
    db,
    glassnode::{self, GlassnodeDataPoint},
    key_value_store, log,
    units::GWEI_PER_ETH_F64,
};

#[derive(Decode)]
pub struct GweiInTimeRow {
    pub timestamp: DateTime<Utc>,
    pub gwei: i64,
}

#[derive(Serialize)]
pub struct GweiInTime {
    pub t: u64,
    pub v: i64,
}

impl From<(DateTime<Utc>, i64)> for GweiInTime {
    fn from((dt, gwei): (DateTime<Utc>, i64)) -> Self {
        GweiInTime {
            t: dt.timestamp().try_into().unwrap(),
            v: gwei,
        }
    }
}

impl From<&GweiInTimeRow> for GweiInTime {
    fn from(row: &GweiInTimeRow) -> Self {
        Self {
            t: row.timestamp.timestamp() as u64,
            v: row.gwei,
        }
    }
}

fn add_beacon_issuance_to_supply(
    beacon_issuance_by_day: &[GlassnodeDataPoint],
    supply_data: &[GlassnodeDataPoint],
) -> Vec<GlassnodeDataPoint> {
    let issuance_by_day_map = beacon_issuance_by_day
        .iter()
        .map(|gwei_in_time| (gwei_in_time.t, gwei_in_time.v))
        .collect::<HashMap<u64, f64>>();

    supply_data
        .iter()
        .map(
            |supply_on_day| match issuance_by_day_map.get(&supply_on_day.t) {
                None => supply_on_day.clone(),
                Some(issuance_on_day) => GlassnodeDataPoint {
                    t: supply_on_day.t,
                    v: supply_on_day.v + issuance_on_day,
                },
            },
        )
        .collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupplyProjectionInputs {
    supply_by_day: Vec<GlassnodeDataPoint>,
    in_contracts_by_day: Vec<GlassnodeDataPoint>,
    in_beacon_validators_by_day: Vec<GlassnodeDataPoint>,
}

pub async fn update_supply_projection_inputs() -> Result<()> {
    log::init_with_env();

    info!("updating supply projection inputs");

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db::get_db_url_with_name("supply_projection_inputs"))
        .await
        .unwrap();

    sqlx::migrate!().run(&pool).await.unwrap();

    let in_contracts_by_day = glassnode::get_locked_eth_data().await.unwrap();

    debug!(
        "got gwei in contracts by day, {} data points",
        in_contracts_by_day.len()
    );

    let in_beacon_validators_by_day = beacon_chain::get_validator_balances_by_start_of_day(&pool)
        .await
        .iter()
        .map(|point| GlassnodeDataPoint {
            t: point.t,
            v: point.v as f64 / GWEI_PER_ETH_F64,
        })
        .collect::<Vec<_>>();

    debug!(
        "got balances in beacon validators by day, {} data points",
        in_beacon_validators_by_day.len()
    );

    let beacon_issuance_by_day = beacon_chain::get_issuance_by_start_of_day(&pool)
        .await
        .iter()
        .map(|point| GlassnodeDataPoint {
            t: point.t,
            v: point.v as f64 / GWEI_PER_ETH_F64,
        })
        .collect::<Vec<GlassnodeDataPoint>>();

    debug!(
        "got beacon issuance by day, {} data points",
        beacon_issuance_by_day.len()
    );

    let supply_data = glassnode::get_circulating_supply_data().await.unwrap();

    debug!("got supply data by day, {} data points", supply_data.len());

    let supply_by_day = add_beacon_issuance_to_supply(&beacon_issuance_by_day, &supply_data);

    debug!("got supply by day, {} data points", supply_by_day.len());

    // Deprecate supplyData, lockedData, stakedData after prod frontend has switched to new supply projection inputs.
    let supply_projetion_inputs = SupplyProjectionInputs {
        supply_by_day,
        in_contracts_by_day,
        in_beacon_validators_by_day,
    };

    key_value_store::set_value(
        &pool,
        &CacheKey::SupplyProjectionInputs.to_db_key(),
        &serde_json::to_value(supply_projetion_inputs).unwrap(),
    )
    .await?;

    debug!("stored fresh projection inputs");

    caching::publish_cache_update(&pool, &CacheKey::SupplyProjectionInputs).await?;

    debug!(
        "published {} cache update",
        CacheKey::SupplyProjectionInputs
    );

    info!("done updating supply projection inputs");

    Ok(())
}
