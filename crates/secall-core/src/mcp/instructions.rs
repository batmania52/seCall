use crate::store::db::Database;

pub fn build_instructions(db: &Database) -> String {
    let session_count = db.count_sessions().unwrap_or(0);
    let projects = db.list_projects().unwrap_or_default();
    let agents = db.list_agents().unwrap_or_default();
    let has_embeddings = db.has_embeddings().unwrap_or(false);

    format!(
        r#"seCall — Agent Session Search Engine

Index contains {session_count} sessions across {project_count} projects.
Projects: {projects}
Agents: {agents}
Vector search: {vector_status}

## Usage Tips
- Use `recall` with keyword type for exact term matches (BM25)
- Use `recall` with semantic type for conceptual search (requires embeddings)
- Combine keyword + semantic queries for best results
- Use `get` with session_id:N to read a specific turn
- Filter by project or agent when searching across many sessions

## Example Queries
- Keyword: {{"queries": [{{"type": "keyword", "query": "SQLite FTS5"}}]}}
- Semantic: {{"queries": [{{"type": "semantic", "query": "how to design database schema"}}]}}
- Combined: {{"queries": [{{"type": "keyword", "query": "kiwi-rs"}}, {{"type": "semantic", "query": "Korean tokenizer comparison"}}]}}
- Temporal: {{"queries": [{{"type": "temporal", "query": "yesterday"}}, {{"type": "keyword", "query": "bugfix"}}]}}
"#,
        session_count = session_count,
        project_count = projects.len(),
        projects = if projects.is_empty() {
            "(none)".to_string()
        } else {
            projects.join(", ")
        },
        agents = if agents.is_empty() {
            "(none)".to_string()
        } else {
            agents.join(", ")
        },
        vector_status = if has_embeddings {
            "enabled"
        } else {
            "disabled (run `secall embed`)"
        },
    )
}
