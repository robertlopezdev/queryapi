use std::time::Duration;

use near_primitives::types::AccountId;
use tokio::time::sleep;
use tracing_subscriber::prelude::*;

use crate::block_streams::{synchronise_block_streams, BlockStreamsHandler};
use crate::executors::{synchronise_executors, ExecutorsHandler};
use crate::redis::RedisClient;
use crate::registry::Registry;

mod block_streams;
mod executors;
mod indexer_config;
mod migration;
mod redis;
mod registry;
mod utils;

const CONTROL_LOOP_THROTTLE_SECONDS: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let rpc_url = std::env::var("RPC_URL").expect("RPC_URL is not set");
    let registry_contract_id = std::env::var("REGISTRY_CONTRACT_ID")
        .expect("REGISTRY_CONTRACT_ID is not set")
        .parse::<AccountId>()
        .expect("REGISTRY_CONTRACT_ID is not a valid account ID");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL is not set");
    let block_streamer_url =
        std::env::var("BLOCK_STREAMER_URL").expect("BLOCK_STREAMER_URL is not set");
    let runner_url = std::env::var("RUNNER_URL").expect("RUNNER_URL is not set");

    let registry = Registry::connect(registry_contract_id.clone(), &rpc_url);
    let redis_client = RedisClient::connect(&redis_url).await?;
    let block_streams_handler = BlockStreamsHandler::connect(&block_streamer_url)?;
    let executors_handler = ExecutorsHandler::connect(&runner_url)?;

    tracing::info!(
        rpc_url,
        registry_contract_id = registry_contract_id.as_str(),
        block_streamer_url,
        runner_url,
        redis_url,
        "Starting Coordinator"
    );

    loop {
        let indexer_registry = registry.fetch().await?;

        let allowlist = migration::fetch_allowlist(&redis_client).await?;

        migration::migrate_pending_accounts(
            &indexer_registry,
            &allowlist,
            &redis_client,
            &executors_handler,
        )
        .await?;

        let indexer_registry =
            migration::filter_registry_by_allowlist(indexer_registry, &allowlist).await?;

        tokio::try_join!(
            synchronise_executors(&indexer_registry, &executors_handler),
            synchronise_block_streams(&indexer_registry, &redis_client, &block_streams_handler),
            async {
                sleep(CONTROL_LOOP_THROTTLE_SECONDS).await;
                Ok(())
            }
        )?;
    }
}
