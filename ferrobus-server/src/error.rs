use axum::{
    BoxError, Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ferrobus_core::Error as CoreError;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ErrorBody {
                code,
                message: message.into(),
                details: None,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorEnvelope { error: self.body })).into_response()
    }
}

pub(crate) fn map_core_error(err: CoreError) -> ApiError {
    match err {
        CoreError::NoPointsFound => ApiError::new(
            StatusCode::BAD_REQUEST,
            "NO_POINTS_FOUND",
            "No nearby points found for snapping",
        ),
        CoreError::InvalidData(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, "INVALID_DATA", message)
        }
        CoreError::InvalidTimeFormat(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, "INVALID_TIME_FORMAT", message)
        }
        CoreError::IoError(message) => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "IO_ERROR",
            format!("I/O error: {message}"),
        ),
        other => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            other.to_string(),
        ),
    }
}

pub(crate) async fn handle_middleware_error(err: BoxError) -> Response {
    if err.is::<tower::timeout::error::Elapsed>() {
        return ApiError::new(
            StatusCode::REQUEST_TIMEOUT,
            "REQUEST_TIMEOUT",
            "Request timed out",
        )
        .into_response();
    }

    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "SERVICE_BUSY",
        "Service is currently busy",
    )
    .into_response()
}
