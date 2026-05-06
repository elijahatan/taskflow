mod api;
mod cli;
mod config;
mod db;
mod models;

use anyhow::Result;
use log::info;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() -> Result<()> {
    let (global, cmd) = cli::parse_args()?;

    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| global.log_level.clone());

    std::env::set_var("RUST_LOG", &log_level);
    env_logger::init();

    info!("TaskFlow v{}", env!("CARGO_PKG_VERSION"));

    let mut config = config::Config::load();
    config.apply_env_overrides();

    let db_path: PathBuf = match &global.db {
        Some(path) => PathBuf::from(path),
        None => config.database.path.clone(),
    };

    let db_path_str = db_path.to_string_lossy().to_string();
    let db_path_ref = Path::new(&db_path_str);
    if let Some(parent) = db_path_ref.parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!("Using database at {}", db_path_str);
    let db = db::Database::open(db_path_ref)?;

    if let Err(e) = cli::run(cmd, &db).await {
        cli::display::error(&format!("{}", e));
        std::process::exit(1);
    }

    Ok(())
}
