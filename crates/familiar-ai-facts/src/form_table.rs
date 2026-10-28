//! The `FORM` table: each fact kind's name, its fields and which of them may be absent.
//! The append validates every fact against it.

/// One kind's form; `optional` lists the fields that may be absent.
pub struct FormRow {
    pub kind: &'static str,
    pub fields: &'static [&'static str],
    pub optional: &'static [&'static str],
}

/// Every kind's form, in `KINDS` order.
pub const FORM: &[FormRow] = &[
    FormRow {
        kind: "intent_submitted",
        fields: &["text", "sha", "for"],
        optional: &["for"],
    },
    FormRow {
        kind: "behavior",
        fields: &["behavior_id", "text"],
        optional: &[],
    },
    FormRow {
        kind: "promise",
        fields: &[
            "promise_id",
            "behavior_id",
            "accepts",
            "gives_back",
            "refuses",
            "cases",
            "opposite_none",
            "contract_version",
        ],
        // the contract [draft names]: optional, so a promise made before it stays whole;
        // contract_version is the claim door's mark that the promise was made under it
        optional: &["cases", "opposite_none", "contract_version"],
    },
    FormRow {
        kind: "criterion",
        fields: &["criterion_id", "text", "path", "behavior_id", "promise_id", "for"],
        optional: &["behavior_id", "promise_id", "for"],
    },
    FormRow {
        kind: "place_chosen",
        fields: &["criterion_id", "places", "hold", "choice"],
        optional: &["hold", "choice"],
    },
    FormRow {
        kind: "new_work_spot_chosen",
        fields: &["criterion_id", "file_path", "symbol", "beside"],
        optional: &[],
    },
    FormRow {
        kind: "new_work_no_spot",
        fields: &["criterion_id", "integration_point"],
        optional: &[],
    },
    FormRow {
        kind: "criterion_head",
        fields: &["criterion_id"],
        optional: &[],
    },
    FormRow {
        kind: "criterion_strung_on_head",
        fields: &["criterion_id", "head"],
        optional: &[],
    },
    FormRow {
        kind: "test_stack_detected",
        fields: &["codebase_id", "project_root", "test_stack", "test_command"],
        optional: &[],
    },
    FormRow {
        kind: "test_written",
        fields: &["criterion_id", "file_path", "test_name"],
        optional: &[],
    },
    FormRow {
        kind: "pass_please_pass",
        fields: &[
            "criterion_id",
            "file_path",
            "test_name",
            "commit",
            "outcome",
            "ran_in",
            "with_anchor_reverted",
            "run_id",
        ],
        optional: &["test_name", "with_anchor_reverted", "run_id"],
    },
    FormRow {
        kind: "fingerprint_stamped",
        fields: &["criterion_id", "commit", "proof_not_tampered", "ran_in"],
        optional: &[],
    },
    FormRow {
        kind: "anchor_sha_matched",
        fields: &[
            "criterion_id",
            "file_path",
            "symbol",
            "line_start",
            "line_end",
            "sha",
            "lines_hash",
            "matched",
            "checked_at",
            "tip_matched",
        ],
        // the owner (2026-10-11): the start check's only
        optional: &["tip_matched"],
    },
    FormRow {
        kind: "test_planned",
        fields: &["criterion_id", "file_path", "will", "test_command"],
        optional: &["test_command"],
    },
    FormRow {
        kind: "touching_test_declared",
        fields: &["criterion_id", "file_path", "test_name"],
        optional: &[],
    },
    FormRow {
        kind: "criterion_assigned",
        fields: &["criterion_id", "agent_conversation_id"],
        optional: &[],
    },
    FormRow {
        kind: "criterion_transfer",
        fields: &["criterion_id", "from_conversation", "to_conversation"],
        optional: &[],
    },
    FormRow {
        kind: "commit_seen",
        fields: &[
            "commit",
            "files",
            "touches_tests",
            "touches_code",
            "anchor_overlap",
            "tests_hash",
            "by",
            "existing_tests_changed",
        ],
        optional: &["tests_hash", "by", "existing_tests_changed"],
    },
    FormRow {
        kind: "hold_answered",
        // from: "clear" on a release familiar-ai clear wrote, the person's (the owner, 2026-10-28)
        fields: &["criterion_id", "hold", "choice", "typed", "run_id", "commit", "approval", "from"],
        optional: &["run_id", "commit", "approval", "from"],
    },
    FormRow {
        kind: "pipeline_run_seen",
        fields: &["head_commit"],
        optional: &[],
    },
    FormRow {
        kind: "test_change_announced",
        fields: &["criterion_id", "reason"],
        optional: &[],
    },
    FormRow {
        kind: "model_seen",
        fields: &["agent_conversation_id", "model"],
        optional: &[],
    },
    FormRow {
        kind: "judge_verdict",
        fields: &["criterion_id", "model", "answers_intent", "reason", "gap", "cases", "places"],
        optional: &["gap", "cases", "places"],
    },
    FormRow {
        kind: "person_said",
        fields: &["text", "agent_conversation_id"],
        optional: &["agent_conversation_id"],
    },
    FormRow {
        kind: "person_said_answered",
        fields: &["for", "how"],
        optional: &[],
    },
    FormRow {
        kind: "judge_unanswered",
        fields: &["criterion_id", "seal", "why"],
        optional: &[],
    },
    FormRow {
        kind: "investigator_unfinished",
        fields: &["criterion_id", "conversation", "why"],
        optional: &[],
    },
    FormRow {
        kind: "fix_location_category",
        fields: &["criterion_id", "category", "where", "needs"],
        optional: &["needs"],
    },
    FormRow {
        kind: "run_reserved",
        fields: &[
            "run_id",
            "criterion_id",
            "criterion_rev",
            "commit",
            "command",
            "resources",
            "wait",
            "by",
            "after",
            "setup_from",
        ],
        optional: &["after", "setup_from"],
    },
    FormRow {
        kind: "run_started",
        fields: &["run_id", "pgid", "pgroup_start"],
        optional: &[],
    },
    FormRow {
        kind: "run_ended",
        fields: &["run_id", "outcome", "end", "exit_code", "signal", "stdout_tail", "stderr_tail", "truncated"],
        optional: &["exit_code", "signal", "stdout_tail", "stderr_tail", "truncated"],
    },
    FormRow {
        kind: "run_cancel_requested",
        fields: &["run_id"],
        optional: &[],
    },
    FormRow {
        kind: "tests_observed",
        fields: &["criterion_id", "criterion_rev", "commit", "tests_hash"],
        optional: &[],
    },
    FormRow {
        kind: "ci_report_seen",
        fields: &["sha", "run_id", "finished_at", "outcome", "report"],
        optional: &[],
    },
    // the owner (2026-10-11): the binary's check that a light criterion's
    // anchor is documentation; a new-work anchor names its file alone
    FormRow {
        kind: "anchor_is_documentation",
        fields: &["criterion_id", "file_path", "symbol", "line_start", "line_end", "sha", "matched"],
        optional: &["symbol", "line_start", "line_end"],
    },
    FormRow {
        kind: "familiar_switch",
        fields: &["to"],
        optional: &[],
    },
    // the test outcome translation's facts (the owner's plan of 2026-10-14, step S2), each the binary's
    FormRow {
        kind: "test_log_translation",
        fields: &[
            "run_id",
            "criterion_id",
            "commit",
            "by",
            "label",
            "evidence",
            "failing_tests",
            "classified_on",
            "location",
            "missing_name",
            "counts",
            "ran_tests",
            "skipped_tests",
            "versions",
            "frames",
        ],
        optional: &["location", "missing_name", "counts", "ran_tests", "skipped_tests", "versions", "frames"],
    },
    FormRow {
        kind: "dependency_drafted",
        fields: &["approval", "criterion_id", "text", "test_file", "place"],
        optional: &["place"],
    },
    FormRow {
        kind: "dependency_unanswered",
        fields: &["approval", "why"],
        optional: &[],
    },
    FormRow {
        kind: "told",
        fields: &["about"],
        optional: &[],
    },
    FormRow {
        kind: "criterion_covered",
        fields: &["criterion_id", "by_criterion"],
        optional: &[],
    },
    FormRow {
        kind: "subagent_ended",
        fields: &["agent_id", "agent_type"],
        optional: &[],
    },
    // the seed (the owner, 2026-10-15): the mark setup's seed writes last, the binary's
    FormRow {
        kind: "record_seeded",
        fields: &["commit", "plan_lines"],
        optional: &[],
    },
    // a resumed lane (the owner, 2026-10-16): its session started again, the binary's
    FormRow {
        kind: "holder_resumed",
        fields: &["agent_id"],
        optional: &[],
    },
    // the investigator's diagnosis of a test that never went red [draft] (the owner, 2026-10-16,
    // S7b), the binary's; each category's own fields are required by its type
    FormRow {
        kind: "investigator_finding",
        fields: &["criterion_id", "category", "model", "runs", "reason", "quotes", "guidance", "missed", "scope"],
        optional: &["guidance", "missed", "scope"],
    },
    // the adapter (the owner, 2026-10-19, lane a1) [draft]: how a runner is read, the binary's
    FormRow {
        kind: "reader_chosen",
        fields: &[
            "run_id", "root", "runner", "reader", "command", "commit", "setup", "versions", "familiar", "watched", "gaps", "why", "ran_in",
        ],
        optional: &["setup", "versions", "ran_in"],
    },
    // the reviewer at green (the owner's plan of 2026-10-20, step 3) [draft], the binary's
    FormRow {
        kind: "fix_verdict",
        fields: &["criterion_id", "commit", "model", "covers", "reason", "gap", "quotes", "why", "detail"],
        optional: &["gap", "quotes", "why", "detail"],
    },
    // an existing test's lines change only when announced (the owner, 2026-10-20) [draft]: the
    // agent's announcement, and the judge's word on it, the binary's
    FormRow {
        kind: "existing_test_announced",
        fields: &["file_path", "reason"],
        optional: &[],
    },
    FormRow {
        kind: "existing_test_verdict",
        fields: &["file_path", "commit", "model", "covers", "reason", "gap", "why"],
        optional: &["gap", "why"],
    },
    // the touching tests the binary derives from the fix's imports [draft], the binary's
    FormRow {
        kind: "touching_test_derived",
        fields: &["criterion_id", "file_path", "imports"],
        optional: &[],
    },
    // places before the fix (the owner's plan of 2026-10-21, step 3) [draft]: the binary's listing
    // and the agent's classification of each site
    FormRow {
        kind: "places_listed",
        fields: &["criterion_id", "sites"],
        optional: &[],
    },
    FormRow {
        kind: "place_classified",
        fields: &["criterion_id", "file", "line", "verdict", "reason"],
        optional: &[],
    },
    // a clarifying question on a signal (the owner's plan of 2026-10-21, step 2) [draft]: the
    // agent's word on how the person's ask reads, more than one way or one
    FormRow {
        kind: "reading_alternatives",
        fields: &["readings"],
        optional: &[],
    },
    FormRow {
        kind: "one_reading",
        fields: &["why"],
        optional: &[],
    },
    // test first, then pick the spot (the owner's plan of 2026-10-23) [draft]: the binary's listing
    // of what a try's failing tests ran, and the agent's reason for a place off that list
    FormRow {
        kind: "trace_listed",
        fields: &["criterion_id", "run_id", "sha", "red", "path_size", "ranker", "query", "top"],
        optional: &[],
    },
    FormRow {
        kind: "off_list_reason",
        fields: &["criterion_id", "reason"],
        optional: &[],
    },
    // the cases card (the owner's plan of 2026-10-25) [draft]: the binary's record of the
    // case-finder's answer, the cases the plan's own do not cover
    FormRow {
        kind: "cases_found",
        fields: &["criterion_id", "outcome", "cases", "why", "model", "tokens", "questions"],
        // the lens panel's synthesis (the owner's plan of 2026-10-26, approved): up to three
        // questions for the person
        optional: &["questions"],
    },
    // the lens panel (the owner's plan of 2026-10-26, approved): which Agent call got which lens,
    // each lens's answer (the binary's), and the person's pick on a synthesis's question
    FormRow {
        kind: "lens_released",
        fields: &["criterion_id", "lens", "tool_use_id", "agent_id"],
        // the agent a background launch runs the call as (the owner's answer, 2026-10-27)
        optional: &["agent_id"],
    },
    FormRow {
        kind: "lens_found",
        fields: &["criterion_id", "lens", "outcome", "findings", "model", "tokens"],
        optional: &[],
    },
    FormRow {
        kind: "question_answered",
        fields: &["criterion_id", "question_id", "choice", "typed"],
        optional: &[],
    },
];

