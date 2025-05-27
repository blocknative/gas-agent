/*
Simply takes the minimum of the last block.
*/

use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};

pub fn get_prediction_last_min(block_distributions: &[BlockDistribution]) -> (f64, Settlement) {
    let last_block_distribution = block_distributions.last().unwrap();

    let last_min = last_block_distribution
        .get(0)
        .map(|dist| dist.gwei)
        .unwrap_or(0.0);

    (round_to_9_places(last_min), Settlement::Fast)
}
