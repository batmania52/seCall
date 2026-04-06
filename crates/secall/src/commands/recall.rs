use anyhow::{anyhow, Result};
use secall_core::{
    search::{Bm25Indexer, SearchEngine, SearchFilters},
    search::hybrid::parse_temporal_filter,
    search::query_expand::expand_query,
    search::tokenizer::create_tokenizer,
    store::{get_default_db_path, Database},
    vault::Config,
};

use crate::output::{print_search_results, OutputFormat};

pub async fn run(
    query: Vec<String>,
    since: Option<String>,
    project: Option<String>,
    agent: Option<String>,
    limit: usize,
    lex_only: bool,
    vec_only: bool,
    expand: bool,
    format: &OutputFormat,
) -> Result<()> {
    if query.is_empty() {
        return Err(anyhow!("Query cannot be empty"));
    }

    let query_str = query.join(" ");
    let query_str = if expand {
        expand_query(&query_str)?
    } else {
        query_str
    };
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    // Build filters
    let mut filters = SearchFilters::default();
    filters.project = project;
    filters.agent = agent;

    if let Some(since_str) = since {
        if let Some(temporal) = parse_temporal_filter(&since_str) {
            filters.since = temporal.since;
            filters.until = temporal.until;
        } else {
            // Try parse as ISO date
            if let Ok(dt) = chrono::NaiveDate::parse_from_str(&since_str, "%Y-%m-%d") {
                filters.since = dt.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
            }
        }
    }

    // Build search engine
    let config = Config::load_or_default();
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow!("tokenizer init failed: {e}"))?;
    let vector_indexer = if !lex_only {
        secall_core::search::vector::create_vector_indexer(&config).await
    } else {
        None
    };
    let engine = SearchEngine::new(Bm25Indexer::new(tok), vector_indexer);

    let results = if vec_only {
        engine.search_vector(&db, &query_str, limit, &filters).await?
    } else if lex_only {
        engine.search_bm25(&db, &query_str, &filters, limit)?
    } else {
        engine.search(&db, &query_str, &filters, limit).await?
    };

    if results.is_empty() {
        println!("No results found for: {}", query_str);
    } else {
        print_search_results(&results, format);
    }

    Ok(())
}
