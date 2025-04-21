use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::time::UtcTime;

pub fn init_logs() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = fmt::Subscriber::builder()
        .json()
        .flatten_event(true)
        .with_timer(UtcTime::rfc_3339())
        // see https://docs.rs/env_logger/latest/env_logger/#enabling-logging for details on how to use the RUST_LOG env var to control logging levels
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
