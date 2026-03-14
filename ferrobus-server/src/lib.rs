mod app;
mod config;
mod error;
mod handlers;
mod state;

pub use app::{init_tracing, run_server};
pub use config::{ConfigError, ServerCli, ServerConfig};

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, OnceLock},
    };

    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode},
    };
    use chrono::NaiveDate;
    use ferrobus_core::{TransitModel, TransitModelConfig, create_transit_model};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::{app::build_app, state::AppState};

    static MODEL: OnceLock<Arc<TransitModel>> = OnceLock::new();

    fn test_model() -> Arc<TransitModel> {
        MODEL
            .get_or_init(|| {
                let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("tests")
                    .join("test-data");
                let config = TransitModelConfig {
                    osm_path: root.join("roads_zhelez.pbf"),
                    gtfs_dirs: vec![root.join("zhelez")],
                    date: NaiveDate::from_ymd_opt(2024, 1, 11),
                    max_transfer_time: 600,
                };
                Arc::new(create_transit_model(&config).expect("test model must be created"))
            })
            .clone()
    }

    fn test_app() -> Router {
        build_app(AppState {
            model: test_model(),
        })
    }

    async fn request_json(
        app: Router,
        method: Method,
        path: &str,
        payload: Option<Value>,
    ) -> (StatusCode, Value) {
        let body = payload.map_or_else(Body::empty, |v| Body::from(v.to_string()));
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(body)
            .expect("request should build");

        let resp = app.oneshot(req).await.expect("request should succeed");
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let value = if bytes.is_empty() {
            json!(null)
        } else {
            serde_json::from_slice(&bytes).expect("response must be valid json")
        };
        (status, value)
    }

    #[tokio::test]
    async fn health_and_meta_smoke() {
        let (status, health) = request_json(test_app(), Method::GET, "/healthz", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(health["status"], "ok");

        let (status, meta) = request_json(test_app(), Method::GET, "/v1/meta", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(meta["stop_count"].as_u64().unwrap_or(0) > 0);
        assert!(meta["route_count"].as_u64().unwrap_or(0) > 0);
    }

    #[tokio::test]
    async fn route_smoke_and_structured_error() {
        let route_req = json!({
            "start_point": { "lat": 56.256657, "lon": 93.533561 },
            "end_point": { "lat": 56.242574, "lon": 93.499159 },
            "departure_time": 43200,
            "max_transfers": 2
        });
        let (status, body) =
            request_json(test_app(), Method::POST, "/v1/route", Some(route_req)).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["travel_time_seconds"].as_u64().unwrap_or(0) > 0);

        let invalid_req = json!({
            "start_point": { "lat": 0.0, "lon": 0.0 },
            "end_point": { "lat": 56.242574, "lon": 93.499159 },
            "departure_time": 43200,
            "max_transfers": 2
        });
        let (status, body) =
            request_json(test_app(), Method::POST, "/v1/route", Some(invalid_req)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "NO_POINTS_FOUND");
    }
}
