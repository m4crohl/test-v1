// File: src/main.rs
// Sandwich Bot v2.0 - Complete Polygon Mempool Monitor

use ethers::prelude::*;
use ethers::providers::{Provider, Http, Ws, Middleware};
use ethers::types::{Address, Transaction, U256, H256, BlockNumber};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use chrono::Local;
use std::collections::HashMap;
use std::str::FromStr;

// ANSI colors for terminal output
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RED: &str = "\x1b[31m";
const MAGENTA: &str = "\x1b[35m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

// Polygon DEX Router Addresses
const QUICKSWAP_ROUTER: &str = "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff";
const UNISWAP_V3_ROUTER: &str = "0xE592427A0AEce92De3Edee1F18E0157C05861564";
const SUSHISWAP_ROUTER: &str = "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506";
const APESWAP_ROUTER: &str = "0xC0788A3aD43d79aa53B09c2EaCc313A787d1d607";
const DFYN_ROUTER: &str = "0xA102072A4C07F06EC3B4900FDC4C7B80b6c57429";

// Swap function selectors
const SWAP_EXACT_TOKENS_FOR_TOKENS: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
const SWAP_EXACT_ETH_FOR_TOKENS: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];
const SWAP_EXACT_TOKENS_FOR_ETH: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];
const SWAP_TOKENS_FEE_ON_TRANSFER: [u8; 4] = [0x5c, 0x11, 0xd7, 0x95];
const SWAP_ETH_FOR_EXACT_TOKENS: [u8; 4] = [0xfb, 0x3b, 0xdb, 0x41];
const SWAP_TOKENS_FOR_EXACT_ETH: [u8; 4] = [0x4a, 0x25, 0xd9, 0x4a];
const SWAP_TOKENS_FOR_EXACT_TOKENS: [u8; 4] = [0x88, 0x03, 0xdb, 0xee];

// Configuration structure
#[derive(Clone)]
struct Config {
    http_rpc: String,
    wss_rpc: String,
    use_websocket: bool,
    log_all_txs: bool,
    min_sandwich_size: U256,
    max_gas_price: U256,
    stats_interval: Duration,
}

impl Config {
    fn from_env() -> Self {
        dotenv::dotenv().ok();
        
        Self {
            http_rpc: std::env::var("HTTP_RPC")
                .unwrap_or_else(|_| "https://polygon-rpc.com".to_string()),
            wss_rpc: std::env::var("WSS_RPC")
                .unwrap_or_else(|_| "wss://polygon-bor.publicnode.com".to_string()),
            use_websocket: std::env::var("USE_WEBSOCKET")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            log_all_txs: std::env::var("LOG_ALL_TXS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            min_sandwich_size: U256::from_dec_str(
                &std::env::var("MIN_SANDWICH_SIZE_MATIC")
                    .unwrap_or_else(|_| "0.5".to_string())
                    .replace(".", "")
            ).unwrap_or(U256::from(500_000_000_000_000_000u64)),
            max_gas_price: U256::from(
                std::env::var("MAX_GAS_GWEI")
                    .unwrap_or_else(|_| "500".to_string())
                    .parse::<u64>()
                    .unwrap_or(500) * 1_000_000_000u64
            ),
            stats_interval: Duration::from_secs(
                std::env::var("STATS_INTERVAL_SEC")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30)
            ),
        }
    }
}

// Statistics tracker
#[derive(Default, Clone)]
struct Stats {
    total_txs: u64,
    total_blocks: u64,
    quickswap_txs: u64,
    uniswap_txs: u64,
    sushiswap_txs: u64,
    apeswap_txs: u64,
    dfyn_txs: u64,
    potential_sandwiches: u64,
    large_swaps: u64,
    current_block: u64,
    start_time: Option<chrono::DateTime<Local>>,
}

