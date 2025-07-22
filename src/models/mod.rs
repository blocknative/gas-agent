use crate::distribution::BlockDistribution;
use crate::types::{ModelKind, Settlement};
use adaptive_threshold::get_prediction_adaptive_threshold;
use distribution_analysis::get_prediction_distribution;
use last_min::get_prediction_last_min;
use moving_average::get_prediction_swma;
use pending_floor::get_prediction_pending_floor;
use percentile::get_prediction_percentile;
use time_series::get_prediction_time_series;

mod adaptive_threshold;
mod distribution_analysis;
mod errors;
mod last_min;
mod moving_average;
mod pending_floor;
mod percentile;
mod time_series;

pub use errors::ModelError;

pub type Prediction = f64;
pub type FromBlock = u64;

/// Will apply a model to a list of block distribution and return a price
/// Block distributions are sorted oldest to newest.
pub async fn apply_model(
    model: &ModelKind,
    block_distributions: &[BlockDistribution],
    pending_block_distribution: Option<BlockDistribution>,
    latest_block: u64,
) -> Result<(Prediction, Settlement, FromBlock), ModelError> {
    match model {
        ModelKind::AdaptiveThreshold => {
            get_prediction_adaptive_threshold(block_distributions, latest_block)
        }
        ModelKind::DistributionAnalysis => {
            get_prediction_distribution(block_distributions, latest_block)
        }
        ModelKind::MovingAverage => get_prediction_swma(block_distributions, latest_block),
        ModelKind::Percentile => get_prediction_percentile(block_distributions, latest_block),
        ModelKind::TimeSeries => get_prediction_time_series(block_distributions, latest_block),
        ModelKind::LastMin => get_prediction_last_min(block_distributions, latest_block),
        ModelKind::PendingFloor => {
            get_prediction_pending_floor(pending_block_distribution, latest_block)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::Bucket;

    #[tokio::test]
    async fn test_apply_model_pending_floor() {
        let pending_distribution = vec![
            Bucket {
                gwei: 10.0,
                count: 5,
            },
            Bucket {
                gwei: 5.0,
                count: 3,
            },
            Bucket {
                gwei: 15.0,
                count: 2,
            },
        ];

        let (price, settlement, from_block) = apply_model(
            &ModelKind::PendingFloor,
            &[],
            Some(pending_distribution),
            100,
        )
        .await
        .unwrap();

        // Should be minimum (5.0) + 1 wei (0.000000001)
        let expected = 5.0 + 0.000000001;
        assert_eq!(price, crate::utils::round_to_9_places(expected));
        assert_eq!(settlement, Settlement::Fast);
        assert_eq!(from_block, 101);
    }

    #[tokio::test]
    async fn test_apply_model_pending_floor_no_pending() {
        let result = apply_model(&ModelKind::PendingFloor, &[], None, 100).await;

        // Should return an error when no pending distribution
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires pending block distribution"));
    }

    #[test]
    fn test_pending_floor_model_kind_parsing() {
        use std::str::FromStr;

        // Test that PendingFloor can be parsed from string
        let model = ModelKind::from_str("pending_floor").unwrap();
        assert!(matches!(model, ModelKind::PendingFloor));

        // Test that it can be converted to string
        assert_eq!(model.to_string(), "pending_floor");
    }

    #[tokio::test]
    async fn test_last_min_model_errors() {
        // Test empty block distributions
        let result = apply_model(&ModelKind::LastMin, &[], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires at least one block"));

        // Test empty last block
        let empty_block = vec![];
        let result = apply_model(&ModelKind::LastMin, &[empty_block], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("non-empty block distribution"));
    }

    #[tokio::test]
    async fn test_percentile_model_errors() {
        // Test empty block distributions
        let result = apply_model(&ModelKind::Percentile, &[], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires at least one block"));

        // Test blocks with no transactions
        let empty_blocks = vec![vec![], vec![]];
        let result = apply_model(&ModelKind::Percentile, &empty_blocks, None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("blocks with transactions"));
    }

    #[tokio::test]
    async fn test_moving_average_model_errors() {
        // Test empty block distributions
        let result = apply_model(&ModelKind::MovingAverage, &[], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires at least one block"));

        // Test blocks with no transactions (should result in zero weight_sum)
        let empty_blocks = vec![vec![], vec![]];
        let result = apply_model(&ModelKind::MovingAverage, &empty_blocks, None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("blocks with transactions"));
    }

    #[tokio::test]
    async fn test_adaptive_threshold_model_errors() {
        // Test empty block distributions
        let result = apply_model(&ModelKind::AdaptiveThreshold, &[], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires at least one block"));

        // Test blocks with no transactions
        let empty_blocks = vec![vec![], vec![]];
        let result = apply_model(&ModelKind::AdaptiveThreshold, &empty_blocks, None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("blocks with transactions"));
    }

    #[tokio::test]
    async fn test_time_series_model_errors() {
        // Test empty block distributions
        let result = apply_model(&ModelKind::TimeSeries, &[], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires at least one block"));

        // Test blocks with no transactions
        let empty_blocks = vec![vec![], vec![], vec![]];
        let result = apply_model(&ModelKind::TimeSeries, &empty_blocks, None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("blocks with transactions"));
    }

    #[tokio::test]
    async fn test_distribution_analysis_model_errors() {
        // Test empty block distributions
        let result = apply_model(&ModelKind::DistributionAnalysis, &[], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires at least one block"));

        // Test empty latest block
        let empty_block = vec![];
        let result = apply_model(&ModelKind::DistributionAnalysis, &[empty_block], None, 100).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("non-empty latest block"));
    }

    #[tokio::test]
    async fn test_models_with_valid_data() {
        let valid_block = vec![
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
            },
        ];
        let blocks = vec![valid_block.clone(), valid_block.clone()];

        // Test all models with valid data
        let result = apply_model(&ModelKind::LastMin, &blocks, None, 100).await;
        assert!(result.is_ok());

        let result = apply_model(&ModelKind::Percentile, &blocks, None, 100).await;
        assert!(result.is_ok());

        let result = apply_model(&ModelKind::MovingAverage, &blocks, None, 100).await;
        assert!(result.is_ok());

        let result = apply_model(&ModelKind::AdaptiveThreshold, &blocks, None, 100).await;
        assert!(result.is_ok());

        let result = apply_model(&ModelKind::TimeSeries, &blocks, None, 100).await;
        assert!(result.is_ok());

        let result = apply_model(&ModelKind::DistributionAnalysis, &blocks, None, 100).await;
        assert!(result.is_ok());
    }
}
