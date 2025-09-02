use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🥪 Sandwich Bot Starting...");
    println!("📍 Target: Polygon");
    println!("✅ Ready to make money!");
    
    // TODO: Implement sandwich logic
    loop {
        println!("Scanning for opportunities...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}
