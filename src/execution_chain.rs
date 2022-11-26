mod balances;
mod base_fees;
mod block_store;
mod eth_prices;
mod export_blocks;
mod logs;
mod node;
mod supply_deltas;
mod sync;

pub use balances::{get_balances_sum, get_closest_balances_sum, ExecutionBalancesSum};
pub use block_store::BlockStore;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
pub use eth_prices::get_eth_price_by_block;
pub use export_blocks::write_blocks_from_august;
pub use export_blocks::write_blocks_from_london;
use lazy_static::lazy_static;
pub use logs::write_heads_log as write_execution_heads_log;
pub use node::{stream_new_heads, BlockNumber, ExecutionNode, ExecutionNodeBlock};
pub use supply_deltas::add_delta;
pub use supply_deltas::stream_supply_delta_chunks;
pub use supply_deltas::stream_supply_deltas_from;
pub use supply_deltas::summary_from_deltas_csv;
pub use supply_deltas::sync_deltas as sync_execution_supply_deltas;
pub use supply_deltas::write_deltas as write_execution_supply_deltas;
pub use supply_deltas::write_deltas_log as write_execution_supply_deltas_log;
pub use supply_deltas::SupplyDelta;
pub use sync::sync_blocks as sync_execution_blocks;

lazy_static! {
    pub static ref LONDON_HARD_FORK_TIMESTAMP: DateTime<Utc> =
        Utc.timestamp_opt(1628166822, 0).unwrap();
    pub static ref BELLATRIX_HARD_FORK_TIMESTAMP: DateTime<Utc> =
        "2022-09-15T06:42:42Z".parse::<DateTime<Utc>>().unwrap();
}

#[allow(dead_code)]
pub const TOTAL_TERMINAL_DIFFICULTY: u128 = 58750000000000000000000;
