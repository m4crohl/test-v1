use crate::config::{RpcConfig, SWAP_TOPIC, REQUEST_DELAY_MS, MAX_RETRIES};
use crate::decode::SwapDecoder;
use ethers::prelude::*;
use std::sync::Arc;
use tokio::time::{sleep, Duration, timeout};
use std::collections::HashMap;

pub struct SwapMonitor {
    provider: Arc<Provider<Http>>,
    rpc_config: Arc<RpcConfig>,
    decoder: SwapDecoder,
    swap_count: u64,
    error_count: u64,
}

impl SwapMonitor {
    pub fn new(provider: Arc<Provider<Http>>, rpc_config: Arc<RpcConfig>) -> Self {
        Self {
            provider,
            rpc_config,
            decoder: SwapDecoder::new(),
            swap_count: 0,
            error_count: 0,
        }
    }
    
    pub async fn start_monitoring(&self, dex_routers: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🔍 Démarrage du monitoring...");
        
        // Créer le filtre pour les événements Swap
        let swap_filter = self.create_swap_filter(&dex_routers)?;
        
        // Monitoring en boucle avec gestion d'erreur
        let mut consecutive_errors = 0;
        let max_consecutive_errors = 10;
        
        loop {
            match self.monitor_once(&swap_filter).await {
                Ok(logs_count) => {
                    consecutive_errors = 0; // Reset le compteur d'erreurs
                    
                    if logs_count > 0 {
                        println!("📈 {} nouveaux swaps détectés", logs_count);
                    }
                    
                    // Pause pour éviter le rate limiting
                    sleep(Duration::from_millis(REQUEST_DELAY_MS)).await;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    println!("⚠️ Erreur de monitoring ({}/{}): {}", 
                             consecutive_errors, max_consecutive_errors, e);
                    
                    if consecutive_errors >= max_consecutive_errors {
                        println!("❌ Trop d'erreurs consécutives. Changement de provider...");
                        
                        // Essayer d'obtenir un nouveau provider
                        match self.rpc_config.get_working_provider().await {
                            Ok(new_provider) => {
                                println!("✅ Nouveau provider obtenu");
                                // Note: Dans une vraie implémentation, on devrait pouvoir
                                // changer le provider de l'instance
                                consecutive_errors = 0;
                            }
                            Err(e) => {
                                println!("❌ Impossible d'obtenir un nouveau provider: {}", e);
                                return Err(e.into());
                            }
                        }
                    }
                    
                    // Backoff exponentiel
                    let delay = Duration::from_secs(2_u64.pow(consecutive_errors.min(5)));
                    println!("⏳ Attente de {:?} avant de réessayer...", delay);
                    sleep(delay).await;
                }
            }
            
            // Afficher les statistiques toutes les 10 itérations
            if self.swap_count % 10 == 0 && self.swap_count > 0 {
                self.print_statistics();
            }
        }
    }
    
    async fn monitor_once(&self, filter: &Filter) -> Result<usize, Box<dyn std::error::Error>> {
        // Obtenir le dernier block
        let latest_block = match timeout(
            Duration::from_secs(10),
            self.provider.get_block_number()
        ).await {
            Ok(Ok(block)) => block,
            Ok(Err(e)) => return Err(format!("Erreur provider: {}", e).into()),
            Err(_) => return Err("Timeout lors de la récupération du block".into()),
        };
        
        // Définir la plage de blocks à analyser (dernier block seulement pour éviter trop de requêtes)
        let from_block = latest_block;
        let to_block = latest_block;
        
        let filter = filter.clone()
            .from_block(from_block)
            .to_block(to_block);
        
        // Récupérer les logs avec timeout
        let logs = match timeout(
            Duration::from_secs(10),
            self.provider.get_logs(&filter)
        ).await {
            Ok(Ok(logs)) => logs,
            Ok(Err(e)) => {
                // Si c'est une erreur 429, on le gère spécialement
                if e.to_string().contains("429") {
                    return Err("Rate limit atteint (429)".into());
                }
                return Err(format!("Erreur lors de la récupération des logs: {}", e).into());
            }
            Err(_) => return Err("Timeout lors de la récupération des logs".into()),
        };
        
        // Traiter les logs
        for log in &logs {
            self.process_swap_log(log).await;
        }
        
        Ok(logs.len())
    }
    
