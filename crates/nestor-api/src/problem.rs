use axum::{Json, http::StatusCode, response::IntoResponse};
use nestor_core::MemoryError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemKind {
    BadRequest,
    NotFound,
    Conflict,
    ThresholdMiss,
    ServiceUnavailable,
    Internal,
}

impl ProblemKind {
    pub fn status_code(self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::ThresholdMiss => StatusCode::OK,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::BadRequest => "bad request",
            Self::NotFound => "not found",
            Self::Conflict => "conflict",
            Self::ThresholdMiss => "retrieval threshold miss",
            Self::ServiceUnavailable => "service unavailable",
            Self::Internal => "internal error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiProblem {
    pub kind: ProblemKind,
    pub title: String,
    pub status: u16,
    pub detail: String,
}

impl ApiProblem {
    pub fn new(kind: ProblemKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            title: kind.title().to_string(),
            status: kind.status_code().as_u16(),
            detail: detail.into(),
        }
    }
}

impl From<MemoryError> for ApiProblem {
    fn from(value: MemoryError) -> Self {
        let kind = match value {
            MemoryError::Validation(_) => ProblemKind::BadRequest,
            MemoryError::NotFound(_) => ProblemKind::NotFound,
            MemoryError::Conflict(_) => ProblemKind::Conflict,
            MemoryError::ThresholdMiss { .. } => ProblemKind::ThresholdMiss,
            MemoryError::StoreUnavailable(_) => ProblemKind::ServiceUnavailable,
            MemoryError::Serialization(_) => ProblemKind::Internal,
        };

        Self::new(kind, value.to_string())
    }
}

impl IntoResponse for ApiProblem {
    fn into_response(self) -> axum::response::Response {
        (self.kind.status_code(), Json(self)).into_response()
    }
}
