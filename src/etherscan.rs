use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::{env, eth_units::WeiString};

lazy_static! {
    static ref ETHERSCAN_API_KEY: String = env::get_env_var_unsafe("ETHERSCAN_API_KEY");
}

const ETHERSCAN_API: &str = "https://api.etherscan.io/api";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EthSupply2Params<'a> {
    module: &'static str,
    action: &'static str,
    api_key: &'a str,
}

fn make_eth_supply_2_url() -> String {
    let params = EthSupply2Params {
        module: "stats",
        action: "ethsupply2",
        api_key: &*ETHERSCAN_API_KEY,
    };

    format!("{ETHERSCAN_API}?{}", serde_qs::to_string(&params).unwrap())
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EthSupply2 {
    /// eth_supply is calculated before adding ETH minted as Eth2Staking rewards and subtracting BurntFees from EIP-1559.
    pub eth_supply: WeiString,
    pub eth2_staking: WeiString,
    pub burnt_fees: WeiString,
}

#[derive(Deserialize)]
struct EthSupply2Response {
    result: EthSupply2,
}

pub async fn get_eth_supply_2() -> reqwest::Result<EthSupply2> {
    reqwest::get(make_eth_supply_2_url())
        .await?
        .error_for_status()?
        .json::<EthSupply2Response>()
        .await
        .map(|body| body.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_eth_supply_2() {
        get_eth_supply_2().await.unwrap();
    }
}
