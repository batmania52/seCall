use anyhow::Result;
use secall_core::{
    graph::{build::build_graph, export::export_graph_json, semantic::extract_and_store},
    ingest::markdown::{extract_body_text, parse_session_frontmatter},
    store::{get_default_db_path, Database},
    vault::Config,
};

/// 전체 세션에 대해 시맨틱 엣지만 재추출. 임베딩은 건드리지 않음.
pub async fn run_semantic(
    delay_secs: f64,
    limit: Option<usize>,
    backend: Option<String>,
    api_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> Result<()> {
    let mut config = Config::load_or_default();

    // CLI 플래그 오버라이드 (우선순위: CLI > 환경변수 > config.toml > 기본값)
    if let Some(b) = backend {
        config.graph.semantic_backend = b;
    }
    if let Some(u) = api_url {
        config.graph.ollama_url = Some(u);
    }
    if let Some(m) = model {
        match config.graph.semantic_backend.as_str() {
            "gemini" => config.graph.gemini_model = Some(m),
            "anthropic" => config.graph.anthropic_model = Some(m),
            _ => config.graph.ollama_model = Some(m),
        }
    }
    if let Some(k) = api_key {
        config.graph.gemini_api_key = Some(k);
    }
    let db = Database::open(&get_default_db_path())?;

    if !config.graph.semantic {
        eprintln!("Semantic extraction is disabled (graph.semantic = false in config).");
        return Ok(());
    }
    if config.graph.semantic_backend == "disabled" {
        eprintln!(
            "Semantic backend is 'disabled'. Set graph.semantic_backend = \"ollama\" in config."
        );
        return Ok(());
    }

    // 임베딩 모델 언로드 (gemma4와 동시 로드 방지)
    if config.embedding.backend == "ollama" {
        let embed_model = config.embedding.ollama_model.as_deref().unwrap_or("bge-m3");
        let ollama_url = config
            .embedding
            .ollama_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let unload_url = format!("{}/api/generate", ollama_url.trim_end_matches('/'));
        let _ = secall_core::http_post_json(
            &unload_url,
            &serde_json::json!({"model": embed_model, "keep_alive": 0}),
        )
        .await;
    }

    // vault_path가 있는 세션만 추출
    let all_sessions: Vec<(String, String)> = db
        .list_session_vault_paths()?
        .into_iter()
        .filter_map(|(id, vp)| vp.map(|p| (id, p)))
        .collect();
    let total = all_sessions.len();
    let sessions: Vec<_> = match limit {
        Some(n) => all_sessions.into_iter().take(n).collect(),
        None => all_sessions,
    };
    let process_count = sessions.len();

    eprintln!(
        "Extracting semantic edges for {process_count}/{total} sessions (backend: {})...",
        config.graph.semantic_backend
    );

    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (i, (session_id, vault_path)) in sessions.iter().enumerate() {
        let short = &session_id[..8.min(session_id.len())];
        let md_path = config.vault.path.join(vault_path);

        let content = match std::fs::read_to_string(&md_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(session = short, "cannot read vault file: {}", e);
                skipped += 1;
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(session = short, "cannot parse frontmatter: {}", e);
                skipped += 1;
                continue;
            }
        };

        let body = extract_body_text(&content);
        match extract_and_store(&db, &config.graph, &fm, &body).await {
            Ok(n) => {
                eprintln!("  [{}/{}] {} — {} edges", i + 1, process_count, short, n);
                ok += 1;
            }
            Err(e) => {
                eprintln!("  [{}/{}] {} — FAILED: {}", i + 1, process_count, short, e);
                failed += 1;
            }
        }

        if delay_secs > 0.0 && i + 1 < process_count {
            tokio::time::sleep(std::time::Duration::from_secs_f64(delay_secs)).await;
        }
    }

    eprintln!("\nDone: {} ok, {} skipped, {} failed", ok, skipped, failed);
    Ok(())
}

pub fn run_build(since: Option<&str>, force: bool) -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    if force {
        eprintln!("Clearing existing graph...");
    }
    eprintln!("Building knowledge graph...");

    let result = build_graph(&db, &config.vault.path, since, force)?;

    eprintln!(
        "  {} sessions processed, {} skipped, {} failed.",
        result.sessions_processed, result.sessions_skipped, result.sessions_failed
    );
    eprintln!(
        "  {} nodes, {} edges created.",
        result.nodes_created, result.edges_created
    );
    Ok(())
}

pub fn run_stats() -> Result<()> {
    let db = Database::open(&get_default_db_path())?;
    let stats = db.graph_stats()?;

    println!("Graph Statistics:");
    println!("  Nodes: {}", stats.node_count);
    println!("  Edges: {}", stats.edge_count);
    println!();

    println!("Nodes by type:");
    for (t, c) in &stats.nodes_by_type {
        println!("  {}: {}", t, c);
    }
    println!();

    println!("Edges by relation:");
    for (r, c) in &stats.edges_by_relation {
        println!("  {}: {}", r, c);
    }
    Ok(())
}

pub fn run_export() -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    let graph_dir = config.vault.path.join("graph");
    std::fs::create_dir_all(&graph_dir)?;

    let output_path = graph_dir.join("graph.json");
    export_graph_json(&db, &output_path)?;

    eprintln!("Exported to {}", output_path.display());
    Ok(())
}
