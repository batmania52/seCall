use anyhow::Result;
use secall_core::{
    ingest::lint::{run_lint, Severity},
    store::{get_default_db_path, Database},
    vault::Config,
};

pub fn run(json: bool, errors_only: bool) -> Result<()> {
    let config = Config::load_or_default();
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    let report = run_lint(&db, &config)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // Text output
    println!("secall lint report");
    println!("==================");

    let mut printed = 0;
    for finding in &report.findings {
        if errors_only && !matches!(finding.severity, Severity::Error) {
            continue;
        }
        let sev = finding.severity.as_str();
        let sid = finding
            .session_id
            .as_deref()
            .map(|s| format!("session {}: ", &s[..s.len().min(8)]))
            .unwrap_or_default();
        println!("{} [{sev:5}] {sid}{}", finding.code, finding.message);
        printed += 1;
    }

    if printed == 0 {
        println!("No issues found.");
    }

    println!();
    println!(
        "Summary: {} sessions, {} errors, {} warnings, {} info",
        report.summary.total_sessions,
        report.summary.errors,
        report.summary.warnings,
        report.summary.info,
    );

    if !report.summary.agents.is_empty() {
        let agent_str: Vec<String> = {
            let mut pairs: Vec<_> = report.summary.agents.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            pairs.iter().map(|(k, v)| format!("{k}({v})")).collect()
        };
        println!("Agents: {}", agent_str.join(", "));
    }

    // Exit with code 1 if there are errors
    if report.summary.errors > 0 {
        std::process::exit(1);
    }

    Ok(())
}
