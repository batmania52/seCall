use std::collections::VecDeque;
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::Deserialize;

use super::types::{Action, AgentKind, Role, Session, Turn};
use super::SessionParser;

pub struct GeminiParser;

impl SessionParser for GeminiParser {
    fn can_parse(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        (path_str.contains("/.gemini/") || path_str.contains("\\.gemini\\"))
            && path.extension().map(|e| e == "json").unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        parse_gemini_json(path)
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::GeminiCli
    }
}

// ─── Serde models ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiSession {
    id: String,
    #[serde(default)]
    create_time: Option<String>,
    #[serde(default)]
    update_time: Option<String>,
    #[serde(default)]
    messages: Vec<GeminiMessage>,
}

#[derive(Deserialize)]
struct GeminiMessage {
    role: String,
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

/// Untagged: variant is selected by which field is present.
/// FunctionCall and FunctionResponse must come before Text so that
/// the more specific variants are tried first.
#[derive(Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
    Text {
        text: String,
    },
}

#[derive(Deserialize)]
struct GeminiFunctionCall {
    name: String,
    #[serde(default)]
    args: serde_json::Value,
}

#[derive(Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    #[serde(default)]
    response: serde_json::Value,
}

// ─── Parser ───────────────────────────────────────────────────────────────────

pub fn parse_gemini_json(path: &Path) -> Result<Session> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 100 * 1024 * 1024 {
        eprintln!(
            "warn: gemini session file is large ({} MB): {}",
            metadata.len() / 1024 / 1024,
            path.display()
        );
    }

    let raw = std::fs::read_to_string(path)?;
    let gs: GeminiSession = serde_json::from_str(&raw)
        .map_err(|e| anyhow!("failed to parse gemini session {}: {e}", path.display()))?;

    // Extract projectId from path: /.gemini/tmp/<projectId>/chats/
    let project = extract_project_id(path);

    let mut turns: Vec<Turn> = Vec::new();
    let mut turn_idx: u32 = 0;
    // Queue of (turn_pos, action_idx) for pending functionCall → functionResponse matching.
    // Responses are consumed in FIFO order, so duplicate function names are matched correctly.
    let mut pending_responses: VecDeque<(usize, usize)> = VecDeque::new();

    for msg in &gs.messages {
        match msg.role.as_str() {
            "user" => {
                let content = collect_text_parts(&msg.parts);
                if !content.is_empty() {
                    turns.push(Turn {
                        index: turn_idx,
                        role: Role::User,
                        timestamp: None,
                        content,
                        actions: Vec::new(),
                        tokens: None,
                        thinking: None,
                        is_sidechain: false,
                    });
                    turn_idx += 1;
                }
            }
            "model" => {
                let content = collect_text_parts(&msg.parts);
                let mut actions = Vec::new();
                // Record turn position (= turns.len()) before pushing this turn
                let turn_pos = turns.len();

                for part in &msg.parts {
                    if let GeminiPart::FunctionCall { function_call } = part {
                        let action_idx = actions.len();
                        actions.push(Action::ToolUse {
                            name: function_call.name.clone(),
                            input_summary: function_call.args.to_string(),
                            output_summary: String::new(),
                            tool_use_id: None,
                        });
                        // Enqueue position so functionResponse can fill it in order
                        pending_responses.push_back((turn_pos, action_idx));
                    }
                }

                turns.push(Turn {
                    index: turn_idx,
                    role: Role::Assistant,
                    timestamp: None,
                    content,
                    actions,
                    tokens: None,
                    thinking: None,
                    is_sidechain: false,
                });
                turn_idx += 1;
            }
            "function" => {
                // Match functionResponse.name against pending functionCall.name
                for part in &msg.parts {
                    if let GeminiPart::FunctionResponse { function_response } = part {
                        // Find the first pending call whose name matches this response
                        let pos = pending_responses.iter().position(|(tp, ai)| {
                            turns
                                .get(*tp)
                                .and_then(|t| t.actions.get(*ai))
                                .map(|a| matches!(a, Action::ToolUse { name, .. } if name == &function_response.name))
                                .unwrap_or(false)
                        });
                        if let Some(idx) = pos {
                            let (turn_pos, action_idx) = pending_responses.remove(idx).unwrap();
                            if let Some(turn) = turns.get_mut(turn_pos) {
                                if let Some(Action::ToolUse { output_summary, .. }) =
                                    turn.actions.get_mut(action_idx)
                                {
                                    *output_summary = function_response.response.to_string();
                                }
                            }
                        }
                    }
                }
            }
            _ => {} // skip unknown roles
        }
    }

    if turns.is_empty() {
        return Err(anyhow!(
            "gemini session has no parseable turns: {}",
            path.display()
        ));
    }

    use chrono::DateTime;

    let start_time = gs
        .create_time
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let end_time = gs
        .update_time
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(Session {
        id: gs.id,
        agent: AgentKind::GeminiCli,
        model: None,
        project,
        cwd: None,
        git_branch: None,
        start_time,
        end_time,
        turns,
        total_tokens: Default::default(),
    })
}

