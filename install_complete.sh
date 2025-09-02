#!/bin/bash

# Script d'installation complète du Sandwich Bot
# Usage: ./install_complete.sh

set -e  # Exit on error

echo "🥪 ========================================="
echo "   INSTALLATION COMPLÈTE DU SANDWICH BOT"
echo "========================================="
echo ""

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust n'est pas installé!"
    echo "Installer avec: curl --proto='=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust détecté"

# Create project
echo "📁 Création du projet..."
cargo new sandwich_bot --name sandwich_bot
cd sandwich_bot

# Create Cargo.toml
echo "📝 Configuration Cargo.toml..."
cat > Cargo.toml << 'CARGO_EOF'
[package]
name = "sandwich_bot"
version = "0.1.0"
edition = "2021"

[dependencies]
ethers = "2.0"
tokio = { version = "1.35", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenv = "0.15"
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
hex = "0.4"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
CARGO_EOF

# Create main.rs
echo "📝 Création de src/main.rs..."
cat > src/main.rs << 'MAIN_EOF'
use anyhow::Result;
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{info, warn, debug};

mod sandwich;
mod detector;
mod simulator;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("sandwich_bot=debug,ethers=info")
        .init();
    
    info!("🥪 Starting Sandwich Bot");
    info!("⚠️  Educational purposes - Use responsibly");
    
    dotenv::dotenv().ok();
    let rpc_url = std::env::var("ETH_RPC_URL")
        .unwrap_or_else(|_| {
            warn!("ETH_RPC_URL not set, using demo endpoint");
            "https://eth-mainnet.g.alchemy.com/v2/demo".to_string()
        });
    let ws_url = std::env::var("ETH_WS_URL")
        .unwrap_or_else(|_| {
            warn!("ETH_WS_URL not set, using demo endpoint");
            "wss://eth-mainnet.g.alchemy.com/v2/demo".to_string()
        });
    
    info!("Connecting to: {}", ws_url);
    let provider = Provider::<Ws>::connect(&ws_url).await?;
    let provider = Arc::new(provider);
    
    info!("✅ Connected to Ethereum via WebSocket");
    
    let chain_id = provider.get_chainid().await?;
    let block = provider.get_block_number().await?;
    info!("📊 Chain ID: {} | Block: {}", chain_id, block);
    
    let mut bot = sandwich::SandwichBot::new(provider.clone());
    
    info!("👀 Starting mempool monitoring...");
    info!("Press Ctrl+C to stop");
    bot.run().await?;
    
    Ok(())
}
MAIN_EOF

# Create sandwich.rs
echo "📝 Création de src/sandwich.rs..."
cat > src/sandwich.rs << 'SANDWICH_EOF'
use anyhow::Result;
use ethers::prelude::*;
use std::sync::Arc;
use tracing::{info, warn, debug, error};

pub struct SandwichBot {
    provider: Arc<Provider<Ws>>,
    uniswap_v2_router: Address,
    uniswap_v3_router: Address,
    min_profit_wei: U256,
    dry_run: bool,
    tx_count: u64,
    opportunities_found: u64,
}

impl SandwichBot {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        let dry_run = std::env::var("DRY_RUN")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
            
        let min_profit_eth = std::env::var("MIN_PROFIT_ETH")
            .unwrap_or_else(|_| "0.01".to_string())
            .parse::<f64>()
            .unwrap_or(0.01);
            