    fn create_swap_filter(&self, dex_routers: &[String]) -> Result<Filter, Box<dyn std::error::Error>> {
        let addresses: Vec<Address> = dex_routers
            .iter()
            .map(|addr| addr.parse::<Address>())
            .collect::<Result<Vec<_>, _>>()?;
        
        let filter = Filter::new()
            .address(addresses)
            .topic0(SWAP_TOPIC.parse::<H256>()?);
        
        Ok(filter)
    }
    
    async fn process_swap_log(&self, log: &Log) {
        // Extraire les informations basiques
        let tx_hash = log.transaction_hash.unwrap_or_default();
        let block = log.block_number.unwrap_or_default();
        let router = log.address;
        
        println!("\n🔄 Swap détecté!");
        println!("  📦 Block: {}", block);
        println!("  🏦 Router: {:?}", router);
        println!("  📝 Tx: {:?}", tx_hash);
        
        // Essayer de décoder les détails du swap
        if log.topics.len() >= 3 && log.data.len() >= 128 {
            // Les topics contiennent généralement:
            // topic[0] = event signature (Swap)
            // topic[1] = sender
            // topic[2] = recipient
            
            let sender = Address::from(log.topics[1]);
            let recipient = Address::from(log.topics[2]);
            
            // Les données contiennent les montants (amount0In, amount1In, amount0Out, amount1Out)
            // Chaque valeur est sur 32 bytes
            if log.data.len() >= 128 {
                let amount0_in = U256::from_big_endian(&log.data[0..32]);
                let amount1_in = U256::from_big_endian(&log.data[32..64]);
                let amount0_out = U256::from_big_endian(&log.data[64..96]);
                let amount1_out = U256::from_big_endian(&log.data[96..128]);
                
                println!("  👤 Sender: {:?}", sender);
                println!("  📬 Recipient: {:?}", recipient);
                
                // Déterminer la direction du swap
                if amount0_in > U256::zero() && amount1_out > U256::zero() {
                    println!("  💱 Token0 → Token1");
                    println!("     In:  {} (token0)", format_units(amount0_in, 18));
                    println!("     Out: {} (token1)", format_units(amount1_out, 18));
                } else if amount1_in > U256::zero() && amount0_out > U256::zero() {
                    println!("  💱 Token1 → Token0");
                    println!("     In:  {} (token1)", format_units(amount1_in, 18));
                    println!("     Out: {} (token0)", format_units(amount0_out, 18));
                }
                
                // Calculer le slippage approximatif (simpliste pour le moment)
                // TODO: Implémenter un calcul de slippage plus précis
            }
        }
        
        // Incrémenter le compteur
        let count = self.swap_count + 1;
        println!("  📊 Total swaps détectés: {}", count);
    }
    
    fn print_statistics(&self) {
        println!("\n📊 === Statistiques === 📊");
        println!("  ✅ Swaps détectés: {}", self.swap_count);
        println!("  ❌ Erreurs: {}", self.error_count);
        let success_rate = if self.swap_count + self.error_count > 0 {
            (self.swap_count as f64 / (self.swap_count + self.error_count) as f64) * 100.0
        } else {
            0.0
        };
        println!("  📈 Taux de succès: {:.2}%", success_rate);
        println!("=" .repeat(30));
    }
}

// Fonction helper pour formater les montants
fn format_units(amount: U256, decimals: u8) -> String {
    let divisor = U256::from(10).pow(U256::from(decimals));
    let whole = amount / divisor;
    let fraction = amount % divisor;
    
    if fraction == U256::zero() {
        format!("{}", whole)
    } else {
        // Afficher seulement 6 décimales pour la lisibilité
        let frac_str = format!("{:0>width$}", fraction, width = decimals as usize);
        let truncated = &frac_str[..6.min(frac_str.len())];
        format!("{}.{}", whole, truncated.trim_end_matches('0'))
    }
}