impl Stats {
    fn print_summary(&self) {
        let runtime = if let Some(start) = self.start_time {
            let duration = Local::now() - start;
            format!("{} min {} sec", duration.num_minutes(), duration.num_seconds() % 60)
        } else {
            "0 min".to_string()
        };
        
        println!("\n{}{}╔════════════════════════════════════════════╗{}", BOLD, CYAN, RESET);
        println!("{}{}║          📊 STATISTICS SUMMARY             ║{}", BOLD, CYAN, RESET);
        println!("{}{}╠════════════════════════════════════════════╣{}", BOLD, CYAN, RESET);
        println!("║ ⏱️  Runtime: {:<31}║", runtime);
        println!("║ 📦 Current Block: #{:<24}║", self.current_block);
        println!("║ 📦 Blocks Processed: {:<22}║", self.total_blocks);
        println!("║ 📄 Total Transactions: {:<20}║", self.total_txs);
        println!("{}║ 🔄 DEX Transactions:{:<23}║{}", GREEN, " ", RESET);
        println!("║   • QuickSwap: {:<28}║", self.quickswap_txs);
        println!("║   • Uniswap V3: {:<27}║", self.uniswap_txs);
        println!("║   • SushiSwap: {:<28}║", self.sushiswap_txs);
        println!("║   • ApeSwap: {:<30}║", self.apeswap_txs);
        println!("║   • Dfyn: {:<33}║", self.dfyn_txs);
        let total_dex = self.quickswap_txs + self.uniswap_txs + 
                       self.sushiswap_txs + self.apeswap_txs + self.dfyn_txs;
        println!("║   {}• Total DEX: {:<28}║{}", BOLD, total_dex, RESET);
        println!("{}║ 🥪 Potential Sandwiches: {:<18}║{}", YELLOW, self.potential_sandwiches, RESET);
        println!("{}║ 🐋 Large Swaps (>100 MATIC): {:<14}║{}", MAGENTA, self.large_swaps, RESET);
        
        // Calculate rates
        if let Some(start) = self.start_time {
            let duration_secs = (Local::now() - start).num_seconds() as f64;
            if duration_secs > 0.0 {
                let tx_per_min = (self.total_txs as f64 / duration_secs) * 60.0;
                let sandwich_per_hour = (self.potential_sandwiches as f64 / duration_secs) * 3600.0;
                println!("║ 📈 TX Rate: {:.1} tx/min{:<19}║", tx_per_min, "");
                println!("║ 📈 Opportunity Rate: {:.1}/hour{:<13}║", sandwich_per_hour, "");
            }
        }
        
        println!("{}{}╚════════════════════════════════════════════╝{}", BOLD, CYAN, RESET);
    }
}

// Swap information structure
#[derive(Debug, Clone)]
struct SwapInfo {
    tx_hash: H256,
    from: Address,
    to: Address,
    router: Address,
    dex_name: String,
    value: U256,
    gas_price: U256,
    swap_type: String,
    input_data: Vec<u8>,
}

// Main monitor structure
pub struct PolygonMonitor {
    config: Config,
    http_provider: Arc<Provider<Http>>,
    ws_provider: Option<Arc<Provider<Ws>>>,
    stats: Arc<tokio::sync::Mutex<Stats>>,
    dex_routers: HashMap<Address, String>,
}

impl PolygonMonitor {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::from_env();
        
        println!("{}{}🚀 Initializing Polygon Sandwich Bot v2.0{}", BOLD, GREEN, RESET);
        println!("{}═══════════════════════════════════════════{}", GREEN, RESET);
        
        // Setup HTTP provider with retry logic
        let http_provider = Provider::<Http>::try_from(&config.http_rpc)?
            .interval(Duration::from_millis(2000));
        let http_provider = Arc::new(http_provider);
        
        // Test connection
        let chain_id = http_provider.get_chainid().await?;
        let block_number = http_provider.get_block_number().await?;
        
        println!("{}✅ Connected to Polygon Network{}", GREEN, RESET);
        println!("   📡 RPC: {}", config.http_rpc);
        println!("   ⛓️  Chain ID: {}", chain_id);
        println!("   📦 Latest Block: #{}", block_number);
        
        // Setup WebSocket provider
        let ws_provider = if config.use_websocket {
            match Provider::<Ws>::connect(&config.wss_rpc).await {
                Ok(ws) => {
                    println!("{}✅ WebSocket connected: {}{}", GREEN, config.wss_rpc, RESET);
                    Some(Arc::new(ws))
                }
                Err(e) => {
                    println!("{}⚠️  WebSocket connection failed: {}{}", YELLOW, e, RESET);
                    println!("   Falling back to HTTP polling mode...");
                    None
                }
            }
        } else {
            println!("{}ℹ️  WebSocket disabled, using HTTP polling{}", CYAN, RESET);
            None
        };
        
