//! A claim's example filled with the values the record or the repository
//! already knows, so an agent that pastes it claims the project's own values.
//! Where nothing is known the form's example stands.

use std::path::Path;

use familiar_ai_facts::{Fact, Log};
use familiar_ai_rules::{plan, View};
use serde_json::{Map, Value};

/// The record and the repository a claim is made in.
#[derive(Clone, Copy)]
pub struct Project<'a> {
    pub log: &'a Log,
    pub root: &'a Path,
}

/// Every criterion's planned test files, distinct, in claim order.
pub fn planned_files(log: &Log) -> Vec<String> {
    let v = View::new(log);
    let mut out: Vec<String> = Vec::new();
    for c in plan::criteria(&v) {
        for f in familiar_ai_rules::criterion::test_files(&v, &c) {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

/// True when every planned file is `.py`.
pub fn looks_like_pytest(files: &[String]) -> bool {
    !files.is_empty() && files.iter().all(|f| f.ends_with(".py"))
}

pub fn pytest_command(files: &[String]) -> Option<String> {
    looks_like_pytest(files).then(|| format!("{} -m pytest -q {}", python(), files.join(" ")))
}

/// `python` when it is on PATH here, else `python3` (the owner, 2026-10-15: try one, then
/// the other); looked up once. Never both in one command: a red test would run twice.
pub fn python() -> &'static str {
    static WORD: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    WORD.get_or_init(|| if on_path("python") { "python" } else { "python3" })
}

/// Whether an executable file `name` is in a folder of PATH.
fn on_path(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|d| std::fs::metadata(d.join(name)).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
    })
}

/// Each planned file run by `sh`, when every planned file is `.sh`.
pub fn sh_command(files: &[String]) -> Option<String> {
    (!files.is_empty() && files.iter().all(|f| f.ends_with(".sh"))).then(|| files.iter().map(|f| format!("sh {f}")).collect::<Vec<_>>().join(" && "))
}

pub fn next_free_criterion_id(v: &View) -> String {
    let taken = plan::criteria(v);
    let mut n = taken.len() + 1;
    loop {
        let id = format!("c{n}");
        if !taken.contains(&id) {
            return id;
        }
        n += 1;
    }
}

/// The first criterion still missing the kind's plan line, else the last
/// criterion claimed.
pub fn criterion_for(v: &View, kind: &str) -> Option<String> {
    // the parts in the plan: a released part is never named (the owner, 2026-10-28)
    let cs = plan::planned(v);
    let missing = |c: &String| match kind {
        "place_chosen" | "new_work_spot_chosen" | "new_work_no_spot" => plan::anchor_of(v, c).is_none(),
        "test_planned" => v.of("test_planned", c).next().is_none(),
        "criterion_head" | "criterion_strung_on_head" => !plan::on_string(v, c),
        "criterion_assigned" => plan::assigned_to(v, c).is_none(),
        "fix_location_category" => v.of("fix_location_category", c).next().is_none(),
        _ => false,
    };
    cs.iter().find(|c| missing(c)).or_else(|| cs.last()).cloned()
}

/// The criterion's latest stringing fact, the one the rule reads (`plan::on_string`).
fn string_fact<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    v.facts.iter().rev().find(|f| f.criterion_id() == Some(c) && matches!(f.kind, "criterion_head" | "criterion_strung_on_head"))
}

/// Whether the criterion's latest stringing fact is `criterion_head`.
pub fn is_head(v: &View, c: &str) -> bool {
    string_fact(v, c).is_some_and(|f| f.kind == "criterion_head")
}

/// The head of the string the criterion is on: itself when it is a head;
/// `None` when it is on no string.
pub fn head_of(v: &View, c: &str) -> Option<String> {
    if !plan::on_string(v, c) {
        return None;
    }
    let f = string_fact(v, c)?;
    if f.kind == "criterion_head" {
        return Some(c.to_string());
    }
    f.fields.get("head").and_then(Value::as_str).map(str::to_string)
}

