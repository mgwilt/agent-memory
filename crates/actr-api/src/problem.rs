use actr_core::MemoryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    BadRequest,
    NotFound,
    Conflict,
    ThresholdMiss,
    ServiceUnavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiProblem {
    pub kind: ProblemKind,
    pub title: String,
    pub detail: String,
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

        Self {
            kind,
            title: format!("{kind:?}"),
            detail: value.to_string(),
        }
    }
}
