//! Shared application services.

use std::sync::Arc;

use crate::job;

/// Services used by request handlers and sync workers.
#[derive(Debug, Clone)]
pub struct State {
    pub db: sqlx::SqlitePool,
    pub jinja: Arc<minijinja::Environment<'static>>,
    pub jobs: job::Manager,
}
