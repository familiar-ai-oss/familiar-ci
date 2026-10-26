//! A shell write through the real binary gets the same refusal an Edit gets,
//! while a write to `/tmp` or a read stays allowed.

mod common;

use common::Project;

/// The session the hook payloads carry; the binary runs with none.
const SESSION: &str = "s1";

impl Project {
    /// Runs `familiar-ai hook PreToolUse` with the tool's input and returns the printed JSON.
    fn pre(&self, tool_input: serde_json::Value, tool_name: &str) -> serde_json::Value {
        let payload = self.payload_as(SESSION, "PreToolUse", serde_json::json!({ "tool_name": tool_name, "tool_input": tool_input }));
        self.hook_with("PreToolUse", &payload)
    }

    fn bash_pre(&self, command: &str) -> serde_json::Value {
        self.pre(serde_json::json!({ "command": command }), "Bash")
    }

    fn edit(&self, rel: &str) -> serde_json::Value {
        self.pre(serde_json::json!({ "file_path": self.path(rel).to_string_lossy() }), "Edit")
    }
}

fn reason(v: &serde_json::Value) -> String {
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny", "{v}");
    v["hookSpecificOutput"]["permissionDecisionReason"].as_str().expect("reason").to_string()
}

const COUNTS: &str = "(a shell write counts as an edit)";

