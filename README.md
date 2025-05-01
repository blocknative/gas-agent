# Gas Agent

A Rust-based agent for estimating gas prices on EVM-compatible blockchains. The agent can run in different modes to predict gas prices using various statistical models or report actual gas prices from network blocks.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Configuration](#configuration)
- [Running Modes](#running-modes)
- [Models](#models)
- [Settlement Time Windows](#settlement-time-windows)
- [Adding Custom Models](#adding-custom-models)
- [Metrics and Logging](#metrics-and-logging)
- [Deployment](#deployment)
- [Security](#security)

## Overview

The Gas Agent is designed to monitor gas prices on EVM-compatible blockchains and provide gas price estimates. It can run in different modes and use different models to generate these estimates. The agent can publish its estimates to an endpoint for evaluation and inclusion on to Gas Network or log them to stdout if no endpoint is configured.

## Installation

### Prerequisites

1. **Install Rust**

   The agent is built with Rust. To install Rust:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

   Follow the instructions on screen. After installation, make sure Rust is in your PATH:

   ```bash
   source $HOME/.cargo/env
   ```

   Verify installation:

   ```bash
   rustc --version
   cargo --version
   ```

2. **Clone the Repository**

   ```bash
   git clone https://github.com/blocknative/gas-agent.git
   cd gas-agent
   ```

### Building

Build the agent with:

```bash
cargo build --release
```

The compiled binary will be located at `target/release/gas-agent`.

## Configuration

The Gas Agent can be configured using environment variables or command line arguments:

```bash
SERVER_ADDRESS=0.0.0.0:8080 CHAIN_ID=1 MODE=moving-average ./target/release/gas-agent
```

or

```bash
./target/release/gas-agent --server-address 0.0.0.0:8080 --chain-id 1 --mode moving-average
```

### Configuration Options

| Parameter            | Environment Variable | Description                                                    | Default                             |
| -------------------- | -------------------- | -------------------------------------------------------------- | ----------------------------------- |
| `--server-address`   | `SERVER_ADDRESS`     | The address to bind the HTTP server to                         | `0.0.0.0:8080`                      |
| `--chain-id`         | `CHAIN_ID`           | The chain ID to monitor (e.g., 1 for Ethereum mainnet)         | Required                            |
| `--mode`             | `MODE`               | The agent operation mode (see [Running Modes](#running-modes)) | Required                            |
| `--publish-endpoint` | `PUBLISH_ENDPOINT`   | The endpoint to publish data to                                | Optional, logs to stdout if not set |
| `--signer-key`       | `SIGNER_KEY`         | The SECP256k1 private key to sign data with                    | Optional, generated if not provided |

You can also configure the logging level using the standard `RUST_LOG` environment variable (e.g., `RUST_LOG=debug`). See the [Metrics and Logging](#metrics-and-logging) section for more details.

### Signer Key

If no signer key is provided through the `--signer-key` parameter or `SIGNER_KEY` environment variable, the agent will automatically generate a new key and store it in a file named `signer_key` in the current working directory (PWD). This key will be used for all future runs unless a different key is explicitly provided.

The signer key is used to sign the `AgentPayload` and will be verified by the Evaluation endpoint.

## Running Modes

The agent can run in three different modes, controlled by the `--mode` parameter:

### 1. Node Mode

```bash
--mode node
```

In this mode, the agent will publish the standard gas estimate directly from the node's RPC API (e.g., `eth_gasPrice`). This is the simplest mode but may not provide optimal gas prices during network congestion or rapid changes.

### 2. Target Mode

```bash
--mode target
```

In this mode, the agent will publish the actual minimum gas price observed in new blocks. This represents the actual gas price that was accepted by the network for transactions in the most recent block.

### 3. Model Mode

```bash
--mode <model-name>
```

In this mode, the agent will apply a specific prediction model to estimate gas prices. Available models include:

- `adaptive-threshold` - Dynamically adjusts thresholds based on network conditions
- `distribution-analysis` - Statistical analysis of gas price distributions
- `moving-average` - Time-weighted averaging of recent gas prices
- `percentile` - Selects specific percentiles from gas price distributions
- `time-series` - Time-series forecasting of gas prices
- `last-min` - Simple model using the most recently observed minimum price

See the [Models](#models) section for details on each model.

## Settlement Time Windows

Every gas price estimate produced by the agent includes a `Settlement` parameter that defines the time window for which the estimate is valid.

### The Settlement Enum

The agent uses a `Settlement` enum to specify different time windows:

```rust
pub enum Settlement {
    /// Next block
    Immediate,
    /// 30 seconds
    Fast,
    /// 15 minutes
    Medium,
    /// 1 hour
    Slow,
}
```

### How Settlement Works

1. **Block Window Calculation**:

   - The agent specifies a `from_block` in each payload, which is the starting block number for the prediction.
   - The `Settlement` value determines how many blocks into the future the prediction is valid for.
   - The evaluation endpoint translates the `Settlement` value into a specific block range based on the chain's block time.

2. **Default Behavior**:

   - For `Target` mode, the agent defaults to `Immediate` since it's reporting the actual minimum gas price for the latest block.
   - For `Model` mode, the agent typically uses `Fast` for predictions that are intended for the next few blocks.

3. **Time to Blocks Translation**:

   | Settlement | Time Window | Translated Blocks (Ethereum) |
   | ---------- | ----------- | ---------------------------- |
   | Immediate  | Next block  | 1 block                      |
   | Fast       | 30 seconds  | 2 blocks                     |
   | Medium     | 15 minutes  | 75 blocks                    |
   | Slow       | 1 hour      | 300 blocks                   |

   The actual number of blocks will vary by chain based on its average block time.

## Models

The Gas Agent supports several prediction models for estimating gas prices:

### Adaptive Threshold

`--mode adaptive-threshold`

This model dynamically adjusts its threshold based on recent transaction inclusion patterns, adapting to changing network conditions. It analyzes the minimum gas prices accepted in recent blocks and adjusts thresholds based on how quickly blocks are being produced and how full they are.

**Best for**: Networks with variable congestion patterns where a single fixed threshold wouldn't be optimal.

### Distribution Analysis

`--mode distribution-analysis`

This model analyzes the statistical distribution of gas prices in recent blocks to identify optimal price points. It examines the shape of the distribution to find natural breakpoints where a small increase in gas price results in a significant improvement in inclusion probability.

**Best for**: Finding the optimal price-to-inclusion probability ratio.

### Moving Average

`--mode moving-average`

This model calculates a sliding window moving average of gas prices over recent blocks, smoothing out short-term fluctuations. It weights more recent blocks higher than older blocks to respond to trends while avoiding overreaction to outliers.

**Best for**: Stable networks with gradual changes in gas prices.

### Percentile

`--mode percentile`

This model selects a specific percentile from the distribution of gas prices in recent blocks. For example, it might use the 25th percentile for a "slow" estimate or the 75th percentile for a "fast" estimate.

**Best for**: Simple, reliable estimates with clear inclusion probability targets.

### Time Series

`--mode time-series`

This model applies time series analysis techniques to predict future gas prices based on historical trends. It can detect cyclical patterns and forecast short-term price movements.

**Best for**: Networks with predictable cyclical patterns in gas prices (e.g., higher during certain times of day).

### Last Min

`--mode last-min`

This model simply uses the minimum gas price from the most recent block as the prediction for the next block. This is the simplest model but can be effective in steady market conditions.

**Best for**: Quick implementation or as a baseline for comparison with more sophisticated models.

## Adding Custom Models

You can extend the Gas Agent with your own custom models. Here's a detailed guide on how to implement a custom model:

1. **Create a new file** in the `src/models/` directory for your model, e.g., `src/models/my_custom_model.rs`

2. **Implement your model function** with the following signature:

   ```rust
   pub fn get_prediction_my_custom_model(block_distributions: &[BlockDistribution]) -> f64 {
       // Your model implementation here
       // Process the block_distributions and return a price prediction as f64
   }
   ```

   The function accepts block distributions and returns a prediction denominated in gwei. A `BlockDistribution` is a list of "buckets" with the count of each fee rate for a given block. The agent by default will keep the last 50 block distributions in memory for analysis.

3. **Add your model to the module system** by modifying `src/models/mod.rs`:

   ```rust
   // Add your module
   mod my_custom_model;
   // Import your function
   pub use my_custom_model::get_prediction_my_custom_model;
   ```

4. **Add a new variant to the `ModelKind` enum** in `src/types.rs`:

   ```rust
   pub enum ModelKind {
       // Existing models...
       MyCustomModel,
   }
   ```

5. **Add your model to the `apply_model` function match statement** in `src/models/mod.rs`:

   ```rust
   pub fn apply_model(kind: ModelKind, block_distributions: &[BlockDistribution]) -> Result<f64> {
       match kind {
           // Existing models...
           ModelKind::MyCustomModel => Ok(get_prediction_my_custom_model(block_distributions)),
       }
   }
   ```

6. **Rebuild the agent** with your new model.

## Logging

The Gas Agent uses the `tracing` crate for structured logging. Logs are output in JSON format by default and include timestamps in RFC 3339 format.

Logging levels can be controlled via the `RUST_LOG` environment variable:

```bash
RUST_LOG=info ./target/release/gas-agent
```

Possible log levels are: `error`, `warn`, `info`, `debug`, and `trace`.

If no logging level is specified, it defaults to `info`.

## Deployment

### Docker

The repository includes a Dockerfile for containerized deployment:

```bash
# Build the Docker image
docker build -t gas-agent .

# Run the container
docker run -p 8080:8080 \
  -e CHAIN_ID=1 \
  -e MODE=moving-average \
  -e PUBLISH_ENDPOINT=https://your-endpoint.com/api \
  gas-agent
```

### Docker Image Structure

The Docker image is built in two stages:

1. **Builder Stage**: Compiles the Rust application
2. **Runtime Stage**: Minimal Debian image with only the necessary dependencies

The container exposes port 8080 for the HTTP server and metrics endpoint.

## Security

### Signer Key Management

The Gas Agent uses a SECP256k1 private key for signing payload data. This key is critical for verifying the authenticity of the gas price estimates. Proper management of this key is important:

1. **Key Generation**: If no key is provided, the agent will automatically generate a new key and store it in a file named `signer_key` in the current working directory.

2. **Key Security**: This key should be treated as sensitive information. In production environments.
