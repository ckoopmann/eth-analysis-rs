mod balances;
pub mod beacon_time;
mod blocks;
mod deposits;
mod effective_balance_sum;
mod issuance;
mod merge_status;
mod node;
mod rewards;
mod states;
mod sync;

use crate::env;
pub use balances::{get_validator_balances_by_start_of_day, BeaconBalancesSum};
pub use blocks::store_block;
pub use blocks::GENESIS_PARENT_ROOT;
pub use deposits::{get_deposits_sum, BeaconDepositsSum};
pub use effective_balance_sum::get_last_stored_effective_balance_sum;
pub use effective_balance_sum::update_effective_balance_sum;
pub use issuance::get_current_issuance;
pub use issuance::get_issuance_by_start_of_day;
pub use issuance::get_last_week_issuance;
pub use merge_status::update_merge_stats_auto;
pub use merge_status::update_merge_stats_by_hand;
pub use node::BeaconHeader;
pub use node::BeaconHeaderEnvelope;
pub use node::BeaconHeaderSignedEnvelope;
pub use node::BeaconNode;
pub use rewards::update_validator_rewards;
pub use states::store_state;
pub use states::Slot;
pub use sync::sync_beacon_states;

lazy_static! {
    static ref BEACON_URL: String = env::get_env_var_unsafe("BEACON_URL");
}
