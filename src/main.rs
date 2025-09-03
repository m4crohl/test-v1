mod config;
mod monitor;

use config::RpcConfig;
use monitor::SwapMonitor;
use ethers::prelude::*;
use ethers::providers::Middleware;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Sandwich Bot v0.2 (V2 + V3 Support)");
    println!("📍 Target: Polygon Mainnet");
    println!("{}", "=".repeat(50));
    
    // Initialize RPC configuration
    let rpc_config = Arc::new(RpcConfig::new());
    
    // Main loop with automatic reconnection
    loop {
        match run_bot(rpc_config.clone()).await {
            Ok(_) => {
                println!("✅ Bot terminated normally");
                break;
            }
            Err(e) => {
                println!("❌ Bot error: {}", e);
                println!("🔄 Restarting in 5 seconds...");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
    
    Ok(())
}

async fn run_bot(rpc_config: Arc<RpcConfig>) -> Result<(), Box<dyn std::error::Error>> {
    // Get a working provider
    let provider = rpc_config.get_working_provider().await?;
    let provider = Arc::new(provider);
    
    // Display network information
    let block_number = provider.get_block_number().await?;
    let chain_id = provider.get_chainid().await?;
    println!("📊 Current block: {}", block_number);
    println!("🔗 Chain ID: {}", chain_id);
    
    // Create monitor with retry logic
    let monitor = SwapMonitor::new(provider.clone(), rpc_config.clone());
    
    // Configure DEXs to monitor
    let mut dex_addresses = vec![
        // === V2 DEXs (Routers) ===
        config::QUICKSWAP_ROUTER.to_string(),
        config::SUSHISWAP_ROUTER.to_string(),
        config::APESWAP_ROUTER.to_string(),
    ];
    
    // Add Uniswap V3 pools
    for pool in config::UNISWAP_V3_POOLS {
        dex_addresses.push(pool.to_string());
    }
    
    println!("\n📡 Monitoring DEXs:");
    println!("\n🔹 V2 DEXs (3 routers):");
    println!("  - QuickSwap: {}", &config::QUICKSWAP_ROUTER[..10]);
    println!("  - SushiSwap: {}", &config::SUSHISWAP_ROUTER[..10]);
    println!("  - ApeSwap: {}", &config::APESWAP_ROUTER[..10]);
    
    println!("\n🔹 Uniswap V3 ({} pools):", config::UNISWAP_V3_POOLS.len());
    println!("  - USDC/WETH 0.05%");
    println!("  - USDC/USDT 0.01%");
    println!("  - WMATIC/USDC 0.05%");
    println!("  - WMATIC/USDT 0.05%");
    println!("  - WETH/USDT 0.05%");
    println!("  - WETH/WMATIC 0.05%");
    
    println!("\n📊 Total: {} addresses monitored", dex_addresses.len());
    println!("{}", "=".repeat(50));
    
    // Start monitoring with error handling
    monitor.start_monitoring(dex_addresses).await?;
    
    Ok(())
}
