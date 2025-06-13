/*
Simply takes the minimum of the last block.
*/

use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};
use anyhow::{anyhow, Result};

pub fn get_prediction_last_min(
    block_distributions: &[BlockDistribution],
) -> Result<(f64, Settlement)> {
    let last_block_distribution = block_distributions
        .last()
        .ok_or_else(|| anyhow!("LastMin model requires at least one block distribution"))?;

    if last_block_distribution.is_empty() {
        return Err(anyhow!(
            "LastMin model requires non-empty block distribution"
        ));
    }

    let last_min = last_block_distribution
        .iter()
        .map(|bucket| bucket.gwei)
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

    Ok((round_to_9_places(last_min), Settlement::Fast))
}