/// The kind names, in `FORM` order.
pub const KINDS: [&str; 61] = [
    "intent_submitted",
    "behavior",
    "promise",
    "criterion",
    "place_chosen",
    "new_work_spot_chosen",
    "new_work_no_spot",
    "criterion_head",
    "criterion_strung_on_head",
    "test_stack_detected",
    "test_written",
    "pass_please_pass",
    "fingerprint_stamped",
    "anchor_sha_matched",
    "test_planned",
    "touching_test_declared",
    "criterion_assigned",
    "criterion_transfer",
    "commit_seen",
    "hold_answered",
    "pipeline_run_seen",
    "test_change_announced",
    "model_seen",
    "judge_verdict",
    "person_said",
    "person_said_answered",
    "judge_unanswered",
    "investigator_unfinished",
    "fix_location_category",
    "run_reserved",
    "run_started",
    "run_ended",
    "run_cancel_requested",
    "tests_observed",
    "ci_report_seen",
    "anchor_is_documentation",
    "familiar_switch",
    "test_log_translation",
    "dependency_drafted",
    "dependency_unanswered",
    "told",
    "criterion_covered",
    "subagent_ended",
    "record_seeded",
    "holder_resumed",
    "investigator_finding",
    "reader_chosen",
    "fix_verdict",
    "existing_test_announced",
    "existing_test_verdict",
    "touching_test_derived",
    "places_listed",
    "place_classified",
    "reading_alternatives",
    "one_reading",
    "trace_listed",
    "off_list_reason",
    "cases_found",
    "lens_released",
    "lens_found",
    "question_answered",
];

/// The kind's row in `FORM`.
pub fn form_row(kind: &str) -> Option<&'static FormRow> {
    FORM.iter().find(|r| r.kind == kind)
}
