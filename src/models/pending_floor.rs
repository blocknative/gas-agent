/*
Pending Floor Prediction Model

This model is specifically designed for block builders who have proprietary private
transaction flow and can see what the next block will contain. It analyzes the
pending block distribution to find the minimum gas price and adds exactly 1 wei
to ensure the transaction will be included while paying the absolute minimum.

How it works:
1. Requires pending block distribution (future block content) - returns error if not provided
2. Finds the minimum gas price in that distribution
3. Adds 1 wei (0.000000001 gwei) to guarantee inclusion
4. Returns this as the optimal price for immediate settlement
*/

use crate::types::Settlement;
use crate::{distribution::BlockDistribution, utils::round_to_9_places};
use anyhow::{anyhow, Result};

const ONE_WEI_IN_GWEI: f64 = 0.000000001; // 1 wei

pub fn get_prediction_pending_floor(
    pending_block_distribution: Option<BlockDistribution>,
) -> Result<(f64, Settlement)> {
    // If no pending block distribution is available, return an error
    let Some(pending_distribution) = pending_block_distribution else {
        return Err(anyhow!(
            "PendingFloor model requires pending block distribution data"
        ));
    };

    // If the pending distribution is empty, return an error
    if pending_distribution.is_empty() {
        return Err(anyhow!(
            "PendingFloor model requires non-empty pending block distribution"
        ));
    }

    // Find the minimum gas price in the pending block distribution
    let min_price = pending_distribution
        .first()
        .map(|dist| dist.gwei)
        .unwrap_or(0.0);

    // Add 1 wei to the minimum price to ensure inclusion
    let prediction = min_price + ONE_WEI_IN_GWEI;

    Ok((round_to_9_places(prediction), Settlement::Immediate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::Bucket;

    #[test]
    fn test_pending_floor_with_pending_distribution() {
        let pending_distribution = vec![
            Bucket {
                gwei: 10.0,
                count: 5,
            },
            Bucket {
                gwei: 15.0,
                count: 3,
            },
            Bucket {
                gwei: 8.0,
                count: 2,
            }, // This should be the minimum
            Bucket {
                gwei: 12.0,
                count: 4,
            },
        ];

        let (price, settlement) = get_prediction_pending_floor(Some(pending_distribution)).unwrap();

        // Should be minimum (8.0) + 1 wei (0.000000001)
        let expected = 8.0 + ONE_WEI_IN_GWEI;
        assert_eq!(price, round_to_9_places(expected));
        assert_eq!(settlement, Settlement::Fast);
    }

    #[test]
    fn test_pending_floor_with_no_pending_distribution() {
        let result = get_prediction_pending_floor(None);

        // Should return an error when no pending distribution available
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires pending block distribution"));
    }

    #[test]
    fn test_pending_floor_with_empty_pending_distribution() {
        let pending_distribution = vec![];

        let result = get_prediction_pending_floor(Some(pending_distribution));

        // Should return an error when pending distribution is empty
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("non-empty pending block distribution"));
    }

    #[test]
    fn test_pending_floor_with_single_bucket() {
        let pending_distribution = vec![Bucket {
            gwei: 25.5,
            count: 10,
        }];

        let (price, settlement) = get_prediction_pending_floor(Some(pending_distribution)).unwrap();

        // Should be 25.5 + 1 wei
        let expected = 25.5 + ONE_WEI_IN_GWEI;
        assert_eq!(price, round_to_9_places(expected));
        assert_eq!(settlement, Settlement::Fast);
    }

    #[test]
    fn test_pending_floor_with_zero_minimum() {
        let pending_distribution = vec![
            Bucket {
                gwei: 0.0,
                count: 1,
            },
            Bucket {
                gwei: 5.0,
                count: 2,
            },
        ];

        let (price, settlement) = get_prediction_pending_floor(Some(pending_distribution)).unwrap();

        // Should be 0.0 + 1 wei
        let expected = 0.0 + ONE_WEI_IN_GWEI;
        assert_eq!(price, round_to_9_places(expected));
        assert_eq!(settlement, Settlement::Fast);
    }

    #[test]
    fn test_pending_floor_rounding() {
        let pending_distribution = vec![Bucket {
            gwei: 1.123456789123456789,
            count: 1,
        }];

        let (price, settlement) = get_prediction_pending_floor(Some(pending_distribution)).unwrap();

        // Should be properly rounded to 9 decimal places
        let expected = 1.123456789123456789 + ONE_WEI_IN_GWEI;
        assert_eq!(price, round_to_9_places(expected));
        assert_eq!(settlement, Settlement::Fast);
    }
}
