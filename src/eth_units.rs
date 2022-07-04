use num_bigint::BigUint;
use std::{
    fmt,
    ops::{Add, Div, Sub},
    str::FromStr,
};

use serde::{
    de::{self, Visitor},
    Deserialize, Serialize,
};

use crate::etherscan::WeiAmount;

pub const GWEI_PER_ETH: u64 = 1_000_000_000;

pub const GWEI_PER_ETH_F64: f64 = 1_000_000_000_f64;

pub const WEI_PER_GWEI_F64: f64 = 1_000_000_000_f64;

// Can handle at most 1.84e19 Gwei, or 9.22e18 when we need to convert to i64 sometimes. That is
// ~9_000_000_000 ETH, which is more than the entire supply.
// TODO: Guard against overflow.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GweiAmount(pub u64);

impl fmt::Display for GweiAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} gwei", self.0)
    }
}

impl GweiAmount {
    pub fn new(gwei: u64) -> Self {
        Self(gwei)
    }

    pub fn from_eth(eth: u64) -> Self {
        Self(eth * GWEI_PER_ETH)
    }

    pub fn from_eth_f64(eth: f64) -> Self {
        Self((eth * GWEI_PER_ETH_F64) as u64)
    }
}

impl From<GweiAmount> for i64 {
    fn from(GweiAmount(amount): GweiAmount) -> Self {
        match i64::try_from(amount) {
            Err(err) => panic!("failed to convert GweiAmount into i64 {}", err),
            Ok(amount_i64) => amount_i64,
        }
    }
}

impl From<i64> for GweiAmount {
    fn from(num: i64) -> Self {
        match u64::try_from(num) {
            Err(err) => panic!("failed to convert i64 into GweiAmount {}", err),
            Ok(num_u64) => GweiAmount(num_u64),
        }
    }
}

impl From<String> for GweiAmount {
    fn from(amount: String) -> Self {
        GweiAmount(
            amount
                .parse::<u64>()
                .expect("amount to be a string of a gwei amount that fits into u64"),
        )
    }
}

impl Add<GweiAmount> for GweiAmount {
    type Output = Self;

    fn add(self, GweiAmount(rhs): Self) -> Self::Output {
        let GweiAmount(lhs) = self;
        GweiAmount(lhs + rhs)
    }
}

impl Sub<GweiAmount> for GweiAmount {
    type Output = Self;

    fn sub(self, GweiAmount(rhs): GweiAmount) -> Self::Output {
        let GweiAmount(lhs) = self;
        GweiAmount(lhs - rhs)
    }
}

impl Div<GweiAmount> for GweiAmount {
    type Output = Self;

    // Consider forbidding integer division and forcing use of float variants.
    fn div(self, GweiAmount(rhs): GweiAmount) -> Self::Output {
        let GweiAmount(lhs) = self;
        GweiAmount(lhs / rhs)
    }
}

impl From<WeiAmount> for GweiAmount {
    fn from(WeiAmount(amount_str): WeiAmount) -> Self {
        let gwei_biguint = BigUint::from_str(&amount_str).unwrap() / BigUint::from(GWEI_PER_ETH);
        let gwei_u64 = u64::try_from(gwei_biguint).unwrap();
        Self(gwei_u64)
    }
}

struct GweiAmountVisitor;

impl<'de> Visitor<'de> for GweiAmountVisitor {
    type Value = GweiAmount;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .write_str("an number encoded as a string smaller than the total supply of ETH in Gwei")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match v.parse::<u64>() {
            Err(error) => Err(E::custom(format!(
                "failed to parse amount as u64: {}",
                error
            ))),
            Ok(amount) => Ok(GweiAmount(amount)),
        }
    }
}

impl<'de> Deserialize<'de> for GweiAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(GweiAmountVisitor)
    }
}
