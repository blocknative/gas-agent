use anyhow::{anyhow, Context, Result};
use opentelemetry::metrics::MeterProvider;
use opentelemetry::KeyValue;
use opentelemetry_sdk::{metrics::SdkMeterProvider, Resource};
use prometheus::Registry;
use std::sync::{Arc, OnceLock};

pub static METRICS: OnceLock<Arc<Metrics>> = OnceLock::new();

pub fn init_metrics(namespace: &str) -> Result<()> {
    let metrics = Arc::new(Metrics::new(namespace)?);

    METRICS
        .set(metrics)
        .map_err(|_| anyhow!("Metric client is already initialized"))?;

    Ok(())
}

pub fn get_metrics() -> Arc<Metrics> {
    let metrics = METRICS.get().expect("Metrics to be initialized");
    metrics.clone()
}

#[derive(Debug)]
pub struct Metrics {
    pub registry: Registry,
    pub agent_payload: opentelemetry::metrics::Gauge<f64>,
    pub agent_score: opentelemetry::metrics::Gauge<f64>,
    #[allow(dead_code)]
    provider: SdkMeterProvider,
}

impl Metrics {
    pub fn new(service: &str) -> Result<Self> {
        let registry = Registry::new();

        let exporter = opentelemetry_prometheus::exporter()
            .with_registry(registry.clone())
            .build()
            .context("Creating metrics exporter")?;

        let provider = SdkMeterProvider::builder()
            .with_reader(exporter)
            .with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service.to_string(),
            )]))
            .build();

        let meter = provider.meter(service.to_string());

        Ok(Self {
            registry,
            provider,
            agent_payload: meter
                .f64_gauge("gas_agent_payload")
                .with_description("The price of gas agent payload with tags.")
                .init(),
            agent_score: meter
                .f64_gauge("gas_agent_score")
                .with_description(
                    "The score assigned to the agent over a rolling 10 estimate window. Lower is better.",
                )
                .init(),
        })
    }
}
