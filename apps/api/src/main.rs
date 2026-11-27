use std::{path::Path, process::ExitCode};

use api::config::Config;

const USAGE: &str = "usage: api [serve | migrate | openapi (--out <file> | --check [<file>])]

  serve              Start the server (default)
  migrate            Run pending database migrations and exit
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