/// The head of the string the last criterion on a string is on, in plan order.
pub fn last_strings_head(v: &View) -> Option<String> {
    plan::criteria(v).iter().rev().find(|c| plan::on_string(v, c)).and_then(|c| head_of(v, c))
}

/// Whether a test file's name is a plan id: a `.py` stem `test_c<n>` or
/// `test_<criterion>`, which names the plan, not what the file tests.
pub fn named_for_a_plan_id(file: &str, criterion: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    let Some(stem) = name.strip_suffix(".py") else {
        return false;
    };
    let numbered = stem.strip_prefix("test_c").is_some_and(|n| !n.is_empty() && n.chars().all(|ch| ch.is_ascii_digit()));
    numbered || stem == format!("test_{criterion}")
}

/// A test file named after the criterion's anchor: `tests/test_<symbol>.py`,
/// the symbol's last part in snake case; `None` with no anchor or no name.
pub fn test_file_for(v: &View, c: &str) -> Option<String> {
    let a = plan::anchor_of(v, c)?;
    let field = |k: &str| a.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let symbol = match a.kind {
        "place_chosen" => plan::places_of(a).first().map(|p| p.symbol.to_string()).unwrap_or_default(),
        "new_work_spot_chosen" => field("symbol"),
        "new_work_no_spot" => {
            let ip = field("integration_point");
            let file = plan::integration_file(&ip).to_string();
            match ip[file.len()..].trim_start_matches(':') {
                "" => Path::new(&file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                symbol => symbol.to_string(),
            }
        }
        _ => String::new(),
    };
    let name = snake(symbol.rsplit(['.', ':']).next().unwrap_or(""));
    let name = name.strip_prefix("test_").unwrap_or(&name);
    let file = format!("tests/test_{name}.py");
    (!name.is_empty() && !named_for_a_plan_id(&file, c)).then_some(file)
}

/// `GenerateJsonSchema` → `generate_json_schema`; anything not a letter or digit is `_`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    let mut after_lower = false;
    for ch in name.chars() {
        if ch.is_uppercase() {
            if after_lower {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
            after_lower = false;
        } else if ch.is_alphanumeric() {
            out.push(ch);
            after_lower = true;
        } else {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            after_lower = false;
        }
    }
    out.trim_matches('_').to_string()
}

/// The first word of the intent's text that names a file in the repository,
/// relative to the project root.
pub fn intent_file(log: &Log, root: &Path) -> Option<String> {
    let v = View::new(log);
    let text = v.last("intent_submitted").and_then(|f| f.fields.get("text")).and_then(Value::as_str)?;
    text.split_whitespace()
        .map(trim_punctuation)
        .filter(|w| !w.is_empty() && (w.contains('/') || w.contains('.')))
        .find_map(|w| {
            let path = Path::new(w);
            let rel = if path.is_absolute() {
                path.strip_prefix(root).ok()?.to_path_buf()
            } else {
                path.strip_prefix("./").map(Path::to_path_buf).unwrap_or_else(|_| path.to_path_buf())
            };
            root.join(&rel).is_file().then(|| rel.to_string_lossy().into_owned())
        })
}

/// Strips quotes, brackets and sentence punctuation from both ends of a word;
/// a leading `.` stays because `./x` is a path.
fn trim_punctuation(word: &str) -> &str {
    let mut w = word;
    loop {
        let t = w
            .trim_matches(|c: char| matches!(c, '`' | '\'' | '"' | '(' | ')' | ',' | ';' | ':' | '[' | ']' | '<' | '>'))
            .trim_end_matches(['.', ',', ';', ':']);
        if t == w {
            return w;
        }
        w = t;
    }
}

/// The name in a `def NAME` or `class NAME` line at column 0.
fn top_level_symbol(line: &str) -> Option<String> {
    let rest = line.strip_prefix("def ").or_else(|| line.strip_prefix("class "))?;
    let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some(name)
}

/// The first top-level `def`/`class` of a file and its 1-based range, ending
/// at the last non-blank line before the next top-level symbol or decorator.
pub fn first_symbol_range(path: &Path) -> Option<(String, u64, u64)> {
    let text = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| top_level_symbol(l).is_some())?;
    let symbol = top_level_symbol(lines[start])?;
    let next = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| top_level_symbol(l).is_some() || l.starts_with('@'))
        .map(|(i, _)| i)
        .unwrap_or(lines.len());
    let end = (start + 1..next).rev().find(|&i| !lines[i].trim().is_empty()).unwrap_or(start);
    Some((symbol, start as u64 + 1, end as u64 + 1))
}

