# Design decisions

Each entry gives the decision, the alternatives, the reason, and the code
and tests that implement it. Commit hashes are short SHAs in this
repository. "Open" marks a decision with no code or test at this commit.
The first section defines the CI check; each later entry either serves it
or states that it does not.

Five invariants. Every later entry follows from one of them:

1. **Facts in, states derived.** The only stored data is an append-only
   list of facts. Every state is computed from it. No mode toggle,
   classifier or watchdog.
2. **A plan change is the agent's, claimed before it acts on it.**
   **The one exception: the binary appends a dependency on the person's
   approval** (D13, plan §7.3). When the person answers a card
   `a_dependency`, the binary asks the holder's model once (with the record
   let go, 50 cents at most) for the dependency's words, test file and
   place, then appends `dependency_drafted` and, only when the whole plan
   holds with them, the dependency's lines in one batch, strung just
   before the part that needs it and held by that part's holder. No line
   is the agent's, and no separate card asks for the draft: the approval
   covers it (D14). A plan that does not hold writes no line and opens the
   `environment_error` card; two misses end the approval and its card
   opens again.
3. **Clearing a hold is the person's.** A card is cleared by the person's
   answer only.
4. **"Proven" is never written bare.** Every proof names its source:
   locally red, locally green, locally verified, or CI-proven at a commit
   and run.
5. **A test that encodes a changed definition is updated with it, never
   loosened.** The commit message names the definition change.

Terms:

- **record**: `.familiar/record`, the append-only list of facts.
- **fact**: one entry in the record, of a registered **kind** with fixed
  fields. There are 47 kinds; the agent may claim 18 of them.
- **criterion**: one testable part of what the person asked for. The person
  sees it as a **part** of their ask.
- **seal**: a criterion's tests, committed alone and seen failing, with a
  fingerprint of the test files stamped.
- **hold**: a derived reason a criterion can't move on. Some holds are
  **cards**: questions only the person can answer.

```
 the person          the agent (Claude Code)          a neutral machine
 ──────────          ───────────────────────          ─────────────────
 types a request ──▶ claims the plan (facts)
                     commits tests, then code
                          │ every event
                          ▼
                     hook ─▶ familiar-ai ─▶ record (append only)
                                  │            │
                                  │      rules (pure) derive the state
                                  ▼            │
 answers a card ◀── the question card  ◀───────┘
 (hold_answered)    in the person's words
                     commits .familiar/plan ──▶ CI: familiar-ai verify
                                                 in a fresh clone
                                                 ─▶ the green check
```

## What the check proves

The deliverable is the `familiar-verify` check on a pull request. This
section defines what the check computes, lists the ways to produce a false
pass, and names the decision that prevents each.

```
 familiar-ai verify, in a fresh clone, on a hosted runner

 reads                            never reads
 ─────                            ──────────
 git history                      the laptop's record
 .familiar/plan (claims only)     any test result from the laptop
 the test files at each commit    the agent's word on an outcome
                                  the network

 per criterion
 ─────────────
 red at the seal ─▶ green at the head ─▶ revert check: the anchor put
                                          back, the tests must fail again
 ─▶ a JSON report, attached to the run id
```