        // Setup DEX router mapping
        let mut dex_routers = HashMap::new();
        dex_routers.insert(Address::from_str(QUICKSWAP_ROUTER)?, "QuickSwap".to_string());
        dex_routers.insert(Address::from_str(UNISWAP_V3_ROUTER)?, "Uniswap V3".to_string());
        dex_routers.insert(Address::from_str(SUSHISWAP_ROUTER)?, "SushiSwap".to_string());
        dex_routers.insert(Address::from_str(APESWAP_ROUTER)?, "ApeSwap".to_string());
        dex_routers.insert(Address::from_str(DFYN_ROUTER)?, "Dfyn".to_string());
        
        let mut stats = Stats::default();
        stats.start_time = Some(Local::now());
        stats.current_block = block_number.as_u64();
        
        Ok(Self {
            config,
            http_provider,
            ws_provider,
            stats: Arc::new(tokio::sync::Mutex::new(stats)),
            dex_routers,
        })
    }
    
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n{}{}👀 Starting Mempool Monitoring...{}", BOLD, CYAN, RESET);
        println!("{}═══════════════════════════════════════════{}\n", CYAN, RESET);
        
        // Start statistics printer task
        let stats_clone = self.stats.clone();
        let stats_interval = self.config.stats_interval;
        tokio::spawn(async move {
            loop {
                sleep(stats_interval).await;
                let stats = stats_clone.lock().await;
                stats.print_summary();
            }
        });
        
        // Choose monitoring method
        if let Some(ws) = &self.ws_provider {
            self.monitor_with_websocket(ws.clone()).await?;
        } else {
            self.monitor_with_http().await?;
        }
        
        Ok(())
    }
    
    async fn monitor_with_websocket(&mut self, provider: Arc<Provider<Ws>>) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}📡 Using WebSocket for real-time monitoring{}\n", GREEN, RESET);
        
        // Subscribe to pending transactions
        let mut stream = provider.subscribe_pending_txs().await?;
        
        while let Some(tx_hash) = stream.next().await {
            // Get full transaction details
            match provider.get_transaction(tx_hash).await {
                Ok(Some(tx)) => {
                    self.process_transaction(tx).await;
                }
                Ok(None) => {
                    // Transaction not found (might be dropped)
                }
                Err(e) => {
                    // Log error but continue
                    if self.config.log_all_txs {
                        println!("{}⚠️  Error fetching tx: {}{}", YELLOW, e, RESET);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn monitor_with_http(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}🔄 Using HTTP polling mode (checking every 2 seconds){}\n", YELLOW, RESET);
        
        let mut last_block = {
            let stats = self.stats.lock().await;
            stats.current_block
        };
        
        loop {
            // Get latest block number
            match self.http_provider.get_block_number().await {
                Ok(block_num) => {
                    let current_block = block_num.as_u64();
                    
                    if current_block > last_block {
                        // Process new blocks
                        for block_number in (last_block + 1)..=current_block {
                            match self.http_provider.get_block_with_txs(block_number).await {
                                Ok(Some(block)) => {
                                    let tx_count = block.transactions.len();
                                    
                                    // Update stats
                                    {
                                        let mut stats = self.stats.lock().await;
                                        stats.current_block = block_number;
                                        stats.total_blocks += 1;
                                    }
                                    
                                    if tx_count > 0 {
                                        println!("{}📦 Block #{} | {} transactions{}", 
                                            CYAN, block_number, tx_count, RESET);
                                        
                                        // Process transactions
                                        for tx in block.transactions {
                                            self.process_transaction(tx).await;
                                        }
                                    }
                                }
                                Ok(None) => {
                                    println!("{}⚠️  Block {} not found{}", YELLOW, block_number, RESET);
                                }
                                Err(e) => {
                                    println!("{}❌ Error fetching block: {}{}", RED, e, RESET);
                                }
                            }
                        }
                        last_block = current_block;
                    }
                }
                Err(e) => {
                    println!("{}❌ Error getting block number: {}{}", RED, e, RESET);
                }
            }
            
            // Wait before next poll
            sleep(Duration::from_secs(2)).await;
        }
    }
    
    async fn process_transaction(&mut self, tx: Transaction) {
        // Update transaction counter
        {
            let mut stats = self.stats.lock().await;
            stats.total_txs += 1;
        }
        
        // Check if transaction is to a DEX router
        if let Some(to_address) = tx.to {
            if let Some(dex_name) = self.dex_routers.get(&to_address) {
                // Found a DEX transaction!
                let swap_info = SwapInfo {
                    tx_hash: tx.hash,
                    from: tx.from,
                    to: to_address,
                    router: to_address,
                    dex_name: dex_name.clone(),
                    value: tx.value,
                    gas_price: tx.gas_price.unwrap_or_default(),
                    swap_type: self.decode_swap_type(&tx.input),
                    input_data: tx.input.to_vec(),
                };
                
                // Log the swap
                self.log_dex_transaction(&swap_info).await;
                
                // Update DEX-specific stats
                {
                    let mut stats = self.stats.lock().await;
                    match dex_name.as_str() {
                        "QuickSwap" => stats.quickswap_txs += 1,
                        "Uniswap V3" => stats.uniswap_txs += 1,
                        "SushiSwap" => stats.sushiswap_txs += 1,
                        "ApeSwap" => stats.apeswap_txs += 1,
                        "Dfyn" => stats.dfyn_txs += 1,
                        _ => {}
                    }
                    
                    // Check if it's a large swap
                    if tx.value > U256::from(100_000_000_000_000_000_000u128) { // 100 MATIC
                        stats.large_swaps += 1;
                    }
                }
                
                // Check sandwich opportunity
                if self.is_sandwich_opportunity(&swap_info) {
                    self.log_sandwich_opportunity(&swap_info).await;
                    
                    let mut stats = self.stats.lock().await;
                    stats.potential_sandwiches += 1;
                }
                
            } else if self.config.log_all_txs {
                // Log non-DEX transaction
                println!("   {:?} → {:?} | {} MATIC", 
                    tx.from,
                    to_address,
                    format_ether(tx.value)
                );
            }
        }
    }
    
    fn decode_swap_type(&self, input: &Bytes) -> String {
        if input.len() < 4 {
            return "Unknown".to_string();
        }
        
        let selector = [input[0], input[1], input[2], input[3]];
        
        match selector {
            SWAP_EXACT_TOKENS_FOR_TOKENS => "swapExactTokensForTokens",
            SWAP_EXACT_ETH_FOR_TOKENS => "swapExactETHForTokens",
            SWAP_EXACT_TOKENS_FOR_ETH => "swapExactTokensForETH",
            SWAP_TOKENS_FEE_ON_TRANSFER => "swapTokensSupportingFeeOnTransfer",
            SWAP_ETH_FOR_EXACT_TOKENS => "swapETHForExactTokens",
            SWAP_TOKENS_FOR_EXACT_ETH => "swapTokensForExactETH",
            SWAP_TOKENS_FOR_EXACT_TOKENS => "swapTokensForExactTokens",
            _ => "Unknown swap type"
        }.to_string()
    }
    
    async fn log_dex_transaction(&self, swap: &SwapInfo) {
        let timestamp = Local::now().format("%H:%M:%S");
        
        println!("\n{}[{}] 🔄 {} SWAP DETECTED{}", 
            GREEN, timestamp, swap.dex_name.to_uppercase(), RESET);
        println!("   📄 Hash: {:?}", swap.tx_hash);
        println!("   👤 From: {:?}", swap.from);
        println!("   💰 Value: {} MATIC", format_ether(swap.value));
        println!("   ⛽ Gas Price: {} Gwei", format_units(swap.gas_price, 9));
        println!("   🔧 Type: {}", swap.swap_type);
        
        // Estimate USD value
        let matic_value = swap.value.as_u128() as f64 / 1e18;
        let usd_value = matic_value * 0.85; // Approximate MATIC price
        if usd_value > 10.0 {
            println!("   💵 ~${:.2} USD", usd_value);
        }
    }
    
    fn is_sandwich_opportunity(&self, swap: &SwapInfo) -> bool {
        // Check multiple criteria
        let is_large_enough = swap.value >= self.config.min_sandwich_size;
        let gas_not_too_high = swap.gas_price <= self.config.max_gas_price;
        let is_good_swap_type = matches!(
            swap.swap_type.as_str(),
            "swapExactTokensForTokens" | 
            "swapExactETHForTokens" | 
            "swapExactTokensForETH"
        );
        
        is_large_enough && gas_not_too_high && is_good_swap_type
    }
    
    async fn log_sandwich_opportunity(&self, swap: &SwapInfo) {
        let timestamp = Local::now().format("%H:%M:%S");
        
        println!("\n{}{}🥪 SANDWICH OPPORTUNITY DETECTED! 🥪{}", BOLD, YELLOW, RESET);
        println!("{}═══════════════════════════════════════{}", YELLOW, RESET);
        println!("   ⏰ Time: {}", timestamp);
        println!("   📍 DEX: {}", swap.dex_name);
        println!("   📄 Target TX: {:?}", swap.tx_hash);
        println!("   💰 Size: {} MATIC", format_ether(swap.value));
        
        // Calculate estimated profit
        let matic_value = swap.value.as_u128() as f64 / 1e18;
        let estimated_profit_matic = matic_value * 0.003; // 0.3% average
        let estimated_profit_usd = estimated_profit_matic * 0.85;
        
        println!("   📊 Estimated Profit: {:.4} MATIC (~${:.2})", 
            estimated_profit_matic, estimated_profit_usd);
        
        // Calculate required capital
        let required_capital = matic_value * 2.0; // Need 2x for sandwich
        println!("   💸 Required Capital: {:.2} MATIC", required_capital);
        
        // Risk assessment
        let risk_level = if matic_value < 10.0 {
            "LOW"
        } else if matic_value < 100.0 {
            "MEDIUM"
        } else {
            "HIGH"
        };
        
        let risk_color = match risk_level {
            "LOW" => GREEN,
            "MEDIUM" => YELLOW,
            "HIGH" => RED,
            _ => RESET,
        };
        
        println!("   ⚠️  Risk Level: {}{}{}", risk_color, risk_level, RESET);
        println!("{}═══════════════════════════════════════{}", YELLOW, RESET);
    }
}

// Helper functions
fn format_ether(wei: U256) -> String {
    let ether = wei.as_u128() as f64 / 1e18;
    if ether < 0.0001 {
        format!("{:.8}", ether)
    } else if ether < 1.0 {
        format!("{:.6}", ether)
    } else if ether < 100.0 {
        format!("{:.4}", ether)
    } else {
        format!("{:.2}", ether)
    }
}

fn format_units(value: U256, decimals: u32) -> String {
    let divisor = 10_u128.pow(decimals) as f64;
    let result = value.as_u128() as f64 / divisor;
    format!("{:.2}", result)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "sandwich_bot=info,ethers=warn".to_string())
        )
        .init();
    
    // Clear terminal and show banner
    print!("\x1B[2J\x1B[1;1H");
    
    println!("{}{}╔══════════════════════════════════════════════╗{}", BOLD, CYAN, RESET);
    println!("{}{}║      🥪 POLYGON SANDWICH BOT v2.0 🥪        ║{}", BOLD, CYAN, RESET);
    println!("{}{}║         MEMPOOL MONITOR ACTIVE               ║{}", BOLD, CYAN, RESET);
    println!("{}{}╚══════════════════════════════════════════════╝{}", BOLD, CYAN, RESET);
    println!();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Create and run monitor
    let mut monitor = PolygonMonitor::new().await?;
    
    // Handle graceful shutdown
    tokio::select! {
        result = monitor.run() => {
            if let Err(e) = result {
                eprintln!("{}❌ Fatal Error: {}{}", RED, e, RESET);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n{}📊 Shutting down...{}", CYAN, RESET);
            let stats = monitor.stats.lock().await;
            stats.print_summary();
            println!("\n{}👋 Goodbye!{}", GREEN, RESET);
        }
    }
    
    Ok(())
}
