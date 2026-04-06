use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use secall_core::{
    hooks::run_post_ingest_hook,
    ingest::{
        detect::{
            detect_parser, find_claude_sessions, find_codex_sessions, find_gemini_sessions,
            find_sessions_for_cwd,
        },
        AgentKind,
    },
    search::tokenizer::create_tokenizer,
    search::{Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database, SessionRepo},
    vault::{Config, Vault},
};

use crate::output::{print_ingest_result, OutputFormat};

pub struct IngestStats {
    pub ingested: usize,
    pub skipped: usize,
    pub errors: usize,
}

pub async fn run(
    path: Option<String>,
    auto: bool,
    cwd: Option<PathBuf>,
    format: &OutputFormat,
) -> Result<()> {
    let config = Config::load_or_default();
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;
    let vault = Vault::new(config.vault.path.clone());
    vault.init()?;

    // Build search engine (BM25 + optional vector)
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow!("tokenizer init failed: {e}"))?;
    let vector_indexer = secall_core::search::vector::create_vector_indexer(&config).await;
    let engine = SearchEngine::new(Bm25Indexer::new(tok), vector_indexer);

    // Collect paths to ingest
    let paths = collect_paths(path.as_deref(), auto, cwd.as_deref())?;

    if paths.is_empty() {
        println!("No sessions to ingest.");
        return Ok(());
    }

    let stats = ingest_sessions(&config, &db, paths, &engine, &vault, format).await?;

    if stats.ingested > 0 || stats.skipped > 0 || stats.errors > 0 {
        eprintln!(
            "\nSummary: {} ingested, {} skipped (duplicate), {} errors",
            stats.ingested, stats.skipped, stats.errors
        );
    }

    Ok(())
}

/// ingest 핵심 로직 — sync.rs에서도 재사용
pub async fn ingest_sessions(
    config: &Config,
    db: &Database,
    paths: Vec<PathBuf>,
    engine: &SearchEngine,
    vault: &Vault,
    format: &OutputFormat,
) -> Result<IngestStats> {
    let mut ingested = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    // BM25/vault 완료 후 벡터 임베딩을 일괄 처리하기 위한 수집 목록.
    let mut vector_tasks: Vec<secall_core::ingest::Session> = Vec::new();

    for session_path in &paths {
        // detect_parser()를 한 번 호출 — 포맷 탐지와 라우팅을 동시에 결정
        let parser = match detect_parser(session_path) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(path = %session_path.display(), error = %e, "failed to detect session format");
                errors += 1;
                continue;
            }
        };

        // ClaudeAiParser는 항상 parse_all() 경로 (1:N)
        // agent_kind()로 판단하여 포맷·인코딩 방식과 무관하게 정확히 라우팅
        if parser.agent_kind() == AgentKind::ClaudeAi {
            match parser.parse_all(session_path) {
                Ok(sessions) => {
                    eprintln!(
                        "Parsed {} conversations from {}",
                        sessions.len(),
                        session_path.display()
                    );
                    for session in sessions {
                        ingest_single_session(
                            config,
                            db,
                            engine,
                            vault,
                            session,
                            format,
                            &mut ingested,
                            &mut skipped,
                            &mut errors,
                            &mut vector_tasks,
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %session_path.display(), error = %e, "failed to parse multi-session file");
                    errors += 1;
                }
            }
            continue;
        }

        // 1:1 파서: filename-stem 힌트로 빠른 중복 체크
        let session_id_hint = session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match db.session_exists(session_id_hint) {
            Ok(true) => {
                skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(path = %session_path.display(), error = %e, "DB check failed, skipping");
                errors += 1;
                continue;
            }
        }

        match parser.parse(session_path) {
            Ok(session) => {
                ingest_single_session(
                    config,
                    db,
                    engine,
                    vault,
                    session,
                    format,
                    &mut ingested,
                    &mut skipped,
                    &mut errors,
                    &mut vector_tasks,
                );
            }
            Err(e) => {
                tracing::warn!(path = %session_path.display(), error = %e, "failed to parse session file");
                errors += 1;
            }
        }
    }

    // 벡터 인덱싱 일괄 처리 (BM25/vault와 분리하여 체감 속도 개선)
    if !vector_tasks.is_empty() {
        eprintln!("Embedding {} session(s)...", vector_tasks.len());
        for session in &vector_tasks {
            if let Err(e) = engine.index_session_vectors(db, session).await {
                tracing::warn!(session = &session.id[..8.min(session.id.len())], error = %e, "vector embedding failed");
            }
        }
    }

    Ok(IngestStats {
        ingested,
        skipped,
        errors,
    })
}

