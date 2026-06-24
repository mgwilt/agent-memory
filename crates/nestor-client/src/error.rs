use nestor_api::{ApiProblem, ProblemKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Usage,
    BadRequest,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

impl ErrorCategory {
    pub fn exit_code(self) -> ExitCode {
        match self {
            Self::Usage => ExitCode(2),
            Self::BadRequest => ExitCode(3),
            Self::NotFound => ExitCode(4),
            Self::Conflict => ExitCode(5),
            Self::Unavailable => ExitCode(6),
            Self::Internal => ExitCode(7),
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    InvalidUrl(String),
    Transport(String),
    Timeout(String),
    Api(ApiProblem),
    InvalidResponse(String),
    Serialization(String),
}

impl ClientError {
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::InvalidUrl(_) | Self::Transport(_) | Self::Timeout(_) => {
                ErrorCategory::Unavailable
            }
            Self::Api(problem) => match problem.kind {
                ProblemKind::BadRequest | ProblemKind::ThresholdMiss => ErrorCategory::BadRequest,
                ProblemKind::NotFound => ErrorCategory::NotFound,
                ProblemKind::Conflict => ErrorCategory::Conflict,
                ProblemKind::ServiceUnavailable => ErrorCategory::Unavailable,
                ProblemKind::Internal => ErrorCategory::Internal,
            },
            Self::InvalidResponse(_) | Self::Serialization(_) => ErrorCategory::Internal,
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        self.category().exit_code()
    }
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(detail) => write!(f, "invalid API URL: {detail}"),
            Self::Transport(detail) => write!(f, "API transport error: {detail}"),
            Self::Timeout(url) => write!(f, "API request timed out: {url}"),
            Self::Api(problem) => write!(f, "{}: {}", problem.title, problem.detail),
            Self::InvalidResponse(detail) => write!(f, "invalid API response: {detail}"),
            Self::Serialization(detail) => write!(f, "serialization error: {detail}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<serde_json::Error> for ClientError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}
