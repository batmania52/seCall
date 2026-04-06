/// Vector indexer using SQLite BLOB storage + in-memory KNN search.
///
/// Note: sqlite-vec 0.1.10-alpha.3 has a C compilation issue on the current
/// macOS environment (Darwin 25.4, arm64). We use BLOB-based storage with
/// in-memory cosine similarity as a fallback. This is functionally equivalent
/// for MVP scale (< 100k chunks).
use anyhow::Result;

use super::bm25::{IndexStats, SearchFilters, SearchResult, SessionMeta};
use super::chunker::chunk_session;
use super::embedding::{Embedder, OllamaEmbedder, OpenAIEmbedder, OrtEmbedder};
use super::model_manager::ModelManager;
use crate::ingest::Session;
use crate::store::db::Database;
use crate::vault::config::Config;

#[derive(Debug)]
pub struct VectorRow {
    pub rowid: i64,
    pub distance: f32,
    pub session_id: String,
    pub turn_index: u32,
    pub chunk_seq: u32,
}

pub struct VectorIndexer {
    embedder: Box<dyn Embedder>,
}

impl VectorIndexer {
    pub fn new(embedder: Box<dyn Embedder>) -> Self {
        VectorIndexer { embedder }
    }

    pub async fn index_session(&self, db: &Database, session: &Session) -> Result<IndexStats> {
        let mut stats = IndexStats::default();
        let chunks = chunk_session(session);

        // Ensure vector table exists
        db.init_vector_table()?;

        // Batch embed
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        let batch_size = 32;

        for (batch_idx, text_batch) in texts.chunks(batch_size).enumerate() {
            match self.embedder.embed_batch(text_batch).await {
                Ok(embeddings) => {
                    for (i, embedding) in embeddings.into_iter().enumerate() {
                        let chunk_idx = batch_idx * batch_size + i;
                        if let Some(chunk) = chunks.get(chunk_idx) {
                            if let Err(e) = db.insert_vector(
                                &embedding,
                                &chunk.session_id,
                                chunk.turn_index,
                                chunk.seq,
                                self.embedder.model_name(),
                            ) {
                                eprintln!("warn: vector insert error: {e}");
                                stats.errors += 1;
                            } else {
                                stats.chunks_embedded += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("warn: embedding batch failed: {e}");
                    stats.errors += chunks.len();
                }
            }
        }

        Ok(stats)
    }

    pub async fn search(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;
        let rows = db.search_vectors(&query_embedding, limit)?;

        let results: Vec<SearchResult> = rows
            .into_iter()
            .filter_map(|row| {
                let session_meta = db.get_session_meta(&row.session_id).ok()?;
                if !passes_filters(&session_meta, filters) {
                    return None;
                }
                Some(SearchResult {
                    session_id: row.session_id,
                    turn_index: row.turn_index,
                    score: 1.0 - row.distance as f64,
                    bm25_score: None,
                    vector_score: Some(1.0 - row.distance as f64),
                    snippet: String::new(),
                    metadata: session_meta,
                })
            })
            .collect();

        Ok(results)
    }

    /// Embed a query string without DB access (safe to call before locking DB mutex).
    pub async fn embed_query(&self, query: &str) -> anyhow::Result<Vec<f32>> {
        self.embedder.embed(query).await
    }

    /// Search vectors using a pre-computed embedding (sync, no async needed).
    pub fn search_with_embedding(
        &self,
        db: &Database,
        embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let rows = db.search_vectors(embedding, limit)?;
        let results = rows
            .into_iter()
            .filter_map(|row| {
                let meta = db.get_session_meta(&row.session_id).ok()?;
                if !passes_filters(&meta, filters) {
                    return None;
                }
                Some(SearchResult {
                    session_id: row.session_id,
                    turn_index: row.turn_index,
                    score: 1.0 - row.distance as f64,
                    bm25_score: None,
                    vector_score: Some(1.0 - row.distance as f64),
                    snippet: String::new(),
                    metadata: meta,
                })
            })
            .collect();
        Ok(results)
    }
}

/// Check whether a session's metadata satisfies project/agent/date filters.
pub fn passes_filters(meta: &SessionMeta, filters: &SearchFilters) -> bool {
    if let Some(proj) = &filters.project {
        if meta.project.as_deref() != Some(proj.as_str()) {
            return false;
        }
    }
    if let Some(ag) = &filters.agent {
        if meta.agent != *ag {
            return false;
        }
    }
    // Date comparison against "YYYY-MM-DD" in meta.date
    if filters.since.is_some() || filters.until.is_some() {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&meta.date, "%Y-%m-%d") {
            if let Some(since) = filters.since {
                if date < since.date_naive() {
                    return false;
                }
            }
            if let Some(until) = filters.until {
                if date >= until.date_naive() {
                    return false;
                }
            }
        }
    }
    true
}

/// Create a VectorIndexer based on config.embedding.backend.
/// Falls back to Ollama if ort fails; returns None if neither is available.
pub async fn create_vector_indexer(config: &Config) -> Option<VectorIndexer> {
    match config.embedding.backend.as_str() {
        "ort" => {
            let model_dir = config
                .embedding
                .model_path
                .clone()
                .unwrap_or_else(default_model_path);

            // Auto-download model if not fully present (model.onnx + tokenizer.json)
            let mgr = ModelManager::new(model_dir.clone());
            if !mgr.is_downloaded() {
                eprintln!("⚠ ONNX model not found. Downloading...");
                if let Err(e) = mgr.download(false).await {
                    eprintln!("⚠ Download failed: {e}. Trying Ollama fallback...");
                    return try_ollama_fallback(config).await;
                }
            }

            match OrtEmbedder::new(&model_dir) {
                Ok(e) => {
                    eprintln!("✓ ort ONNX loaded. Local vector search enabled.");
                    Some(VectorIndexer::new(Box::new(e)))
                }
                Err(e) => {
                    eprintln!("⚠ ort load failed: {e}. Trying Ollama fallback...");
                    try_ollama_fallback(config).await
                }
            }
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            if !api_key.is_empty() {
                let model = config.embedding.openai_model.as_deref();
                let embedder = OpenAIEmbedder::new(&api_key, model);
                eprintln!("✓ OpenAI embedder ready ({}).", embedder.model_name());
                Some(VectorIndexer::new(Box::new(embedder)))
            } else {
                eprintln!("⚠ OPENAI_API_KEY not set. Trying Ollama fallback...");
                try_ollama_fallback(config).await
            }
        }
        _ => {
            // "ollama" or any unknown value → Ollama
            try_ollama_fallback(config).await
        }
    }
}

async fn try_ollama_fallback(config: &Config) -> Option<VectorIndexer> {
    let base_url = config.embedding.ollama_url.as_deref();
    let model = config.embedding.ollama_model.as_deref();
    let embedder = OllamaEmbedder::new(base_url, model);
    if embedder.is_available().await {
        eprintln!("✓ Ollama available. Vector search enabled.");
        Some(VectorIndexer::new(Box::new(embedder)))
    } else {
        eprintln!("⚠ Ollama not available. Vector search disabled. BM25-only mode.");
        None
    }
}

fn default_model_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".cache")
        .join("secall")
        .join("models")
        .join("bge-m3-onnx")
}

