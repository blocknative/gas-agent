# Gas Agent

A Rust-based agent system that generates and submits gas price predictions for EVM networks to the [Gas Network](https://gas.network/) for evaluation.

## Overview

The Gas Agent monitors EVM networks and provides gas price predictions by running configurable agents that can:

- Accept pending block / mempool data that can be used to generate gas price predictions
- Generate gas price predictions from historical block data using various algorithms
- Support multiple chains and algorithms simultaneously

## Prerequisites

### Install Rust

You'll need Rust installed on your system. The recommended way is using `rustup`:

#### macOS and Linux

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### Windows

Download and run the installer from [rustup.rs](https://rustup.rs/)

Verify your installation:

```bash
rustc --version
cargo --version
```

## Quick Start

1. **Clone the repository**

   ```bash
   git clone https://github.com/blocknative/gas-agent
   cd gas-agent
   ```

2. **Set up environment variables**

   ```bash
   cp .env.example .env
   ```

   Edit `.env` with your configuration:

3. **Generate signing keys**
   You will need to generate a key pair for each agent that you configure, using the private key for the signer key and contacting Blocknative to get your address(es) whitelisted to be able to post to the collector for evaluation. There is a helper CLI command if you would like to generate some fresh random keys:

   ```bash
   cargo run -- generate-keys
   ```

   This will output a new key pair. Save the private key securely for agent configuration.

4. **Build the project**

   ```bash
   cargo build --release
   ```

5. **Configure chains and agents**
   A list of chains and the agents to run for each chain can be configured and will run in parallel with each chain running on it's own thread. Set the `CHAINS` env variable with a JSON string:

   ```json
   [
     {
       "system": "ethereum",
       "network": "mainnet",
       "json_rpc_url": "https://ethereum-rpc.publicnode.com",
       "agents": [
         {
           "kind": "percentile",
           "signer_key": "YOUR-GENERATED-PRIVATE-KEY",
           "prediction_trigger": "block"
         }
       ]
     }
   ]
   ```

6. **Run the agent**
   ```bash
   cargo run -- start
   ```

## Development

### Project Structure

```
gas-agent/
├── src/
│   ├── agent.rs           # Core agent logic
│   ├── blocks.rs          # Block monitoring
│   ├── chain/             # Chain-specific implementations
│   ├── config.rs          # Configuration parsing
│   ├── distribution.rs    # Gas price distribution analysis
│   ├── models/            # Data models
│   ├── server/            # HTTP server components
│   └── main.rs            # Application entry point
├── Cargo.toml             # Rust dependencies
└── README.md              # This file
```

### Running in Development Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- start --chains 'YOUR-CONFIG-JSON'

# Run tests
cargo test

# Run with automatic recompilation
cargo install cargo-watch
cargo watch -x run
```

### Configuration Options

The agent supports the following command-line arguments and environment variables:

- `--server-address` / `SERVER_ADDRESS`: HTTP server bind address (default: `0.0.0.0:8080`)
- `--chains` / `CHAINS`: JSON configuration for blockchain networks and agents
- `--env` / `ENV`: Environment mode (default: `local`)
- `--collector-endpoint` / `COLLECTOR_ENDPOINT`: Telemetry collector endpoint

### Chain Configuration

Each chain configuration supports:

```json
{
  "system": "ethereum|polygon|arbitrum|...",
  "network": "mainnet|testnet|...",
  "json_rpc_url": "https://your-rpc-endpoint",
  "pending_block_data_source": {
    "JsonRpc": {
      "url": "https://additional-data-source",
      "method": "eth_pendingBlock",
      "params": [],
      "poll_rate_ms": 1000
    }
  },
  "agents": [
    {
      "kind": "simple|advanced",
      "signer_key": "0x...",
      "prediction_trigger": "block" | {"poll": {"rate_ms": 5000}}
    }
  ]
}
```

### Prediction Triggers

- **Block**: Generate a prediction after a new block is detected
- **Poll**: Generate predictions at a regular ms interval

## Building for Production

```bash
# Optimized release build
cargo build --release

# The binary will be available at
./target/release/gas-agent
```

## Docker Support

A Dockerfile is included for containerized deployments:

```bash
docker build -t gas-agent .
docker run -e CHAINS='YOUR-CONFIG' gas-agent start
```

## Monitoring

The agent exposes kubernetes probe endpoints:

- Liveness: `GET /internal/probe/liveness`
- Readiness: `GET /internal/probe/readiness`

## License

See [LICENSE](LICENSE) file for details.
