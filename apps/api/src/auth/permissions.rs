//! Every permission the code checks, defined in one place (ADR-003).
//!
//! The list here is the source of truth for permission keys. [`sync`] runs on startup and
//! makes the `permissions` table match it. Which roles hold which permissions is data
//! (`role_permissions`), seeded with drafted defaults in
//! `migrations/20261127000000_staff_rbac.sql`.
//!
//! Adding a permission: add a line to the list below. Mapping it to a role needs a migration
//! that inserts the key (with `ON CONFLICT DO NOTHING`) and the `role_permissions` rows.

use std::collections::BTreeSet;

use sqlx::PgPool;

macro_rules! permissions {
    ($($variant:ident => $key:literal, $description:literal;)*) => {
        /// A permission a staff member may hold through their roles.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Permission {
            $($variant,)*
        }

        impl Permission {
            /// Every permission, in definition order.
            pub const ALL: &'static [Permission] = &[$(Permission::$variant,)*];

            /// The stable key stored in the database, e.g. `driver.approve`.
            pub const fn key(self) -> &'static str {
                match self {
                    $(Permission::$variant => $key,)*
                }
            }

            pub const fn description(self) -> &'static str {
                match self {
                    $(Permission::$variant => $description,)*
                }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                match key {
                    $($key => Some(Permission::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

permissions! {
    StaffView => "staff.view", "See staff accounts and their roles";
    StaffCreate => "staff.create", "Invite new staff members";
    StaffEditRole => "staff.edit_role", "Change the roles of staff members";
    StaffArchive => "staff.archive", "Archive staff who have left";
    StaffRestore => "staff.restore", "Restore archived staff";
    AuditView => "audit.view", "See the audit log";
    DriverView => "driver.view", "See driver applications and driver accounts";
    DriverApprove => "driver.approve", "Approve or reject drivers and vehicles";
    DriverSuspend => "driver.suspend", "Suspend and reinstate drivers";
    CustomerView => "customer.view", "See customer accounts";
    CustomerSuspend => "customer.suspend", "Suspend and reinstate customers";
    FareRuleEdit => "fare_rule.edit", "Change fare rules and prices";
    ServiceAreaEdit => "service_area.edit", "Change service areas";
    ReportView => "report.view", "See business reports";
    PayoutView => "payout.view", "See driver payouts";
    PayoutApprove => "payout.approve", "Approve driver payouts";
    TicketView => "ticket.view", "See support tickets";
    TicketReply => "ticket.reply", "Reply to support tickets";
    TicketAssign => "ticket.assign", "Assign support tickets to staff";
    RefundCreate => "refund.create", "Refund customers";
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key())
    }
}

/// What [`sync`] changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Keys in the code that were missing from the database and have been inserted.
    pub inserted: Vec<String>,
    /// Keys no longer in the code and mapped to no role; deleted.
    pub removed: Vec<String>,
    /// Keys no longer in the code but still mapped to a role; kept (and logged) so that no
    /// role silently loses a permission. Remove the mappings with a migration to drop them.
    pub stale_in_use: Vec<String>,
}

/// Makes the `permissions` table match [`Permission::ALL`]: inserts missing keys, updates
/// descriptions, deletes keys no longer in the code unless a role still holds them (those are
/// kept and logged as warnings). Runs in one transaction.
pub async fn sync(db: &PgPool) -> Result<SyncReport, sqlx::Error> {
    let mut tx = db.begin().await?;
    let mut report = SyncReport::default();

    for p in Permission::ALL {
        let inserted: bool = sqlx::query_scalar(
            "INSERT INTO permissions (key, description) VALUES ($1, $2)
             ON CONFLICT (key) DO UPDATE SET description = EXCLUDED.description
             RETURNING (xmax = 0)",
        )
        .bind(p.key())
        .bind(p.description())
        .fetch_one(&mut *tx)
        .await?;
        if inserted {
            report.inserted.push(p.key().to_string());
        }
    }

    let known: Vec<&str> = Permission::ALL.iter().map(|p| p.key()).collect();
    let stale: Vec<(String, bool)> = sqlx::query_as(
        "SELECT p.key, EXISTS (SELECT 1 FROM role_permissions rp WHERE rp.permission_key = p.key)
         FROM permissions p WHERE p.key <> ALL($1) ORDER BY p.key",
    )
    .bind(&known)
    .fetch_all(&mut *tx)
    .await?;
    for (key, in_use) in stale {
        if in_use {
            report.stale_in_use.push(key);
        } else {
            sqlx::query("DELETE FROM permissions WHERE key = $1")
                .bind(&key)
                .execute(&mut *tx)
                .await?;
            report.removed.push(key);
        }
    }
    tx.commit().await?;

    for key in &report.inserted {
        tracing::info!(permission = %key, "permission added");
    }
    for key in &report.removed {
        tracing::info!(permission = %key, "unused permission removed");
    }
    for key in &report.stale_in_use {
        tracing::warn!(
            permission = %key,
            "permission is no longer in the code but roles still hold it; kept until a migration removes its mappings"
        );
    }
    Ok(report)
}

/// The keys as a set, for comparing with the database.
pub fn all_keys() -> BTreeSet<&'static str> {
    Permission::ALL.iter().map(|p| p.key()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_round_trip() {
        assert_eq!(all_keys().len(), Permission::ALL.len());
        for p in Permission::ALL {
            assert_eq!(Permission::from_key(p.key()), Some(*p));
            assert!(!p.description().is_empty());
        }
        assert_eq!(Permission::from_key("nope.nothing"), None);
    }

    #[test]
    fn keys_have_the_resource_dot_action_shape() {
        for p in Permission::ALL {
            let (resource, action) = p.key().split_once('.').expect("one dot");
            let ok =
                |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
            assert!(ok(resource) && ok(action), "bad key {}", p.key());
        }
    }
}