/// A `place_chosen`'s places naming `file`: its first top-level symbol and that symbol's whole
/// range, since an anchor names a whole symbol, never one line; else `fallback`'s symbol and lines.
pub fn places_of_file(root: &Path, file: &str, fallback: (String, u64, u64)) -> String {
    let (symbol, start, end) = first_symbol_range(&root.join(file)).unwrap_or(fallback);
    serde_json::json!([{"file_path": file, "symbol": symbol, "line_start": start, "line_end": end}]).to_string()
}

/// The example values the project already knows for the kind's fields.
/// `criterion`, when the refusal names one, wins over the derived one.
pub fn known(kind: &str, p: &Project, criterion: Option<&str>) -> Map<String, Value> {
    let v = View::new(p.log);
    let mut out = Map::new();
    let cid: Option<String> = match kind {
        "criterion" => Some(next_free_criterion_id(&v)),
        _ => criterion.map(str::to_string).or_else(|| criterion_for(&v, kind)),
    };
    if let Some(c) = &cid {
        out.insert("criterion_id".into(), Value::from(c.as_str()));
    }
    match kind {
        "test_stack_detected" => {
            let files = planned_files(p.log);
            let stack = pytest_command(&files).map(|cmd| ("pytest", cmd)).or_else(|| sh_command(&files).map(|cmd| ("sh", cmd)));
            if let Some((stack, cmd)) = stack {
                out.insert("test_stack".into(), Value::from(stack));
                out.insert("test_command".into(), Value::from(cmd));
            }
        }
        "place_chosen" => {
            if let Some(file) = intent_file(p.log, p.root) {
                out.insert("places".into(), Value::from(places_of_file(p.root, &file, ("greet".to_string(), 10, 20))));
            }
        }
        "new_work_spot_chosen" => {
            if let Some(file) = intent_file(p.log, p.root) {
                out.insert("file_path".into(), Value::from(file));
            }
        }
        "test_planned" => {
            if let Some(file) = cid.as_deref().and_then(|c| test_file_for(&v, c)) {
                out.insert("file_path".into(), Value::from(file));
            }
        }
        // Every link names its string's head (the owner, 2026-10-15): the head of the last criterion on a string.
        "criterion_strung_on_head" => {
            if let Some(head) = last_strings_head(&v) {
                out.insert("head".into(), Value::from(head));
            }
        }
        _ => {}
    }
    out
}

