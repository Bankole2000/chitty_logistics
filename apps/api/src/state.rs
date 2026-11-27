use std::sync::Arc;

use sqlx::PgPool;

use crate::{
    config::Config,
    staff::invitation::{self, InvitationSender},
};

/// Shared by every handler. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    /// Delivers staff invitations; chosen by environment ([`invitation::for_config`]).
    pub invitations: Arc<dyn InvitationSender>,
}

impl AppState {
    /// The state with the invitation sender for the configured environment.
    pub fn new(db: PgPool, config: Config) -> Self {
        let invitations = invitation::for_config(&config);
        Self {
            db,
            config: Arc::new(config),
            invitations,
        }
    }
}
