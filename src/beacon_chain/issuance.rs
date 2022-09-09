use std::thread::current;

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::{postgres::PgRow, Acquire, PgConnection, PgExecutor, Row};

use crate::{
    eth_units::GweiAmount,
    supply_projection::{GweiInTime, GweiInTimeRow},
};

use super::{
    beacon_time::{self, FirstOfDaySlot},
    Slot,
};

pub async fn store_issuance_for_day<'a>(
    executor: impl PgExecutor<'a>,
    state_root: &str,
    FirstOfDaySlot(slot): &FirstOfDaySlot,
    gwei: &GweiAmount,
) {
    let gwei: i64 = gwei.to_owned().into();

    sqlx::query!(
        "
            INSERT INTO beacon_issuance (timestamp, state_root, gwei) VALUES ($1, $2, $3)
        ",
        beacon_time::get_date_time_from_slot(&slot),
        state_root,
        gwei,
    )
    .execute(executor)
    .await
    .unwrap();
}

pub fn calc_issuance(
    validator_balances_sum_gwei: &GweiAmount,
    deposit_sum_aggregated: &GweiAmount,
) -> GweiAmount {
    *validator_balances_sum_gwei - *deposit_sum_aggregated
}

pub async fn get_issuance_by_start_of_day<'a>(
    pool: impl PgExecutor<'a>,
) -> sqlx::Result<Vec<GweiInTime>> {
    sqlx::query_as!(
        GweiInTimeRow,
        "
            SELECT timestamp, gwei FROM beacon_issuance
        "
    )
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.iter()
            .map(|row| {
                (
                    row.timestamp.duration_trunc(Duration::days(1)).unwrap(),
                    row.gwei,
                )
            })
            .map(|row| row.into())
            .collect()
    })
}

pub struct BeaconIssuance {
    pub gwei: GweiAmount,
    timestamp: DateTime<Utc>,
}

pub async fn get_current_issuance(executor: impl PgExecutor<'_>) -> BeaconIssuance {
    sqlx::query(
        "
            SELECT gwei, timestamp FROM beacon_issuance
            ORDER BY timestamp DESC
            LIMIT 1
        ",
    )
    .map(|row: PgRow| {
        let timestamp = row.get::<DateTime<Utc>, _>("timestamp");
        let gwei = row.get::<i64, _>("gwei") as u64;
        BeaconIssuance {
            timestamp,
            gwei: GweiAmount(gwei),
        }
    })
    .fetch_one(executor)
    .await
    .unwrap()
}

pub async fn delete_issuances<'a>(connection: impl PgExecutor<'a>, greater_than_or_equal: &Slot) {
    sqlx::query!(
        "
            DELETE FROM beacon_issuance
            WHERE state_root IN (
                SELECT state_root FROM beacon_states
                WHERE slot >= $1
            )
        ",
        *greater_than_or_equal as i32
    )
    .execute(connection)
    .await
    .unwrap();
}

pub async fn get_day7_ago_issuance(
    executor: impl PgExecutor<'_>,
    last_timestamp: DateTime<Utc>,
) -> GweiAmount {
    sqlx::query(
        "

            SELECT
                gwei
            FROM
                beacon_issuance
            WHERE timestamp = $1 - '7 days'::INTERVAL
        ",
    )
    .bind(last_timestamp)
    .map(|row: PgRow| {
        let gwei_i64 = row.get::<i64, _>("gwei") as u64;
        GweiAmount(gwei_i64)
    })
    .fetch_one(executor)
    .await
    .unwrap()
}

