use std::time::Duration;

use axum::{
    Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::DefaultBodyLimit,
    http::Request,
    routing::{MethodRouter, get, post},
};
use ferrobus_core::{Error as CoreError, TransitModel, TransitModelConfig, create_transit_model};
use tower::{ServiceBuilder, limit::ConcurrencyLimitLayer, timeout::TimeoutLayer};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::{Level, info, warn};

use crate::{
    config::{ServerCli, ServerConfig},
    error::handle_middleware_error,
    handlers,
    state::AppState,
};

fn with_medium_route_policy(route: MethodRouter<AppState>) -> MethodRouter<AppState> {
    route.route_layer(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_middleware_error))
            .layer(ConcurrencyLimitLayer::new(8))
            .layer(TimeoutLayer::new(Duration::from_secs(30))),
    )
}

fn with_heavy_route_policy(route: MethodRouter<AppState>) -> MethodRouter<AppState> {
    route.route_layer(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_middleware_error))
            .layer(ConcurrencyLimitLayer::new(4))
            .layer(TimeoutLayer::new(Duration::from_secs(60))),
    )
}

pub fn build_app(state: AppState) -> Router {
    let cors_layer = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &Request<Body>| {
            tracing::info_span!(
                "http_request",
                method = %request.method(),
                path = %request.uri().path()
            )
        })
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/v1/meta", get(handlers::meta))
        .route("/v1/route", post(handlers::route))
        .route(
            "/v1/routes-one-to-many",
            with_medium_route_policy(post(handlers::routes_one_to_many)),
        )
        .route(
            "/v1/detailed-journey",
            with_medium_route_policy(post(handlers::detailed_journey)),
        )
        .route(
            "/v1/matrix",
            with_heavy_route_policy(post(handlers::matrix)),
        )
        .route(
            "/v1/statistics",
            with_heavy_route_policy(post(handlers::statistics)),
        )
        .route(
            "/v1/range-route",
            with_heavy_route_policy(post(handlers::range_route)),
        )
        .route(
            "/v1/pareto-range-route",
            with_heavy_route_policy(post(handlers::pareto_range_route)),
        )
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(cors_layer)
        .layer(trace_layer)
        .with_state(state)
}

pub fn build_model(config: &ServerConfig) -> Result<TransitModel, CoreError> {
    let model_config = TransitModelConfig {
        osm_path: config.osm_path.clone(),
        gtfs_dirs: config.gtfs_dirs.clone(),
        date: config.date,
        max_transfer_time: config.max_transfer_time,
    };
    create_transit_model(&model_config)
}

pub async fn run_server(cli: ServerCli) -> Result<(), Box<dyn std::error::Error>> {
    let config = cli.resolve()?;
    let model = build_model(&config)?;
    let app = build_app(AppState::new(model));
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    info!(addr = %config.bind, "ferrobus server is listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("ferrobus server has stopped");
    Ok(())
}

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .with_target(false)
        .compact()
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => info!("shutdown signal received: ctrl_c"),
            Err(err) => warn!(%err, "failed to install ctrl_c handler"),
        }
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = ctrl_c => {}
                    _ = sigterm.recv() => info!("shutdown signal received: sigterm"),
                }
            }
            Err(err) => {
                warn!(%err, "failed to install SIGTERM handler, falling back to ctrl_c only");
                ctrl_c.await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}
