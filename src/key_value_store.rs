use serde_json::Value;
use sqlx::{postgres::PgRow, PgExecutor, Row};

// Do we need a distinction between key/value pair isn't there and value is null?
pub async fn get_value<'a>(executor: impl PgExecutor<'a>, key: &str) -> Option<Value> {
    tracing::debug!("getting key: {}", key);

    sqlx::query(
        "
            SELECT value FROM key_value_store
            WHERE key = $1
        ",
    )
    .bind(key)
    .map(|row: PgRow| row.get::<Option<Value>, _>("value"))
    .fetch_optional(executor)
    .await
    .unwrap()
    .flatten()
}

pub async fn set_value<'a>(executor: impl PgExecutor<'a>, key: &str, value: &Value) {
    tracing::debug!("storing key: {}", &key,);

    sqlx::query(
        "
            INSERT INTO key_value_store (key, value) VALUES ($1, $2)
            ON CONFLICT (key) DO UPDATE SET
                value = excluded.value
        ",
    )
    .bind(key)
    .bind(value)
    .execute(executor)
    .await
    .unwrap();
}

pub async fn set_value_str<'a>(executor: impl PgExecutor<'a>, key: &str, value_str: &str) {
    tracing::debug!("storing key: {}", &key,);

    sqlx::query(
        "
            INSERT INTO key_value_store (key, value) VALUES ($1, $2::jsonb)
            ON CONFLICT (key) DO UPDATE SET
                value = excluded.value
        ",
    )
    .bind(key)
    .bind(value_str)
    .execute(executor)
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use sqlx::{Connection, PgConnection};

    use crate::{config, db_testing};

    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct TestJson {
        name: String,
        age: i32,
    }

    #[tokio::test]
    async fn get_set_value_test() {
        let mut connection = db_testing::get_test_db().await;
        let mut transaction = connection.begin().await.unwrap();

        let test_json = TestJson {
            name: "alex".to_string(),
            age: 29,
        };

        set_value(
            &mut transaction,
            "test-key",
            &serde_json::to_value(&test_json).unwrap(),
        )
        .await;

        let test_json_from_db = serde_json::from_value::<TestJson>(
            get_value(&mut transaction, "test-key").await.unwrap(),
        )
        .unwrap();

        assert_eq!(test_json_from_db, test_json)
    }

    #[tokio::test]
    async fn get_null_value_test() {
        let mut connection = db_testing::get_test_db().await;
        let mut transaction = connection.begin().await.unwrap();

        set_value(
            &mut transaction,
            "test-key",
            &serde_json::to_value(json!(None::<String>)).unwrap(),
        )
        .await;

        let test_json_from_db = serde_json::from_value::<Option<String>>(
            get_value(&mut transaction, "test-key").await.unwrap(),
        )
        .unwrap();

        assert_eq!(test_json_from_db, None)
    }

    #[tokio::test]
    async fn set_value_str_test() {
        let mut connection = PgConnection::connect(&config::get_db_url()).await.unwrap();
        let mut transaction = connection.begin().await.unwrap();

        let test_json = TestJson {
            name: "alex".to_string(),
            age: 29,
        };

        let test_json_str = serde_json::to_string(&json!({
            "name": "alex",
            "age": 29
        }))
        .unwrap();

        set_value_str(&mut transaction, "test-key", &test_json_str).await;

        let test_json_from_db = serde_json::from_value::<TestJson>(
            get_value(&mut transaction, "test-key").await.unwrap(),
        )
        .unwrap();

        assert_eq!(test_json_from_db, test_json)
    }
}
