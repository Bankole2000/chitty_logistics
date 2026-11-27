-- Staff accounts, roles, permissions and audit entries (TASK-004; FR-019, FR-022; ADR-003).
--
-- The code checks permissions; roles are data that map to permissions, so changing what a
-- role may do is a data change (role_permissions), never a code change. The permission keys
-- themselves are defined in src/auth/permissions.rs and synced to `permissions` on startup.

-- Keeps updated_at current on every UPDATE of a table that uses it.
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- R-14 Staff account. Sign-in fields (password, second factor) come with TASK-005.
CREATE TABLE staff (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email       text NOT NULL CHECK (email <> ''),
    name        text NOT NULL CHECK (name <> ''),
    status      text NOT NULL DEFAULT 'invited' CHECK (status IN ('invited', 'active', 'archived')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    archived_at timestamptz,
    CHECK ((status = 'archived') = (archived_at IS NOT NULL))
);
-- Emails are unique regardless of case.
CREATE UNIQUE INDEX staff_email_key ON staff (lower(email));
CREATE TRIGGER staff_set_updated_at BEFORE UPDATE ON staff
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Permission keys, e.g. 'driver.approve'. Rows are owned by the code (synced on startup).
CREATE TABLE permissions (
    key         text PRIMARY KEY CHECK (key ~ '^[a-z_]+\.[a-z_]+$'),
    description text NOT NULL
);

-- Roles are data: add a role or change its permissions without a code change.
CREATE TABLE roles (
    key         text PRIMARY KEY CHECK (key ~ '^[a-z_]+$'),
    name        text NOT NULL,
    description text NOT NULL DEFAULT '',
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE role_permissions (
    role_key       text NOT NULL REFERENCES roles (key) ON DELETE CASCADE,
    permission_key text NOT NULL REFERENCES permissions (key) ON DELETE RESTRICT,
    PRIMARY KEY (role_key, permission_key)
);
CREATE INDEX role_permissions_permission_key ON role_permissions (permission_key);

CREATE TABLE staff_roles (
    staff_id   uuid NOT NULL REFERENCES staff (id) ON DELETE CASCADE,
    role_key   text NOT NULL REFERENCES roles (key) ON DELETE RESTRICT,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (staff_id, role_key)
);
CREATE INDEX staff_roles_role_key ON staff_roles (role_key);

-- R-16 Audit entry. Written by src/audit.rs inside the transaction of the change it records.
-- staff_id is null for changes made by the system. Entries are append-only.
CREATE TABLE audit_entries (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    staff_id    uuid REFERENCES staff (id) ON DELETE RESTRICT,
    action      text NOT NULL CHECK (action <> ''),
    record_type text NOT NULL CHECK (record_type <> ''),
    record_id   text NOT NULL CHECK (record_id <> ''),
    before      jsonb,
    after       jsonb,
    reason      text,
    request_id  text,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audit_entries_record ON audit_entries (record_type, record_id, created_at);
CREATE INDEX audit_entries_staff ON audit_entries (staff_id, created_at);
CREATE INDEX audit_entries_created_at ON audit_entries (created_at);

CREATE FUNCTION audit_entries_append_only() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit entries are append-only';
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER audit_entries_no_update_or_delete BEFORE UPDATE OR DELETE ON audit_entries
    FOR EACH ROW EXECUTE FUNCTION audit_entries_append_only();

-- Initial permission keys. Must match src/auth/permissions.rs (a test checks this); the
-- startup sync keeps them in step afterwards. A later migration that maps a new permission
-- to a role inserts the key first with ON CONFLICT DO NOTHING.
INSERT INTO permissions (key, description) VALUES
    ('staff.view',         'See staff accounts and their roles'),
    ('staff.create',       'Invite new staff members'),
    ('staff.edit_role',    'Change the roles of staff members'),
    ('staff.archive',      'Archive staff who have left'),
    ('staff.restore',      'Restore archived staff'),
    ('audit.view',         'See the audit log'),
    ('driver.view',        'See driver applications and driver accounts'),
    ('driver.approve',     'Approve or reject drivers and vehicles'),
    ('driver.suspend',     'Suspend and reinstate drivers'),
    ('customer.view',      'See customer accounts'),
    ('customer.suspend',   'Suspend and reinstate customers'),
    ('fare_rule.edit',     'Change fare rules and prices'),
    ('service_area.edit',  'Change service areas'),
    ('report.view',        'See business reports'),
    ('payout.view',        'See driver payouts'),
    ('payout.approve',     'Approve driver payouts'),
    ('ticket.view',        'See support tickets'),
    ('ticket.reply',       'Reply to support tickets'),
    ('ticket.assign',      'Assign support tickets to staff'),
    ('refund.create',      'Refund customers');

-- DRAFTED DEFAULTS, for the owner to change (PRD, Open decisions: staff roles). These are
-- data: change them with a later migration or an admin screen, not in code.
-- - admin: everything.
-- - operations: drivers, customers and support tickets; no money, prices, staff or audit log.
-- - finance: payouts, refunds and reports, and seeing drivers they pay.
-- - hr: only seeing staff; HR keeps no employee records in the product, and what else it
--   does is deferred for management.
INSERT INTO roles (key, name, description) VALUES
    ('admin',      'Admin',      'Manages staff, prices and service areas and sees the whole business'),
    ('operations', 'Operations', 'Approves drivers and handles customers and support'),
    ('finance',    'Finance',    'Handles payouts, refunds and reports'),
    ('hr',         'HR',         'Sees staff accounts (drafted minimum; to be decided by management)');

INSERT INTO role_permissions (role_key, permission_key)
SELECT 'admin', key FROM permissions;

INSERT INTO role_permissions (role_key, permission_key) VALUES
    ('operations', 'driver.view'),
    ('operations', 'driver.approve'),
    ('operations', 'driver.suspend'),
    ('operations', 'customer.view'),
    ('operations', 'customer.suspend'),
    ('operations', 'ticket.view'),
    ('operations', 'ticket.reply'),
    ('operations', 'ticket.assign'),
    ('finance',    'driver.view'),
    ('finance',    'payout.view'),
    ('finance',    'payout.approve'),
    ('finance',    'refund.create'),
    ('finance',    'report.view'),
    ('hr',         'staff.view');
