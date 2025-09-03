use crate::config::{RpcConfig, SWAP_TOPIC_V2, SWAP_TOPIC_V3, REQUEST_DELAY_MS, MIN_SWAP_AMOUNT};
use ethers::prelude::*;
use ethers::providers::Middleware;
use std::sync::Arc;
use tokio::time::{sleep, Duration, timeout};

pub struct SwapMonitor {
    provider: Arc<Provider<Http>>,
    rpc_config: Arc<RpcConfig>,
}

impl SwapMonitor {
    pub fn new(provider: Arc<Provider<Http>>, rpc_config: Arc<RpcConfig>) -> Self {
        Self {
            provider,
            rpc_config,
        }
    }
    
    pub async fn start_monitoring(&self, dex_addresses: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🔍 Starting V2 + V3 monitoring...");
        
        // Create filter for V2 and V3 swap events
        let swap_filter = self.create_swap_filter(&dex_addresses)?;
        
        // Monitoring loop with error handling
        let mut consecutive_errors = 0;
        let max_consecutive_errors = 10;
        let mut swap_count_v2 = 0u64;
        let mut swap_count_v3 = 0u64;
        let mut opportunity_count = 0u64;
        let mut error_count = 0u64;
        let mut last_block = U64::zero();
        
        loop {
            match self.monitor_once(&swap_filter, last_block).await {
                Ok((v2_count, v3_count, opportunities, new_block)) => {
                    consecutive_errors = 0;
                    swap_count_v2 += v2_count as u64;
                    swap_count_v3 += v3_count as u64;
                    opportunity_count += opportunities as u64;
                    last_block = new_block;
                    
                    if v2_count > 0 || v3_count > 0 {
                        println!("📈 New swaps: {} V2, {} V3 ({} opportunities)", 
                                 v2_count, v3_count, opportunities);
                        println!("📊 Total: {} V2, {} V3, {} opportunities", 
                                 swap_count_v2, swap_count_v3, opportunity_count);
                    }
                    
                    // Delay to avoid rate limiting
                    sleep(Duration::from_millis(REQUEST_DELAY_MS * 2)).await;
                }
                Err(e) => {
                    if !e.to_string().contains("Deserialization Error") {
                        consecutive_errors += 1;
                        error_count += 1;
                        println!("⚠️ Monitoring error ({}/{}): {}", 
                                 consecutive_errors, max_consecutive_errors, e);
                    }
                    
                    if consecutive_errors >= max_consecutive_errors {
                        println!("❌ Too many consecutive errors. Switching provider...");
                        
                        match self.rpc_config.get_working_provider().await {
                            Ok(_new_provider) => {
                                println!("✅ New provider obtained");
                                consecutive_errors = 0;
                            }
                            Err(e) => {
                                println!("❌ Unable to get new provider: {}", e);
                                return Err(e.into());
                            }
                        }
                    }
                    
                    let delay = if e.to_string().contains("Deserialization Error") {
                        Duration::from_millis(500)
                    } else {
                        Duration::from_secs(2_u64.pow((consecutive_errors as u32).min(5)))
                    };
                    sleep(delay).await;
                }
            }
            
            // Display statistics every 20 iterations
            if (swap_count_v2 + swap_count_v3) % 20 == 0 && (swap_count_v2 + swap_count_v3) > 0 {
                print_statistics(swap_count_v2, swap_count_v3, opportunity_count, error_count);
            }
        }
    }
    
    async fn monitor_once(&self, filter: &Filter, last_block: U64) 
        -> Result<(usize, usize, usize, U64), Box<dyn std::error::Error>> {
        // Get latest block
        let latest_block = match timeout(
            Duration::from_secs(10),
            self.provider.get_block_number()
        ).await {
            Ok(Ok(block)) => block,
            Ok(Err(e)) => return Err(format!("Provider error: {}", e).into()),
            Err(_) => return Err("Timeout getting block number".into()),
        };
        
        // Skip if we already scanned this block
        if latest_block <= last_block {
            return Ok((0, 0, 0, latest_block));
        }
        
        // Scan only new blocks since last scan
        let from_block = if last_block == U64::zero() {
            latest_block - 20  // First scan: check last 20 blocks
        } else {
            last_block + 1  // Subsequent scans: only new blocks
        };
        let to_block = latest_block;
        
        println!("🔍 Scanning blocks {} to {} ({} new blocks)", 
                 from_block, to_block, to_block - from_block + 1);
        
        let filter = filter.clone()
            .from_block(from_block)
            .to_block(to_block);
        
        // Get logs with timeout
        let logs = match timeout(
            Duration::from_secs(10),
            self.provider.get_logs(&filter)
        ).await {
            Ok(Ok(logs)) => {
                if !logs.is_empty() {
                    println!("📦 {} events found", logs.len());
                }
                logs
            },
            Ok(Err(e)) => {
                if e.to_string().contains("429") {
                    return Err("Rate limit reached (429)".into());
                }
                if e.to_string().contains("Deserialization Error") {
                    return Ok((0, 0, 0, latest_block));
                }
                return Err(format!("Error getting logs: {}", e).into());
            }
            Err(_) => return Err("Timeout getting logs".into()),
        };
        
        // Process logs
        let mut v2_count = 0usize;
        let mut v3_count = 0usize;
        let mut opportunities = 0usize;
        
        for log in &logs {
            let (is_v2, is_v3, is_opportunity) = self.process_swap_log(log).await;
            v2_count += is_v2;
            v3_count += is_v3;
            opportunities += is_opportunity;
        }
        
        Ok((v2_count, v3_count, opportunities, latest_block))
    }
    
