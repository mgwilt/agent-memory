use std::time::Duration;

use nestor_client::{ClientError, ErrorCategory, ExitCode};

use crate::output::format_duration;

#[derive(Debug)]
pub enum CliError {
    Usage { detail: String, hint: String },
    Client { source: ClientError, hint: String },
    Internal { detail: String, hint: String },
}

impl CliError {
    pub fn usage(detail: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::Usage {
            detail: detail.into(),
            hint: hint.into(),
        }
    }

    pub fn client(source: ClientError, hint: impl Into<String>) -> Self {
        Self::Client {
            source,
            hint: hint.into(),
        }
    }

    pub fn internal(detail: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::Internal {
            detail: detail.into(),
            hint: hint.into(),
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage { .. } => ErrorCategory::Usage.exit_code(),
            Self::Client { source, .. } => source.exit_code(),
            Self::Internal { .. } => ErrorCategory::Internal.exit_code(),
        }
    }

    pub fn print(&self, include_footer: bool, elapsed: Duration) {
        match self {
            Self::Usage { detail, hint } => {
                eprintln!("[error] {detail}");
                eprintln!("{hint}");
            }
            Self::Client { source, hint } => {
                eprintln!("[error] {source}");
                eprintln!("{hint}");
            }
            Self::Internal { detail, hint } => {
                eprintln!("[error] {detail}");
                eprintln!("{hint}");
            }
        }
        if include_footer {
            eprintln!(
                "[exit:{} | {}]",
                self.exit_code().0,
                format_duration(elapsed)
            );
        }
    }
}