/// The plan confirmed: anchor `pydantic/json_schema.py`, planned test `tests/test_schema.py`.
fn confirm_plan(p: &Project) {
    p.claim(&["intent_submitted", "--for", "0", "--text", "fix the schema in pydantic/json_schema.py"]);
    p.claim(&["behavior", "--behavior_id", "b1", "--text", "the greeting"]);
    p.claim(&["promise", "--promise_id", "p1", "--behavior_id", "b1", "--accepts", "a name", "--gives_back", "a greeting", "--refuses", "nothing", "--cases", r#"[{"case_id":"n1","kind":"normal","given":"a name","expect":"a greeting"},{"case_id":"o1","kind":"opposite","given":"no name","expect":"no greeting"}]"#]);
    p.claim(&["criterion", "--criterion_id", "c1", "--text", "the schema names the model", "--path", "full", "--behavior_id", "b1", "--promise_id", "p1"]);
    p.cases_kept("c1"); // the cases card passed (plan cases-card, 2026-10-25) [draft names]
    p.claim(&["place_chosen", "--criterion_id", "c1", "--places", r#"[{"file_path":"pydantic/json_schema.py","symbol":"schema","line_start":1,"line_end":2}]"#]);
    // definition change (lane l1, 2026-10-15): a test file is named after what it tests, never a plan id
    p.claim(&["test_planned", "--criterion_id", "c1", "--file_path", "tests/test_schema.py", "--will", "create"]);
    p.claim(&["test_stack_detected", "--project_root", ".", "--test_stack", "pytest", "--test_command", "pytest -q tests/test_schema.py"]);
    // definition change (the owner's plan of 2026-10-23; owner approved 2026-10-23) [draft]: a pytest
    // part's place holds line 4 after a red try that lists it, the binary's listing
    let listed = serde_json::json!({ "criterion_id": "c1", "run_id": "try-1", "sha": "base", "red": true, "path_size": 1, "ranker": "words", "query": "the schema", "top": [{"rank": 1, "file_path": "pydantic/json_schema.py", "symbol": "schema", "line_start": 1, "line_end": 2}] });
    familiar_ai_facts::Record::open(&p.record_path()).expect("the record").append_raw("trace_listed", listed.as_object().cloned().unwrap_or_default(), 0.0, familiar_ai::TENANT).expect("the try's listing");
    p.claim(&["criterion_head", "--criterion_id", "c1"]);
    p.claim(&["criterion_assigned", "--criterion_id", "c1", "--agent_conversation_id", "s1"]);
    let out = p.claim(&["fix_location_category", "--criterion_id", "c1", "--category", "this_repository", "--where", ""]);
    assert!(out.contains("familiar-ai: the plan is confirmed"), "{out}");
}

#[test]
fn a_shell_write_is_refused_with_the_edits_words_and_allowed_where_an_edit_is() {
    let p = Project::builder("counts").no_session().no_app().file("pydantic/json_schema.py", "def schema():\n    return {}\n").create();

    // before the plan: the Edit's refusal
    let edits = reason(&p.edit("pydantic/json_schema.py"));
    assert!(edits.starts_with("familiar-ai: the plan comes first"), "{edits}");
    // both forms of a shell write, refused with the same words and the line
    let heredoc = "cat > pydantic/json_schema.py << 'EOF'\nimport json\nEOF";
    assert_eq!(reason(&p.bash_pre(heredoc)), format!("{edits}\n{COUNTS}"));
    let python = "python3 - << 'EOF'\nimport pathlib\nsrc = open('pydantic/json_schema.py').read()\nopen('pydantic/json_schema.py', 'w').write(src.replace('a', 'b'))\nEOF";
    assert_eq!(reason(&p.bash_pre(python)), format!("{edits}\n{COUNTS}"));
    let append = "cat >> tests/test_x.py << 'EOF'\ndef test_x():\n    assert 1 > 0\nEOF";
    let r = reason(&p.bash_pre(append));
    assert!(r.starts_with("familiar-ai: the plan comes first") && r.ends_with(COUNTS), "{r}");
    for cmd in ["sed -i 's/a/b/' pydantic/json_schema.py", "cp /tmp/x.py pydantic/json_schema.py", "git apply fix.diff", "echo x | tee pydantic/json_schema.py"] {
        let r = reason(&p.bash_pre(cmd));
        assert!(r.ends_with(COUNTS), "{cmd}: {r}");
    }
    // a write outside the project, a read, a command with no write: allowed
    for cmd in ["echo x > /tmp/outside", "cat pydantic/json_schema.py", "ls -la", "git status", "pytest -q", "familiar-ai claim intent_submitted --text 'a > b'"] {
        assert_eq!(p.bash_pre(cmd), serde_json::json!({}), "{cmd}");
    }
    assert!(!p.path("tests").exists(), "nothing was written by a refusal");

    // the plan confirmed: the same write to the anchor is allowed
    confirm_plan(&p);
    assert_eq!(p.edit("pydantic/json_schema.py"), serde_json::json!({}));
    assert_eq!(p.bash_pre(heredoc), serde_json::json!({}), "the anchor, by the shell");
    assert_eq!(p.bash_pre(python), serde_json::json!({}));
    // the planned test, not yet sealed: told what it is for before the write
    let out = p.bash_pre("cat > tests/test_schema.py << 'EOF'\ndef test_c1():\n    pass\nEOF");
    assert!(!out.to_string().contains("permissionDecision"), "the planned test: {out}");
    assert!(out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").starts_with("familiar-ai: tests/test_schema.py is the planned test of \"the schema names the model\"\n  asked for: "), "{out}");
    // another file: told, as an Edit is told
    let out = p.bash_pre("cat > pydantic/other.py << 'EOF'\nx = 1\nEOF");
    // the agent reads it before the write, and no decision is set
    assert!(!out.to_string().contains("permissionDecision"), "no decision: {out}");
    assert!(out["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").starts_with("familiar-ai: this edit is outside the place the plan named for "), "{out}");
}

fn confirmed(name: &str) -> Project {
    let p = Project::builder(name).no_session().no_app().file("pydantic/json_schema.py", "def schema():\n    return {}\n").create();
    confirm_plan(&p);
    p
}

fn told(v: &serde_json::Value) -> String {
    assert!(!v.to_string().contains("permissionDecision"), "no decision: {v}");
    v["hookSpecificOutput"]["additionalContext"].as_str().unwrap_or("").to_string()
}

#[test]
fn a_python_heredoc_writing_through_a_variable_path_is_the_file_it_names() {
    let p = confirmed("variable-path");
    let anchor = "python3 - << 'EOF'\np = 'pydantic/json_schema.py'\nsrc = open(p).read()\nopen(p, 'w').write(src)\nEOF";
    assert_eq!(p.bash_pre(anchor), serde_json::json!({}), "the anchor through a variable: no outside notice");
    let other = "python3 - << 'EOF'\np = \"pydantic/other.py\"\nopen(p, 'w').write('x')\nEOF";
    assert!(told(&p.bash_pre(other)).contains("outside the place the plan named"), "another file is still told");
}

#[test]
fn a_command_on_a_second_line_is_a_write() {
    let p = confirmed("second-line");
    let out = p.bash_pre("echo hi\ncp /tmp/x.py pydantic/other.py");
    assert!(told(&out).contains("outside the place the plan named"), "{out}");
}

#[test]
fn a_variable_target_resolves_through_its_assignment_and_what_does_not_is_a_file() {
    let before = Project::builder("var-target").no_session().no_app().file("pydantic/json_schema.py", "x = 1\n").create();
    let edits = reason(&before.edit("pydantic/json_schema.py"));
    assert_eq!(reason(&before.bash_pre("F=pydantic/json_schema.py\ncat > $F << 'EOF'\nx\nEOF")), format!("{edits}\n{COUNTS}"), "before the plan: the edit event");
    assert!(reason(&before.bash_pre("cat > $UNSET << 'EOF'\nx\nEOF")).ends_with(COUNTS), "an unresolved target is a file, not dropped");
    let p = confirmed("var-target-planned");
    assert_eq!(p.bash_pre("F=pydantic/json_schema.py\ncat > $F << 'EOF'\nx\nEOF"), serde_json::json!({}), "the anchor");
    assert!(told(&p.bash_pre("F=pydantic/other.py\ncat > $F << 'EOF'\nx\nEOF")).contains("outside the place the plan named"));
    assert!(told(&p.bash_pre("cat > $UNSET << 'EOF'\nx\nEOF")).contains("outside the place the plan named"), "a file");
    for t in ["$HOME/x", "${HOME}/x"] {
        let cmd = format!("cat > {t} << 'EOF'\nx\nEOF");
        assert_eq!(p.bash_pre(&cmd), serde_json::json!({}), "{t}: outside the project, passive");
        assert_eq!(before.bash_pre(&cmd), serde_json::json!({}), "{t}, before the plan");
    }
    for t in ["$TMPDIR/x", "${TMPDIR}/x"] {
        let cmd = format!("cat > {t} << 'EOF'\nx\nEOF");
        assert!(told(&p.bash_pre(&cmd)).contains("outside the place the plan named"), "{t}: a file");
        assert!(reason(&before.bash_pre(&cmd)).ends_with(COUNTS), "{t}, before the plan");
    }
    assert_eq!(p.bash_pre("cat > ~/notes.txt << 'EOF'\nx\nEOF"), serde_json::json!({}), "~ stays outside the project");
}