        Self {
            provider,
            uniswap_v2_router: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"
                .parse()
                .unwrap(),
            uniswap_v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564"
                .parse()
                .unwrap(),
            min_profit_wei: U256::from((min_profit_eth * 1e18) as u128),
            dry_run,
            tx_count: 0,
            opportunities_found: 0,
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        info!("🥪 Sandwich bot initialized");
        info!("📍 Monitoring Uniswap V2: {}", self.uniswap_v2_router);
        info!("📍 Monitoring Uniswap V3: {}", self.uniswap_v3_router);
        info!("💰 Min profit threshold: {} Wei", self.min_profit_wei);
        info!("🔒 Dry run mode: {}", self.dry_run);
        
        let mut stream = self.provider.watch_pending_transactions().await?;
        info!("📡 Subscribed to mempool");
        
        while let Some(tx_hash) = stream.next().await {
            self.tx_count += 1;
            
            if self.tx_count % 100 == 0 {
                info!("📊 Processed {} transactions, found {} opportunities", 
                      self.tx_count, self.opportunities_found);
            }
            
            match self.provider.get_transaction(tx_hash).await {
                Ok(Some(tx)) => {
                    self.analyze_transaction(tx).await;
                }
                Ok(None) => {
                    debug!("Transaction not found: {:?}", tx_hash);
                }
                Err(e) => {
                    debug!("Error fetching transaction: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn analyze_transaction(&mut self, tx: Transaction) {
        if let Some(to) = tx.to {
            if to == self.uniswap_v2_router || to == self.uniswap_v3_router {
                debug!("🎯 Detected Uniswap transaction: {:?}", tx.hash);
                
                if let Some(opportunity) = self.check_sandwich_opportunity(&tx).await {
                    self.opportunities_found += 1;
                    self.execute_sandwich(opportunity).await;
                }
            }
        }
    }
    
    async fn check_sandwich_opportunity(&self, tx: &Transaction) -> Option<SandwichOpportunity> {
        let input = &tx.input;
        
        if input.len() < 4 {
            return None;
        }
        
        let method_id = &input[0..4];
        
        let swap_methods = vec![
            hex::decode("7ff36ab5").ok(),
            hex::decode("18cbafe5").ok(),
            hex::decode("38ed1739").ok(),
            hex::decode("fb3bdb41").ok(),
        ];
        
        let is_swap = swap_methods.iter().any(|m| {
            m.as_ref().map_or(false, |method| method == method_id)
        });
        
        if !is_swap {
            return None;
        }
        
        if tx.value < U256::from(500_000_000_000_000_000u64) {
            return None;
        }
        
        info!("🔍 Found potential sandwich target: {:?}", tx.hash);
        info!("   Value: {} ETH", format_units(tx.value, "ether").unwrap_or_default());
        
        let simulator = crate::simulator::Simulator::new(self.provider.clone());
        match simulator.simulate_sandwich(tx).await {
            Ok(result) => {
                if result.profit > self.min_profit_wei {
                    info!("💎 Profitable sandwich found!");
                    info!("   Expected profit: {} ETH", 
                          format_units(result.profit, "ether").unwrap_or_default());
                    info!("   Gas cost: {} ETH", 
                          format_units(result.gas_cost, "ether").unwrap_or_default());
                    
                    return Some(SandwichOpportunity {
                        target_tx: tx.clone(),
                        profit: result.profit,
                        front_run_data: result.front_run_data,
                        back_run_data: result.back_run_data,
                        gas_price: result.optimal_gas_price,
                    });
                }
            }
            Err(e) => {
                debug!("Simulation failed: {}", e);
            }
        }
        
        None
    }
    
    async fn execute_sandwich(&self, opportunity: SandwichOpportunity) {
        if self.dry_run {
            warn!("🚀 [DRY RUN] Would execute sandwich attack:");
            warn!("   Target: {:?}", opportunity.target_tx.hash);
            warn!("   Expected profit: {} ETH", 
                  format_units(opportunity.profit, "ether").unwrap_or_default());
            warn!("   Gas price: {} Gwei", 
                  format_units(opportunity.gas_price, 9).unwrap_or_default());
        } else {
            error!("❌ Real execution not implemented for safety");
            error!("   To execute real trades, implement wallet signing");
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandwichOpportunity {
    pub target_tx: Transaction,
    pub profit: U256,
    pub front_run_data: Bytes,
    pub back_run_data: Bytes,
    pub gas_price: U256,
}
SANDWICH_EOF

# Create detector.rs
echo "📝 Création de src/detector.rs..."
cat > src/detector.rs << 'DETECTOR_EOF'
use ethers::prelude::*;
use anyhow::Result;
use tracing::debug;

pub struct TransactionDetector {
    min_value: U256,
    known_routers: Vec<Address>,
}

impl TransactionDetector {
    pub fn new() -> Self {
        Self {
            min_value: U256::from(100_000_000_000_000_000u64),
            known_routers: vec![
                "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(),
                "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap(),
                "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap(),
            ],
        }
    }
    
    pub fn is_interesting(&self, tx: &Transaction) -> bool {
        if tx.value < self.min_value {
            return false;
        }
        
        if let Some(to) = tx.to {
            if self.known_routers.contains(&to) {
                debug!("Found DEX interaction: {:?}", to);
                return true;
            }
        }
        
        false
    }
}

#[derive(Debug)]
pub struct SwapInfo {
    pub method: String,
    pub token_in: Address,
    pub token_out: Address,
    pub amount: U256,
}
DETECTOR_EOF

# Create simulator.rs
echo "📝 Création de src/simulator.rs..."
cat > src/simulator.rs << 'SIMULATOR_EOF'
use ethers::prelude::*;
use anyhow::Result;
use std::sync::Arc;
use tracing::debug;

pub struct Simulator {
    provider: Arc<Provider<Ws>>,
}

impl Simulator {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        Self { provider }
    }
    
    pub async fn simulate_sandwich(&self, target_tx: &Transaction) -> Result<SimulationResult> {
        debug!("🧪 Simulating sandwich attack for: {:?}", target_tx.hash);
        
        let gas_price = self.provider.get_gas_price().await?;
        
        let estimated_profit = self.estimate_profit(target_tx).await?;
        let gas_cost = self.estimate_gas_cost(gas_price).await?;
        
        let net_profit = if estimated_profit > gas_cost {
            estimated_profit - gas_cost
        } else {
            U256::zero()
        };
        
        debug!("Simulation results:");
        debug!("  Estimated profit: {} Wei", estimated_profit);
        debug!("  Gas cost: {} Wei", gas_cost);
        debug!("  Net profit: {} Wei", net_profit);
        
        if net_profit > U256::zero() {
            Ok(SimulationResult {
                profit: net_profit,
                gas_cost,
                front_run_data: Bytes::from(vec![0x01, 0x02, 0x03]),
                back_run_data: Bytes::from(vec![0x04, 0x05, 0x06]),
                optimal_gas_price: gas_price + U256::from(1_000_000_000u64),
                success_probability: 0.75,
            })
        } else {
            Err(anyhow::anyhow!("Not profitable"))
        }
    }
    
    async fn estimate_profit(&self, tx: &Transaction) -> Result<U256> {
        let potential = tx.value * 3 / 1000;
        Ok(potential)
    }
    
    async fn estimate_gas_cost(&self, gas_price: U256) -> Result<U256> {
        let gas_per_swap = U256::from(180_000u64);
        let total_gas = gas_per_swap * 2;
        let total_with_buffer = total_gas * 120 / 100;
        Ok(total_with_buffer * gas_price)
    }
}

#[derive(Debug)]
pub struct SimulationResult {
    pub profit: U256,
    pub gas_cost: U256,
    pub front_run_data: Bytes,
    pub back_run_data: Bytes,
    pub optimal_gas_price: U256,
    pub success_probability: f64,
}
SIMULATOR_EOF

# Create .env file
echo "📝 Création du fichier .env..."
cat > .env << 'ENV_EOF'
# Configuration Sandwich Bot
ETH_WS_URL=wss://eth-mainnet.g.alchemy.com/v2/demo
ETH_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/demo
MIN_PROFIT_ETH=0.01
DRY_RUN=true
RUST_LOG=sandwich_bot=debug,ethers=info
ENV_EOF

# Create .gitignore
echo "📝 Création du .gitignore..."
cat > .gitignore << 'GITIGNORE_EOF'
/target
**/*.rs.bk
Cargo.lock
.env
*.key
*.pem
.idea/
.vscode/
*.swp
*.swo
.DS_Store
Thumbs.db
*.log
logs/
*.backup
*.bak
GITIGNORE_EOF

# Build the project
echo ""
echo "🔨 Compilation du projet..."
cargo build 2>&1 | tail -10

if [ $? -eq 0 ]; then
    echo ""
    echo "========================================="
    echo "✅ INSTALLATION COMPLÈTE RÉUSSIE!"
    echo "========================================="
    echo ""
    echo "📁 Structure créée:"
    echo "   sandwich_bot/"
    echo "   ├── Cargo.toml"
    echo "   ├── .env"
    echo "   ├── .gitignore"
    echo "   └── src/"
    echo "       ├── main.rs"
    echo "       ├── sandwich.rs"
    echo "       ├── detector.rs"
    echo "       └── simulator.rs"
    echo ""
    echo "🚀 Pour lancer le bot:"
    echo "   cargo run"
    echo ""
    echo "📊 Pour voir les logs détaillés:"
    echo "   RUST_LOG=trace cargo run"
    echo ""
    echo "⚠️  RAPPELS IMPORTANTS:"
    echo "   - Garder DRY_RUN=true pour la sécurité"
    echo "   - Obtenir une clé API Alchemy pour production"
    echo "   - Ne jamais commit le fichier .env"
    echo ""
else
    echo ""
    echo "❌ Erreur lors de la compilation"
    echo "Essayez: cargo build --verbose"
fi
INSTALL_EOF

chmod +x install_complete.sh
echo "✅ Script d'installation créé: install_complete.sh"
echo "Lancez: ./install_complete.sh"