// Vector table operations on Database
impl Database {
    pub fn init_vector_table(&self) -> Result<()> {
        self.conn().execute_batch(
            "
            CREATE TABLE IF NOT EXISTS turn_vectors (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  TEXT NOT NULL,
                turn_index  INTEGER NOT NULL,
                chunk_seq   INTEGER NOT NULL,
                model       TEXT NOT NULL,
                embedded_at TEXT NOT NULL,
                embedding   BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_vectors_session ON turn_vectors(session_id);
        ",
        )?;
        Ok(())
    }

    pub fn insert_vector(
        &self,
        embedding: &[f32],
        session_id: &str,
        turn_index: u32,
        chunk_seq: u32,
        model: &str,
    ) -> Result<i64> {
        let bytes = floats_to_bytes(embedding);
        self.conn().execute(
            "INSERT INTO turn_vectors(session_id, turn_index, chunk_seq, model, embedded_at, embedding)
             VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
            rusqlite::params![session_id, turn_index as i64, chunk_seq as i64, model, bytes],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn search_vectors(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors",
        )?;

        let rows: Vec<(i64, String, u32, u32, Vec<u8>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get::<_, i64>(2)? as u32,
                    row.get::<_, i64>(3)? as u32,
                    row.get(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut scored: Vec<(f32, VectorRow)> = rows
            .into_iter()
            .map(|(id, session_id, turn_index, chunk_seq, bytes)| {
                let embedding = bytes_to_floats(&bytes);
                let distance = cosine_distance(query_embedding, &embedding);
                (
                    distance,
                    VectorRow {
                        rowid: id,
                        distance,
                        session_id,
                        turn_index,
                        chunk_seq,
                    },
                )
            })
            .collect();

        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }
}

fn floats_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

fn bytes_to_floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    1.0 - (dot / (norm_a * norm_b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::Database;

    #[test]
    fn test_vector_indexer_with_trait_object() {
        // Compile-time check: Box<dyn Embedder> works as VectorIndexer embedder
        let embedder: Box<dyn Embedder> = Box::new(OllamaEmbedder::new(None, None));
        let _indexer = VectorIndexer::new(embedder);
    }

    #[test]
    fn test_init_vector_table() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        // Re-init should be idempotent
        db.init_vector_table().unwrap();
    }

    #[test]
    fn test_insert_and_search_vectors() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();

        let emb1: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb2: Vec<f32> = vec![0.0, 1.0, 0.0];
        let query: Vec<f32> = vec![1.0, 0.1, 0.0];

        db.insert_vector(&emb1, "s1", 0, 0, "bge-m3").unwrap();
        db.insert_vector(&emb2, "s2", 0, 0, "bge-m3").unwrap();

        let rows = db.search_vectors(&query, 2).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session_id, "s1");
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!((cosine_distance(&a, &b) - 0.0).abs() < 0.001);

        let c = vec![0.0, 1.0];
        assert!((cosine_distance(&a, &c) - 1.0).abs() < 0.001);
    }
}
