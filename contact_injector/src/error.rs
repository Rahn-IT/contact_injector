//! Application errors translated at the HTTP boundary.

use std::fmt;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// A failed application operation, preserving its underlying cause.
#[derive(Debug)]
pub struct Error(anyhow::Error);

impl Error {
    /// Preserves the error chain when returning to the process boundary.
    pub fn into_report(self) -> anyhow::Error {
        self.0
    }
}

impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self.0, "request failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        )
            .into_response()
    }
}
