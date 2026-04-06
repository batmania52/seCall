use std::path::PathBuf;

pub mod db;
pub mod schema;

pub use db::Database;

pub fn get_default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("SECALL_DB_PATH") {
        return PathBuf::from(p);
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secall")
        .join("index.sqlite")
}
