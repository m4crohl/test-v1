// File: src/bin/test_connection.rs
// Usage: cargo run --bin test_connection

use sandwich_bot::config::RpcConfig;
use ethers::providers::{Provider, Http, Middleware};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing RPC endpoints for Polygon");
    println!("{}", "=".repeat(50));
    
    let config = RpcConfig::new();
    
    println!("📝 Testing {} HTTP endpoints\n", config.http_endpoints.len());
    
    for endpoint in &config.http_endpoints {
        print!("Testing {} ... ", endpoint);
        let start = Instant::now();
        
        match Provider::<Http>::try_from(endpoint.as_str()) {
            Ok(provider) => {
                match provider.get_block_number().await {
                    Ok(block) => {
                        let latency = start.elapsed();
                        println!("✅ OK (Block: {}, Latency: {:?})", block, latency);
                    }
                    Err(e) => {
                        println!("❌ FAIL - {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ FAIL - Connection error: {}", e);
            }
        }
        
        // Small delay between tests
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    println!("\n{}", "=".repeat(50));
    println!("🎯 Testing automatic rotation:");
    
    match config.get_working_provider().await {
        Ok(provider) => {
            let block = provider.get_block_number().await?;
            println!("✅ Successfully connected! Current block: {}", block);
            
            // Test log retrieval
            println!("\n📊 Testing log retrieval...");
            let filter = ethers::types::Filter::new()
                .from_block(block - 5)
                .to_block(block);
            
            match provider.get_logs(&filter).await {
                Ok(logs) => {
                    println!("✅ {} logs retrieved", logs.len());
                }
                Err(e) => {
                    println!("⚠️ Error retrieving logs: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Unable to find working provider: {}", e);
        }
    }
    
    Ok(())
}
