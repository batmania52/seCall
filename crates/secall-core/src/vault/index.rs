use std::path::Path;

use anyhow::Result;

use crate::ingest::Session;

pub fn update_index(
    vault_path: &Path,
    session: &Session,
    md_path: &Path,
    tz: chrono_tz::Tz,
) -> Result<()> {
    let index_path = vault_path.join("index.md");
    let mut content = if index_path.exists() {
        std::fs::read_to_string(&index_path)?
    } else {
        "---\ntype: index\n---\n\n# seCall Index\n\n".to_string()
    };

    // Extract first user turn for title
    let title = session
        .turns
        .iter()
        .find(|t| t.role == crate::ingest::Role::User)
        .map(|t| {
            let s: String = t.content.chars().take(50).collect();
            if t.content.len() > 50 {
                format!("{}...", s)
            } else {
                s
            }
        })
        .unwrap_or_else(|| "Untitled Session".to_string());

    // Build the vault-relative link path (without .md extension for Obsidian)
    let link_path = md_path
        .to_string_lossy()
        .trim_end_matches(".md")
        .to_string();

    let agent = session.agent.as_str();
    let _project = session.project.as_deref().unwrap_or("unknown");
    let time_str = session
        .start_time
        .with_timezone(&tz)
        .format("%H:%M")
        .to_string();
    let turns = session.turns.len();

    let new_entry = format!(
        "- [[{}|{}]] — {}턴, {}, {}\n",
        link_path, title, turns, agent, time_str
    );

    // Insert after "## Sessions\n\n" header (at the beginning of the list)
    if let Some(pos) = content.find("## Sessions\n\n") {
        let insert_at = pos + "## Sessions\n\n".len();
        content.insert_str(insert_at, &new_entry);
    } else {
        content.push_str("\n## Sessions\n\n");
        content.push_str(&new_entry);
    }

    std::fs::write(&index_path, content)?;
    Ok(())
}