/// Collect all text parts into a single string
fn collect_text_parts(parts: &[GeminiPart]) -> String {
    parts
        .iter()
        .filter_map(|p| {
            if let GeminiPart::Text { text } = p {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract projectId from path: ~/.gemini/tmp/<projectId>/chats/session-*.json
/// Supports both Unix (`/.gemini/tmp/`) and Windows (`\.gemini\tmp\`) separators.
fn extract_project_id(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    // Try Unix separator first, then Windows
    for marker in &["/.gemini/tmp/", "\\.gemini\\tmp\\"] {
        if let Some(pos) = path_str.find(marker) {
            let after = &path_str[pos + marker.len()..];
            // Split on either separator
            let end = after
                .find(|c| c == '/' || c == '\\')
                .unwrap_or(after.len());
            let project_id = &after[..end];
            if !project_id.is_empty() {
                return Some(project_id.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::Builder;

    fn make_gemini_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = Builder::new()
            .prefix("session-test-")
            .suffix(".json")
            .tempfile()
            .unwrap();
        write!(f, "{content}").unwrap();
        f
    }

    const BASIC_SESSION: &str = r#"{
        "id": "session-abc123",
        "createTime": "2026-04-05T10:00:00Z",
        "updateTime": "2026-04-05T10:30:00Z",
        "messages": [
            {"role":"user","parts":[{"text":"검색 기능 구현해줘"}]},
            {"role":"model","parts":[{"text":"네, 구현하겠습니다."}]}
        ]
    }"#;

    #[test]
    fn test_gemini_parse_basic() {
        let f = make_gemini_file(BASIC_SESSION);
        let session = parse_gemini_json(f.path()).unwrap();
        assert_eq!(session.id, "session-abc123");
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert!(session.turns[0].content.contains("검색"));
        assert_eq!(session.agent, AgentKind::GeminiCli);
    }

    #[test]
    fn test_gemini_parts_union() {
        let json = r#"{
            "id": "s1",
            "messages": [
                {"role":"user","parts":[{"text":"hello"}]},
                {"role":"model","parts":[
                    {"text":"ok"},
                    {"functionCall":{"name":"edit_file","args":{"path":"main.rs"}}}
                ]},
                {"role":"function","parts":[
                    {"functionResponse":{"name":"edit_file","response":{"result":"ok"}}}
                ]}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        let model_turn = session.turns.iter().find(|t| t.role == Role::Assistant).unwrap();
        assert_eq!(model_turn.actions.len(), 1);
        assert!(matches!(&model_turn.actions[0], Action::ToolUse { name, .. } if name == "edit_file"));
    }

    #[test]
    fn test_gemini_function_matching() {
        let json = r#"{
            "id": "s2",
            "messages": [
                {"role":"user","parts":[{"text":"run ls"}]},
                {"role":"model","parts":[
                    {"functionCall":{"name":"shell","args":{"cmd":"ls"}}}
                ]},
                {"role":"function","parts":[
                    {"functionResponse":{"name":"shell","response":{"output":"file1\nfile2"}}}
                ]}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        let model_turn = session.turns.iter().find(|t| t.role == Role::Assistant).unwrap();
        match &model_turn.actions[0] {
            Action::ToolUse { name, output_summary, .. } => {
                assert_eq!(name, "shell");
                assert!(output_summary.contains("file1") || output_summary.contains("output"));
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn test_gemini_detect_path() {
        let parser = GeminiParser;
        let p = Path::new("/Users/user/.gemini/tmp/proj123/chats/session-abc.json");
        assert!(parser.can_parse(p));
        let p2 = Path::new("/Users/user/.claude/projects/proj/session.jsonl");
        assert!(!parser.can_parse(p2));
        // must be .json not .jsonl
        let p3 = Path::new("/Users/user/.gemini/tmp/proj/chats/session.jsonl");
        assert!(!parser.can_parse(p3));
    }

    #[test]
    fn test_gemini_timestamps_parsed() {
        let f = make_gemini_file(BASIC_SESSION);
        let session = parse_gemini_json(f.path()).unwrap();
        // createTime "2026-04-05T10:00:00Z" → start_time
        assert_eq!(session.start_time.date_naive().to_string(), "2026-04-05");
        // updateTime "2026-04-05T10:30:00Z" → end_time
        assert!(session.end_time.is_some());
        assert_eq!(session.end_time.unwrap().date_naive().to_string(), "2026-04-05");
    }

    #[test]
    fn test_gemini_missing_timestamps_fallback() {
        let json = r#"{"id": "s-no-time", "messages": [
            {"role":"user","parts":[{"text":"hello"}]},
            {"role":"model","parts":[{"text":"hi"}]}
        ]}"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        // create_time 없으면 Utc::now() 근처 시간이어야 함
        let diff = (Utc::now() - session.start_time).num_seconds().abs();
        assert!(diff < 5, "fallback start_time should be near Utc::now()");
        assert!(session.end_time.is_none());
    }

    #[test]
    fn test_gemini_project_extraction() {
        let path = Path::new("/Users/user/.gemini/tmp/myproject123/chats/session-abc.json");
        let project = extract_project_id(path);
        assert_eq!(project, Some("myproject123".to_string()));

        let path2 = Path::new("/no/gemini/path/session.json");
        assert_eq!(extract_project_id(path2), None);
    }

    #[test]
    fn test_gemini_function_matching_by_name() {
        // Two different function calls — responses arrive in matching order
        let json = r#"{
            "id": "s-name-match",
            "messages": [
                {"role":"user","parts":[{"text":"do both"}]},
                {"role":"model","parts":[
                    {"functionCall":{"name":"read_file","args":{"path":"a.rs"}}},
                    {"functionCall":{"name":"edit_file","args":{"path":"b.rs"}}}
                ]},
                {"role":"function","parts":[
                    {"functionResponse":{"name":"edit_file","response":{"result":"edited"}}},
                    {"functionResponse":{"name":"read_file","response":{"content":"fn main()"}}}
                ]}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        let model_turn = session.turns.iter().find(|t| t.role == Role::Assistant).unwrap();
        assert_eq!(model_turn.actions.len(), 2);
        // read_file action should get read_file response (not edit_file's)
        match &model_turn.actions[0] {
            Action::ToolUse { name, output_summary, .. } => {
                assert_eq!(name, "read_file");
                assert!(output_summary.contains("fn main()"), "read_file should get read_file response, got: {output_summary}");
            }
            _ => panic!("expected ToolUse"),
        }
        // edit_file action should get edit_file response
        match &model_turn.actions[1] {
            Action::ToolUse { name, output_summary, .. } => {
                assert_eq!(name, "edit_file");
                assert!(output_summary.contains("edited"), "edit_file should get edit_file response, got: {output_summary}");
            }
            _ => panic!("expected ToolUse"),
        }
    }
}