/// 단일 Session을 vault + BM25 + 벡터 목록에 ingest
#[allow(clippy::too_many_arguments)]
fn ingest_single_session(
    config: &Config,
    db: &Database,
    engine: &SearchEngine,
    vault: &Vault,
    session: secall_core::ingest::Session,
    format: &OutputFormat,
    ingested: &mut usize,
    skipped: &mut usize,
    errors: &mut usize,
    vector_tasks: &mut Vec<secall_core::ingest::Session>,
) {
    // 실제 session.id 기준 중복 체크
    match db.session_exists(&session.id) {
        Ok(true) => {
            *skipped += 1;
            return;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(session = &session.id, error = %e, "DB check failed, skipping");
            *errors += 1;
            return;
        }
    }

    // 1. vault 파일 쓰기
    let rel_path = match vault.write_session(&session) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(session = &session.id, error = %e, "vault write failed");
            *errors += 1;
            return;
        }
    };

    let vault_path_str = rel_path.to_string_lossy().to_string();

    // 2. BM25 인덱싱 + vault_path 저장 (트랜잭션)
    let bm25_result = db.with_transaction(|| {
        let stats = engine.index_session_bm25(db, &session)?;
        db.update_session_vault_path(&session.id, &vault_path_str)?;
        Ok(stats)
    });

    let index_stats = match bm25_result {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(session = &session.id, error = %e, "indexing failed, rolling back");
            if let Err(rm_err) = std::fs::remove_file(config.vault.path.join(&rel_path)) {
                tracing::warn!(error = %rm_err, "failed to cleanup vault file");
            }
            *errors += 1;
            return;
        }
    };

    let abs_path = config.vault.path.join(&rel_path);
    print_ingest_result(&session, &abs_path, &index_stats, format);
    *ingested += 1;

    if let Err(e) = run_post_ingest_hook(config, &session, &abs_path) {
        tracing::warn!(session = &session.id[..8.min(session.id.len())], error = %e, "post-ingest hook failed");
    }

    // 3. 벡터 임베딩을 위해 수집
    vector_tasks.push(session);
}

fn collect_paths(path: Option<&str>, auto: bool, cwd: Option<&Path>) -> Result<Vec<PathBuf>> {
    if auto {
        if let Some(cwd) = cwd {
            find_sessions_for_cwd(cwd)
        } else {
            // Collect sessions from all supported agents
            let mut paths = find_claude_sessions(None)?;
            paths.extend(find_codex_sessions(None)?);
            paths.extend(find_gemini_sessions(None)?);
            Ok(paths)
        }
    } else if let Some(p) = path {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            Ok(vec![pb])
        } else if pb.is_dir() {
            let mut paths = find_claude_sessions(Some(&pb))?;
            paths.extend(find_codex_sessions(Some(&pb))?);
            paths.extend(find_gemini_sessions(Some(&pb))?);
            Ok(paths)
        } else {
            // Treat as session ID — search in ~/.claude/projects/
            find_session_by_id(p)
        }
    } else {
        Err(anyhow!("Provide a path, session ID, or use --auto"))
    }
}

fn find_session_by_id(id: &str) -> Result<Vec<PathBuf>> {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects");

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(&base)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            if stem == id
                || stem.starts_with(&format!("{id}_"))
                || stem.starts_with(&format!("{id}-"))
            {
                found.push(p.to_path_buf());
            }
        }
    }
    Ok(found)
}

