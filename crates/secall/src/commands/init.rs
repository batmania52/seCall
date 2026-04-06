use std::path::PathBuf;

use anyhow::Result;
use secall_core::{
    store::{get_default_db_path, Database},
    vault::{Config, Vault},
};

pub fn run(vault: Option<PathBuf>) -> Result<()> {
    let mut config = Config::load_or_default();

    if let Some(v) = vault {
        config.vault.path = v;
    }

    let vault_path = config.vault.path.clone();
    println!("Initializing seCall...");
    println!("  Vault:  {}", vault_path.display());
    println!("  Config: {}", Config::config_path().display());
    println!("  DB:     {}", get_default_db_path().display());

    // Save config
    config.save()?;

    // Init vault
    let v = Vault::new(vault_path.clone());
    v.init()?;

    // Init database
    let db_path = get_default_db_path();
    let _ = Database::open(&db_path)?;

    println!("\n✓ Initialization complete.");
    println!("\nTo configure Claude Code for auto-ingest, add to ~/.claude/settings.json:");
    println!(r#"{{
  "hooks": {{
    "PostToolUse": [{{
      "matcher": "Exit",
      "hooks": [{{"type": "command", "command": "secall ingest --auto --cwd $PWD"}}]
    }}]
  }}
}}"#);
    println!("\nTo start MCP server, add to ~/.claude/settings.json:");
    println!(r#"{{
  "mcpServers": {{
    "secall": {{
      "command": "secall",
      "args": ["mcp"]
    }}
  }}
}}"#);

    Ok(())
}
