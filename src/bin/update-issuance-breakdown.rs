#[tokio::main]
pub async fn main() {
    eth_analysis::update_issuance_breakdown().await;
}
