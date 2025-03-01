use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    fmt::Display,
    ops::{Add, Mul, Rem, Sub},
    str::FromStr,
};

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};

use crate::beacon_chain::GENESIS_TIMESTAMP;

// Beacon chain slots are defined as 12 second periods starting from genesis. With u32 our program
// would overflow when the slot number passes 2_147_483_647. i32::MAX * 12 seconds = ~817 years.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Ord, PartialOrd, PartialEq, Serialize, sqlx::Type,
)]
#[sqlx(transparent)]
pub struct Slot(pub i32);

impl Slot {
    pub const GENESIS: Self = Self(0);
    pub const SECONDS_PER_SLOT: i32 = 12;

    pub fn date_time(&self) -> DateTime<Utc> {
        self.into()
    }

    pub fn from_date_time(date_time: &DateTime<Utc>) -> Option<Self> {
        let seconds_since_genesis = date_time.timestamp() - GENESIS_TIMESTAMP.timestamp();
        if seconds_since_genesis % Self::SECONDS_PER_SLOT as i64 != 0 {
            None
        } else {
            let slots_since_genesis = seconds_since_genesis / Self::SECONDS_PER_SLOT as i64;
            Some(Self(slots_since_genesis as i32))
        }
    }

    /// Returns the most recent slot before the given date_time.
    pub fn from_date_time_rounded_down(date_time: &DateTime<Utc>) -> Self {
        let diff_seconds = *date_time - *GENESIS_TIMESTAMP;
        let slot = diff_seconds.num_seconds() / Slot::SECONDS_PER_SLOT as i64;
        Self(slot as i32)
    }

    pub fn is_first_of_day(&self) -> bool {
        if self.0 == 0 {
            return true;
        };

        let day_previous_slot = Self(self.0 - 1).date_time().day();
        let day = Self(self.0).date_time().day();

        day_previous_slot != day
    }

    pub fn is_first_of_minute(&self) -> bool {
        if self.0 == 0 {
            return true;
        };

        let minute_previous_slot = Self(self.0 - 1).date_time().minute();
        let minute = Self(self.0).date_time().minute();

        minute_previous_slot != minute
    }

    pub async fn is_first_of_day_with_block(&self) -> Result<bool> {
        // We use a tiny algo here to figure out if we're the first of all slots in the day with a
        // block.
        // If we don't have a block, we know we're not.
        // If we do have a block and we're also the first slot of the day, we are.
        // If we do have a block and we're not the first slot of the day, we still may be the first
        // in the day with a slot. Search backwards for a slot that is the first of the day without
        // a block.
        // If we hit a slot with the block, we can stop, we're not.
        // If we hit a slot without a block and its the first of the day, we are!
        // If we hit a slot without a block, we keep searching.
        unimplemented!()
    }
}

impl Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add<i32> for Slot {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub<i32> for Slot {
    type Output = Self;

    fn sub(self, rhs: i32) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl Mul<i32> for Slot {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Rem<i32> for Slot {
    type Output = Self;

    fn rem(self, rhs: i32) -> Self::Output {
        Self(self.0 % rhs)
    }
}

impl From<Slot> for DateTime<Utc> {
    fn from(slot: Slot) -> Self {
        let seconds = slot.0 as i64 * Slot::SECONDS_PER_SLOT as i64;
        *GENESIS_TIMESTAMP + Duration::seconds(seconds)
    }
}

impl From<&Slot> for DateTime<Utc> {
    fn from(slot: &Slot) -> Self {
        let seconds = slot.0 as i64 * Slot::SECONDS_PER_SLOT as i64;
        *GENESIS_TIMESTAMP + Duration::seconds(seconds)
    }
}

impl From<&Slot> for i32 {
    fn from(slot: &Slot) -> Self {
        slot.0
    }
}

impl From<i32> for Slot {
    fn from(slot: i32) -> Self {
        Self(slot)
    }
}

impl From<Slot> for i64 {
    fn from(slot: Slot) -> Self {
        slot.0 as i64
    }
}

impl From<Slot> for u64 {
    fn from(slot: Slot) -> Self {
        slot.0 as u64
    }
}

impl FromStr for Slot {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl From<&i32> for Slot {
    fn from(slot: &i32) -> Self {
        Self(*slot)
    }
}

pub fn slot_from_string<'de, D>(deserializer: D) -> Result<Slot, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)
        .map(|slot_text| slot_text.parse().expect("expect slot to be i32"))
        .map(Slot)
}

#[derive(Debug)]
pub struct FirstOfDaySlotWithBlock(pub Slot);

impl FirstOfDaySlotWithBlock {}

#[cfg(test)]
mod tests {
    use crate::execution_chain::LONDON_HARD_FORK_TIMESTAMP;

    use super::*;

    #[test]
    fn first_of_day_genesis_test() {
        assert!(Slot(0).is_first_of_day());
    }

    #[test]
    fn first_of_day_test() {
        assert!(Slot(3599).is_first_of_day());
    }

    #[test]
    fn not_first_of_day_test() {
        assert!(!Slot(1).is_first_of_day());
        assert!(!Slot(3598).is_first_of_day());
        assert!(!Slot(3600).is_first_of_day());
    }

    #[test]
    fn get_timestamp_test() {
        assert_eq!(
            Slot(0).date_time(),
            "2020-12-01T12:00:23Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            Slot(3599).date_time(),
            "2020-12-02T00:00:11Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[test]
    fn start_of_day_test() {
        assert!(Slot(0).is_first_of_day());
        assert!(Slot(3599).is_first_of_day());
    }

    #[test]
    fn not_start_of_day_test() {
        assert!(!Slot(1).is_first_of_day());
        assert!(!Slot(3598).is_first_of_day());
        assert!(!Slot(3600).is_first_of_day());
    }

    #[test]
    fn first_of_minute_genesis_test() {
        assert!(Slot(0).is_first_of_minute());
    }

    #[test]
    fn first_of_minute_test() {
        assert!(Slot(4).is_first_of_minute());
    }

    #[test]
    fn not_first_of_minute_test() {
        assert!(!Slot(3).is_first_of_minute());
        assert!(!Slot(5).is_first_of_minute());
    }

    #[test]
    fn merge_from_date_time() {
        dbg!(Slot::from_date_time(&LONDON_HARD_FORK_TIMESTAMP));
    }
}
