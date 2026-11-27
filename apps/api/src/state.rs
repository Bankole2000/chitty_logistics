use sqlx::PgPool;

use crate::config::Config;

/// Shared by every handler. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: std::sync::Arc<Config>,
}
