use ntex::web::{self, get, App, ServiceConfig};
use std::{net::SocketAddr, sync::Arc};

mod responders;

type ConfigFn = fn(&mut ServiceConfig);

/// Starts a server without state
pub async fn start_server_without_state(
    server_address: &SocketAddr,
    configure_app: Option<ConfigFn>,
) -> std::io::Result<()> {
    start_server::<()>(&server_address, None, configure_app).await
}

/// Starts a server that will serve metrics and probes
pub async fn start_server<T: 'static + Send + Sync>(
    server_address: &SocketAddr,
    app_state: Option<Arc<T>>,
    configure_app: Option<ConfigFn>,
) -> std::io::Result<()> {
    web::server(move || {
        let mut app = App::new()
            // ==== INTERNAL ==== //
            .route(
                "/internal/probe/readiness",
                get().to(responders::probe::readiness),
            )
            .route(
                "/internal/probe/liveness",
                get().to(responders::probe::liveness),
            );

        // Apply app_state if provided
        if let Some(state) = &app_state {
            app = app.state(state.clone());
        }

        // Apply additional configuration if provided
        if let Some(config) = &configure_app {
            app = app.configure(config);
        }

        app
    })
    .bind(server_address)
    .expect("Bind to server address")
    .run()
    .await
    .expect("Running server");

    Ok(())
}
