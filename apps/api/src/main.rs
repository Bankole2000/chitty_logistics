use std::process::ExitCode;

use api::config::Config;

const USAGE: &str = "usage: api [serve | migrate]

  serve    Start the server (default)
  migrate  Run pending database migrations and exit";

#[tokio::main]
async fn main() -> ExitCode {
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

    let command = std::env::args().nth(1);
    let result = match command.as_deref() {
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
