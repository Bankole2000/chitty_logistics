//! Delivering staff invitations (FT-063) through a swappable [`InvitationSender`].
//!
//! No email provider is chosen yet (an open decision), so the only senders are:
//! - [`DevelopmentLogSender`], used when `API_ENV=development`: logs the invitation, with its
//!   one-time token, at info level so a developer can enrol the invited account locally.
//!   It can only be built in development, so the token never reaches deployed logs.
//! - [`UnavailableSender`], used in every other environment: refuses, so `POST /staff`
//!   answers 503 `invitation_delivery_unavailable` and creates nothing, rather than creating
//!   an account whose invitation nobody receives.
//!
//! Adding a provider: implement [`InvitationSender`] and choose it in [`for_config`].

use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::config::{AppEnv, Config};

/// An invitation to deliver. No Debug: holds the one-time token.
pub struct Invitation {
    pub staff_id: Uuid,
    pub email: String,
    pub name: String,
    /// The one-time enrolment token for `POST /staff/enrolment`. Only its hash is stored.
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// No way to deliver invitations is configured in this environment.
    #[error("no invitation sender is configured in this environment")]
    Unavailable,
    /// The sender tried and failed. The message must not contain the token.
    #[error("sending the invitation failed: {0}")]
    Failed(String),
}

pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + 'a>>;

/// Delivers invitations. Called inside the transaction that creates the invitation, before
/// commit: when sending fails, nothing is created.
pub trait InvitationSender: Send + Sync {
    fn send<'a>(&'a self, invitation: &'a Invitation) -> SendFuture<'a>;
}

/// The sender for an environment (see the module documentation).
pub fn for_config(config: &Config) -> Arc<dyn InvitationSender> {
    match DevelopmentLogSender::new(config.env) {
        Some(sender) => Arc::new(sender),
        None => Arc::new(UnavailableSender),
    }
}

/// Logs invitations, token included. Development only.
pub struct DevelopmentLogSender {
    _development_only: (),
}

impl DevelopmentLogSender {
    /// `None` outside `API_ENV=development`.
    pub fn new(env: AppEnv) -> Option<Self> {
        (env == AppEnv::Development).then_some(Self {
            _development_only: (),
        })
    }
}

impl InvitationSender for DevelopmentLogSender {
    fn send<'a>(&'a self, invitation: &'a Invitation) -> SendFuture<'a> {
        Box::pin(async move {
            tracing::info!(
                staff_id = %invitation.staff_id,
                email = %invitation.email,
                expires_at = %invitation.expires_at.to_rfc3339(),
                enrolment_token = %invitation.token,
                "staff invitation (API_ENV=development, no email provider): send the token with a new password to POST /staff/enrolment"
            );
            Ok(())
        })
    }
}

/// Refuses every invitation: used where no email provider is configured.
pub struct UnavailableSender;

impl InvitationSender for UnavailableSender {
    fn send<'a>(&'a self, invitation: &'a Invitation) -> SendFuture<'a> {
        Box::pin(async move {
            tracing::warn!(
                staff_id = %invitation.staff_id,
                "staff invitation not created: no email provider is configured in this environment"
            );
            Err(SendError::Unavailable)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logging_sender_exists_only_in_development() {
        assert!(DevelopmentLogSender::new(AppEnv::Development).is_some());
        for env in [AppEnv::Test, AppEnv::Staging, AppEnv::Production] {
            assert!(DevelopmentLogSender::new(env).is_none(), "{env:?}");
        }
    }

    #[tokio::test]
    async fn the_unavailable_sender_refuses() {
        let invitation = Invitation {
            staff_id: Uuid::new_v4(),
            email: "a@example.test".into(),
            name: "A".into(),
            token: "secret".into(),
            expires_at: Utc::now(),
        };
        assert!(matches!(
            UnavailableSender.send(&invitation).await,
            Err(SendError::Unavailable)
        ));
    }
}
