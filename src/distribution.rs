pub type BlockDistribution = Vec<Bucket>;

#[derive(Debug, Clone)]
pub struct Bucket {
    pub gwei: f64,
    pub count: u32,
}

#[derive(Debug)]
pub struct DistributionCreator {
    pub buckets: Vec<Bucket>,
    bucket_size: f64,
}

impl DistributionCreator {
    pub fn new(bucket_size: f64) -> Self {
        Self {
            buckets: Vec::new(),
            bucket_size,
        }
    }

    pub fn add(&mut self, value: f64) {
        // Calculate the rounding factor based on bucket_size
        let decimal_places = (-self.bucket_size.log10().floor()) as i32;
        let rounding_factor = 10.0f64.powi(decimal_places);

        let bucket_index =
            (((value / self.bucket_size).floor() * self.bucket_size) * rounding_factor).round()
                / rounding_factor;

        if let Some(pos) = self
            .buckets
            .iter()
            .position(|bucket| bucket.gwei == bucket_index)
        {
            self.buckets[pos].count += 1;
        } else {
            self.buckets.push(Bucket {
                gwei: bucket_index,
                count: 1,
            });
        }
    }
}