    fn create_swap_filter(&self, dex_addresses: &[String]) -> Result<Filter, Box<dyn std::error::Error>> {
        let addresses: Vec<Address> = dex_addresses
            .iter()
            .map(|addr| addr.parse::<Address>())
            .collect::<Result<Vec<_>, _>>()?;
        
        // Create filter for both V2 and V3 swap topics
        let topic_v2 = SWAP_TOPIC_V2.parse::<H256>()?;
        let topic_v3 = SWAP_TOPIC_V3.parse::<H256>()?;
        
        let filter = Filter::new()
            .address(addresses)
            .topic0(vec![topic_v2, topic_v3]);
        
        println!("📋 Filter created for {} addresses (V2 + V3)", dex_addresses.len());
        Ok(filter)
    }
    
    async fn process_swap_log(&self, log: &Log) -> (usize, usize, usize) {
        // Verify it's a swap event
        if log.topics.is_empty() {
            return (0, 0, 0);
        }
        
        let topic_v2 = SWAP_TOPIC_V2.parse::<H256>().unwrap_or_default();
        let topic_v3 = SWAP_TOPIC_V3.parse::<H256>().unwrap_or_default();
        
        // Determine if V2 or V3
        if log.topics[0] == topic_v2 {
            let is_opportunity = self.process_v2_swap(log);
            return (1, 0, is_opportunity);
        } else if log.topics[0] == topic_v3 {
            let is_opportunity = self.process_v3_swap(log);
            return (0, 1, is_opportunity);
        }
        
        (0, 0, 0)
    }
    
    fn process_v2_swap(&self, log: &Log) -> usize {
        let tx_hash = log.transaction_hash.unwrap_or_default();
        let block = log.block_number.unwrap_or_default();
        let router = log.address;
        
        let mut is_opportunity = 0usize;
        
        println!("\n✅ === V2 SWAP DETECTED ===");
        println!("  📦 Block: {}", block);
        println!("  🏦 DEX: {}", self.get_dex_name_v2(router));
        println!("  📝 Transaction: https://polygonscan.com/tx/{:?}", tx_hash);
        
        // Decode V2 amounts
        if log.data.len() >= 128 {
            let amount0_in = U256::from_big_endian(&log.data[0..32]);
            let amount1_in = U256::from_big_endian(&log.data[32..64]);
            let amount0_out = U256::from_big_endian(&log.data[64..96]);
            let amount1_out = U256::from_big_endian(&log.data[96..128]);
            
            if amount0_in > U256::zero() && amount1_out > U256::zero() {
                println!("  💱 Direction: Token0 → Token1");
                println!("  💰 IN:  {} Token0", format_units(amount0_in, 18));
                println!("  💵 OUT: {} Token1", format_units(amount1_out, 18));
                
                if amount0_in > U256::from(MIN_SWAP_AMOUNT) {
                    println!("  🎯 LARGE SWAP! Potential sandwich opportunity");
                    is_opportunity = 1;
                }
            } else if amount1_in > U256::zero() && amount0_out > U256::zero() {
                println!("  💱 Direction: Token1 → Token0");
                println!("  💰 IN:  {} Token1", format_units(amount1_in, 18));
                println!("  💵 OUT: {} Token0", format_units(amount0_out, 18));
                
                if amount1_in > U256::from(MIN_SWAP_AMOUNT) {
                    println!("  🎯 LARGE SWAP! Potential sandwich opportunity");
                    is_opportunity = 1;
                }
            }
        }
        
        // Sender and recipient
        if log.topics.len() >= 3 {
            println!("  👤 From: {:?}", Address::from(log.topics[1]));
            println!("  📬 To: {:?}", Address::from(log.topics[2]));
        }
        
        println!("========================");
        is_opportunity
    }
    
