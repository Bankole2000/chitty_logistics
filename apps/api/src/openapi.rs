//! The OpenAPI 3.1 document for the api, committed as `packages/api-contract/openapi.json`.
//!
//! Every route and every named schema is listed in [`ApiDoc`]: when you add a route, add its
//! handler to `paths(...)` (and its schemas, if they are not reached from a path) and
//! regenerate with [`GENERATE_COMMAND`]. `cargo test` fails while the committed file is stale,
//! and while an endpoint has no Bruno request in `packages/api-contract/bruno/`.

use std::{fmt, fs, io, path::Path};

use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

use crate::{
    error::{ErrorBody, ErrorDetail},
    health::Health,
};

/// Where the committed spec lives, relative to the repository root.
pub const DEFAULT_PATH: &str = "packages/api-contract/openapi.json";

/// The command that regenerates the committed spec, run from the repository root.
pub const GENERATE_COMMAND: &str =
    "cargo run -p api -- openapi --out packages/api-contract/openapi.json";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Chitty Logistics api",
        description = "The one server every Chitty Logistics client calls. Every error response has the `ErrorBody` shape (docs/TECHNICAL.md, Error responses).",
        license(name = "MIT", identifier = "MIT"),
    ),
    servers((url = "http://127.0.0.1:18081", description = "Local development")),
    paths(
        crate::health::health,
        crate::auth::sign_in::enrol,
        crate::auth::sign_in::sign_in,
        crate::auth::sign_in::verify,
        crate::auth::sign_in::sign_out,
        crate::auth::sign_in::me,
        crate::staff::list,
        crate::staff::show,
        crate::staff::create,
        crate::staff::reissue_invitation,
        crate::staff::edit_role,
        crate::staff::archive,
        crate::staff::restore,
    ),
    components(schemas(ErrorBody, ErrorDetail, Health)),
    modifiers(&StaffSessionScheme),
    tags(
        (name = "health", description = "Liveness of the api and its database"),
        (name = "staff-auth", description = "Staff sign-in with email, password and an authenticator code; sessions and sign-out"),
        (name = "staff", description = "Staff accounts: invite with a role, change roles, archive and restore (admins)"),
    ),
)]
pub struct ApiDoc;

/// The `staff_session` security scheme: the opaque token from `POST /staff/sign-in/verify`,
/// sent as `Authorization: Bearer <token>` (ADR-003).
struct StaffSessionScheme;

impl Modify for StaffSessionScheme {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "staff_session",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .description(Some(
                            "Opaque staff session token from POST /staff/sign-in/verify.",
                        ))
                        .build(),
                ),
            );
        }
    }
}

/// The spec as pretty-printed JSON with a trailing newline. Paths, schemas and properties
/// are sorted, so the output only changes when the api does.
pub fn to_json() -> String {
    let mut json = ApiDoc::openapi()
        .to_pretty_json()
        .expect("the OpenAPI document serializes to JSON");
    json.push('\n');
    json
}

/// Writes the spec to `path`, creating its folder if needed.
pub fn write(path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, to_json())
}

/// Why [`check`] failed.
#[derive(Debug)]
pub enum CheckError {
    /// The file differs from what the code generates.
    Stale { path: String },
    /// The file could not be read.
    Unreadable { path: String, error: io::Error },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (path, problem) = match self {
            CheckError::Stale { path } => (path, "is out of date with the api code".to_string()),
            CheckError::Unreadable { path, error } => {
                (path, format!("could not be read ({error})"))
            }
        };
        let command = if path.replace('\\', "/") == DEFAULT_PATH {
            GENERATE_COMMAND.to_string()
        } else {
            format!("cargo run -p api -- openapi --out {path}")
        };
        write!(
            f,
            "{path} {problem}. Regenerate it with `{command}` and commit the result."
        )
    }
}

impl std::error::Error for CheckError {}

/// Compares the file at `path` with what the code generates.
pub fn check(path: &Path) -> Result<(), CheckError> {
    let display = path.display().to_string();
    let committed = fs::read_to_string(path).map_err(|error| CheckError::Unreadable {
        path: display.clone(),
        error,
    })?;
    if committed == to_json() {
        Ok(())
    } else {
        Err(CheckError::Stale { path: display })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_openapi_3_1_with_the_named_error_schema() {
        let spec: serde_json::Value = serde_json::from_str(&to_json()).unwrap();
        assert_eq!(spec["openapi"], "3.1.0");
        assert!(spec["components"]["schemas"]["ErrorBody"].is_object());
        assert!(spec["components"]["schemas"]["ErrorDetail"].is_object());
        assert!(spec["paths"]["/health"]["get"].is_object());
    }

    #[test]
    fn output_is_stable_and_ends_with_a_newline() {
        let json = to_json();
        assert_eq!(json, to_json());
        assert!(json.ends_with("}\n"));
    }

    #[test]
    fn stale_message_names_the_regenerate_command() {
        let e = CheckError::Stale {
            path: DEFAULT_PATH.into(),
        };
        assert!(e.to_string().contains(GENERATE_COMMAND), "{e}");
    }
}
