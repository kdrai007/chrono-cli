use chrono_cli::cli::{run_cli, Cli};
use chrono_cli::config::AppConfig;
use chrono_cli::storage::Database;
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = if let Some(path) = cli.get_config_path() {
        AppConfig::load_from_path(&path)?
    } else {
        AppConfig::load()?
    };
    let db_path = cli.get_db_path();
    let mut db = Database::open_file(&db_path)?;

    if run_cli(cli, &mut db, &config)? {
        return Ok(());
    }

    chrono_cli::tui::run_tui(&mut db, config)?;
    Ok(())
}
