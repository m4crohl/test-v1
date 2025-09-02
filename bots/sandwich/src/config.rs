use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use ethers::providers::{Provider, Http, Ws};
use std::time::Duration;
use tokio::time::sleep;

pub struct RpcConfig {
    pub http_endpoints: Vec<String>,
    pub ws_endpoints: Vec<String>,
    current_http: AtomicUsize,
    current_ws: AtomicUsize,
}

impl RpcConfig {
    pub fn new() -> Self {
        Self {
            http_endpoints: vec![
                // Endpoints gratuits pour Polygon
                "https://polygon-rpc.com".to_string(),
                "https://rpc.ankr.com/polygon".to_string(),
                "https://polygon-mainnet.public.blastapi.io".to_string(),
                "https://1rpc.io/matic".to_string(),
                "https://polygon.llamarpc.com".to_string(),
                "https://rpc-mainnet.matic.quiknode.pro".to_string(),
                "https://polygon-mainnet.g.alchemy.com/v2/demo".to_string(),
                "https://polygon.blockpi.network/v1/rpc/public".to_string(),
                "https://rpc-mainnet.maticvigil.com".to_string(),
                "https://polygon-bor.publicnode.com".to_string(),
            ],
            ws_endpoints: vec![
                "wss://polygon-bor.publicnode.com".to_string(),
                "wss://polygon-mainnet.g.alchemy.com/v2/demo".to_string(),
                // Ajouter d'autres WSS si disponibles
            ],
            current_http: AtomicUsize::new(0),
            current_ws: AtomicUsize::new(0),
        }
    }
    
    pub fn get_next_http_endpoint(&self) -> String {
        let index = self.current_http.fetch_add(1, Ordering::SeqCst) % self.http_endpoints.len();
        self.http_endpoints[index].clone()
    }
    
    pub fn get_next_ws_endpoint(&self) -> String {
        let index = self.current_ws.fetch_add(1, Ordering::SeqCst) % self.ws_endpoints.len();
        self.ws_endpoints[index].clone()
    }
    
    pub async fn get_working_provider(&self) -> Result<Provider<Http>, Box<dyn std::error::Error>> {
        let mut attempts = 0;
        let max_attempts = self.http_endpoints.len() * 2;
        
        while attempts < max_attempts {
            let endpoint = self.get_next_http_endpoint();
            println!("🔄 Tentative de connexion à: {}", endpoint);
            
            match Provider::<Http>::try_from(endpoint.as_str()) {
                Ok(provider) => {
                    // Test rapide pour vérifier que le provider fonctionne
                    match provider.get_block_number().await {
                        Ok(block) => {
                            println!("✅ Connecté avec succès! Block actuel: {}", block);
                            return Ok(provider);
                        }
                        Err(e) => {
                            println!("⚠️ Provider non fonctionnel: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Échec de connexion: {}", e);
                }
            }
            
            attempts += 1;
            
            // Backoff exponentiel
            let delay = Duration::from_millis(100 * (2_u64.pow(attempts.min(5))));
            sleep(delay).await;
        }
        
        Err("Impossible de se connecter à un RPC après plusieurs tentatives".into())
    }
    
    pub async fn get_working_ws_provider(&self) -> Result<Provider<Ws>, Box<dyn std::error::Error>> {
        for endpoint in &self.ws_endpoints {
            println!("🔄 Tentative WebSocket: {}", endpoint);
            
            match Provider::<Ws>::connect(endpoint).await {
                Ok(provider) => {
                    println!("✅ WebSocket connecté!");
                    return Ok(provider);
                }
                Err(e) => {
                    println!("❌ Échec WebSocket: {}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
        
        Err("Aucun endpoint WebSocket disponible".into())
    }
}

// Configuration des DEX à monitorer
pub const UNISWAP_V2_ROUTER: &str = "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff"; // QuickSwap sur Polygon
pub const SUSHISWAP_ROUTER: &str = "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506";

// Topics pour les événements Swap
pub const SWAP_TOPIC: &str = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";

// Configuration des limites
pub const MAX_REQUESTS_PER_SECOND: u32 = 10;
pub const REQUEST_DELAY_MS: u64 = 100; // 100ms entre chaque requête
pub const MAX_RETRIES: u32 = 3;
pub const BACKOFF_BASE_MS: u64 = 100;
