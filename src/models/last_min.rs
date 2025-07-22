/*
Simply takes the minimum of the last block.
*/

use crate::models::{FromBlock, ModelError, Prediction};
use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};

pub fn get_prediction_last_min(
    block_distributions: &[BlockDistribution],
    latest_block: u64,
) -> Result<(Prediction, Settlement, FromBlock), ModelError> {
    let last_block_distribution = block_distributions.last().ok_or_else(|| {
        ModelError::insufficient_data("LastMin model requires at least one block distribution")
    })?;

    if last_block_distribution.is_empty() {
        return Err(ModelError::insufficient_data(
            "LastMin model requires non-empty block distribution",
        ));
    }

    let last_min = last_block_distribution
        .iter()
        .map(|bucket| bucket.gwei)
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

    Ok((
        round_to_9_places(last_min),
        Settlement::Fast,
        latest_block + 1,
    ))
}
