#!/bin/bash
# Launch script for Sandwich Bot

echo "🥪 Starting Polygon Sandwich Bot v2.0..."

# Check if .env exists
if [ ! -f .env ]; then
    echo "❌ Error: .env file not found!"
    echo "Please copy .env.example to .env and configure it."
    exit 1
fi

# Build in release mode
echo "🔨 Building in release mode..."
cargo build --release

# Clear terminal
clear

# Run the bot
echo "🚀 Launching bot..."
./target/release/sandwich_bot

# On exit
echo "👋 Bot stopped."