Neutral is defined as three properties: the run starts from a fresh clone
with no record and no laptop results; it reads only committed files, and
the committed plan holds claims, not outcomes; it computes every result
itself, on a machine neither the agent nor the author operates. Neutral
does not cover the test suite. CI runs the command the plan names, written
by the author; the judge and the revert check address that. Nor does it
cover the criteria: the agent's reading of the ask is a claim no machine
grades. The person grades it, reading the parts beside their ask
([the parts are the person's check](#the-parts-are-the-persons-check)).

The binary is not in the clone. The action downloads it from the published
release and checks it against the release's checksum, so the author cannot
substitute a modified program. The trust anchor is GitHub Actions running
the job as written; nothing here helps a reader who doesn't grant that.

False passes and the decision that prevents each:

| way to fake a green | closed by |
|---|---|
| a test that never fails | [tests first, alone, red](#red-green-revert) |
| weaken the test after it went red | [the same fingerprint at green](#red-green-revert) |
| red for an unrelated reason, then fixed | [the revert check](#red-green-revert) |
| edit a sealed test after green | [the way back](#the-way-back): announced and re-earned, or unproven |
| new code that nothing reaches | [the integration point](#new-work-names-its-integration-point) |
| another criterion's failing test counted as this criterion's | [a scoped command, or "unscoped" reported](#the-command-is-claimed-derived-or-unscoped) |
| one criterion's proof stamped with another criterion's test hash | [one fingerprint per criterion per commit](#one-stamp-per-criterion-per-run) |
| the agent reports a result | [the plan holds claims only](#the-plan-is-committed); CI recomputes |
| the agent claims the binary's own kinds | [which kinds the agent may write](#which-kinds-the-agent-may-write) |
| the agent clears its own hold | [holds are the person's](#holds-are-the-persons) |
| the agent answers a card for the person | [fixed choices, checked pieces](#the-agent-cannot-answer-for-the-person) |
| an orchestrator's brief taken for the person's words | [the person's words travel with a lane](#the-persons-words-travel-with-a-lane) |
| a tampered laptop record | [CI never reads it](#ci-proves-the-laptop-guides) |
| a laptop result shown as CI's | ["proven" is never written bare](#proven-is-never-written-bare) |
| a weakened test passed off as a definition change | [named in the commit](#definition-changes-are-named); enforced by review, not code |
| a criterion too weak for the ask, or a part of the ask with no criterion | [the parts are the person's check](#the-parts-are-the-persons-check); the judge checks each part's tests, not the list |
| a test command that returns a false exit code | [a reading is the truth or can't tell](#a-reading-is-the-truth-or-cant-tell): the exit code alone decides only what it shows, and output that contradicts it reads can't-tell. Not prevented: a harness that forges its output too, committed before the seal; one changed after it is [re-checked](#the-seal-is-re-checked-under-a-changed-setup) (open). |
| a test that passes for a reason the judge misses | not prevented. The judge is one model's reading; see [the judge](#a-judge-one-rung-up). |

Output: `verify` prints one line per criterion and writes
`.familiar/verify-report.json`, which the action uploads.

```
{"run": {"id", "at", "runner"}, "sha", "plan_hash",
 "criteria": [{"id", "text", "outcome", "evidence"}], "exit"}
```

A proven criterion's evidence reads `hosted CI-proven at <sha7>, run
<id>`. Anything else reads as the criterion's own line, with no "proven"
in it. `familiar-ai ci fetch` appends the report to the record; the
laptop's status reads CI's result from there.

Status: a hosted runner has run an earlier form of the job ten times on a
private fixture (run ids in the README). A correct change passed; the fix
undone, a sealed test edited and the plan removed each failed or exited 2.

## The record

### Append-only facts

The record holds facts and nothing else. No fact is changed or removed;
every state is derived from it on read. We didn't store a `state` field per
criterion. It's cheaper to read, but it goes stale, it can be edited, and
two writers can disagree about it. Deriving costs a full read of the log per
question. The laptop and CI compute the same state from the same facts, and
no fact can be rewritten.

Code: `crates/familiar-ai-facts/src/record.rs`,
`crates/familiar-ai-rules/src/lib.rs`. Tests: `crates/familiar-ai-facts/tests/append.rs`,
`crates/familiar-ai-plan/tests/write_read.rs`. (affe3ed, 9a5609e)

### The record is a database file

The record is a `redb` file, and the agent is told once at session start
to read it with `familiar-ai status`, never to open it. The plan file is its readable mirror. The reason is `append_if`:
a run's reservation reads the whole log and writes its fact in one
transaction, so of two writers racing for the same work exactly one is
accepted. A JSON Lines file with a lock could be `cat`-ed, but that
transaction would have to be built by hand. The session-start line exists
because a local model in a benchmark `cat`-ed the record in six of ten
cases and its output collapsed into NUL bytes in five.

Code: `crates/familiar-ai/src/teach/discipline.rs`.
Test: `crates/familiar-ai/tests/session_record_via_status.rs`.
(80cdc59, f53c927)

### The binary stamps position, time, kind and tenant

A fact's position (`t`), time (`at`), kind and tenant are stamped by the
record. A claim that hands one in is refused. Position in the log is what
"before" and "after" mean everywhere in the rules, so only the record may
assign it.

Code: `crates/familiar-ai-facts/src/append.rs` (`STAMPED`).
Tests: `crates/familiar-ai-facts/tests/validation.rs`,
`crates/familiar-ai/tests/session_stamped_fields.rs`.
(affe3ed, 733c378)

### Which kinds the agent may write

Of the 36 fact kinds, the agent may claim 18: what it plans and announces.
The kinds that record what happened (test results, stamps, verdicts, run
facts, `tests_observed`, `ci_report_seen`, `familiar_switch`) are written by
the binary only and refused at the claim door. An agent that could write
them could write its own proof. Each kind has a fixed set of fields and
values; an unknown kind is refused rather than skipped (see
[compatibility](#older-binaries-refuse-newer-records)).

Protects: the agent claiming the binary's own kinds, and with them its own
proof.

Code: `crates/familiar-ai-facts/src/form_table.rs`,
`crates/familiar-ai/src/door/claim.rs` (`BINARY_KINDS`).
Tests: `crates/familiar-ai-facts/tests/forms.rs`,
`crates/familiar-ai/tests/claim_forms.rs`. (affe3ed, 552ed2f)

### The plan is committed

`.familiar/plan` is committed with the work, from one branch, so every clone
carries it and CI has something to recompute from. It holds claims only,
never a test's output. Keeping it local was simpler, but then CI had nothing
to verify.

Without the plan, a clone is commits and test files with nothing that says
what any of them were meant to prove. The plan is the list of claims
`verify` checks, and each line is an instruction to it: `test_planned`
names the criterion's test file, so `verify` can find the commit that added
it alone and run it there; `test_stack_detected` names the command to run;
`place_chosen` and `anchor_sha_matched` name the lines the fix lands in and
hash them at the planning commit, so the revert check knows what to put
back. The plan says where to look; git says what happened; `verify` says
whether the two agree.

Protects: a result the agent reports; CI gets claims and recomputes every
result.

Code: `crates/familiar-ai-plan/src/lib.rs`, `format.rs`.
Tests: `crates/familiar-ai-plan/tests/write_read.rs`, `from_log.rs`. (da53460)

### One record per project

A subagent in a git worktree writes to the main checkout's record, not to
one of its own. Two records would have to be merged, and a merge decides
which fact came first, which is what the record exists to settle. Open: a
worktree with its own `.familiar/` is accepted; it should be refused.
Changed 2026-10-12: a hook's project is the nearest `.familiar` within the
same git worktree as its working folder, so a worktree nested under the
project's folder (a subagent's `.claude/worktrees/agent-…`) is governed by
its own record, or not at all, never by the parent checkout's
(`door/project.rs` `hook_project`, `a_nested_worktree_is_its_own_project.rs`).

Code: `crates/familiar-ai-facts/src/record.rs` (`Record::open_waiting`).
Test: `crates/familiar-ai-facts/tests/record_concurrency.rs`. (f0033b2, d283707)

## The rules

### Pure rules in their own crate

Every rule lives in `familiar-ai-rules`, which has no I/O, no clock and no
git; time is an input, so any past state can be recomputed. The binary
gathers facts (`door`), asks the rules (`decide`), then words the answer
(`teach`).

Code: `crates/familiar-ai-rules/src/lib.rs`; its `Cargo.toml` depends on
`familiar-ai-facts` alone. Tests: `crates/familiar-ai-rules/tests/`. (9a5609e)

### Seven states in a fixed order

A criterion is `unplanned`, `waiting`, `testing`, `sealed`, `green`,
`proven` or `held`. Each is a predicate, checked in a fixed order; the first
true one wins, so exactly one is true by construction and every surface
names the same one.

Code: `crates/familiar-ai-rules/src/lifecycle.rs`, `holds.rs`.
Tests: `crates/familiar-ai-rules/tests/sample_fixture_derives.rs`,
`crates/familiar-ai/tests/walk_criterion_lifecycle.rs`. (d283707)

<a id="exit-codes-not-output"></a>

### The exit code says how a run ended, not what it was

A command that could not start is `did_not_run`, a third outcome beside red
and green, because a missing runner read as red would seal a test that never
ran. The run crate reports only how the command ended (an exit code, a
signal, a timeout, a start that failed) and never parses output: "passed" in
the output decides nothing there. The binary reads the output afterwards,
per runner, and the exit code alone decides only what it shows
([a reading is the truth or can't tell](#a-reading-is-the-truth-or-cant-tell)).
This entry once said runner output is never parsed, so as not to tie the
product to known runners. The per-runner tables of 2026-10-14 replaced that:
a red counts only when it points at the plan's place, and only the output
says where it points.

Protects: a command that couldn't run being read as red.

Code: `crates/familiar-ai-run/src/lib.rs` (its unit tests: "passed" in the
output decides nothing).
Test: `crates/familiar-ai/tests/run_missing_command.rs`.
(d77e06b, 99e6a07)

### A reading is the truth or can't tell

The owner, 2026-10-19. A run's reading is what happened or "can't tell", and
can't-tell is a card, never a guess. "Passed" and "an honest red" are read
only on positive evidence in the output; the exit code alone decides only
what it shows: a signal, 126 or 127 with the shell's line, a timeout. It
replaces the 2026-10-14 default "exit 0 with no match is `passed`". A run
can be read wrong four ways, by stage:

```
 stage 1: the tests-first commit    stage 2: the fix commits
 wants a red for the right reason   wants green
 (1) a seal not true to its         (2) a green not true to its
     assumptions: false security        assumptions: false security
 (3) an honest red refused:         (4) a real pass read as a
     false alarm                        failure: false alarm
```

What each reading needs, and where it stands:

| reading | needs | at this commit |
|---|---|---|
| `passed` | a row of the runner's table that says the tests passed; exit 0 alone is `unclassified` | landed, H2 (72ed01d) |
| a red label | an exit that agrees: a red at exit 0, or `passed` at a non-zero exit, is `unclassified` | landed |
| an honest red | a location whose kind says why: an assertion at a planned test line, or a failure inside the place's function; no location, an exception in a planned test (D1) and an assertion elsewhere (D2) can't tell | open, H6; `outcome.rs` still reads a red with no location as an assertion |
| green after the seal | counts that agree with the seal run's: none failed, no new skip, passed at least its passed plus failed (D6) | open, H6 |
| a shell script's red | the shell's own trace names a failing check at a line of the planned script | open, H5 |

And what goes with it:

| rule | what | at this commit |
|---|---|---|
| stored (D7) | every reading keeps the kind of its location (`assertion`, `exception`, `compile`, `signal`) and the counts the runner's summary gave (`passed`, `failed`, `skipped`, `deselected`; `{}` when none); a translation with no `counts` key is a legacy one and reads as before; four rules pick which location is kept ([reference](reference.md#outcomes)) | landed, H1 and H3 (1422689) |
| the person confirms (H-4) | the can't-tell card adds `its_the_red`, which seals at stage 1, and `its_the_pass`, the green at stage 2 [draft]; the card is plain words, never a raw log (what ran, the counts, the deciding line, the failing tests and where, why Familiar couldn't tell, where the full output is); signed like every answer, and CI accepts it only where its own run at that commit also reads can't-tell with the same label and deciding line | open, H8 |
| skips (H-3) | the person's own skips, deselects and filters are ignored; the tests that ran at the seal must run again at the green, and a newly skipped test asks | open, H6 |
| shell scripts keep their tests (H-2) | Familiar reads the shell's own trace in its own run (bash: an ERR trap through `BASH_ENV`; other `sh`: `-x` with Familiar's `PS4`), no script rewritten; a failing check (`test`, `[`, `[[`, `grep -q`, `diff`, `cmp`) at a line of the planned script is an assertion there, the place's own script failing goes to the place rule, `command not found` is a missing name, anything else can't tell; at stage 2 the sealed script's exit 0 is its green; replaces 2026-10-15's "exit 1 with empty stderr is `tests_failed`" | open, H5 |
| this repository's proofs are measured (H-5) | a fresh-clone `verify` before and after; parts that drop to can't-tell (an `unwrap` panic or a custom-message `assert!` at the test line) are listed and confirmed on their cards | open, H10 |

Protects: a seal or a green that is not true to its assumptions; an honest red
or a real pass refused with nothing the person can do about it.

Code: `crates/familiar-ai/src/door/kept.rs` (`exit_default`, `contradicts`,
`test_log_translation`), `door/runner_tables.rs`;
`crates/familiar-ai-rules/src/outcome.rs`;
`crates/familiar-ai-facts/src/forms/runs.rs` (`Counts`, `LocationKind`).
Tests: `crates/familiar-ai/tests/a_reading_is_the_truth_or_cant_tell.rs`
(every recorded run, and output never seen generated per runner, read as the
binary reads it; `KNOWN` pins the violations left: 56 rows at 8c2f45e, all
stage-1 readings, no false green),
`no_empty_exit_zero_row_reads_passed.rs`. (2401c81, 40e96eb, 501866b, 72ed01d,
3267150, 79615a6, 5575b63, 1422689)

### Regex or Familiar's own reporter; never a model

The owner, 2026-10-19 (A-2). It replaces 2026-10-14's "regex is the only
classifier". Only Familiar's own code reads a run: the regex tables recorded
from real output, or a reporter Familiar injects into its own run. No model
labels a run. The readers, best first:

| reader | what it is |
|---|---|
| Familiar's reporter | a plugin or reporter flag added to Familiar's own run only, for any runner; the planned command and the person's terminal stay as they are |
| the runner's report files | JUnit, TRX, test2json or libtest-json, converted to one per-test file |
| a pinned console format | the runner's own console, pinned to the format a table was recorded from |
| the regex tables | `crates/familiar-ai/src/door/runner_tables.rs`: the only reader at this commit |

The list is Familiar's data, never the repository's, and never trusted blindly
(the owner: a "fixed list" sounds brittle). Each runner has several readers
with fallbacks, and a dry run proves each one in this project before the
adapter names it, so a stale flag or format fails there and falls to the next
reader: a card at worst, never a false reading. A reader falls back to the
tables for a run it did not see. The list changes with releases, driven by a
weekly watcher [draft].

Protects: an outcome decided by a model's guess or by a reporter the project
supplies.

Open: no reporter, report-file reader or dry run exists at this commit.
Code: `crates/familiar-ai/src/door/runner_tables.rs`, `kept.rs`.

### Why the adapter needs no signature

`.familiar/adapter` [draft] is a choice file only the binary writes, committed
and used by every run on the laptop and in CI. It names, per runner, a reader
from Familiar's built-in list, by id; never a pattern. An approval needs a
signature because it makes a hold disappear and CI must know the person gave
it. A reader can't: editing the file can only pick another reader that reads
the truth or can't tell, or none (every run of that runner can't tell, which
is a card), and an id Familiar does not know is none. CI does not take the
file on trust either: it runs its own dry run at HEAD, uses its own choice,
and prints one line when the committed file differs, without changing the exit
code. The laptop reads its choice from the record, never the file, and an
agent's write to the file is refused beside the plan's.

Protects: a choice file the agent edits becoming a way to change what a run
reads as.

Open: no adapter file or dry run exists at this commit.

### Git reads in pure Rust, two writes through git

Every read goes through `gix`. The `git` program runs for exactly two
writes: the commit `setup` makes and the rewrite `tidy` makes. Shelling out
for everything means a process per read and output to parse; `gix` for
everything means no commit hooks, no signing, and a checkout that drops
untracked files. A commit familiar-ai makes should be the same as one the
person makes, hooks and signature included. The reads the rules depend on
stay free of C.

Code: `crates/familiar-ai-git/src/lib.rs`,
`crates/familiar-ai/src/door/git_program.rs`, `scripts/check-no-c.sh`.
Tests: `crates/familiar-ai-git/tests/` (each read compared with git's own
answer). (98eaa5f, a82d670)

## The plan

The agent claims eleven lines before any edit, in one `familiar-ai plan`
call (from `crates/familiar-ai/src/teach/discipline.rs`):

```
  1 the intent       intent_submitted (--for names the person's prompt)
  2 a criterion      criterion --path light | full
  3 full path only   behavior, promise
  4 its anchor       place_chosen | new_work_spot_chosen |
                     new_work_no_spot
  5 the check        anchor_sha_matched (the binary writes it)
  6 its tests        test_planned
  7 the command      test_stack_detected
  8 touching tests   touching_test_declared
  9 the string       criterion_head | criterion_strung_on_head
 10 who holds it     criterion_assigned
 11 where it lives   fix_location_category
```

The eleven lines are one claim, made once per request, at the first
refusal. Each line removes one way to produce a false pass. The intent and
the criterion give the judge the person's words to check against. The
anchor and its check fix what the revert check puts back. The tests and the
command fix what runs, so another criterion's failing test is not counted
as this criterion's.
The string and the holder fix who proves what, in parallel. The location
fixes whether a test in this repository can prove it. How well agents carry
the eleven is measured by the benchmark in the README.

### Nothing is governed until code changes

familiar-ai is idle until the agent first edits or commits code. Questions,
reads and plans of thought are not governed. A second request after the first is
proved again as a new head. We didn't build an on switch (forgotten) or a
prompt classifier (wrong some of the time, and uncheckable). The trigger
is one refusal at the first edit, where the agent stops and claims the
intent. A file outside the project's folder (after canonicalising) is never
this project's code: an Edit, Write or shell write there is passive however
far the plan has got (`door/person.rs` `outside_the_project`,
`edit_outside_the_project_is_passive.rs`).

Code: `crates/familiar-ai/src/teach/person.rs`, `door/person.rs`,
`decide/restrict.rs`.
Tests: `crates/familiar-ai/tests/idle_conversation_ungoverned.rs`,
`intent_first_edit.rs`, `intent_second_request.rs`,
`edit_prose_passive.rs`. (9e89207, cc02913)

### The intent names the prompt

`intent_submitted --for <t>` names the prompt the intent answers, and the
judge reads that prompt, not every prompt of the session. Otherwise one
session's old requests count against today's test. The person's own words
decide what the test must cover.

Code: `crates/familiar-ai/src/teach/person.rs`, `door/person.rs`.
Test: `crates/familiar-ai/tests/judge_named_prompt.rs`.
(cc02913, ec41607)

### The parts are the person's check

The agent's reading of the ask, the list of criteria, is itself a claim,
and it is the one claim no machine grades. The judge is asked about one
part's tests and told a sibling's path is not a gap, so it cannot see a
part that was never claimed; `verify` checks the history against the plan
and cannot see that the plan was too small. Only a person can say whether
a criterion is strong enough to stand for what they asked. So the parts are
shown as a tool, not a display: numbered, beside the ask's own words, on
the status line and in the pane with each text whole, before any test is
written. The person judges the parts; the judge reads each part's tests
against the person's words; the machine checks that the history backs the
claims.

Protects: a weak criterion proven to the letter; a part of the ask that no
criterion claims.

Code: `crates/familiar-ai/src/teach/mod.rs` (`name`, `part`, `ask` per
criterion), `plugin/hooks/status.ts`, `pane.tsx`.
Tests: `crates/familiar-ai/tests/status_json_same_picture.rs`,
`plugin/tests/familiar-line.test.ts`.

### Light is for documentation only

`--path light` (no behavior or promise) is allowed only when the anchor is
documentation: a markdown file or a docstring. The binary checks it and
writes `anchor_is_documentation`. Everything else is `--path full`. Light
used to be a self-reported label with a cheaper option, and it was gamed:
in a controlled test Haiku, Sonnet and Opus each picked light for 3 of 6
ambiguous cases labelled as new features. Dropping light altogether would
make documentation-only fixes pay for a promise they can't have. The check
reads the anchored lines at the record's tip sha after the anchor claim: a
`.md` file, or lines that are one docstring (PEP 257's first-statement
string, Rust `///` / `//!` lines, one JSDoc `/** … */` block); new work is
documentation only when its file is markdown. Plan line 2 holds for a light
criterion only when every spot's check says `matched: true`. A false check
is refused in the words `its path is light but its anchor is not
documentation: claim it --path full`, with the re-claim to paste.
The example the binary and the plan skill hand out is a code anchor's
(`--path full --behavior_id b1 --promise_id p1`, the behavior and promise
claimed before it); light is shown nowhere as the default (the owner,
2026-10-13, finding 35: nearly every first `plan` call of the 44 benchmark
copied a light example and was refused twice).

Code: `crates/familiar-ai/src/door/documentation.rs`,
`crates/familiar-ai/src/door/anchor.rs` (`binary_checks`),
`crates/familiar-ai-rules/src/plan.rs` (`line_2`, `documented`,
`not_documentation`), `crates/familiar-ai-rules/src/terminal.rs`
(`LIGHT_NOT_DOCUMENTATION`), `crates/familiar-ai/src/teach/full_path.rs`
(`light_not_documentation`), `crates/familiar-ai/src/teach/discipline.rs`,
`crates/familiar-ai/src/teach/form.rs`.
Tests: `crates/familiar-ai/tests/light_is_for_documentation_only.rs`,
`crates/familiar-ai-rules/tests/light_is_for_documentation_only.rs`,
`crates/familiar-ai-facts/tests/anchor_is_documentation.rs`.

### The anchor is a whole function; a shift is not a move

The anchor is the whole function or class the fix will touch, hashed at the
planning commit. At the criterion's start the planned range is mapped to the
working tree, then hashed. Same bytes at a new place: no card, and the agent
is told where the lines sit now. Changed bytes or lines gone: the
`anchor_moved` card. Hashing at the planned line numbers, as before, raised
a card for any edit above the function. Cards for untouched code reduce the
attention the person gives to cards. An anchor of several places gets a
start check for each place, each compared with that place's own planning
check, and the anchor moved when any place's lines changed; comparing the
latest start check with the latest planning check, whatever place each was
for, opened a card at once on a two-place anchor.

Code: `crates/familiar-ai/src/door/anchor.rs` (`moved_to`),
`crates/familiar-ai-git/src/range.rs` (`map_range`),
`crates/familiar-ai-rules/src/lifecycle.rs` (`place_checks`, `anchor_moved`).
Tests: `crates/familiar-ai/tests/anchor_shift_two_criteria.rs`,
`anchor_line_shift.rs`, `anchor_two_places.rs`,
`crates/familiar-ai-rules/tests/a_second_place_gets_its_own_start_check.rs`.
(5c67ae1, 4ce893b)

### A plan's own earlier changes are not outside edits

When an anchor changed between planning and start only through recorded
commits of another criterion of the same plan, the range is mapped and work
carries on. Any change not in the record (an uncommitted edit, a teammate's
pull) still raises `anchor_moved`. The planning hash is never rewritten to
match; that would record a hash that was never observed. Without this, a
plan whose second
part builds on its first stops at every step. The start check carries
`tip_matched` (the working tree's mapped lines equal the tip commit's); the
rule reads it with every commit after the planning check whose
`anchor_overlap` names the criterion, each stamped on the laptop before the
start for another criterion of the plan that the commit also names. The
agent is told `its lines were changed by the work on "<text>"; they now sit
at <file> a–b (commit <sha7>)`. Such a sibling's commit before the start is
not the criterion's code (`sibling_before_start`).

Code: `crates/familiar-ai-rules/src/lifecycle.rs` (`own_work_moved`),
`crates/familiar-ai-rules/src/holds.rs` (`holds_of`, `anchor_moved`),
`crates/familiar-ai-rules/src/criterion.rs` (`sibling_before_start`,
`overlaps`), `crates/familiar-ai/src/door/anchor.rs` (`look`),
`crates/familiar-ai/src/teach/words.rs` (`lines_now_sit`).
Tests: `crates/familiar-ai/tests/own_work_on_the_anchor_is_not_an_outside_edit.rs`,
`anchor_shift_two_criteria.rs`,
`crates/familiar-ai-rules/tests/own_work_is_not_an_outside_edit.rs`,
`a_siblings_commit_before_the_start_is_not_its_code.rs`,
`crates/familiar-ai-facts/tests/anchor_sha_matched_tip_matched.rs`.

### New work names its integration point

`new_work_no_spot` must name an `integration_point`: where the new code is
reached from, or, for net-new work only, where it's stored. The revert check
puts that whole file back. Without it a greenfield criterion had nothing to
put back, and a test that depended on nothing could prove it.

Protects: new code nothing reaches; the revert check has something to put
back.

Code: `crates/familiar-ai-rules/src/plan.rs`,
`crates/familiar-ai-git/src/scratch.rs`,
`crates/familiar-ai/src/door/verify_run.rs`.
Tests: `crates/familiar-ai/tests/claim_greenfield_integration_point.rs`,
`teeth_restore_integration_point.rs`. (c996cbf, 1c22e5f)

### Planned tests and touching tests are separate

Line 6 lists each test file the criterion will create or modify; line 8
lists existing tests the change may break. Line 8's tests run at the head
and are not sealed. In one list, an edit to a test the fix may legitimately
break reads as tampering. Open: `test_planned.will` and
`touching_test_declared.test_name` are recorded but unread.

Protects: an edit to a watched test being read as tampering with a sealed one.

Code: `crates/familiar-ai/src/teach/form.rs`.
Test: `crates/familiar-ai/tests/plain_commands.rs`. (c7011c1)

### The touching tests run on their own [draft]

A part's own run is its own test files only; its touching tests (line 8's
and those the binary derives from the fix's imports) run under a second
command, the stack's runner applied to them, with a run id of its own (the
run's command is part of its work and its id), and each touching result names
that run. The owner's plan of 2026-10-21, step 0: chained into the own run,
a touching file's skips and extra tests changed the counts H6 compares with
the seal run's, and real fixes read as can't-tell (8 on corpus2). With no
stack, or a runner the table cannot express them in, the touching results are
the own run's, as before.

A touching result is read on its own, never against a seal run, since it was
never red: a pass is green when nothing failed beside it, the table's
`passed` with no failure counted or a run that exited 0 whose counts show
tests and no failure, whatever the table called it (pytest's reads a pass
beside a skip as unclassified). A skip in a touching file is the project's
and never makes the part can't-tell. A run of them that failed a test keeps
the part from green and is the regression when the file passed at the
tests-first commit. With no counts, or a non-zero exit, it is no pass. CI's
action still runs only the declared touching tests and gives them the own
run's word.

Protects: a real fix read as can't-tell because the touching tests skipped
something; a touching result taken from the part's own run.

Code: `crates/familiar-ai-rules/src/criterion.rs` (`derived_command`,
`touching_command`), `outcome.rs` (`touching_family`);
`crates/familiar-ai/src/door/run.rs`, `runner.rs` (`run_id`), `runs.rs`
(`eligibility`).
Tests: `crates/familiar-ai-rules/tests/a_touching_test_runs_on_its_own.rs`;
`crates/familiar-ai/tests/the_touching_tests_have_a_run_of_their_own.rs`,
`the_binary_runs_the_existing_tests_that_import_the_fix.rs`.

### The command is claimed, derived or unscoped

Criteria proved in parallel each have a test that is red on purpose, so
running the whole suite for every criterion would count one criterion's red
as another's. Each criterion therefore runs its own command: the claimed
`test_command`, or one the binary derives from the criterion's test files
and the stack's runner (pytest, cargo, jest, vitest, go). A runner outside
that table derives nothing; the criterion is marked **unscoped**, runs the
whole stack, and its seal does not stand while another criterion of the
same stack has an outstanding red.

Plan it again keeps the command of the place it is leaving until the new
place is chosen. Answering anchor_moved with re_plan unchooses the place for
planning, the checks and the holds (plan::anchor_of stays as it is); it does
not take away the part's test command. (the owner, 2026-10-12: `stack_of`
and `command_at` read the last anchor exit, `plan::last_exit`.)

Among the stacks with a command whose root covers every anchor file, the
longest root wins; equal roots, the latest, so `.` never displaces `crates/x/`
(`plan::stack_covering`, shared by `stack_of` and `command_at`; test
`the_most_specific_stack_root_wins_over_the_latest.rs`).

Protects: another criterion's failing test counted as this criterion's.

Code: `crates/familiar-ai-rules/src/criterion.rs` (`command`,
`runner_form`, `stack_of`), `lifecycle.rs` (`command_at`), `action.rs`.
Tests: `crates/familiar-ai/tests/status_unscoped_criterion.rs`,
`run_two_heads_separately.rs`,
`crates/familiar-ai-rules/tests/plan_it_again_keeps_the_command.rs`.
(6e1ce2e, ccbc771)

### Strings only where a test needs another's code

Criterion B is strung on A's string only when B's test can't be set up
without A's code. Every link names the string's head, and the links take
their turns in the order they were claimed (the owner, 2026-10-15): a third
part C names the head A, not B, and a head that is not a head is refused at
the claim. Every other criterion is its own head, so they can be proved in
parallel. A global order is simpler and runs nothing in parallel.

Code: `crates/familiar-ai-rules/src/plan.rs`.
Test: `crates/familiar-ai/tests/run_two_heads_separately.rs`.
(487fead)

### One holder per criterion, no model logic in the binary

`criterion_assigned` names the conversation that holds a criterion, so a
commit is charged only to the criterion it touches and a hold pauses only
its own. The binary works with whatever model the holder is, and reads the
model only to pick the judge's and investigator's rung above it. Choosing
models for lanes would make the product depend on a model list it doesn't
own. Open: binding a lane to its conversation, a cap on lanes.

Code: `crates/familiar-ai-rules/src/plan.rs`, `ladder.rs`.
Test: `crates/familiar-ai-rules/tests/each_criterion_is_proved_on_its_own.rs`.
(487fead)

### A fix outside the repository is a card

Every criterion says where the failing code lives: `this_repository`,
`dependency`, `environment` or `request`. Any value but `this_repository`
raises the `fix_elsewhere` card before a test or code is written. A test can
only prove code this checkout controls; a workaround in this repository,
with a test, would prove the wrong thing.

Protects: a proof of a workaround for code this checkout doesn't control.

Code: `crates/familiar-ai-rules/src/holds.rs`, `lifecycle.rs`.
Test: `crates/familiar-ai/tests/card_fix_elsewhere.rs`.
(72f2f44)

### Places before the fix [draft]

Once the plan is confirmed, the binary lists the sites that name what a part's place names, and
the agent classifies each before its tests-first commit (`places_listed`, `place_classified`,
`places_unclassified`; the owner's plan of 2026-10-21, step 3).

- At most 6 of the 12 sites come from the place's own file (`MOST_OWN`), so "other places" keeps reaching other files; the own file fills what other files leave free. Approved by the owner, 2026-10-21.

Code: `crates/familiar-ai/src/door/siblings.rs`, `door/places.rs`,
`crates/familiar-ai-rules/src/places.rs`.

## The proof chain

```
 tests-first commit ─▶ run: must FAIL ─▶ sealed (fingerprint stamped)
        │                                     │
        │                     judge, one rung up: do the tests
        │                     answer the person's words?
        ▼                                     ▼
 code commit on the anchor ─▶ run: must PASS under the same stamp: green
        │
        ▼  CI, fresh clone, committed plan
 verify: red at the seal · green at the head · revert check (the
         anchor put back, the test must fail again)
         ─▶ CI-proven at <sha>, run <id>
```

### Red, green, revert

A criterion's tests are committed alone and must fail when run; their
fingerprint is then stamped (the seal). A test that passes at once is a try
(`tests_not_red`, then `two_tries_not_red`). The criterion is green only
when the same test files, under the same fingerprint, pass at a later
commit. `verify` then puts the anchor's lines back to their pre-fix text in
a scratch copy, and the tests must fail again; a red for an unrelated
reason, such as a missing import, passes red-then-green without touching
the change, and the revert check catches it. Known gap: the put-back uses
the planned line numbers, so in a file whose code has moved since planning
it can restore the wrong lines; applying the start check's mapping is open.

Protects: a test that never fails; a test weakened between red and green; a
red for an unrelated reason.

Code: `crates/familiar-ai-rules/src/holds.rs`, `criterion.rs`,
`lifecycle.rs`; `crates/familiar-ai/src/door/verify_run.rs`;
`crates/familiar-ai-git/src/scratch.rs`.
Tests: `crates/familiar-ai-rules/tests/a_tests_first_run_that_passes_is_a_try_not_red.rs`,
`crates/familiar-ai/tests/walk_criterion_lifecycle.rs`,
`verify_rechecks_proof.rs`. (6fd62a9, d283707, 2ac15d3)

### The way back

After green, a sealed test may be edited only after
`test_change_announced`. The commit that edits it is the new stamp; green
and the revert check are re-established under it, and the first red seal
stands. An announcement never covers an earlier edit. `tidy` folds the
announced edit into the fix commit. Without this, a proven criterion whose
test is ever touched again stays unproven for the life of the branch.
Before green, an announced re-commit of the test keeps the fix already on
the anchor; code on the anchor counts from the first seal of the epoch.

The owner, 2026-10-13: "k6 is a second red seal, that requires green".
After the way back's green, a tests-only commit of the part's tests that
the laptop stamped again under the proof claimed now (k6, after the tests
went back to the red seal's and came back) is the part's seal: CI proves it
from there, red at k6 and green with teeth after it, and the way back is
spent. CI reads only the plan's stamp at k6, never the laptop's result
there, and the lifecycle reads the same history the same way. A rebase
before CI's run renames k6; CI finds the re-stamp by its content (the
stamp's proof as the tests hash of its own tests-only commit after the way
back's), which may bind to an earlier commit of the same content.

Protects: an unannounced edit of a sealed test after green; CI passing a
part from its first seal when the laptop sealed it again at k6.

Code: `crates/familiar-ai-rules/src/criterion.rs`, `lifecycle.rs`,
`action.rs`; `crates/familiar-ai/src/door/tidy.rs`.
Tests: `crates/familiar-ai-rules/tests/an_announced_edit_after_green_reearns_green_under_the_new_stamp.rs`,
`an_unannounced_edit_of_a_sealed_test_after_green_never_reproves.rs`,
`an_announced_edit_before_green_keeps_the_fix_on_the_anchor.rs`,
`a_second_seal_after_the_way_back_requires_green.rs`,
`a_rebased_second_seal_is_found_by_its_content.rs`,
`crates/familiar-ai/tests/verify_announced_edit.rs`,
`verify_second_seal.rs`, `tidy_folds_announced_edit.rs`. (94cbe5f, 2232f40)

### The seal is re-checked under a changed setup

The owner, 2026-10-19 (A-5). A commit after a seal that changes the runner's
setup files (a conftest, a test config, a lockfile) re-checks that seal under
the new setup: the tests-first commit's code, run with the current setup files
put back. Still an honest red, the seal stands. Anything else is the
`setup_changed` card [draft], which pauses work (`re_plan` · `release`), and a
part's green waits while a re-check is owed. CI takes the same step after the
teeth. A setup file may change; the harness cannot be changed to turn a red
green. Known limit: a harness that forges outcomes and was committed before
the tests-first commit is not caught (the owner's call, later).

Protects: a seal outliving the setup it was earned under; a harness changed
after the seal to turn a red green.

Open: the re-check and its card have no code at this commit; only the form
field `run_reserved.setup_from`, which names the commit whose setup files are
put back, is landed (7b04519).

### One stamp per criterion per run

Every landed commit writes one `tests_observed` per criterion, over that
criterion's own test files, from one helper that the laptop, CI and `tidy`
share; and a stamp is written only where a run happened, so every runnable
criterion is run at every tip, not only the newest. One hash per commit
over the first runnable criterion's files stamped one criterion's proof
with another's; stamping only at the newest tip left a tests-first commit
that landed in a batch unsealed.

Protects: one criterion's proof stamped with another criterion's test hash;
a seal with no run behind it; the laptop and CI disagreeing on a
fingerprint.

Code: `crates/familiar-ai/src/door/commits.rs`, `run.rs`;
`crates/familiar-ai-git/src/facts.rs`; `crates/familiar-ai-rules/src/reader.rs`.
Tests: `crates/familiar-ai-rules/tests/a_commit_that_touches_two_criteria_gives_each_its_own_fingerprint.rs`,
`crates/familiar-ai/tests/ci_laptop_same_fingerprint.rs`,
`run_every_tip.rs`, `run_owed_commit_stamp.rs`. (f7f2707, fdcdfda,
cb09b76, 14fa175)

### Runs

A test run is a detached child that writes its own result as facts
(`run_reserved`, `run_started`, `run_ended`, `run_cancel_requested`), with
one reservation per work item decided inside the write transaction; a run
inside the hook would lock the record for the length of the suite. A run at
a past commit is a real checkout in a scratch copy with its own build
folder. Linking the project's `target/` into the copy baked the copy's path
into the project's own test binaries, which failed after the copy was gone
until a `cargo clean`.

Code: `crates/familiar-ai/src/door/runs.rs`, `verify_run.rs`
(`link_setup`); `crates/familiar-ai-facts/src/record.rs` (`append_if`).
Tests: `crates/familiar-ai/tests/run_detached_child.rs`,
`run_copy_own_target.rs`, `run_copy_sees_git.rs`;
`crates/familiar-ai-facts/tests/append_if.rs`. (abe4624, 572e0b2,
489f973, b36e58a)

### CI proves, the laptop guides

The laptop's checks read the agent's shell commands as text and can be got
past; they guide. CI starts from nothing and never reads the record. What
CI said comes back as a fact: `verify` writes a JSON report that the action
uploads, `familiar-ai ci fetch` reads it with `gh` and appends
`ci_report_seen`, and `status` reads that from the record, never from
GitHub, so it answers the same offline. A fetched fact can be stale; the
line says so.

`verify` proves each part at its own commits. The owner: "later claims
operate under the assumption that their previous claims (especially if there
is a dependency) are 100 percent behaviorally operating. this would mean that
the teeth directly prove any one given claim. thus, yes, a directly
proceeding commit with the fix removed. the mentality is this as closely
controls for externalities that would lead to a false read of a mutation".
So each part is walked from its own intent's sha to its own last commit (the
last that touches its place or its tests, carried over commits that touch
neither, such as the plan committed after the fix), and is red, green and
has teeth there, under the stack line committed at that commit. A later
part's stack line never changes an earlier part's command, and the merged
head is not where parts are proven. Which files are tests is still read from
the plan at HEAD, since lanes commit their tests before their plan lands. A
sealed test a later part changes, unannounced, still reads as changed for
its part.

Protects: the laptop's record and the agent's shell, which CI never reads; a
status that reports CI without having fetched it; one lane's command or
intent deciding another lane's proof.

Code: `crates/familiar-ai/src/door/verify.rs`, `verify_run.rs`, `ci.rs`;
`crates/familiar-ai/src/teach/ci.rs`; `crates/familiar-ai-rules/src/terminal.rs`;
`action/action.yml`.
Tests: `crates/familiar-ai/tests/verify_rechecks_proof.rs`, `verify_merged_history.rs`,
`verify_ci_report.rs`, `ci_fetch_appends_reports.rs`,
`status_ci_from_record.rs`; `plugin/tests/test-ci.sh`. (2ac15d3, f6b7f6f,
247bc47, c86c5a9)

### "Proven" is never written bare

Every claim of proof says which machine computed it: locally red or sealed,
locally green, locally verified, or CI-proven at a commit and run. Open: the
green line still reads "proven on this laptop", and `not_yet_words` holds a
bare "proven" in an unreached arm.

Protects: a laptop result shown as CI's.

Code: `crates/familiar-ai/src/teach/words.rs`, `door/verify_report.rs`,
`teach/ci.rs`.

### Definition changes are named

When a definition changes, every test that encodes the old one is updated in
the tests-first commit, and the commit message names each as a definition
change. A test is never loosened to make it pass. A reader of the history
has to be able to tell a definition change from a weakened test. This is
enforced by review, not by code; 6e1ce2e, 34dee21 and 80cdc59 are examples.

Protects: a weakened test passed off as a definition change.

### Tidy

`familiar-ai tidy` rewrites only the unpushed commits the record saw the
agent make, so each criterion ends as one seal commit and one fix commit.
Failed attempts are dropped; an announced test edit folds into the fix.
Raw history is complete but noisy; squashing loses the seal. Tidy runs only
before the first push, re-checks every state before the branch moves, and
keeps the old head under `refs/familiar-ai/pre-tidy/`.

Protects: the seal disappearing in a history rewrite.

Code: `crates/familiar-ai/src/door/tidy.rs`, `tidy_plan.rs`.
Tests: `crates/familiar-ai/tests/tidy_rewrites_unpushed.rs`,
`tidy_folds_announced_edit.rs`. (a82d670, 0b9e12e)

## The judge and the investigator

### A judge one rung up

At the seal, the binary asks one model, one rung above the holder's and
never below Opus, whether the sealed tests answer the person's words. Tests
alone prove the code does what the test says, not what was asked, and the
holder's own model would grade its own work. A rung up costs money and up to
60 seconds per seal; `FAMILIAR_AI_JUDGE=off` turns it off.

The verdict enters the record as a fact, so the derived state is
reproducible even though the verdict itself is not. `verify` never asks the
judge, so CI's proof doesn't depend on it; the judge applies to the laptop
loop only. At the top of the ladder the judge is the holder's own model,
and its verdict is a second reading rather than an independent one. Known
gap: the judge's effect on state is decided in `decide/`, outside the rules
crate.

Code: `crates/familiar-ai/src/door/judge.rs`, `teach/judge.rs`,
`crates/familiar-ai-rules/src/ladder.rs`.
Tests: `crates/familiar-ai/tests/judge_sealed_test.rs`,
`judge_seal_gate.rs`. (127defe, cbc5669)

### The judge's cap

The judge runs `claude -p` on the person's own Claude Code login, no API
key, capped at $0.50 a call. A 10-cent cap made every judgement
unanswerable; 25 cents still failed for Fable at about 33 cents a call.
Running out says "not judged", never a verdict.

Code: `crates/familiar-ai/src/door/judge.rs` (`--max-budget-usd`), with its
argument tests. (34dee21)

### The judge sees one part

The judge is asked whether the tests cover every path of the person's words
that this criterion claims; a path another criterion claims is not a gap.
Asking every criterion to cover the whole request failed each one for the
paths the other criteria claimed: six "no" verdicts in one plan.

Code: `crates/familiar-ai/src/teach/judge.rs`.
Test: `crates/familiar-ai/tests/judge_criterion_scope.rs`.
(e1c3b1d, 15f3894)

### Unanswered judges

Every miss writes `judge_unanswered`. After two for the same seal, the seal
stands as "not judged". A judge that can't see the tests never says "no".
Before this, one seal was asked 22 times.

Code: `crates/familiar-ai/src/door/judge.rs`, `decide/seal.rs`.
Test: `crates/familiar-ai/tests/judge_seal_gate.rs`. (a27e733)

### Two failures send an investigator

After two failed tries in a row (`two_failures`; `two_tries_not_red` gets a
read-only diagnosis instead and goes to the person), the
binary launches an investigator one rung above the holder's model: sonnet
for a haiku holder, opus for sonnet, fable for opus, and a second fable for
fable. An unknown model counts as fable. At most two launches per criterion,
$3.00 a run. After that
the person gets the card. Refusing the stop and telling the agent to spawn a
stronger model was tried first: across live rounds the refusal named the way out
55 to 74 times and Haiku never did it.
`FAMILIAR_AI_INVESTIGATOR=off` turns it off; `FAMILIAR_AI_INVESTIGATOR_BUDGET`
sets the budget.

Code: `crates/familiar-ai/src/door/investigator.rs`, `investigator_run.rs`,
`crates/familiar-ai-rules/src/ladder.rs`.
Tests: `crates/familiar-ai/tests/investigator_binary_launch.rs`,
`investigator_spawn_round_trip.rs`,
`crates/familiar-ai-rules/tests/two_failures_sends_an_investigator_one_rung_up.rs`.
(7c891f3, 5eb029c)

### The lens panel is released by the orchestrator

Once a full-path part's contract is whole, the lens panel runs before its test
(the owner's plan of 2026-10-26, lens-panel.md, approved with its names; it
replaces the case-finder of 2026-10-25, whose release and card it reuses). Five
lenses read the plan in parallel, `ambiguity`, `completeness`, `happy_path`,
`edge_cases` and `order_and_coupling`, then a `synthesis` merges their findings
into extra cases and at most three questions for the person. The main agent
releases each as a subagent; the binary does not start them.

While a part's panel is owed, the PreToolUse of an Agent call from the part's
holder (or anyone, while it has none) records which lens that call is
(`lens_released`, the binary's kind, with the call's `tool_use_id`): the next
lens not yet released, or, once the five answered, the synthesis. So parallel
calls in one message each get a different lens. The call is rewritten:
`subagent_type` `familiar-ai:lens`, the model from `FAMILIAR_AI_LENS_MODEL`
(opus when blank or missing, the owner's answer of 2026-10-26: opus for every
lens and the synthesis, as tested; `FAMILIAR_AI_CASE_FINDER_MODEL` is gone),
`run_in_background` false (honored only with fork mode off: from an interactive
session Claude Code starts every subagent in the background, fork mode being on by
default), and that lens's brief in place of the agent's prompt
(`teach/lens.rs`: the offline trial's words, every variable read from recorded
facts). A spawn while every owed lens is out is refused, naming what runs. Its
SubagentStart carries no context beside the brief. A lens only reads: any call
of its `agent_type` but Read, Grep and Glob is refused. The synthesis has no
tools: while only syntheses run, every call of the lens agent is refused; while
a lens of any part runs too, the two cannot be told apart by `agent_type`, and
the reads go ahead. Stops are not held. The turn limit is 25, fixed in
`plugin/agents/lens.md`.

Each answer is read at the main agent's PostToolUse on its Agent call (or its
PostToolUseFailure: failed, the error as why), found by the call's
`tool_use_id`. A lens's is recorded as `lens_found`: found with its findings,
none, or failed (a malformed answer, its why the note of its one finding of
kind `failed`). The synthesis's is recorded as `cases_found`, as the finder's
was: `found` with ids `a<n>` continuing past the part's earlier findings (the
first 8 kept, a long short line kept whole), `none` with why, or `failed` with
why; with its first three well-formed questions as `questions` (q1..q3, two to
four options each). A call never released, or an answer once its lens is
answered, records nothing. Not at SubagentStop: it did not fire when the turn
limit ended a subagent (spike of 2026-10-25).

A call run in the background returns at its launch (`async_launched`, with the
agent's id and no answer). The launch writes `lens_released` again for the call
with that `agent_id` (the owner's answer of 2026-10-27), once. Its answer comes
back in one of two ways. In auto mode the lens hands it back through
`SubagentHandback`: read at the lens's own PostToolUse on that call, found by its
agent (the plugin sends that PostToolUse to the binary, not its PreToolUse), and
nothing is told to the lens. Otherwise its report, the task notification Claude
Code sends the main conversation when the call ends, carries it: read at that
UserPromptSubmit, found by the call's `tool-use-id`, its `<result>` the answer
and its `subagent_tokens` the tokens. Either is checked as a returned answer is,
on the model the panel runs on. A lens that ended with no answer is failed (the
owner's words, 2026-10-27): a report with no result ("ended with no answer"), a
failed or stopped call (Claude Code's summary after it), the turn limit, or an
empty hand-back. The main agent is told what comes next at the report, even when
the hand-back recorded the answer first.

A stop of the main agent goes while every owed call of the held part's panel is
out, so there is nothing to start and the agent only waits (the owner's answer of
2026-10-27, "only waiting is left"); with a lens or the synthesis still to start
it is refused on `cases_owed` as before, and an edit still waits for the panel.
Its rail and mutant in the checks crate are owed (the owner's answer, 2026-10-27):
no walk there releases a call, so the situation is never reached.

The person's card then shows the planned and found cases and the panel's
open questions; the agent asks it with one AskUserQuestion call, the hook
setting the card's own question and one question per open panel question. The
picker records the card's choice as `hold_answered` and each question's pick
as `question_answered` (an option's text, or `typed` with the words); a
partial answer asks again only what is open. `question_answered` is the
person's, refused from an agent's shell as a card's answer is.

Code: `crates/familiar-ai/src/decide/lens.rs`,
`crates/familiar-ai/src/door/lens.rs`, `crates/familiar-ai/src/teach/lens.rs`,
`crates/familiar-ai-rules/src/cases.rs` (`only_waiting`),
`crates/familiar-ai-rules/src/hooks.rs` (the stop), `plugin/hooks/hooks.json`,
`plugin/agents/lens.md`.
Tests: `crates/familiar-ai/tests/lens_panel_round_trip.rs`,
`crates/familiar-ai-rules/tests/the_lens_panel.rs`,
`crates/familiar-ai-facts/tests/the_lens_panel_records_its_answers.rs`,
`crates/familiar-ai/tests/the_lens_briefs_are_read_from_the_record.rs`,
`crates/familiar-ai/tests/the_cases_card_asks_the_panels_questions.rs`.

## Cards

### Holds are the person's

The agent never claims `hold_answered`; the binary writes it only from the
person's pick on a card it placed. When a card is open, the agent must ask
the person with Claude Code's AskUserQuestion tool; the hook turns that call
into the question card and records the pick, and the terminal command stays
as a fallback. Before the rule, the rulebook described the card as facts
and printed the terminal command, and every model ran that command instead
of asking; one ran it twenty times, and one tried to claim `hold_answered`
twenty times and was refused each time. With the rule, six of six test
sessions asked. Consequence: with no person present, a card is a timeout.

A part whose tests pass while no commit changed the code at its anchor
(`anchor_untouched`) is a card the person answers, so the stop has a way out
while the part waits on a decision: `re_plan` chooses the place again and the
proof starts over there, `keep_going` makes the change where it landed the
part's own and it goes green, `release` lets it go.

The owner (2026-10-13): "at keep_going where there is actually a change
outside the original plan we have to append an anchor to the plan and use the
existing infrastructure. so we update the plan, with a stamp that the user
basically overrode it." Right after the person's `keep_going` the binary
appends a `place_chosen` naming where the change landed since the seal: each
file the code commits touched, the symbol the repository's diff prints after
the hunk's `@@` with the lines from it to the hunk's end (a file-level place,
no symbol, where none is printed), and each place's planning check at the
tests-first commit. The place carries the answer's `hold` and `choice`, the
stamp that the person overrode the plan, so the committed plan says it too
(the plan carries no `hold_answered`). It does not move the part's epoch: the
red seal, the stamps and the answers before it still count, and the teeth bite
at the new place with the change put back, as for any part. Only the binary
writes it, only right after that answer: the rules offer it as the binary's
move alone, and an agent's `place_chosen` carrying `hold` or `choice` is
refused at the door. The earlier reading (after `keep_going` any commit that
touched code counted) is gone (`keep_going_names_the_place_and_keeps_the_proof.rs`,
`keep_going_appends_the_place.rs`).

A card reached the person only through its holder's own picker, so a card in
a headless lane's worktree never reached them: the person's session, in
another checkout, read only its own record. Now another checkout's open card
reaches the person's session whatever its own switch says. `status --json`,
the status line and the band list it with that checkout's folder; the agent
is told it at the person's prompt and session start; the person's own
AskUserQuestion with the `familiar-ai` header, while no card of its own
checkout is open, becomes that card with the same quote check; and the pick is
written to that checkout's record, never this one's. A subagent's or a launched
session's question is never the person's, nor is a message Claude Code
relays from an agent as a prompt (framed `Another Claude session sent a
message:`, `<agent-message from=` or `[Subagent hand-back]`, the one list in
`door/person.rs` `RELAYED_AGENT_FRAMES`): it is never `person_said`
(`a_relayed_agent_message_is_not_the_persons.rs`). A headless session (`claude -p`)
counts as the bench, so its stop with only the person's cards left goes and the
card waits in its record. That stop answers with the card in `systemMessage`
only and nothing for the agent, because Claude Code continues the turn on a
stop's `additionalContext` and counts it toward its stop-hook cap: the 44
benchmark at f51f5f6 ended three such sessions (`fix_elsewhere`,
`two_tries_not_red`) at that cap (`bench_stop_goes_with_the_card.rs`). Every checkout shows every other (their words, asks,
parts and who holds each), so the view is central from main or from wherever
the orchestrating session is.

Protects: the agent clearing its own hold.

Code: `crates/familiar-ai/src/teach/card.rs`, `teach/discipline.rs`,
`teach/picker.rs`, `door/claim.rs`, `door/picker.rs` (`write_elsewhere`,
`told_elsewhere`), `decide/picker.rs` (`rewrite`, `picked_elsewhere`),
`decide/tell.rs`, `door/project.rs` (`other_checkouts`).
Tests: `crates/familiar-ai/tests/card_person_answers.rs`,
`card_picker_answer.rs`, `card_picker_required.rs`,
`green_call_to_action.rs`, `card_from_another_checkout.rs`,
`status_every_checkout.rs`. (214b491, 0d22a5f, ddeca8d, 11bfb14, ff4e807,
707eb98)

### The agent cannot answer for the person

The card's choices are fixed by the binary: at most four, plus "something
else" while there is room. The agent's explanation is included in the question text. An agent
that pre-fills answers, or copies the card's header with its own options, is
never recorded as the person. This closed a defect: a result carrying the
card's header and question but the agent's own options was recorded as the
person's typed words.

Protects: an answer the person didn't give.

Code: `crates/familiar-ai/src/decide/picker.rs`, `door/picker.rs`.
Tests: `crates/familiar-ai/tests/card_fixed_choices.rs`,
`crates/familiar-ai/src/decide/picker_tests.rs`. (496bba7, 632e3de)

### The passphrase is the presence

CI trusts a card's answer only when a witness the agent cannot write vouches
for it; the plan file never carries `hold_answered`. On the open-source path
the witness is the person's signature: `familiar-ai approve` decrypts an
ordinary encrypted ed25519 key with the passphrase typed at the person's own
terminal with echo off, else read on stdin (never from the command line or the
environment, both readable by the agent's processes),
signs the card's payload (the repository's root commit, the criterion, the
hold, the choice, the card's run, the commit, the pick's `t`) as SSHSIG in the
namespace `familiar-ai-approval`, writes `.familiar/approvals/<t>.json` and
`.sig`, and records `hold_answered` naming it. `approve` is refused from an
agent's shell, as `on` and `off` are, and the agent's writes of
`.familiar/approvals/` and `.github/` are refused. These refusals read the
command's text, so they fail closed where the text hides the path or the
program (a variable, `~`, `eval`, a backtick, `patch`, an interpreter's
call naming either folder), and `.github` is read from the repository's top,
not a nested project's; a script file the agent wrote can still get past
them. That is why they are a courtesy and the passphrase is the guarantee:
an `approve` that got past the hook still needs it.

The passphrase typed at each answer is the presence: no hardware flag is
read. The agent can read the encrypted key file, so the guarantee rests on
the passphrase's strength, said plainly to the person at setup. The key is
never loaded into `ssh-agent`: an agent-held key signs for anyone who can
reach the agent's socket, the agent's own shell included, with no passphrase
asked, and the signature would prove nothing about the person.

`verify` reads every approval committed at HEAD and checks it against the
allowed signers of CI's own configuration (`FAMILIAR_AI_ALLOWED_SIGNERS`, a
repository secret or variable mapped into the job; never a file in the tree,
which the agent could write; never `check-novalidate`, which accepts any
key). Its lines are read as `ssh-keygen -Y verify` reads them: a
`cert-authority` line, a key outside its `valid-after`/`valid-before` window
(read against the time of the check), or one whose `namespaces=` leaves out
`familiar-ai-approval` signs no approval, and an option it cannot read fails
closed. Only a verified approval, bound to this repository and found in the
history, becomes the person's `hold_answered`, appended right after the
commit it was answered at. Anything else is "approval not verified" and the
part is not proven (fail closed). A file in the approvals folder that
fails its check stands against the part it names, beside any verified
approval for that part, since it was written where only the person writes;
one that names no part stands against every part. The hosted witness is
parked.

Code: `crates/familiar-ai/src/door/approval.rs`, `door/verify.rs`.
Tests: `crates/familiar-ai/tests/approval_witness.rs`,
`crates/familiar-ai/tests/approvals_are_the_persons.rs`.

### Cards speak the person's words

A card starts from the person's own ask, names the part, says what happened,
carries the agent's one sentence, and offers choices in the person's terms.
It shows no commit sha; a sha does not describe behaviour to the person. The binary
hands over the pieces and checks that the agent's question quotes each one;
the agent writes the prose around them. Open: each choice naming the state
it leads to, and the words of cards other than `anchor_moved`.

Code: `crates/familiar-ai/src/teach/person_words.rs`, `decide/picker.rs`.
Test: `crates/familiar-ai/tests/card_person_words.rs`.
(f2fa8aa, 41fe8c8)

### Asks, answers and notes

Every prompt is recorded as `person_said` and marked from facts alone: an
**ask** when an intent names it, an **answer** when a card's pick or
`person_said_answered` names it, a **note** otherwise. No model classifies
prompts. A new prompt after the plan is confirmed blocks the stop until
something names it, so no request is dropped. Open: the pane's
queue (see [plugin-notes.md](plugin-notes.md)).

Code: `crates/familiar-ai/src/teach/person.rs`, `door/person.rs`.
Test: `crates/familiar-ai/tests/judge_named_prompt.rs`.

### The person's words travel with a lane

Lanes were started by an orchestrator typing `claude -p "<brief>"`, so
each lane recorded its brief as `person_said`, and the record, the pane
and the judge took the orchestrator's words for the person's. A lane now
starts with `familiar-ai launch_lane`: the brief is its prompt, the
person's words of the current ask are copied into its record, and the
session carries `FAMILIAR_AI_CONVERSATION_ID`, so none of its prompts is
recorded as the person's. A lane carries only words typed in the
session that launches it (each `person_said` keeps its session) or named
by the run's intent; with neither, `launch_lane` refuses before anything
is made, naming the latest prompt's first words and the day it was
typed. One lane runs per worktree. An agent's
`claude -p` through Bash is refused naming the command, read from the
text as shell writes are; `familiar-ai on`, `off` and a card's answer
are refused the same way, whatever the agent unsets. A subagent started
through the Agent tool is told the person's words at its start instead.
`launch_lane` reads the launching checkout's record once, for those
words, and lets it go before the lane is made and seeded, so the
person's own session there is never refused as busy while the lane's
setup reruns the history's tests (the owner: "now the off button is
gone … it is supposed to stay with you as orchestrator").
The owner: "you are exclusively going to create a new command
launch_lane … we are going to make sure the claude -p is intercepted so
we do not have two sessions going".

Code: `crates/familiar-ai/src/door/lane.rs`, `door/shell_write.rs`,
`door/person.rs`, `teach/investigator.rs`.
Tests: `crates/familiar-ai/tests/launch_lane.rs`,
`agent_shell_refusals.rs`, `subagent_start_person_words.rs`,
`switch_off_records_prompt.rs`.

## What the agent reads

Nothing here changes what CI computes; these decide what the agent is told.

### A short rulebook, and every refusal names its way out

A rulebook of at most 32 lines is given once, at SessionStart. Every
refusal names the missing line, the command that claims it, and the kind's
form. A short rulebook costs context in every session, so it's capped. A
refusal without the way out is a block the agent cannot resolve.

Code: `crates/familiar-ai/src/teach/discipline.rs` (its unit test: at most
32 lines), `teach/form.rs`, `teach/words.rs`.
Test: `crates/familiar-ai/tests/refusal_teaches_way_out.rs`.
(a7ff23c, 1c6c763)

## The plugin

Nothing here changes what CI computes; the hooks are how the laptop loop is
wired into Claude Code.

### Every event goes through the binary

`hooks.json` sends every event to `run.sh`, which hands it to the binary.
The binary decides; Claude Code's hook protocol carries allow, deny or
block. Logic in the shell scripts would be untestable and duplicated; logic
in the TypeScript mod exists only where Claude Code's interface does.

Code: `plugin/hooks/hooks.json`, `run.sh`,
`crates/familiar-ai/src/decide/hook.rs`.
Tests: `plugin/tests/test-run.sh`, `test-agent.sh`, `test-ci.sh`,
`crates/familiar-ai/tests/hook_input_payloads.rs`.
(ffcd3a6, f992d1d)

### The hook fails closed

Bad input, an unreadable record or a missing binary refuses the action.
Exceptions: the exact setup commands and AskUserQuestion pass while the
binary is absent, and Stop and the person's prompts are never blocked. A
blocked Stop would re-prompt forever; a blocked prompt would lock the person
out of `/familiar-ai:setup`. Failing open would leave a broken install
ungoverned without notice.

Protects: a broken install that governs nothing and says nothing.

Code: `plugin/hooks/run.sh`, `crates/familiar-ai/src/decide/hook.rs`.
Tests: `crates/familiar-ai/tests/hook_fails_closed.rs`,
`plugin/tests/test-run.sh`. (f992d1d, f0033b2)

### Plan mode is never blocked

In Claude Code's plan mode, familiar-ai allows everything and says nothing.
It writes nothing but the person's own prompt: approving the plan sends no
prompt after it, so an ask typed in plan mode would leave the intent nothing
to tie to (2026-10-27). SessionStart still gives the rulebook, since the mode
can change mid-session with no hook. The product must not interfere with Claude Code's own
features.

Code: `crates/familiar-ai/src/decide/hook.rs`.
Test: `crates/familiar-ai/tests/hook_plan_mode_silent.rs`. (56859ca, b0324b6)

### The off switch

`familiar-ai off` and `on`, run from the person's own terminal, append
`familiar_switch {to: off | on}`. While the latest switch says off, every
hook allows everything and writes nothing but the person's own prompts
(the owner, 2026-10-12: "Yes, record my prompts while off"). From an
agent's shell both commands are refused, and so is a Bash command whose
text runs either, whatever it does to its environment; the kind can't be
claimed. A band button is the
second way to flip it. The judge and investigator keep their own switches,
`FAMILIAR_AI_JUDGE=off` and `FAMILIAR_AI_INVESTIGATOR=off`. A setting file
or environment variable wouldn't be a fact, and the record couldn't say when
governing stopped.

Each checkout has its own record, so a fresh worktree used to start ON while
main was OFF, and nothing on screen showed it. Now a linked worktree with no
`familiar_switch` of its own follows main's latest switch, read from main's
record in the same project folder; once the worktree is switched itself, its
own wins. `on` and `off` switch the checkout they run in and say which
("Familiar is turned on for this worktree. Main stays off."). Every surface
says whose switch it is: the status line (`OFF · as main`, `ON · here · main
OFF`), the band and the session's start ("…for this worktree, as main is."),
`status`, and `status --json` (`off`, `switch_from`, `main_off`). Main's record
is opened with the hook's wait while the worktree's own is let go, so two
checkouts never wait on each other; busy past the wait fails closed as a busy
record does. Rejected: copying main's switch into each new record (a fact no
one observed) and a setting shared by all checkouts (not a fact). One thing
works while off: another checkout's card still reaches the person's session
([holds are the person's](#holds-are-the-persons)).

Code: `crates/familiar-ai/src/door/switch.rs` (`here`),
`crates/familiar-ai-git/src/ignore.rs` (`main_worktree_root`),
`crates/familiar-ai-facts/src/form_table.rs`.
Tests: `crates/familiar-ai/tests/switch_off_on.rs`, `switch_off_shown.rs`,
`switch_worktree_follows_main.rs`. (9b0b448, d1c06fe, 394ff9b)

### Two-command install

Install is a marketplace add and a plugin install, then
`/familiar-ai:setup` in the project. The binary ships for Linux and macOS;
Windows runs it in WSL. The hooks are shell scripts and the runs use Unix
process groups, so native Windows would be a port, and waits for a Windows
team to ask.

Code: `README.md`, `plugin/skills/setup/SKILL.md`, `plugin/hooks/run.sh`.
Test: `crates/familiar-ai/tests/setup_starts_a_project.rs`. (6e63dcf)

## The TUI

### One source for the TUI

The band, the pane and the status line read only `familiar-ai status
--json`, which reads only the record. The JSON carries
names, parts, the ask, counts, runs, choices and `off`; the mod computes
nothing.

Code: `crates/familiar-ai/src/teach/mod.rs`, `plugin/hooks/status.ts`,
`plugin/types/index.d.ts`.
Tests: `crates/familiar-ai/tests/status_json_same_picture.rs`,
`plugin/tests/familiar-line.test.ts`. (a0f81bf, de7b48e)

## Compatibility

### Upgrades keep proofs

A legacy per-commit fingerprint is read but never used to stamp. An old
record is replayed from git on first use, and a legacy seal is re-stamped
from its observation when a laptop result exists at that commit. Throwing
old proofs away removed the seals from one existing record. Re-stamping
requires a laptop result at that commit; without one, no stamp is written.

Protects: an upgrade inventing a proof, or losing one.

Code: `crates/familiar-ai/src/door/commits.rs`.
Tests: `crates/familiar-ai/tests/record_replay_first_use.rs`,
`record_legacy_seal_restamp.rs`,
`crates/familiar-ai-rules/tests/a_legacy_fingerprint_is_read_but_never_stamped.rs`.
(2cef061, 873a663)

### Older binaries refuse newer records

An unknown kind is refused, and a stored fact that won't parse is an error.
Skipping unknown kinds would derive states from part of the record and could
call something proven that isn't. Open: a version marker, or a message that
says to upgrade.

Protects: a state derived from part of the record.

Code: `crates/familiar-ai-facts/src/append.rs`, `record.rs`.
Test: `crates/familiar-ai-facts/tests/validation.rs`. (affe3ed)

### A removed folder refuses until setup

Setup leaves a marker outside the repository. If `.familiar/` is gone but
the marker remains, the hooks refuse until setup runs again. An agent did
delete the folder and carry on. The person has `familiar-ai off` for that.

Protects: removing the record as a way around it.

Code: `crates/familiar-ai/src/door/set_up.rs`, `door/marker.rs`.
Tests: `crates/familiar-ai/tests/setup_removed_project_refuses.rs`,
`setup_marker_removal.rs`. (7262493, 15ce71b)
