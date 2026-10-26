//! Read the session's model from the transcript's last assistant entry, the
//! fallback when no hook input or model switch has named it. The format is
//! Claude Code's own and not a contract, so an unreadable model is no model.

use std::path::Path;

use serde_json::Value;

/// The placeholder Claude Code writes for an assistant entry no model produced.
const SYNTHETIC: &str = "<synthetic>";

/// The `message.model` of the transcript's last assistant entry.
pub fn last_assistant_model(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().rev().find_map(model_of_line)
}

fn model_of_line(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let m = v.get("message")?.get("model")?.as_str()?;
    (!m.is_empty() && m != SYNTHETIC).then(|| m.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_assistant_entrys_model_is_read_and_a_synthetic_one_skipped() {
        let dir = std::env::temp_dir().join(format!("familiar-ai-transcript-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.jsonl");
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"model":"claude-haiku-4-5-20251001","content":[]}}"#,
            r#"{"type":"assistant","message":{"model":"claude-sonnet-5-5","content":[]}}"#,
            r#"{"type":"assistant","message":{"model":"<synthetic>","content":[]}}"#,
            r#"{"type":"user","message":{"role":"user","content":"ok"}}"#,
            "not json",
        ];
        let _ = std::fs::write(&path, lines.join("\n"));
        assert_eq!(last_assistant_model(&path).as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(last_assistant_model(&dir.join("absent.jsonl")), None);
        let _ = std::fs::write(&path, lines[0]);
        assert_eq!(last_assistant_model(&path), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
