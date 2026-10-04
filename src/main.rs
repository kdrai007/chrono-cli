use clap::Parser;
use clockify_tui::cli::{run_cli, Cli};
use clockify_tui::config::AppConfig;
use clockify_tui::storage::Database;

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

    clockify_tui::tui::run_tui(&mut db, config)?;
    Ok(())
}
