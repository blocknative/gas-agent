# Gas Agent

A Rust-based agent system that generates and submits gas price predictions for EVM networks to the [Gas Network](https://gas.network/) for evaluation.

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
   All agent payloads are signed before submission to the Gas Network so that they are verifiable and attributable. Predictions will be evaluated by the combination of the `System`, `Network`, `Settlement` and `from_block` of the payload. It is recommended that you use a unique key pair for each `AgentKind` otherwise the predictions for the same combination will be averaged and evaluated, rather then evaluated separately (which might be what you want!). Use the private key for the `signer_key` field for each agent.

   There is a helper CLI command if you would like to generate some fresh random keys:

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

## Agent Registration

Before your agent can submit predictions to the Gas Network, you need to register and get your signing addresses whitelisted.

### How Agent Authentication Works

1. **Signed Predictions**: All agent payloads are cryptographically signed using your private key before submission
2. **Address Extraction**: The Gas Network collector validates the signature and extracts the corresponding Ethereum address
3. **Whitelist Validation**: Only predictions from whitelisted addresses are accepted and processed by the network

### Getting Whitelisted

To register your agent and get your signing addresses whitelisted:

1. **Generate Your Keys**: Use the built-in key generation tool to create your signing keys:
   ```bash
   cargo run -- generate-keys
   ```

2. **Save Your Keys**: Securely store the private key for your agent configuration and note the corresponding public address

3. **Submit Whitelist Request**: Contact the Blocknative team with your public address(es):
   - **Email**: [support@blocknative.com](mailto:support@blocknative.com)
   - **Discord**: Join our community at [https://discord.com/invite/KZaBVME](https://discord.com/invite/KZaBVME)

4. **Include in Your Request**:
   - Your Ethereum address(es) that will be signing predictions
   - Brief description of your prediction model/strategy
   - Expected prediction frequency and settlement types
   - Your intended use case or research goals

### Multiple Agents and Addresses

- **Unique Keys Recommended**: Use different signing keys for different agent types or models
- **Separate Evaluation**: Each unique combination of address, system, network, and settlement is evaluated independently
- **All Addresses Need Whitelisting**: Each signing address you plan to use must be individually whitelisted

### Testing Before Whitelisting

While waiting for whitelist approval, you can:
- Test your agent locally with mock endpoints
- Verify your prediction logic and model performance
- Ensure your signing and payload generation works correctly

Once whitelisted, your agent can begin submitting predictions that will be evaluated and potentially published to the Gas Network for end users.

## Development

### Running in Development Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- start --chains 'YOUR-CONFIG-JSON'

# Run tests
cargo test
```

### Configuration Options

The agent supports the following command-line arguments and environment variables:

- `--server-address` / `SERVER_ADDRESS`: HTTP server bind address (currently used only for kubernetes probes) (default: `0.0.0.0:8080`)
- `--chains` / `CHAINS`: JSON configuration for EVM networks and agents
- `--collector-endpoint` / `COLLECTOR_ENDPOINT`: The Gas Network endpoint for payload evaluation

### Chain Configuration

The chain configuration is specified as a JSON array where each object represents an EVM network and its associated agents. Each chain configuration supports the following fields:

#### ChainConfig Fields

- **`system`** (required): The blockchain system to connect to

  - Available options: `"ethereum"`, `"base"`, `"polygon"`

- **`network`** (required): The network within the system

  - Available options: `"mainnet"`

- **`json_rpc_url`** (required): The JSON-RPC endpoint URL for the blockchain network

  - Example: `"https://ethereum-rpc.publicnode.com"`

- **`pending_block_data_source`** (optional): Configuration for fetching pending block data

  - See [Pending Block Data Source](#pending-block-data-source) section below

- **`agents`** (required): Array of agent configurations to run on this chain
  - See [Agent Configuration](#agent-configuration) section below

#### Pending Block Data Source

When specified, this configures how to fetch pending block (mempool) data which can be passed to models that can be more reactive to changes in the mempool and to make use of private data to create more accurate predictions:

```json
{
  "pending_block_data_source": {
    "json_rpc": {
      "url": "https://api.example.com/pending",
      "method": "eth_getPendingBlock",
      "params": ["latest"],
      "poll_rate_ms": 1000
    }
  }
}
```

**Fields:**

- **`url`** (required): The JSON-RPC endpoint URL
- **`method`** (required): The RPC method to call
- **`params`** (optional): Parameters to pass to the RPC method
- **`poll_rate_ms`** (required): Polling interval in milliseconds

Currently, JSON RPC is the only source available, but other sources are coming soon. Please create an issue if there is a specific source that you would like to see supported.

#### Agent Configuration

Each agent in the `agents` array supports the following configuration:

```json
{
  "kind": "percentile",
  "signer_key": "0x1234567890abcdef...",
  "prediction_trigger": "block"
}
```

**Fields:**

- **`kind`** (required): The type of agent to run

  - `"node"`: Publishes the standard estimate from the node
  - `"target"`: Publishes the actual minimum price for new blocks
  - Model-based agents:
    - `"adaptive_threshold"`: Uses adaptive threshold analysis
    - `"distribution_analysis"`: Analyzes gas price distributions
    - `"moving_average"`: Uses moving average calculations
    - `"percentile"`: Uses percentile-based predictions
    - `"time_series"`: Uses time series analysis
    - `"last_min"`: Takes the minimum from the previous block and uses that as the prediction for the next block.

- **`signer_key`** (required): Private key for signing predictions (use `cargo run -- generate-keys` to create)

- **`prediction_trigger`** (required): When to generate predictions
  - `"block"`: Generate prediction when a new block is detected
  - `{"poll": {"rate_ms": 5000}}`: Generate predictions at regular intervals (rate in milliseconds)

## Models

The gas agent includes several built-in prediction models that analyze recent block data to estimate optimal gas prices. Each model uses different strategies to predict gas prices based on historical transaction patterns.

### Available Models

#### `percentile`

Analyzes the distribution of gas prices across the 5 most recent blocks and selects the 75th percentile to ensure high inclusion probability. This model is particularly effective during periods of high volatility, as it targets a price that would have included 75% of recent transactions.

#### `last_min`

Simply takes the minimum gas price from the most recent block and uses it as the prediction for the next block. This is the most aggressive pricing strategy and works well when gas prices are stable.

#### `moving_average`

Calculates a Simple Weighted Moving Average (SWMA) of recent gas prices, giving more weight to more recent blocks (up to 10 blocks). This approach works well when gas prices are relatively stable and provides smooth price transitions.

#### `adaptive_threshold`

Identifies the minimum gas price that would have been included in each recent block (up to 50 blocks) and applies an adaptive premium based on price volatility. When prices are stable, it applies a small premium; when volatile, it applies a larger premium (up to 50%). This provides a balance between cost and inclusion probability.

#### `time_series`

Uses simple linear regression to identify trends in gas prices and predict the next value based on the median gas price of the last 20 blocks. This model is particularly useful when gas prices show a consistent trend over time (either increasing or decreasing). Falls back to moving average if insufficient data is available.

#### `distribution_analysis`

Analyzes the cumulative distribution function (CDF) of gas prices in the most recent block to find "sweet spots" where many transactions are being included. It identifies points where the rate of change in the CDF decreases significantly, representing efficient gas price levels, then applies a 10% premium for higher inclusion probability.

### Creating Custom Models

To create a custom prediction model, you'll need to fork the repository and implement your own model logic. Here's how:

#### Step 1: Fork and Clone

```bash
git fork https://github.com/blocknative/gas-agent
git clone https://github.com/YOUR-USERNAME/gas-agent
cd gas-agent
```

#### Step 2: Add Your Model Type

Add your model to the `ModelKind` enum in `src/types.rs`:

```rust
#[derive(Debug, Clone, EnumString, Display, Deserialize, Serialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    AdaptiveThreshold,
    DistributionAnalysis,
    MovingAverage,
    Percentile,
    TimeSeries,
    LastMin,
    YourCustomModel,  // Add this line
}
```

#### Step 3: Create Your Model Implementation

Create a new file `src/models/your_custom_model.rs`:

```rust
/*
Your Custom Model Description
Explain what your model does and how it works.
*/

use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};

