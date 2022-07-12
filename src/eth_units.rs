use std::{
    fmt,
    ops::{Add, Sub},
    str::FromStr,
};

use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize};

use crate::decoders::from_u64_string;

pub const GWEI_PER_ETH: u64 = 1_000_000_000;

pub const GWEI_PER_ETH_F64: f64 = 1_000_000_000_f64;

// Can handle at most 1.84e19 Gwei, or 9.22e18 when we need to convert to i64 sometimes. That is
// ~9_000_000_000 ETH, which is more than the entire supply.
// TODO: Guard against overflow.
// Consider replacing with simple type alias.
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
        i64::try_from(amount).unwrap()
    }
}

impl From<i64> for GweiAmount {
    fn from(gwei_i64: i64) -> Self {
        GweiAmount(u64::try_from(gwei_i64).expect("failed to convert i64 into GweiAmount {}"))
    }
}

impl From<String> for GweiAmount {
    fn from(gwei_str: String) -> Self {
        GweiAmount(
            gwei_str
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

impl From<WeiString> for GweiAmount {
    fn from(WeiString(amount_str): WeiString) -> Self {
        let gwei_u128 = u128::from_str(&amount_str).unwrap() / u128::from(GWEI_PER_ETH);
        let gwei_u64 = u64::try_from(gwei_u128).unwrap();
        Self(gwei_u64)
    }
}

pub fn gwei_from_u64_string<'de, D>(deserializer: D) -> Result<GweiAmount, D::Error>
where
    D: Deserializer<'de>,
{
    let gwei_u64 = from_u64_string(deserializer)?;
    Ok(GweiAmount(gwei_u64))
}

struct GweiAmountVisitor;

impl<'de> Visitor<'de> for GweiAmountVisitor {
    type Value = GweiAmount;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .write_str("a number, or string of number, smaller u64::MAX representing some amount of ETH in Gwei")
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

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(GweiAmount(u64::try_from(v).unwrap()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(GweiAmount(v))
    }
}

impl<'de> Deserialize<'de> for GweiAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(GweiAmountVisitor)
    }
}

pub type Wei = u128;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WeiString(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gwei_from_wei_string_test() {
        let wei_string = WeiString("118068179561500000000000000".to_string());
        let gwei = GweiAmount::from(wei_string);
        assert_eq!(gwei, GweiAmount(118068179561500000));
    }

    #[test]
    fn gwei_from_string_test() {
        let gwei = GweiAmount::from("1234567890".to_string());
        assert_eq!(gwei, GweiAmount(1234567890));
    }

    #[test]
    fn gwei_add_test() {
        assert_eq!(GweiAmount(1) + GweiAmount(1), GweiAmount(2));
    }

    #[test]
    fn gwei_sub_test() {
        assert_eq!(GweiAmount(1) - GweiAmount(1), GweiAmount(0));
    }

    #[test]
    fn gwei_from_eth() {
        assert_eq!(GweiAmount::from_eth(1), GweiAmount(GWEI_PER_ETH))
    }
}
