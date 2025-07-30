use ntex::rt::spawn;
use std::{future::Future, panic};
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use tracing::info;

pub fn on_panic<F>(func: F)
where
    F: Fn(&panic::PanicHookInfo) + Send + Sync + 'static,
{
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        func(panic_info);
        default_hook(panic_info);
        std::process::exit(0);
    }));
}

#[cfg(unix)]
pub fn on_sigterm<F, Fut>(func: F) -> ntex::rt::JoinHandle<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut sigterm_stream =
        signal(SignalKind::terminate()).expect("Setup terminate signal stream");

    let mut sigint_stream = signal(SignalKind::interrupt()).expect("Setup interrupt signal stream");

    let shutdown_handler = spawn(async move {
        tokio::select! {
            _ = sigterm_stream.recv() => {
                info!("Received SIGTERM signal");
            }
            _ = sigint_stream.recv() => {
                info!("Received SIGINT signal");
            }
        }

        func().await;

        std::process::exit(0);
    });

    shutdown_handler
}

#[cfg(windows)]
pub fn on_sigterm<F, Fut>(func: F) -> ntex::rt::JoinHandle<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let shutdown_handler = spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl+c");
        info!("Received Ctrl+C signal");

        func().await;

        std::process::exit(0);
    });

    shutdown_handler
}
