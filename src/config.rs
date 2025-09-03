use std::sync::atomic::{AtomicUsize, Ordering};
use ethers::providers::{Provider, Http, Middleware};
use std::time::Duration;
use tokio::time::sleep;

pub struct RpcConfig {
    pub http_endpoints: Vec<String>,
    current_http: AtomicUsize,
}

impl RpcConfig {
    pub fn new() -> Self {
        Self {
            http_endpoints: vec![
                // Free RPC endpoints for Polygon
                "https://polygon-rpc.com".to_string(),
                "https://polygon-bor.publicnode.com".to_string(),
                "https://polygon-mainnet.public.blastapi.io".to_string(),
                "https://1rpc.io/matic".to_string(),
                "https://polygon.llamarpc.com".to_string(),
                "https://rpc.ankr.com/polygon".to_string(),
            ],
            current_http: AtomicUsize::new(0),
        }
    }
    
    pub fn get_next_http_endpoint(&self) -> String {
        let index = self.current_http.fetch_add(1, Ordering::SeqCst) % self.http_endpoints.len();
        self.http_endpoints[index].clone()
    }
    
    pub async fn get_working_provider(&self) -> Result<Provider<Http>, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        let max_attempts = self.http_endpoints.len() * 2;
        
        while attempts < max_attempts {
            let endpoint = self.get_next_http_endpoint();
            println!("🔄 Attempting connection to: {}", endpoint);
            
            match Provider::<Http>::try_from(endpoint.as_str()) {
                Ok(provider) => {
                    // Quick test to verify provider works
                    match provider.get_block_number().await {
                        Ok(block) => {
                            println!("✅ Successfully connected! Current block: {}", block);
                            return Ok(provider);
                        }
                        Err(e) => {
                            println!("⚠️ Provider not functional: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Connection failed: {}", e);
                }
            }
            
            attempts += 1;
            
            // Exponential backoff
            let delay = Duration::from_millis(100 * (2_u64.pow((attempts as u32).min(5))));
            sleep(delay).await;
        }
        
        Err("Unable to connect to any RPC after multiple attempts".into())
    }
}

// === DEX CONFIGURATION ===

// Uniswap V2 and fork routers
pub const QUICKSWAP_ROUTER: &str = "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff";
pub const SUSHISWAP_ROUTER: &str = "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506";
pub const APESWAP_ROUTER: &str = "0xC0788A3aD43d79aa53B09c2EaCc313A787d1d607";

// Popular Uniswap V3 pools on Polygon
pub const UNISWAP_V3_POOLS: &[&str] = &[
    "0x45dDa9cb7c25131DF268515131f647d726f50608", // USDC/WETH 0.05%
    "0x0e44cEb592AcFC5D3F09D996302eB4C499ff8c10", // USDC/USDT 0.01%
    "0x88f3C15523544835fF6c738DDb30995339AD57d6", // WMATIC/USDC 0.05%
    "0xA374094527e1673A86dE625aa59517c5dE346d32", // WMATIC/USDT 0.05%
    "0x167384319B41F7094e62f7506409Eb38079AbfF8", // WETH/USDT 0.05%
    "0x86f1d8390222A3691C28938eC7404A1661E618e0", // WETH/WMATIC 0.05%
];

// Swap event topics
pub const SWAP_TOPIC_V2: &str = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";
pub const SWAP_TOPIC_V3: &str = "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67";

// Rate limiting configuration
pub const REQUEST_DELAY_MS: u64 = 100; // 100ms between requests

// Minimum amounts for sandwich opportunities (in wei)
pub const MIN_PROFIT_THRESHOLD: u64 = 10_000_000_000_000_000; // 0.01 ETH
pub const MIN_SWAP_AMOUNT: u64 = 100_000_000_000_000_000_000; // 100 tokens
