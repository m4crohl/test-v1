// Fichier: src/bin/test_connection.rs
// Pour tester: cargo run --bin test_connection

use sandwich_bot::config::RpcConfig;
use ethers::providers::{Provider, Http};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Test des endpoints RPC pour Polygon");
    println!("=" .repeat(50));
    
    let config = RpcConfig::new();
    
    println!("📝 Test de {} endpoints HTTP\n", config.http_endpoints.len());
    
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
        
        // Petite pause entre les tests
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    println!("\n" + &"=" .repeat(50));
    println!("🎯 Test de connexion avec rotation automatique:");
    
    match config.get_working_provider().await {
        Ok(provider) => {
            let block = provider.get_block_number().await?;
            println!("✅ Connexion réussie! Block actuel: {}", block);
            
            // Test de récupération de logs
            println!("\n📊 Test de récupération des logs...");
            let filter = ethers::types::Filter::new()
                .from_block(block - 5)
                .to_block(block);
            
            match provider.get_logs(&filter).await {
                Ok(logs) => {
                    println!("✅ {} logs récupérés", logs.len());
                }
                Err(e) => {
                    println!("⚠️ Erreur lors de la récupération des logs: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Impossible de trouver un provider fonctionnel: {}", e);
        }
    }
    
    Ok(())
}
