mod config;
mod monitor;
mod decode;

use config::RpcConfig;
use monitor::SwapMonitor;
use ethers::prelude::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Démarrage du Sandwich Bot v0.1");
    println!("📍 Cible: Polygon Mainnet");
    println!("=" .repeat(50));
    
    // Initialiser la configuration RPC
    let rpc_config = Arc::new(RpcConfig::new());
    
    // Boucle principale avec reconnexion automatique
    loop {
        match run_bot(rpc_config.clone()).await {
            Ok(_) => {
                println!("✅ Bot terminé normalement");
                break;
            }
            Err(e) => {
                println!("❌ Erreur du bot: {}", e);
                println!("🔄 Redémarrage dans 5 secondes...");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
    
    Ok(())
}

async fn run_bot(rpc_config: Arc<RpcConfig>) -> Result<(), Box<dyn std::error::Error>> {
    // Obtenir un provider fonctionnel
    let provider = rpc_config.get_working_provider().await?;
    let provider = Arc::new(provider);
    
    // Afficher les informations du réseau
    let block_number = provider.get_block_number().await?;
    let chain_id = provider.get_chainid().await?;
    println!("📊 Block actuel: {}", block_number);
    println!("🔗 Chain ID: {}", chain_id);
    
    // Créer le moniteur avec retry logic
    let monitor = SwapMonitor::new(provider.clone(), rpc_config.clone());
    
    // Configuration des DEX à surveiller
    let dex_routers = vec![
        config::UNISWAP_V2_ROUTER.to_string(),
        config::SUSHISWAP_ROUTER.to_string(),
    ];
    
    println!("\n📡 Surveillance des DEX:");
    for router in &dex_routers {
        println!("  - {}", router);
    }
    
    // Lancer le monitoring avec gestion d'erreur
    monitor.start_monitoring(dex_routers).await?;
    
    Ok(())
}