pub async fn get_last_week_issuance(executor: &mut PgConnection) -> GweiAmount {
    let current_issuance = get_current_issuance(executor.acquire().await.unwrap()).await;
    let day7_ago_issuance = get_day7_ago_issuance(executor, current_issuance.timestamp).await;
    current_issuance.gwei - day7_ago_issuance
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::{beacon_chain::states::store_state, db_testing};

    #[test]
    fn calc_issuance_test() {
        let validator_balances_sum_gwei = GweiAmount(100);
        let deposit_sum_aggregated = GweiAmount(50);

        assert_eq!(
            calc_issuance(&validator_balances_sum_gwei, &deposit_sum_aggregated),
            GweiAmount(50)
        )
    }

    #[tokio::test]
    async fn timestamp_is_start_of_day_test() {
        let mut connection = PgConnection::connect(&config::get_db_url()).await.unwrap();
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut transaction, "0xtest_issuance", &3599)
            .await
            .unwrap();

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance",
            &FirstOfDaySlot::new(&3599).unwrap(),
            &GweiAmount(100),
        )
        .await;

        let validator_balances_by_day = get_issuance_by_start_of_day(&mut transaction)
            .await
            .unwrap();

        let unix_timestamp = validator_balances_by_day.first().unwrap().t;

        let datetime = Utc.timestamp(unix_timestamp.try_into().unwrap(), 0);

        let start_of_day_datetime = datetime.duration_trunc(Duration::days(1)).unwrap();

        assert_eq!(datetime, start_of_day_datetime);
    }

    #[tokio::test]
    async fn get_current_issuance_test() {
        let mut connection = PgConnection::connect(&config::get_db_url()).await.unwrap();
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut transaction, "0xtest_issuance_1", &3599)
            .await
            .unwrap();

        store_state(&mut transaction, "0xtest_issuance_2", &10799)
            .await
            .unwrap();

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance_1",
            &FirstOfDaySlot::new(&3599).unwrap(),
            &GweiAmount(100),
        )
        .await;

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance_2",
            &FirstOfDaySlot::new(&10799).unwrap(),
            &GweiAmount(110),
        )
        .await;

        let current_issuance = get_current_issuance(&mut transaction).await.unwrap();

        assert_eq!(current_issuance, GweiAmount(110));
    }

    #[tokio::test]
    async fn delete_issuance_test() {
        let mut connection = PgConnection::connect(&config::get_db_url()).await.unwrap();
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut transaction, "0xtest_issuance", &3599)
            .await
            .unwrap();

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance",
            &FirstOfDaySlot::new(&3599).unwrap(),
            &GweiAmount(100),
        )
        .await;

        let issuance_by_day = get_issuance_by_start_of_day(&mut transaction)
            .await
            .unwrap();

        assert_eq!(issuance_by_day.len(), 1);

        delete_issuances(&mut transaction, &3599).await;

        let issuance_by_day_after = get_issuance_by_start_of_day(&mut transaction)
            .await
            .unwrap();

        assert_eq!(issuance_by_day_after.len(), 0);
    }

    #[tokio::test]
    async fn get_day7_ago_issuance_test() {
        let mut connection = db_testing::get_test_db().await;
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut transaction, "0xtest_issuance_1", &3599)
            .await
            .unwrap();

        store_state(&mut transaction, "0xtest_issuance_2", &10799)
            .await
            .unwrap();

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance_1",
            &FirstOfDaySlot::new(&3599).unwrap(),
            &GweiAmount(100),
        )
        .await;

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance_2",
            &FirstOfDaySlot::new(&53999).unwrap(),
            &GweiAmount(110),
        )
        .await;

        let current_issuance = get_current_issuance(&mut transaction).await;
        let day7_ago_issuance =
            get_day7_ago_issuance(&mut transaction, current_issuance.timestamp).await;

        assert_eq!(day7_ago_issuance, GweiAmount(100));
    }

    #[tokio::test]
    async fn get_last_week_issuance_test() {
        let mut connection = db_testing::get_test_db().await;
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut transaction, "0xtest_issuance_1", &3599)
            .await
            .unwrap();

        store_state(&mut transaction, "0xtest_issuance_2", &10799)
            .await
            .unwrap();

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance_1",
            &FirstOfDaySlot::new(&3599).unwrap(),
            &GweiAmount(100),
        )
        .await;

        store_issuance_for_day(
            &mut transaction,
            "0xtest_issuance_2",
            &FirstOfDaySlot::new(&53999).unwrap(),
            &GweiAmount(110),
        )
        .await;

        let issuance = get_last_week_issuance(&mut transaction).await;

        assert_eq!(issuance, GweiAmount(10));
    }
}
