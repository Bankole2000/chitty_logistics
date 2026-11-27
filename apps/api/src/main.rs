use std::{path::Path, process::ExitCode};

use api::config::Config;

const USAGE: &str = "usage: api [serve | migrate | create-admin --email <email> --name <name>
           | openapi (--out <file> | --check [<file>])]

  serve              Start the server (default)
  migrate            Run pending database migrations and exit
  create-admin --email <email> --name <name>
                     Create an active admin and print a one-time enrolment token, with
                     which they choose a password and enrol an authenticator app
                     (POST /staff/enrolment). No default credentials exist.
  openapi --out <file>
                     Write the OpenAPI 3.1 spec (JSON) to <file>, e.g.
                     packages/api-contract/openapi.json. Needs no database.
  openapi --check [<file>]
                     Exit non-zero if <file> (default packages/api-contract/openapi.json)
                     differs from the spec the code generates. Needs no database.";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // The contract commands need no configuration or database, so they run first.
    if args.first().map(String::as_str) == Some("openapi") {
        return openapi(&args[1..]);
    }

    // A missing .env is fine: deployed environments set real variables.
    let _ = dotenvy::dotenv();
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    api::init_tracing(config.log_format);

    let result = match args.first().map(String::as_str) {
        None | Some("serve") => api::serve(config).await,
        Some("migrate") => migrate(config).await,
        Some("create-admin") => match create_admin_args(&args[1..]) {
            Ok((email, name)) => create_admin(config, &email, &name).await,
            Err(e) => {
                eprintln!("{e}\n\n{USAGE}");
                return ExitCode::FAILURE;
            }
        },
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("unknown command {other:?}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "api failed");
            ExitCode::FAILURE
        }
    }
}

async fn migrate(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let db = api::pool(&config)?;
    api::MIGRATOR.run(&db).await?;
    tracing::info!("migrations up to date");
    Ok(())
}

/// `--email <email> --name <name>`, in either order.
fn create_admin_args(args: &[String]) -> Result<(String, String), String> {
    let (mut email, mut name) = (None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let slot = match flag.as_str() {
            "--email" => &mut email,
            "--name" => &mut name,
            other => return Err(format!("unknown argument {other:?}")),
        };
        *slot = Some(it.next().ok_or(format!("{flag} needs a value"))?.clone());
    }
    match (email, name) {
        (Some(e), Some(n)) => Ok((e, n)),
        _ => Err("create-admin needs --email and --name".into()),
    }
}

async fn create_admin(
    config: Config,
    email: &str,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = api::pool(&config)?;
    if config.migrate_on_start {
        api::MIGRATOR.run(&db).await?;
    }
    let created = api::auth::admin::create_admin(&db, &config.staff_auth, email, name).await?;
    // Printed once, to stdout only; the database keeps only its hash.
    println!(
        "Created admin {email} ({id}).\n\n\
         One-time enrolment token (valid until {expires}):\n\n  {token}\n\n\
         Give it to the admin privately. They send it with a new password to\n\
         POST /staff/enrolment, add the returned secret to an authenticator app, then\n\
         complete sign-in with POST /staff/sign-in/verify and a code from the app.",
        id = created.staff_id,
        expires = created.expires_at.to_rfc3339(),
        token = created.enrolment_token,
    );
    Ok(())
}

/// `api openapi --out <file>` or `api openapi --check [<file>]`.
fn openapi(args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["--out", file] => match api::openapi::write(Path::new(file)) {
            Ok(()) => {
                println!("wrote {file}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("could not write {file}: {e}");
                ExitCode::FAILURE
            }
        },
        ["--check"] | ["--check", _] => {
            let file = args.get(1).copied().unwrap_or(api::openapi::DEFAULT_PATH);
            match api::openapi::check(Path::new(file)) {
                Ok(()) => {
                    println!("{file} is up to date");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("openapi needs --out <file> or --check [<file>]\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
