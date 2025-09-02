# 🥪 Polygon Sandwich Bot v2.0

A high-performance MEV bot for detecting and executing sandwich attacks on Polygon Network.

## Features

- ✅ Real-time mempool monitoring via WebSocket
- ✅ Multi-DEX support (QuickSwap, Uniswap V3, SushiSwap, ApeSwap, Dfyn)
- ✅ Automatic sandwich opportunity detection
- ✅ Profit estimation and risk assessment
- ✅ Detailed statistics and logging
- ✅ Fallback to HTTP polling if WebSocket fails

## Quick Start

1. **Install Rust**
```bash
curl --proto='=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