/// The line after the example, for a fix location claim or for planned
/// files that are not pytest's.
pub fn note(kind: &str, p: &Project) -> Option<String> {
    if kind == "fix_location_category" {
        return Some(crate::teach::words::CLAIM_ELSEWHERE.to_string());
    }
    if kind != "test_stack_detected" {
        return None;
    }
    let files = planned_files(p.log);
    if files.is_empty() || looks_like_pytest(&files) || sh_command(&files).is_some() {
        return None;
    }
    Some(format!(
        "(the example's command is pytest's; this plan's tests are {}: --test_command is the command that runs these files, from the project's root, and --test_stack its stack)",
        files.join(" ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_ai_facts::append_raw;
    use serde_json::json;

    fn log_with(claims: &[(&str, Value)]) -> Log {
        let mut log: Log = Vec::new();
        for (i, (kind, fields)) in claims.iter().enumerate() {
            let Value::Object(m) = fields else { panic!("object") };
            log = append_raw(&log, kind, m.clone(), i as f64, "t").unwrap_or_else(|e| panic!("{kind}: {e}"));
        }
        log
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("familiar-ai-aware-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("pydantic")).expect("dir");
        std::fs::write(dir.join("pydantic/json_schema.py"), "x\n").expect("file");
        dir
    }

    #[test]
    fn the_stack_example_runs_the_planned_py_or_sh_files_and_notes_others() {
        let root = scratch("stack");
        let log = log_with(&[
            ("intent_submitted", json!({"text": "fix it", "sha": "abc"})),
            ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"})),
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_mutable_sequence.py", "will": "create"})),
        ]);
        let p = Project { log: &log, root: &root };
        let k = known("test_stack_detected", &p, None);
        // definition change (the owner, 2026-10-15, §9 Q3): the example runs python -m pytest, python3 when no python is on PATH
        assert_eq!(k["test_command"], json!(format!("{} -m pytest -q tests/test_mutable_sequence.py", python())));
        assert_eq!(k["test_stack"], json!("pytest"));
        assert!(note("test_stack_detected", &p).is_none());
        let log = log_with(&[
            ("intent_submitted", json!({"text": "fix it", "sha": "abc"})),
            ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"})),
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.sh", "will": "create"})),
        ]);
        let p = Project { log: &log, root: &root };
        let k = known("test_stack_detected", &p, None);
        assert_eq!((&k["test_stack"], &k["test_command"]), (&json!("sh"), &json!("sh tests/test_c1.sh")), "{k:?}");
        assert!(note("test_stack_detected", &p).is_none());
        let log = log_with(&[
            ("intent_submitted", json!({"text": "fix it", "sha": "abc"})),
            ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"})),
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/c1.test.js", "will": "create"})),
        ]);
        let p = Project { log: &log, root: &root };
        let k = known("test_stack_detected", &p, None);
        assert!(!k.contains_key("test_command"), "{k:?}");
        let n = note("test_stack_detected", &p).expect("note");
        assert!(n.contains("tests/c1.test.js") && n.contains("the command that runs these files"), "{n}");
        assert!(!n.contains('<') && !n.contains('>'));
        let empty: Log = Vec::new();
        let p = Project { log: &empty, root: &root };
        assert!(known("test_stack_detected", &p, None).is_empty());
        assert!(note("test_stack_detected", &p).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_criterion_id_is_the_next_free_one_or_the_first_missing_the_line_and_the_head_is_the_last_strung() {
        let root = scratch("ids");
        let log = log_with(&[
            ("intent_submitted", json!({"text": "fix it", "sha": "abc"})),
            ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"})),
            ("criterion", json!({"criterion_id": "c2", "text": "u", "path": "light"})),
            ("new_work_no_spot", json!({"criterion_id": "c1", "integration_point": "src/app.py:main"})),
            ("test_planned", json!({"criterion_id": "c1", "file_path": "tests/test_c1.py", "will": "create"})),
            ("criterion_head", json!({"criterion_id": "c1"})),
        ]);
        let p = Project { log: &log, root: &root };
        assert_eq!(known("criterion", &p, None)["criterion_id"], json!("c3"));
        assert_eq!(known("place_chosen", &p, None)["criterion_id"], json!("c2"));
        let k = known("test_planned", &p, None);
        assert_eq!(k["criterion_id"], json!("c2"));
        // definition change (lane l1, 2026-10-15): the example test file is named after what it tests, never a plan id
        assert!(!k.contains_key("file_path"), "c2 has no anchor: the form's example stands: {k:?}");
        assert_eq!(known("test_planned", &p, Some("c1"))["file_path"], json!("tests/test_main.py"), "named after c1's integration point");
        let k = known("criterion_strung_on_head", &p, None);
        assert_eq!(k["criterion_id"], json!("c2"));
        assert_eq!(k["head"], json!("c1"));
        assert_eq!(known("criterion_assigned", &p, None)["criterion_id"], json!("c1"), "none assigned: the first");
        assert_eq!(known("criterion_assigned", &p, Some("c2"))["criterion_id"], json!("c2"), "the refusal's wins");
        assert_eq!(known("hold_answered", &p, None)["criterion_id"], json!("c2"), "no line of its own: the last claimed");
        let empty: Log = Vec::new();
        let p = Project { log: &empty, root: &root };
        assert_eq!(known("criterion", &p, None)["criterion_id"], json!("c1"));
        assert!(known("place_chosen", &p, None).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_anchor_example_names_a_file_the_intent_names_when_it_is_in_the_repository() {
        let root = scratch("intent");
        for text in ["fix the schema in pydantic/json_schema.py", "see `pydantic/json_schema.py`.", "edit ./pydantic/json_schema.py, please", "(pydantic/json_schema.py)"] {
            let log = log_with(&[("intent_submitted", json!({"text": text, "sha": "abc"})), ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"}))]);
            let p = Project { log: &log, root: &root };
            assert_eq!(intent_file(&log, &root).as_deref(), Some("pydantic/json_schema.py"), "{text}");
            let k = known("place_chosen", &p, None);
            assert_eq!(k["places"], json!(r#"[{"file_path":"pydantic/json_schema.py","symbol":"greet","line_start":10,"line_end":20}]"#), "{text}");
            assert_eq!(known("new_work_spot_chosen", &p, None)["file_path"], json!("pydantic/json_schema.py"));
        }
        // With a top-level symbol in the file, the example anchors all of it.
        std::fs::write(
            root.join("pydantic/json_schema.py"),
            "import x\n\n\nclass GenerateJsonSchema:\n    def build(self):\n        return {}\n\n\n@decorated\ndef helper():\n    pass\n",
        )
        .expect("file");
        let log = log_with(&[("intent_submitted", json!({"text": "fix pydantic/json_schema.py", "sha": "abc"})), ("criterion", json!({"criterion_id": "c1", "text": "t", "path": "light"}))]);
        let p = Project { log: &log, root: &root };
        assert_eq!(known("place_chosen", &p, None)["places"], json!(r#"[{"file_path":"pydantic/json_schema.py","symbol":"GenerateJsonSchema","line_start":4,"line_end":6}]"#));
        assert_eq!(first_symbol_range(&root.join("pydantic/json_schema.py")), Some(("GenerateJsonSchema".into(), 4, 6)));
        std::fs::write(root.join("pydantic/json_schema.py"), "def f(a):\n    return a\n").expect("file");
        assert_eq!(first_symbol_range(&root.join("pydantic/json_schema.py")), Some(("f".into(), 1, 2)));
        std::fs::write(root.join("pydantic/json_schema.py"), "x\n").expect("file");
        assert_eq!(first_symbol_range(&root.join("pydantic/json_schema.py")), None);
        assert_eq!(first_symbol_range(&root.join("pydantic/missing.py")), None);
        let abs = root.join("pydantic/json_schema.py").to_string_lossy().into_owned();
        let log = log_with(&[("intent_submitted", json!({"text": format!("fix {abs}"), "sha": "abc"}))]);
        assert_eq!(intent_file(&log, &root).as_deref(), Some("pydantic/json_schema.py"), "absolute inside the project");
        for text in ["fix the schema", "fix pydantic/nothing.py", "fix /elsewhere/x.py", "version 1.2.3"] {
            let log = log_with(&[("intent_submitted", json!({"text": text, "sha": "abc"}))]);
            assert_eq!(intent_file(&log, &root), None, "{text}");
            let p = Project { log: &log, root: &root };
            assert!(!known("place_chosen", &p, None).contains_key("places"), "{text}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