pub fn get_prediction_your_custom_model(block_distributions: &[BlockDistribution], pending_block_distribution: &Option<BlockDistribution>) -> (f64, Settlement) {
    // Your model logic here
    //
    // block_distributions is a Vec<BlockDistribution> where:
    // - BlockDistribution = Vec<Bucket>
    // - Bucket { gwei: f64, count: u32 }
    //
    // Each BlockDistribution represents gas price buckets from a block
    // sorted from oldest to newest blocks

    // Example: Get the most recent block
    let latest_block = block_distributions.last().unwrap();

    // Example: Calculate some prediction logic
    let mut total_gas_price = 0.0;
    let mut total_transactions = 0u32;

    for bucket in latest_block {
        total_gas_price += bucket.gwei * bucket.count as f64;
        total_transactions += bucket.count;
    }

    let predicted_price = if total_transactions > 0 {
        total_gas_price / total_transactions as f64
    } else {
        1.0 // fallback price
    };

    // Return the prediction and settlement time
    (round_to_9_places(predicted_price), Settlement::Fast)
}
```

#### Step 4: Register Your Model

Add your model to the module system in `src/models/mod.rs`:

```rust
use your_custom_model::get_prediction_your_custom_model;

mod your_custom_model;

// In the apply_model function, add your case:
pub async fn apply_model(
    model: &ModelKind,
    block_distributions: &[BlockDistribution],
    pending_block_distribution: Option<BlockDistribution>,
) -> Result<(f64, Settlement)> {
    // ... existing code ...

    match model {
        // ... existing cases ...
        ModelKind::YourCustomModel => Ok(get_prediction_your_custom_model(block_distributions, pending_block_distribution)),
    }
}
```

#### Step 5: Build and Test

```bash
cargo build
cargo test