    fn process_v3_swap(&self, log: &Log) -> usize {
        let tx_hash = log.transaction_hash.unwrap_or_default();
        let block = log.block_number.unwrap_or_default();
        let pool = log.address;
        
        let mut is_opportunity = 0usize;
        
        println!("\n✅ === V3 SWAP DETECTED ===");
        println!("  📦 Block: {}", block);
        println!("  🏊 Pool: {}", self.get_pool_name_v3(pool));
        println!("  📝 Transaction: https://polygonscan.com/tx/{:?}", tx_hash);
        
        // Decode V3 amounts (different structure)
        if log.data.len() >= 160 {
            // V3 Swap event: (address sender, address recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)
            let amount0 = I256::from_raw(U256::from_big_endian(&log.data[0..32]));
            let amount1 = I256::from_raw(U256::from_big_endian(&log.data[32..64]));
            
            // Determine direction based on signs
            if amount0.is_negative() && !amount1.is_negative() {
                println!("  💱 Direction: Token0 → Token1");
                let amount0_abs = amount0.abs().into_raw();
                println!("  💰 IN:  {} Token0", format_units(amount0_abs, 18));
                println!("  💵 OUT: {} Token1", format_units(amount1.into_raw(), 18));
                
                if amount0_abs > U256::from(MIN_SWAP_AMOUNT) {
                    println!("  🎯 LARGE V3 SWAP! High-value sandwich opportunity");
                    is_opportunity = 1;
                }
            } else if amount1.is_negative() && !amount0.is_negative() {
                println!("  💱 Direction: Token1 → Token0");
                let amount1_abs = amount1.abs().into_raw();
                println!("  💰 IN:  {} Token1", format_units(amount1_abs, 18));
                println!("  💵 OUT: {} Token0", format_units(amount0.into_raw(), 18));
                
                if amount1_abs > U256::from(MIN_SWAP_AMOUNT) {
                    println!("  🎯 LARGE V3 SWAP! High-value sandwich opportunity");
                    is_opportunity = 1;
                }
            }
            
            // Price after swap (sqrtPriceX96)
            if log.data.len() >= 96 {
                println!("  📊 Pool price updated");
            }
        }
        
        // Sender and recipient from topics
        if log.topics.len() >= 3 {
            println!("  👤 Sender: {:?}", Address::from(log.topics[1]));
            println!("  📬 Recipient: {:?}", Address::from(log.topics[2]));
        }
        
        println!("========================");
        is_opportunity
    }
    
    fn get_dex_name_v2(&self, router: Address) -> String {
        let router_str = format!("{:?}", router).to_lowercase();
        match router_str.as_str() {
            "0xa5e0829caced8ffdd4de3c43696c57f7d7a678ff" => "QuickSwap".to_string(),
            "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506" => "SushiSwap".to_string(),
            "0xc0788a3ad43d79aa53b09c2eacc313a787d1d607" => "ApeSwap".to_string(),
            _ => format!("Unknown V2 DEX ({:?})", router),
        }
    }
    
    fn get_pool_name_v3(&self, pool: Address) -> String {
        let pool_str = format!("{:?}", pool).to_lowercase();
        match pool_str.as_str() {
            "0x45dda9cb7c25131df268515131f647d726f50608" => "USDC/WETH 0.05%".to_string(),
            "0x0e44ceb592acfc5d3f09d996302eb4c499ff8c10" => "USDC/USDT 0.01%".to_string(),
            "0x88f3c15523544835ff6c738ddb30995339ad57d6" => "WMATIC/USDC 0.05%".to_string(),
            "0xa374094527e1673a86de625aa59517c5de346d32" => "WMATIC/USDT 0.05%".to_string(),
            "0x167384319b41f7094e62f7506409eb38079abff8" => "WETH/USDT 0.05%".to_string(),
            "0x86f1d8390222a3691c28938ec7404a1661e618e0" => "WETH/WMATIC 0.05%".to_string(),
            _ => format!("Unknown V3 Pool ({:?})", pool),
        }
    }
}

// Helper function to display statistics
fn print_statistics(swap_count_v2: u64, swap_count_v3: u64, opportunities: u64, error_count: u64) {
    println!("\n📊 === STATISTICS === 📊");
    println!("  ✅ V2 Swaps detected: {}", swap_count_v2);
    println!("  ✅ V3 Swaps detected: {}", swap_count_v3);
    println!("  🎯 Opportunities found: {}", opportunities);
    println!("  📈 Total swaps: {}", swap_count_v2 + swap_count_v3);
    println!("  ❌ Errors: {}", error_count);
    let total = swap_count_v2 + swap_count_v3 + error_count;
    let success_rate = if total > 0 {
        ((swap_count_v2 + swap_count_v3) as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    println!("  📈 Success rate: {:.2}%", success_rate);
    
    if opportunities > 0 {
        let opportunity_rate = (opportunities as f64 / (swap_count_v2 + swap_count_v3) as f64) * 100.0;
        println!("  💰 Opportunity rate: {:.2}%", opportunity_rate);
    }
    
    println!("==============================");
}

// Helper function to format amounts
fn format_units(amount: U256, decimals: u8) -> String {
    if amount == U256::zero() {
        return "0".to_string();
    }
    
    let divisor = U256::from(10).pow(U256::from(decimals));
    let whole = amount / divisor;
    let fraction = amount % divisor;
    
    if fraction == U256::zero() {
        format!("{}", whole)
    } else {
        let frac_str = format!("{:0>width$}", fraction, width = decimals as usize);
        let truncated = &frac_str[..4.min(frac_str.len())];
        format!("{}.{}", whole, truncated.trim_end_matches('0'))
    }
}
