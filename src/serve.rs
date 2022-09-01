use axum::http::header;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Extension;
use axum::Json;
use axum::Router;
use etag::EntityTag;
use futures::TryStreamExt;
use reqwest::StatusCode;
use serde_json::Value;
use sha1::Digest;
use sha1::Sha1;
use sqlx::Connection;
use sqlx::PgConnection;
use std::sync::Arc;
use std::sync::RwLock;

use crate::caching::CacheKey;
use crate::config;
use crate::key_value_store;

type StateExtension = Extension<Arc<State>>;

type Hash = String;
type CachedValue = RwLock<Option<(Value, Hash)>>;

#[derive(Debug)]
struct Cache {
    block_lag: CachedValue,
    eth_price: CachedValue,
    eth_supply_parts: CachedValue,
    merge_estimate: CachedValue,
    total_difficulty_progress: CachedValue,
}

pub struct State {
    cache: Arc<Cache>,
}

fn hash_from_u8(text: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(text);
    let hash = hasher.finalize();
    base64::encode(hash)
}

fn hash_from_json(v: &Value) -> String {
    let v_bytes = serde_json::to_vec(v).unwrap();
    hash_from_u8(&v_bytes)
}

async fn get_value_etag_pair(
    connection: &mut PgConnection,
    key: &CacheKey<'_>,
) -> Option<(Value, String)> {
    let value = key_value_store::get_value(connection, &key.to_db_key()).await?;
    let hash = hash_from_json(&value);
    Some((value, hash))
}

async fn get_value_hash_lock(connection: &mut PgConnection, key: &CacheKey<'_>) -> CachedValue {
    let pair = get_value_etag_pair(connection, key).await;
    RwLock::new(pair)
}

async fn get_cached<'a>(cached_value: &CachedValue, cache_control: &str) -> impl IntoResponse {
    match &*cached_value.read().unwrap() {
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Some((merge_estimate, hash)) => {
            let mut headers = HeaderMap::new();

            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_str(cache_control).unwrap(),
            );

            let etag = EntityTag::strong(&hash);
            headers.insert(
                header::ETAG,
                HeaderValue::from_str(&etag.to_string()).unwrap(),
            );

            (headers, Json(merge_estimate).into_response()).into_response()
        }
    }
}

async fn update_cache_from_key(
    connection: &mut PgConnection,
    cached_value: &CachedValue,
    cache_key: &CacheKey<'_>,
) {
    tracing::debug!("{} cache update", cache_key);
    let pair = get_value_etag_pair(connection, cache_key).await;
    let mut cache_wlock = cached_value.write().unwrap();
    *cache_wlock = pair;
}

async fn update_cache_from_notifications(state: Arc<State>, mut connection: PgConnection) {
    tracing::debug!("setting up listening for cache updates");
    let mut listener = sqlx::postgres::PgListener::connect(&config::get_db_url())
        .await
        .unwrap();
    listener.listen("cache-update").await.unwrap();
    let mut notification_stream = listener.into_stream();

    tokio::spawn(async move {
        while let Some(notification) = notification_stream.try_next().await.unwrap() {
            let payload = notification.payload();
            let payload_cache_key = CacheKey::from(payload);
            match payload_cache_key {
                key @ CacheKey::BlockLag => {
                    update_cache_from_key(&mut connection, &state.cache.block_lag, &key).await
                }
                key @ CacheKey::EthPrice => {
                    update_cache_from_key(&mut connection, &state.cache.eth_price, &key).await
                }
                key @ CacheKey::EthSupplyParts => {
                    update_cache_from_key(&mut connection, &state.cache.eth_supply_parts, &key)
                        .await
                }
                key @ CacheKey::MergeEstimate => {
                    update_cache_from_key(&mut connection, &state.cache.merge_estimate, &key).await
                }
                key @ CacheKey::TotalDifficultyProgress => {
                    update_cache_from_key(
                        &mut connection,
                        &state.cache.total_difficulty_progress,
                        &key,
                    )
                    .await
                }
                key => {
                    tracing::debug!("received unsupported cache key: {key:?}, skipping");
                }
            }
        }
    });
}

pub async fn start_server() {
    tracing_subscriber::fmt::init();

    let mut connection = PgConnection::connect(&config::get_db_url_with_name("eth-analysis-serve"))
        .await
        .unwrap();

    sqlx::migrate!().run(&mut connection).await.unwrap();

    tracing::debug!("warming up total difficulty progress cache");

    let base_fee_per_gas = get_value_hash_lock(&mut connection, &CacheKey::BaseFeePerGas).await;
    let block_lag = get_value_hash_lock(&mut connection, &CacheKey::BlockLag).await;
    let eth_price = get_value_hash_lock(&mut connection, &CacheKey::EthPrice).await;
    let eth_supply_parts = get_value_hash_lock(&mut connection, &CacheKey::EthSupplyParts).await;
    let merge_estimate = get_value_hash_lock(&mut connection, &CacheKey::MergeEstimate).await;
    let total_difficulty_progress =
        get_value_hash_lock(&mut connection, &CacheKey::TotalDifficultyProgress).await;

    let cache = Arc::new(Cache {
        block_lag,
        eth_price,
        eth_supply_parts,
        merge_estimate,
        total_difficulty_progress,
    });

    tracing::debug!("cache warming done");

    let shared_state = Arc::new(State { cache });

    update_cache_from_notifications(shared_state.clone(), connection).await;

    let app = Router::new()
        .route(
            "/api/v2/fees/block-lag",
            get(|state: StateExtension| async move {
                get_cached(
                    &state.clone().cache.block_lag,
                    "max-age=4, stale-while-revalidate=60",
                )
                .await
            }),
        )
        .route(
            "/api/v2/fees/eth-price",
            get(|state: StateExtension| async move {
                get_cached(
                    &state.clone().cache.eth_price,
                    "max-age=60, stale-while-revalidate=600",
                )
                .await
                .into_response()
            }),
        )
        .route(
            "/api/v2/fees/eth-supply",
            get(|state: StateExtension| async move {
                get_cached(
                    &state.clone().cache.eth_supply_parts,
                    "max-age=4, stale-while-revalidate=60",
                )
                .await
                .into_response()
            }),
        )
        .route(
            "/api/v2/fees/eth-supply-parts",
            get(|state: StateExtension| async move {
                get_cached(
                    &state.clone().cache.eth_supply_parts,
                    "max-age=4, stale-while-revalidate=60",
                )
                .await
                .into_response()
            }),
        )
        .route(
            "/api/v2/fees/merge-estimate",
            get(|state: StateExtension| async move {
                get_cached(
                    &state.clone().cache.merge_estimate,
                    "max-age=4, stale-while-revalidate=14400",
                )
                .await
                .into_response()
            }),
        )
        .route(
            "/api/v2/fees/total-difficulty-progress",
            get(|state: StateExtension| async move {
                get_cached(
                    &state.clone().cache.total_difficulty_progress,
                    "max-age=600, stale-while-revalidate=86400",
                )
                .await
                .into_response()
            }),
        )
        .route("/healthz", get(|| async { StatusCode::OK }))
        .layer(Extension(shared_state));

    let port = config::get_env_var("PORT").unwrap_or("3002".to_string());

    tracing::info!("listening on {port}");
    axum::Server::bind(&format!("0.0.0.0:{port}").parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