# Test your model
cargo run -- start --chains '[{
  "system": "ethereum",
  "network": "mainnet",
  "json_rpc_url": "https://ethereum-rpc.publicnode.com",
  "agents": [{
    "kind": "your_custom_model",
    "signer_key": "YOUR-PRIVATE-KEY",
    "prediction_trigger": "block"
  }]
}]'
```

#### Model Development Tips

1. **Understand the Data Structure**: Each `BlockDistribution` contains buckets of gas prices with transaction counts, representing the gas price distribution for that block.

2. **Handle Edge Cases**: Always check for empty distributions and provide fallback values.

3. **Consider Settlement Times**: Choose appropriate `Settlement` values:

   - `Immediate`: Next block
   - `Fast`: ~15 seconds
   - `Medium`: ~15 minutes
   - `Slow`: ~1 hour

4. **Use Utility Functions**: The `round_to_9_places()` function ensures consistent precision across predictions.

5. **Test with Different Market Conditions**: Test your model during periods of high volatility, network congestion, and normal conditions.

6. **Leverage Pending Block Data**: If available, you can access `pending_block_distribution` parameter in the `apply_model` function for more reactive predictions.

## Settlement Times and Block Windows

As a prediction provider, you can generate gas price predictions for different settlement windows that end users will consume to price their transactions. Each settlement time represents a different urgency level and block window that your predictions target.

### Settlement Options for Prediction Models

Your models can return one of four settlement times, each targeting different end-user needs:

#### `immediate`

- **Target Time**: Next block (0ms)
- **Block Window**: 0 blocks (next block only)
- **End User Profile**: Arbitrage bots, MEV strategies, time-critical DeFi operations
- **Prediction Strategy**: Should predict the minimum gas price needed for immediate inclusion

#### `fast`

- **Target Time**: ~15 seconds (15,000ms)
- **End User Profile**: Standard DeFi interactions, swaps, NFT minting
- **Prediction Strategy**: Balance between inclusion probability and cost efficiency

#### `medium`

- **Target Time**: ~15 minutes (900,000ms)
- **End User Profile**: Regular transfers, non-urgent contract interactions
- **Prediction Strategy**: Focus on cost optimization while maintaining reasonable inclusion probability

#### `slow`

- **Target Time**: ~1 hour (3,600,000ms)
- **End User Profile**: Batch operations, low-priority transactions, cost-sensitive users
- **Prediction Strategy**: Minimize gas costs, accepting longer wait times

### Block Window Calculation

Settlement times are converted to block windows based on each network's block time:

#### Network Block Times

- **Ethereum**: 12 seconds per block (12,000ms)
- **Base**: 2 seconds per block (2,000ms)
- **Polygon**: 2 seconds per block (2,000ms)

#### Settlement to Block Window Translation

The number of blocks for each settlement is calculated as: `floor(settlement_time_ms / network_block_time_ms)`

| Settlement  | Ethereum (12s blocks) | Base (2s blocks)      | Polygon (2s blocks)   |
| ----------- | --------------------- | --------------------- | --------------------- |
| `immediate` | 0 blocks (next block) | 0 blocks (next block) | 0 blocks (next block) |
| `fast`      | ~1 block              | ~8 blocks             | ~8 blocks             |
| `medium`    | ~75 blocks            | ~450 blocks           | ~450 blocks           |
| `slow`      | ~300 blocks           | ~1,800 blocks         | ~1,800 blocks         |

### How Your Predictions Are Evaluated

When you submit a gas price prediction, the Gas Network evaluates its accuracy within the specified block window:

1. **Prediction Submitted**: Your model predicts 20 gwei with `fast` settlement for Ethereum
2. **Block Window Calculated**: `fast` on Ethereum = ~1 block window starting from `from_block`
3. **Evaluation Period**: The system monitors min price for blocks within that window
4. **Scoring**: Your prediction is scored on:
   - **Inclusion Rate**: Did your prediction price get onchain within the block window
   - **Cost Efficiency**: Percentage overpayment

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
