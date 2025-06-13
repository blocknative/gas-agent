use crate::{
    distribution::{BlockDistribution, DistributionCreator},
    rpc::{BlockHeader, Transaction},
};
use anyhow::{anyhow, Result};
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};

pub fn block_to_block_distribution(
    transactions: &[Transaction],
    base_fee: &Option<u64>,
) -> BlockDistribution {
    let mut distribution = DistributionCreator::new(0.000000001);

    for tx in transactions.iter() {
        let Transaction {
            hash,
            gas_price,
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } = tx;

        if (gas_price.is_some() && gas_price.unwrap() > 0)
            || (max_priority_fee_per_gas.is_some() && max_priority_fee_per_gas.unwrap() > 0)
        {
            match calc_fee_gwei(
                gas_price,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                base_fee,
            ) {
                std::result::Result::Ok(effective_gas_price) => {
                    distribution.add(effective_gas_price)
                }
                Err(e) => {
                    eprint!(
                        "Failed to calculate miner reward for transaction with hash: {}, error: {}",
                        &hash, e
                    );
                }
            }
        }
    }

    // Sort ASC
    distribution
        .buckets
        .sort_by(|a, b| a.gwei.partial_cmp(&b.gwei).unwrap());

    distribution.buckets
}

pub fn wei_to_gwei(wei: u128) -> Result<f64> {
    // Convert u128 to Decimal for precision
    let wei_decimal = Decimal::from_u128(wei).unwrap_or_default();

    // 1 Gwei = 10^9 Wei
    let gwei_conversion_factor = Decimal::new(1_000_000_000, 0);

    // Perform the division with Decimal precision
    let gwei_decimal = wei_decimal / gwei_conversion_factor;

    // Convert the result back to f64
    gwei_decimal
        .round_dp(9)
        .to_f64()
        .ok_or(anyhow!("Failed to convert wei to gwei"))
}

pub fn calc_fee_gwei(
    gas_price: &Option<u128>,
    max_fee_per_gas: &Option<u128>,
    max_priority_fee_per_gas: &Option<u128>,
    base_fee_per_gas: &Option<u64>,
) -> Result<f64> {
    let base_fee_per_gas = base_fee_per_gas.ok_or(anyhow!("No base fee per gas value"))?;
    if let Some(gas_price) = gas_price {
        wei_to_gwei(*gas_price)
    } else {
        let max_fee_per_gas =
            max_fee_per_gas.ok_or(anyhow!("Missing max_fee_per_gas for effective calc"))?;

        let max_priority_fee_per_gas = max_priority_fee_per_gas.ok_or(anyhow!(
            "Missing max_priority_fee_per_gas for effective calc"
        ))?;

        let a = max_fee_per_gas - base_fee_per_gas as u128;
        let wei = a.min(max_priority_fee_per_gas);
        wei_to_gwei(wei)
    }
}

const ELASTICITY_MULTIPLIER: u64 = 2;
const BASE_FEE_CHANGE_DENOMINATOR: u64 = 8;

pub fn calc_base_fee(latest_block: &BlockHeader) -> Option<u64> {
    if let Some(parent_base_fee) = latest_block.base_fee_per_gas {
        let parent_gas_target = latest_block.gas_limit / ELASTICITY_MULTIPLIER;

        // If the parent gasUsed is the same as the target, the baseFee remains unchanged
        if latest_block.gas_used == parent_gas_target {
            return Some(parent_base_fee);
        }

        if latest_block.gas_used > parent_gas_target {
            // If the parent block used more gas than its target, the baseFee should increase
            let gas_used_delta = latest_block.gas_used - parent_gas_target;
            let x = parent_base_fee * gas_used_delta;
            let y = x / parent_gas_target;
            let base_fee_delta = std::cmp::max(y / BASE_FEE_CHANGE_DENOMINATOR, 1);

            return Some(parent_base_fee + base_fee_delta);
        } else {
            // Otherwise if the parent block used less gas than its target, the baseFee should decrease
            let gas_used_delta = parent_gas_target - latest_block.gas_used;
            let x = parent_base_fee * gas_used_delta;
            let y = x / parent_gas_target;
            let base_fee_delta = y / BASE_FEE_CHANGE_DENOMINATOR;

            return Some(std::cmp::max(
                parent_base_fee.saturating_sub(base_fee_delta),
                0,
            ));
        }
    }

    None
}
