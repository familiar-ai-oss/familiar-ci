# Reference

Every command, hook event, surface and term, as the binary and the plugin implement them at
this commit.

The plugin in `plugin/` is hooks that relay each Claude Code event to the `familiar-ai`
binary, two skills (`setup`, `plan`), two commands (`/familiar-ai:setup`,
`/familiar-ai:prove`), the investigator agent, and three surfaces: the band, the pane and
the status line. The binary decides; Claude Code enforces. Everything the binary knows is
in the project's record, `.familiar/record`. The words used here are defined under
[Terms](#terms). The rulebook the agent reads is
`crates/familiar-ai/src/teach/discipline.rs`. Why each part is the way it is lives in
[decisions.md](decisions.md).

## Commands

| command | what it does |
| --- | --- |
| `familiar-ai setup` | makes `.familiar`, git-ignores the record, logs and kept runs, writes a marker outside the repository, adds the status line if none is set, adds the familiar-ai lines to the end of `CLAUDE.md`; brings the plan back from git and starts the record from it (see [The seed](#the-seed)); its own commit, safe to re-run |
| `familiar-ai claim` | lists the 22 claimable kinds in the plan's order |
| `familiar-ai claim <kind>` | prints the kind's form: every field, its shape, any closed set, one complete example |
| `familiar-ai claim <kind> --<field> <value> ...` | appends one fact the agent or the person declares, then shows what changed and the picture |
| `familiar-ai plan` | claims the plan's lines in one call, from a JSON array on stdin; with nothing on stdin, prints the array's form |
| `familiar-ai status` | the whole picture: every criterion, its state, its holds, open question cards, what comes next; `--json` for the same as one object, `--said` adds every prompt the person typed and what it became, `--pr` prints the pull request body as markdown |
| `familiar-ai statusline` | prints the status line from Claude Code's status-line JSON; reads only |
| `familiar-ai run` | one catch-up: records landed commits, recovers runs nobody is running, launches the test runs owed, then lists the runs |
| `familiar-ai cancel --run_id <id>` | ends a run's process group; a cancelled run is no result |
| `familiar-ai ci fetch [--sha <sha>]` | asks GitHub (`gh`) for the `familiar-verify` runs of a commit and records each finished one; nothing else asks the network |
| `familiar-ai tidy` | before the first push, turns each criterion's unpushed commits into one seal commit and one fix commit (see [Tidy](#tidy)) |
| `familiar-ai verify` | rechecks the committed proof in a clean clone, as CI does (see [Verify](#verify)) |
| `familiar-ai approve --criterion_id <c> --hold <hold> --choice <choice> [--run_id <id>] [--typed <words>] [--key <file>] [--unsigned]` | the person's signed answer to a card (see [Signed answers](#signed-answers)); refused from an agent's shell |
| `familiar-ai off` / `on` | the person's switch for this project; refused from an agent's shell |
| `familiar-ai launch_lane --brief <file> [--worktree <folder>] [--resume]` | starts a lane: a headless `claude -p` in a worktree of its own (made beside the repository on the branch `lane-<name>` from HEAD, unless named), the brief as its prompt, the person's words of the current ask copied into its record as `person_said`, our plugin (`--plugin-dir`, else `CLAUDE_PLUGIN_ROOT`), this binary first on PATH and `FAMILIAR_AI_CONVERSATION_ID` set, so the brief is never the person's; detached, its stream-json log and session under the lane's `.familiar/runs/lane`; refused while a lane runs in that worktree; `--resume` continues its conversation with the brief as the next prompt; `--model` and `--max-budget-usd` are passed on |
| `familiar-ai judge-prompt <criterion_id>` | prints the exact prompt the judge is asked; asks no model, writes nothing |
| `familiar-ai await-run` | internal: the async hook after `git commit`; waits for the runs that commit reserved, tells each result once, wakes the agent with it (exit 2) |
| `familiar-ai hook <Event>` | what `plugin/hooks/run.sh` calls for every event, with the event's JSON on stdin |
| `familiar-ai --version` | the binary's version |

**A claim.** A value that reads as JSON (`10`, `true`, `[...]`) is JSON, anything else
text (`--criterion_id 1` is `"1"`); the fields may instead be one JSON object on stdin.
Left out, the binary fills `intent_submitted.sha` (the head), `intent_submitted.for` (the
person's latest prompt; with none yet the intent is refused),
`criterion_assigned.agent_conversation_id` (`CLAUDE_CODE_SESSION_ID`; a subagent passes
`<session_id>/<agent_id>`) and `test_stack_detected.codebase_id` (the repository folder's
name, a label). `--project_root .` is the whole project, `src/` only the anchors under it
(the longest covering root wins),
a path outside is refused; a `--test_command` naming a test file neither in the tree nor
planned is refused. The examples carry values the record knows (planned test files, the
next free id); the agent's own values win.

The 30 kinds the binary writes are refused whoever claims them: the runs (`run_reserved`,
`run_started`, `run_ended`, `run_cancel_requested`, `test_log_translation`, `reader_chosen`,
`pass_please_pass`, `fingerprint_stamped`), the checks (`commit_seen`,
`anchor_sha_matched`, `anchor_is_documentation`, `test_written`, `tests_observed`,
`pipeline_run_seen`, `ci_report_seen`), the models (`model_seen`, `judge_verdict`,
`judge_unanswered`, `investigator_unfinished`, `investigator_finding`, `fix_verdict`) and the session
(`person_said`, `familiar_switch`, `subagent_ended`, `holder_resumed`, `told`,
`record_seeded`, `criterion_covered`, `dependency_drafted`, `dependency_unanswered`).

**The run facts** (the runner and `verify` write them; a laptop run and a `verify` run
alike):

| kind or field | what it holds |
| --- | --- |
| `run_ended.exit_code` | the command's exit code, a whole number; optional |
| `run_ended.signal` | the signal that ended the run, a whole number; optional |
| `run_ended.stdout_tail` | the cut of the run's cleaned standard output; optional |
| `run_ended.stderr_tail` | the cut of the run's cleaned standard error; optional |
| `run_ended.truncated` | one row per stream the drain cut, `{stream: stdout\|stderr\|report, total_bytes, dropped_bytes}`; optional |
| `pass_please_pass.run_id` | the run the result came from; optional |
| `test_log_translation` | what a run's output says: `{run_id, criterion_id, commit, by, label, evidence, failing_tests, classified_on, location?, missing_name?, counts?, ran_tests?, skipped_tests?, versions?}`; `by` is `regex`, `binary` or `reporter` [draft], `label` one of `passed`, `tests_failed`, `collection_error`, `missing_target_symbol`, `no_tests_collected`, `setup_error`, `usage_error`, `environment_error`, `runner_crash`, `killed`, `crashed`, `timed_out`, `unclassified`; `classified_on` is `full` or `cut`; `location` is `{file_path, line, function?, kind?}`, `kind` one of `assertion`, `exception`, `compile`, `signal` [draft]; `counts` is `{passed?, failed?, skipped?, deselected?}`, whole numbers and no other key [draft]; `ran_tests` and `skipped_tests` are lists of test names and `versions` the runner's versions as one string [draft]; a reading writes `kind` with its location and `counts` (`{}` when the output counted nothing, none on a run read from the cut; see [Outcomes](#outcomes)); nothing writes `ran_tests`, `skipped_tests`, `versions` or `by: reporter` yet; a translation with no `counts` key is a legacy one and reads as before |
| `reader_chosen` | how a runner is read here, written by a dry run [draft]: `{run_id, root, runner, reader, command, commit, setup?, versions?, familiar, watched, gaps, why, ran_in?}`; `watched` and `gaps` are lists of text, `ran_in` is `laptop` or `pipeline`; never a plan line; nothing writes it yet |
| `run_reserved.setup_from` | the commit whose watched setup files a re-check puts back; optional form field [draft], written raw; `run_reserved.by` may also be `dry_run` |
| `dependency_drafted` | the dependency drafted for an approved `a_dependency`: `{approval, criterion_id, text, test_file, place?}`, `approval` the `t` of the `hold_answered`, `place` a `place_chosen` place |
| `dependency_unanswered` | an approval that could not be drafted: `{approval, why}` |
| `told` | something owed to an agent was told, once: `{about}`, a run id or a `hold_answered`'s `t` as text |
| `criterion_covered` | a criterion closed as covered by its dependency: `{criterion_id, by_criterion}` |
| `subagent_ended` | a subagent, or a lane's headless session, ended: `{agent_id, agent_type}` |
| `holder_resumed` | a resumed lane's headless session started |
| `investigator_finding` | the diagnosis of a test that never went red (see [Model calls](#model-calls)) |
| `fix_verdict` | the reviewer's word on a green fix [draft]: `{criterion_id, commit, model, covers, reason, gap?, quotes?, why?, detail?}`; `covers` is `yes`, `no` or `not_judged`; `quotes` are `{file_path, line, text}`; `why` is `error`, `timeout`, `unparseable` or `unverified` (see [Model calls](#model-calls)) |
| `record_seeded` | the seed's mark: `{commit, plan_lines}` (see [The seed](#the-seed)) |
| `hold_answered.approval` | on a signed answer, the `t` of its approval (see [Signed answers](#signed-answers)); none on an unsigned one |

`.familiar/plan` carries none of them, and no `hold_answered`: nothing mirrored into the
plan may let CI accept a person's approval, and `verify` would read a release written
there by hand as the person's and pass a part with no tests. A plan line of any of these
kinds is refused.

**The plan.** `familiar-ai plan` reads a JSON array of `{"kind": ...}` objects on stdin,
claims each through `claim`'s path, and stops at the first refusal (`familiar-ai: line N
of M, <kind>: ...`); the lines before it stay. The status comes back with the answer.
`claim` and `plan` wait 500 ms for stdin, so an open pipe nobody writes never hangs. A
heredoc is the way in:

```
familiar-ai plan <<'EOF'
[{"kind":"intent_submitted","text":"greet with hello"},
 {"kind":"behavior","behavior_id":"b1","text":"greeting by name"},
 {"kind":"promise","promise_id":"p1","behavior_id":"b1","accepts":"a name","gives_back":"a greeting","refuses":"an empty name"},
 {"kind":"criterion","criterion_id":"c1","text":"greet says hello","path":"full","behavior_id":"b1","promise_id":"p1"}]
EOF
```

The example is a code anchor's: the full path, with the behavior and promise claimed
before the criterion that names them. `light` is for a documentation anchor only. In one
planning, the first `plan` reply that leaves the plan incomplete shows the whole
eleven-line checklist; a later one shows only the lines still missing, each with its
claim, and "N of 11 hold". The hook's refusal at an edit keeps the whole checklist.

**A refusal** names the missing line, the command that claims it (`claim it:`) and the
kind's form. Every printed line begins `familiar-ai:`, names a criterion by its text in
quotes and says what comes next: `familiar-ai: not done yet · "greet says hello" · the
test comes first`.

## The plan's eleven lines

The agent claims these before its first edit or commit of code. One criterion on a
code anchor (the full path), with the example values the binary itself shows:

    1  the intent      familiar-ai claim intent_submitted --text 'greet with hello'
    2  a criterion     familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path full --behavior_id b1 --promise_id p1
    3  full path only  familiar-ai claim behavior --behavior_id b1 --text 'greeting by name'
                       familiar-ai claim promise --promise_id p1 --behavior_id b1 --accepts 'a name' --gives_back 'a greeting' --refuses 'an empty name' --cases '[{"case_id":"n1","kind":"normal","short":"a plain name is greeted","given":"the name Ada","expect":"hello Ada"},{"case_id":"n2","kind":"normal","short":"a name with a space is greeted in full","given":"a name with a space, Ada Lovelace","expect":"hello Ada Lovelace"},{"case_id":"e1","kind":"edge","short":"a very long name is greeted in full","given":"a name of 200 characters","expect":"greeted in full"},{"case_id":"o1","kind":"opposite","short":"an empty name is refused","given":"an empty name","expect":"refused, no greeting"}]'
    4  its anchor      familiar-ai claim place_chosen --criterion_id c1 --places '[{"file_path":"src/app.py","symbol":"greet","line_start":10,"line_end":20}]'
    5  the check       the binary checks those lines at the sha: anchor_sha_matched
    6  its tests       familiar-ai claim test_planned --criterion_id c1 --file_path tests/test_greet.py --will create
    7  the command     familiar-ai claim test_stack_detected --project_root . --test_stack pytest --test_command 'python -m pytest -q tests/test_greet.py'
    8  touching tests  familiar-ai claim touching_test_declared --criterion_id c1 --file_path tests/test_old.py --test_name test_greet
    9  the string      familiar-ai claim criterion_head --criterion_id c1
    10 who holds it    familiar-ai claim criterion_assigned --criterion_id c1
    11 where it lives  familiar-ai claim fix_location_category --criterion_id c1 --category '' --where ''

Line 7's example names `python -m pytest`, or `python3 -m pytest` where no `python` is on
PATH; never both in one command, since a red test exits non-zero and would run twice.
Line 4 may instead be `new_work_spot_chosen` or `new_work_no_spot`, and line 9
`criterion_strung_on_head` for a criterion whose test cannot be set up without another's
code. Every link names its string's head, a third part too
(`familiar-ai claim criterion_strung_on_head --criterion_id c3 --head c1`), and the links
take their turns in the order they were claimed; a `--head` that is not a head is refused
at the claim, naming the head.

Then the order that matters: the agent commits the tests alone and the binary runs them,
and they must fail (**sealed**); the judge reads them against the person's words; the
agent commits the code on its anchor, and the same tests must pass under the same stamp
(**green**). A sealed test is never edited without announcing it first
(`test_change_announced`).

**Several criteria.** Each criterion runs its own command. The binary derives it from the
stack's runner and the criterion's own test files (pytest, cargo, jest, vitest and go; a
command the agent claims wins). For a runner it does not know, the criterion is
unscoped: `status` says `unscoped · scope the command to prove in parallel`, and its
seal does not stand while another criterion of the same stack has an outstanding red.

**A sealed test that was wrong.** Announce the edit
(`familiar-ai claim test_change_announced --criterion_id c1 --reason 'the assertion named the wrong file'`).
The commit that edits the test is the new stamp; green and the revert check are earned again under
it, and the seal's red stands.

## Hooks

`hooks.json` hands every event to `run.sh`, which pipes it to `familiar-ai hook <Event>`.
PostToolUse and Stop have 1800 s (a test run, the judge, the investigator), PreModelSwitch
30 s, the others 200 s.

| event | what the binary answers |
| --- | --- |
| SessionStart | the session-start text (below); records the session's model |
| UserPromptSubmit | records the prompt as `person_said`, also while off, never in a session the binary launched; adds open question cards and the push facts as context |
| PreToolUse, an edit (`Edit`, `Write`, `MultiEdit`, `NotebookEdit`) | allow or refuse, by the rules |
| PreToolUse, `Bash` | its file writes decided as edits; a commit checked; removing `.familiar` refused; by its text alone, `claude -p` refused naming `launch_lane`, and `familiar-ai on`, `off` and a card's answer refused whatever the command does to its environment (the switch also while off) |
| PreToolUse, `Agent`/`Task` | the investigator spawn: rewritten, allowed or refused (see [Model calls](#model-calls)) |
| PreToolUse, `AskUserQuestion` | the question card (see [The person's decisions](#the-persons-decisions)) |
| PostToolUse | records landed commits, runs the tests owed, asks the judge, tells the agent where it stands; each result and each pick is told once (`told`), to the part's holder, else the orchestrator |
| PostToolUse, `Bash` `git commit *` | also `await-run`, async with `asyncRewake`: the result of a run that outlasts the hook's inline window wakes the agent |
| Stop, SubagentStop | refused until every criterion the conversation holds is green, proven or released, no question card is open, and every prompt the person typed after the plan was confirmed is answered; an allowed stop records `subagent_ended` |
| SubagentStart, PostModelSwitch | record the model; the investigator is told its criterion; any other subagent, while a run is open, is told the person's words (`THE PERSON ASKED (their words):`) and the part it holds |
| PreModelSwitch | allowed, nothing opened |

An allow is `{}`, never `"permissionDecision": "allow"` (which skips the person's own
permission prompt) except for the investigator rewrite. The agent reads
`additionalContext`, the person `systemMessage`; a refusal is a `deny` on PreToolUse,
`{"decision": "block"}` elsewhere.

**What is protected.**
- `.familiar/plan`, `.familiar/record` and a sealed test: the binary writes the first
  two, and a sealed test changes only after `test_change_announced`.
- The `.familiar` folder: a shell command naming it with `rm`, `rmdir`, `unlink`, `mv`,
  `truncate`, `>`/`>>`, `git clean`, `git checkout --` or `git restore`, or a
  `git reset --hard` or `git clean -x` while it holds uncommitted content, is refused.
  A marker setup writes outside the repository (under `familiar-ai/projects/` in
  `$XDG_DATA_HOME`, else `~/.local/share`) means a lost folder refuses every call but
  `familiar-ai setup`, which restores it.
- Shell writes: `>`, `tee`, `sed -i`, `cp`, `python ... open(p, 'w')` and the other
  obvious forms are decided as edits of the files they name; `/tmp` and paths outside the
  project are allowed. Only the command's text is read; CI is the backstop.
- An edit outside the anchor is told, not refused.

**Plan mode is never blocked**: every hook allows, says nothing and writes nothing, but
SessionStart still gives the rulebook, since the mode can change mid-session. **The
session-start text** is the rulebook, the whole picture and the first missing line; in a
project with no `.familiar`, one line naming `familiar-ai setup`; switched off, one line
saying so. **Without the binary** on PATH, `run.sh` refuses every tool call except the
exact setup commands and `AskUserQuestion` (telling them apart needs `node`; without it,
every tool call is refused); a binary that crashes on PreToolUse refuses.

**Failing closed.** Bad hook input is refused with exit 2. An unknown event or command is
a usage error, also exit 2. A hook that cannot read the record exits 2 rather than let the
action through.

## Outcomes

A run's `test_log_translation` gives it a label, the label a family
(`crates/familiar-ai-rules/src/outcome.rs`), the family a route:

| family | labels | before the seal | after the fix |
| --- | --- | --- | --- |
| green | `passed` | refused (`tests_not_red`) | green, once the counts agree with the seal run's [draft] |
| valid red | `tests_failed`, `missing_target_symbol`, where the failure points at the place | sealed | attempt |
| test broken | `collection_error`, `no_tests_collected`, `setup_error`, or a red pointing elsewhere | refused (`red_for_another_reason`) | attempt |
| too slow | `timed_out` | refused (`red_for_another_reason`) | attempt |
| crashed | `crashed` | `crashed` card | attempt |
| command broken | `usage_error` | `command_broke` card if the command once worked, else `red_for_another_reason` | the same |
| not the agent's | `environment_error` | `environment_error` card | the same |
| transient | `runner_crash`, `killed` | retried once after 5 s, then `retry_spent` card | the same |
| can't tell | `unclassified`, a missing symbol with no location | `run_unclassified` card | the same |

**A reading is the truth or can't tell.** A label is read only on positive evidence in
the output; what no row can read is `unclassified`, the `run_unclassified` card, never a
pass and never a red. The exit code alone decides only what it shows:

| exit or end | reads |
| --- | --- |
| 126, 127 | the shell's line decides: a bare command not on `PATH` is `environment_error`, a path (`./build/x`) `usage_error` |
| 132, 134, 135, 136, 139, or a signal 4, 6, 7, 8, 11 | `crashed` |
| 130, 137, 143, or a signal 2, 9, 15 | `killed` |
| a timeout | `timed_out`, the output read for evidence only |
| 0, and a row of the runner's table says it passed | `passed` |
| 0, and a row says no tests ran | `no_tests_collected` |
| 0, and no row matches | `unclassified` |
| non-zero, and no row matches | `unclassified` |
| a red at exit 0, or `passed` at a non-zero exit | `unclassified` |

Every runner's table has a positive pass row; the shell's is its exit 0. Each stage wants
a different reading, and each can be wrong two ways:

| | stage 1: the tests-first commit | stage 2: the fix commits |
| --- | --- | --- |
| wants | a red for the right reason: sealed | green |
| wrong, false security | a seal not true to its assumptions | a green not true to its assumptions |
| wrong, false alarm | an honest red refused | a real pass read as a failure |
| can't tell | `run_unclassified`; `its_the_red` seals [draft] | `run_unclassified`; `its_the_pass` is the green [draft]; a red read this way still counts as a `tests_failed` try [draft] |

**Where a failed test points** [draft: the family by kind is H6's. Today a red with no
location, or one in a planned test file, reads as an assertion and seals]:

| label | location | kind | family |
| --- | --- | --- | --- |
| `tests_failed` | none | | can't tell |
| `tests_failed` | a planned test file | assertion | valid red |
| `tests_failed` | a planned test file | exception | can't tell (a stub returning `None`, an `unwrap` on a stub's `Err`) |
| `tests_failed` | inside the place's function | any | valid red |
| `tests_failed` | elsewhere | exception | test broken |
| `tests_failed` | elsewhere | assertion | can't tell |
| `missing_target_symbol` | as the place rule | | as before |
| any | a translation with no `counts` key (legacy) | | read as before |

After the seal [draft], green needs the counts to agree with the seal run's: none failed,
skipped plus deselected no more, passed at least its passed plus failed. With no baseline,
green only with nothing skipped; a baseline with counts and none here can't tell. The
person's own skips, deselects and filters are ignored.

**Counts and kind.** A reading stores two things beside its label:

| field | holds | read from |
| --- | --- | --- |
| `counts` | `passed`, `failed`, `skipped`, `deselected`, each left out when the output did not say; `{}` when it counted nothing; none on a run read from the cut (`classified_on` `cut`) | the runner's own summary, by the table's count rows (errors count as failed; with no passed row, the total less failed, errors and skipped is passed); `sh` has no count rows, so its `counts` is `{}` |
| `location.kind` | `assertion`, `exception`, `compile` or `signal`: why the output points there | the kind of the table row that found the place (a `frame` row is an exception) |

Four rules keep one location: (1) the first candidate inside the project, with its kind;
(2) one file and line that an assertion row and another row both give is an assertion;
(3) an exception row that matches with no file or line leaves no location; (4) for a
failed test, candidates inside the project of different kinds at different places leave
no location, since the output does not say which one failed.

**The place rule.** A missing name points at the place when it is the place's symbol and
sits in a planned test file; an exception when its deepest project frame is in the place's
file and function. Before H6 a failure with no location, or in a planned test file,
counts as an assertion whatever its kind. A result with no `run_id` counts by its word
only before the record's first translation, after it as unknown.

**Attempts** are counted per family (valid red, test broken, crashed, too slow), each at a
code commit since the seal. Two of one family raise its card (`two_failures`,
`two_test_broken`, `two_crashed`, `two_timed_out`); `try_again` resets that family's
count. Before the seal, two tries that did not go red raise `two_tries_not_red`.

## Readers and the adapter [draft]

Open: the binary writes none of this at this commit; only the facts it names are landed
(`reader_chosen`, `by: reporter`, `ran_tests`, `skipped_tests`, `versions`,
`run_reserved.setup_from`). The regex tables are the only reader. Why:
[decisions.md](decisions.md#regex-or-familiars-own-reporter-never-a-model).

A run is read by the best reader the project's dry run proved, falling back to the tables
for a run that reader did not see. The list is Familiar's own, never the repository's:

| reader | what it reads |
| --- | --- |
| Familiar's reporter | a plugin or reporter flag added to Familiar's own run only; the planned command and the person's terminal stay as they are |
| report files | the runner's own (JUnit, TRX, test2json, libtest-json), converted to the per-test file |
| pinned console | the runner's console, pinned to a recorded format |
| tables | the runner's regex table |

**The per-test result file.** JSON Lines v1, written to the path in `FAMILIAR_AI_REPORT`
(a temp path outside the work tree). A kit never prints and never raises; an internal
error writes a `kit_error` line and the next reader takes over.

| line | holds |
| --- | --- |
| `session` | `runner`, `runner_version`, `language`, `plugins` |
| `collect_error` | `file`, `error {type, message, kind, frames}` |
| `test` | `id`, `file`, `line`, `outcome` (`passed`, `failed`, `skipped`, `error`), `skip_reason`, `error {type, message, kind, frames}` |
| `end` | `exit`, `counts {passed, failed, skipped, deselected, errors}` |

`error.kind` is `assertion`, `exception`, `missing_name` (the name the test asks for does
not exist: Python `cannot import name`, JS `is not a function`, Go `undefined:`, Rust
E0425, PHP `Call to undefined`, Java `cannot find symbol`, C# CS0103) or `setup` (a fixture
or hook); a missing module is an `exception`. `frames` run outermost first. A file is read
first match wins:

| the file | reads |
| --- | --- |
| no `session`, no `end`, or a `kit_error` | no structured output: the next reader |
| a line that is not JSON, a test id twice, or an `end` that contradicts the exit code | `unclassified` |
| a `missing_name` collect error, or test | `missing_target_symbol` |
| another collect error | `collection_error` |
| a setup error | `setup_error` |
| a failed test | `tests_failed` (an assertion: the assert line; an exception: the deepest project frame) |
| one passed or more, no failure | `passed` |
| nothing ran | `no_tests_collected` |

**The dry run.** A part-less run, `run_reserved` with `by: dry_run`, holding the `tests`
resource like any run; it never settles a part and `await-run` skips it. In a scratch
copy, one probe at a time, each with its truth:

| probe | stage | truth |
| --- | --- | --- |
| P1 | 1 | a pass reads `passed` |
| P2 | 1 | an assertion reads `tests_failed`, kind assertion |
| P3 | 1 | a missing function reads `missing_target_symbol` |
| P4 | 1 | a broken import reads `collection_error` (for .NET and Java, a CS0246-style missing name) |
| P5 | 1 | a pass beside a skip reads `passed`, the skip counted (exact names, or can't tell) |
| P6 | 1 | nothing selected reads `no_tests_collected` |
| P3b | 2 | the function now exists: a pass |
| P7 | 2 | an exception in the code, located inside the probe function: `tests_failed`, kind exception |

The command is the project's own, re-aimed: wrappers, runner, subcommand and flags are
kept, the positional targets and the runner's selection flags (pytest `-k -m --lf`, jest
`-t`, go `-run`, …) dropped, the probe's targets appended. A command naming no runner
(`make test`) gets no dry run: tables only, and the gap card says so. Right is the truth,
can't tell is `unclassified`, wrong is any other label. The first reader right at both
stages is chosen; else the first with no wrong reading, its can't-tell probes the gaps;
else none (every run of that runner can't tell) and the card names the probe that read
wrong. P1 failing under every reader is a kit failure: the tables stay and the card says
so. The choice is a `reader_chosen` fact per root and runner. A dry run is owed when:
there is no choice for the runner or its command changed (setup, the first claimed test
command), a later commit changes a watched setup file (`pyproject.toml`, `conftest.py`,
`package.json` and lockfiles, a jest or vitest config, `Cargo.toml`, `go.mod`, `pom.xml`,
`phpunit.xml`, …), the Familiar version changed, a run read can't-tell (once per choice),
the reporter's own versions drifted, or the person picked `try_again` on the gap card.

**The adapter file** `.familiar/adapter`: JSON Lines sorted by root and runner,
byte-stable, reader ids only:

    {"root":"","runner":"pytest","reader":"pytest:reporter","setup":"sha256:…","familiar":"0.1.0","gaps":[]}

`setup` is a sha256 over the watched files, `familiar` the binary's version, `gaps` the
probes no reader could read. The binary writes it beside `.familiar/plan` and it is
committed with the work; an agent's write to it is refused, and setup refuses an ignore
that would hide it. A laptop run takes its choice from the record, never the file. `verify`
runs its own dry run at HEAD, uses its own choice and prints one line when the committed
file differs; the exit code never changes. Setup starts the dry runs.

**The setup part.** A gap is one card on a reserved project-level part `setup` ("the test
setup"), through the same card machinery as any part: the hold `reading_gap` names the
setting or plugin and which outcomes will ask, choices `keep_going` · `try_again`, and it
does not pause work. The claim door refuses `criterion_id: "setup"`.

**A setup change after a seal** re-checks the seal: the tests-first commit's code run with
the changed setup files put back (`run_reserved.setup_from` names the commit). Still an
honest red, the seal stands; else the hold `setup_changed` (pauses; `re_plan` · `release`),
and green waits while a re-check is owed. `verify` takes the same step after the teeth.
Why: [decisions.md](decisions.md#the-seal-is-re-checked-under-a-changed-setup).

**Gap recordings** are Familiar's own probe output, never the person's code, kept in
`.familiar/runs/gaps/<run_id>/<probe>/` (`cmd`, `exit`, `stdout`, `stderr`, `version`,
`report`). They go to Familiar only after the person's one-time yes at setup; the endpoint
is the owner's to name, and until then they queue locally.

## The person's decisions

Fifteen holds put a question to the person; the first two failure holds only once no
stronger model is left to take the part. Each is answered by one of its choices
(`ANSWERS` in `crates/familiar-ai-rules/src/criterion.rs`):

| hold | raised when | choices |
| --- | --- | --- |
| `anchor_moved` | the place's lines moved | `re_plan` · `keep_going` · `release` |
| `anchor_untouched` | the tests passed with no change on the place | `re_plan` · `keep_going` · `release` |
| `fix_elsewhere` | the fix lives outside this repository | `release` · `re_plan` · `take_over` |
| `two_failures` | two valid reds after the seal | `try_again` · `take_over` · `a_dependency` · `release` |
| `two_tries_not_red` | two tries to go red failed | `try_again` · `re_plan` · `release` |
| `two_test_broken`, `two_crashed`, `two_timed_out` | two attempts of that family | `try_again` · `re_plan` · `a_dependency` · `release` |
| `environment_error` | the environment failed the run, or a dependency's plan did not hold | `try_again` · `re_plan` · `release` |
| `run_unclassified` | the output was not read: Familiar can't tell | `try_again` · `re_plan` · `release`, and `its_the_red` before the seal or `its_the_pass` after it [draft] |
| `retry_spent` | the retry died too | `try_again` · `re_plan` · `release` |
| `crashed` | a crash at the tests-first commit | `its_the_bug` · `a_dependency` · `release` |
| `command_broke` | a command that worked now fails | `fix_what_broke_it` · `a_dependency` · `release` |
| `miscoped` | the part passed once its dependency was fixed | `covered_by_dependency` · `re_plan` · `release` |
| `no_test_stack` | no test command | the typed command |

`try_again` runs it again (the run again, for the last three above), `re_plan` chooses the
place again, `keep_going` carries on on the code as it is, `release` ends the part,
`its_the_bug` seals the crash as the red, `fix_what_broke_it` locks the command until the
code is fixed, `a_dependency` and `covered_by_dependency` are under [Dependencies](#dependencies).
On a `run_unclassified` card [draft], `its_the_red` takes the run as the red and seals it,
`its_the_pass` takes it as the green; each is the person's word for a run Familiar could not
read. The card is plain words, never a raw log: what ran, the counts, the deciding line, the
failing tests and where, why Familiar couldn't tell, where the full output is. It is signed
like any answer, the approval binds the run's label and deciding line, and CI accepts it only
where its own run at that commit also reads can't-tell with the same label and deciding line.
The run's own reading stays can't-tell: the person's word seals or greens it, and relabels
nothing.
The pick is told to the part's holder with its next step (`AFTER_PICK` in
`crates/familiar-ai/src/teach/words.rs`). While a question card is open the agent is told
its facts: what the person asked, the parts, what happened.

The agent asks with `AskUserQuestion`; at PreToolUse the binary answers `ask` with the
call rewritten into the question card: the person's ask, the part, what happened, the
agent's own sentence, the header `familiar-ai`, the choices (at most four) and, while
there is room, "something else" (`no_test_stack`: "type the command"). A question missing a
piece is refused, naming what to quote. At PostToolUse the pick is recorded as
`hold_answered`, unsigned (`no_test_stack`: the typed command as `test_stack_detected`). The
agent never answers a question card: `claim hold_answered` from an agent's shell is refused.

A card open in another checkout of the repository (a lane's worktree, or main) reaches
the person's own session too, whatever that session's checkout's switch says. At its
UserPromptSubmit and SessionStart the agent is told the card's facts, opening
`familiar-ai: in the checkout <folder>, a question for the person is open …` and ending
`Give the question the header familiar-ai; the answer is recorded in the checkout
<folder>.` With no card of its own checkout open, the person's AskUserQuestion with
that header is rewritten into the other checkout's card, with the same quote check, and
its PostToolUse writes `hold_answered` into that checkout's record, never its own; the
person reads `… · in the checkout <folder>` and the agent `… It went to the checkout
<folder>.` A subagent's question, or one from a session the binary launched, is never
rewritten. A headless session (`claude -p`, `CLAUDE_CODE_ENTRYPOINT=sdk-cli`) counts as
the bench: its stop with only the person's cards left goes (the card in `systemMessage`,
no `additionalContext`, which Claude Code would take as a reason to go on), and the card
waits in its record for the person's session.

The push question goes the same way: once every criterion is green or proven here and no
pull request is open, the agent's question becomes "Every criterion is proven on this
laptop. Push and open the pull request so CI proves it?" (`yes, push and open it` ·
`not now`); the agent is told the answer, and nothing is recorded. The push begins with
`familiar-ai tidy` and follows `plugin/commands/prove.md`; then `familiar-ai status` and
the next session start say `CI is proving it`, `proven ✓` or
`CI found: <check> (see the PR)`.

**The off switch.** After `familiar-ai off` (typed in the person's own terminal, or the
band's `turn off` button) every hook allows, says nothing and writes nothing but the
person's own prompts until `familiar-ai on`; from an agent's shell (one with
`CLAUDE_CODE_SESSION_ID` set, or a session the binary launched) both are refused, and the
Bash hook refuses an agent's command whose text runs either, whatever its environment.
Each switches the checkout it runs in. A
linked worktree with no switch of its own follows main's latest (main's record, the same
project folder in main's working directory); once switched itself, its own wins. Main,
and a repository with no worktrees, are as before; a worktree whose main is not set up
is on unless switched. In a worktree the words name it: `Familiar is turned on for this
worktree. Main stays off.` (and the other three in that shape); while off, `Familiar is
turned off for this worktree, as main is.` or `…; main is on.`; on while main is off,
`status` opens `Familiar is on here; main is off.` `status --json` carries `off` (this
checkout's switch as it decides), `switch_from` (`main` or `here`) and `main_off`
(`null` in main, or with main not set up). Still done while off: another checkout's card
reaching the person's session (above), and the person's own prompts recorded.

## Signed answers

CI trusts a card's answer only when a signature it can verify stands behind it.

- `familiar-ai approve`, in a terminal of the person's own, asks the passphrase of an
  encrypted ed25519 key (`~/.ssh/familiar_ai_approve`, or `--key`) with echo off; off a
  terminal it is stdin's first line, never argv or the environment. The pane takes no
  passphrase: it shows the exact command to run.
- It signs the card (repository, part, hold, choice, run, commit, `t`) as SSHSIG in the
  namespace `familiar-ai-approval`, writes `.familiar/approvals/<t>.json` and `.sig`, and
  records `hold_answered` naming it. Commit the approvals with the plan.
- `--unsigned` records the answer with no signature (no key yet): it reads
  "answered, not verified" and CI does not accept it.
- **The verified flag.** An answer reads verified only when an approval committed at HEAD
  verifies against the allowed signers (`FAMILIAR_AI_ALLOWED_SIGNERS`: OpenSSH
  allowed_signers lines or bare public keys, one per line): in the person's own environment for `status`,
  `status --json` (each part's `answers`: `hold`, `choice`, `approval`, `verified`, `words`),
  the status line and the pane; in CI's configuration for `verify`. A file's presence is
  never enough.
- CI's check: a variable or secret mapped into the verify job, never a file in the tree.
  A `cert-authority` line, an expired key or one whose `namespaces=` leaves out
  `familiar-ai-approval` signs nothing. An approval that fails stands against its part
  ("approval not verified"); a verified one becomes the person's `hold_answered` right
  after the commit it was answered at.
- An agent's write to `.familiar/approvals/` or `.github/` is refused, by the command's text.

## Dependencies

`a_dependency` on a card that offers it asks for a new part that must be fixed first.

- On the pick the binary drafts it (the holder's model, one prompt, at most $0.50) and
  claims its lines in one batch: a criterion (`dependency_drafted` names it), a place,
  its test, the string with the asking part behind it, `criterion_assigned` to the holder.
  A plan that does not hold is `dependency_unanswered` and the `environment_error` card; two
  misses end the approval and its card opens again. While it is drafted, other
  conversations' edits, commits and stops wait; the holder is told the dependency
  (`{c_dep} was added for {c}`).
- The asking part (c) waits. When the dependency (c_dep) is green at G, c's tests-first
  moves to the re-earn commit: B, the first commit after G that touches no tests, code or
  place, if it was seen before that green was stamped, else G (`crates/familiar-ai-rules/src/dependency.rs`).
  It holds only if c_dep is green there, c's tests there are its proof, and no commit
  since the crash touched c's place. c's tests are run there: red resumes it, a crash is
  a new `crashed` card (one re-earn per approval), a pass is `miscoped`.
- `covered_by_dependency` records `criterion_covered`: the part is closed as covered.
- `verify` re-earns the same from its own runs and the verified approvals.

## Model calls

Both run `claude -p` on the person's own Claude Code login, with no settings, MCP servers
or tools loaded, and pick their model from one ladder: haiku, sonnet, opus, fable. A
**rung** is one step on that ladder.

**The judge** asks whether a sealed test answers the person's words. Any hook call asks
about the oldest unjudged seal, one per call, of the model one rung above the holder's,
never below opus (fable, unknown or off-ladder models get fable). It reads the person's
words, the agent's reading of them, and each planned test file's test functions at the
seal commit, changed ones first, cut at 8,000 characters; the person's words decide. A
another criterion's path is not a gap; a path of this criterion's words no test exercises
is. One answer by `--json-schema`, at most $0.50 and 60 s, each call logged in
`.familiar/judge.log`; `FAMILIAR_AI_JUDGE=off` turns it off.
- "yes": recorded as `judge_verdict`; the code may go ahead.
- "no" with a gap: recorded, told with its reason, and counted as `test_change_announced`;
  the next tests-only commit is a new seal, judged again.
- No verdict (an error, a timeout, an unreadable answer, or an empty gap: it could not see
  the tests): `judge_unanswered`; after two misses the seal stands unjudged, and is never
  a "no".
- `status` and a refused stop show `asked for:`, `tested by:`, `judge:` per seal.

**The reviewer at green** [draft] (`door/review.rs`) reads the fix once a part is green: at a
stop with no seal left to judge, for a part whose seal the judge covered, once per green
commit (twice at most with no verdict there). It is the judge's model and cap, with `Read`
and `Grep` in a scratch copy of the green commit (5 minutes). It reads the person's words,
the agent's reading, the criterion, the sealed test, the fix from the tests-first commit to
the green commit with every line numbered, and the code around each change (the file's head,
the definitions the change is in, the innermost whole, and its siblings' first lines; 8,000
characters per file each). It answers `{covers, gap, quotes: [{file, line, text}], reason}`.
- "yes": `fix_verdict` `covers: yes`; the stop goes ahead.
- "no": the binary reads each quote again at the green commit, through the investigator's
  check (whitespace runs as one space; up to two lines off; at least six characters).
  Every quote found: `covers: no`, and the stop is refused with the gap, the reason and the
  quotes until a later green commit is reviewed. Any quote not found, or none:
  `covers: not_judged`, `why: unverified`; recorded, never blocking.
- Two verified "no"s on one part hold it on `review_disputed` [draft] (pausing): the person
  keeps the fix (`keep_going`), asks for one more review after another fix (`try_again`),
  or releases it. It only blocks: it marks nothing done, and green is read as before.

**The investigator** takes over a criterion held on `two_failures` (valid reds only): one
rung up the ladder, then a second fable in a fresh conversation, then only the question
card. The binary launches it from the PostToolUse that records the
second failed try (the Stop as fallback) as
`claude -p --model <model> --plugin-dir <plugin> --max-budget-usd 3.00`, so the plugin's
hooks govern it too, after transferring the criterion to it. It is told what the person
asked, the criterion, the judge's verdict, the anchor's lines, the sealed test, the
command and the last two tries. Done (green), it hands
the criterion back; otherwise `investigator_unfinished` is written and the hold shows.
It is logged in `.familiar/investigator.log` and `.familiar/investigator/`, its test
output in `.familiar/runs/`; `FAMILIAR_AI_INVESTIGATOR=off` stops it and
`FAMILIAR_AI_INVESTIGATOR_BUDGET` sets its cap. An Agent spawn by the holder becomes
`familiar-ai:investigator`, one per session; the holder's edits and commits wait until
the criterion comes back. `crates/familiar-ai/src/door/judge.rs` and
`crates/familiar-ai/src/door/investigator_run.rs` have the exact arguments.

**The diagnosis** (`door/diagnosis.rs`) is the investigator's read-only work on
`two_tries_not_red`, which gets no takeover: a model one rung above the holder, never below
opus, with `Read`, `Grep` and `Glob` only, no plugin and no transfer (10 minutes, the
investigator's budget cap). It gives one category, a reason and quotes of the test, the
code or the person's words:

| category | means | card's recommended choice |
| --- | --- | --- |
| `wrong_test` | the test does not check what was asked; carries `guidance` | `try_again` |
| `behavior_exists` | the code as planned already does it; the binary runs the test at the planning commit | `release` |
| `requirement_missed` | the part leaves out something asked; carries `missed` and `scope` | `re_plan` |

The binary re-reads every quote (a person's quote holds four words or twenty characters,
a commit quote names a commit of the brief) and writes `investigator_finding` only if all
hold, else `investigator_unfinished` (unparseable, unverified, budget); two misses and the
card goes to the person bare. One finding is owed per rise of the card. It clears no hold
and relabels no run; the card shows it first, with its recommended choice first. After
`re_plan` on `requirement_missed`, plan lines 2 and 3 wait for the part's new criterion and
promise. `FAMILIAR_AI_INVESTIGATOR=off` stops it.

## What the person sees

**The band**, above the prompt, says what needs the person now and what CI said, then a
`details` button (hotkey `d`) and a `turn off` button:

    Familiar · 2 need you · CI: no run for 5ebd6ba     [ details ] [ turn off ]
    part 2 needs you · the agent will ask you here

The second line shows while a question card is open. The CI words are those of
`crates/familiar-ai/src/teach/ci.rs`, from the record only: `CI: no run for <sha>`,
`CI: stale (ran at <sha>)`, `CI: proven at <sha>, run <id>`, `CI: cancelled (run <id>)`,
`CI: timed out (run <id>)` and `CI: not proven at <sha>, run <id>`, each sha cut to
seven characters. Idle: `Familiar · idle`; switched off:
`Familiar is turned off for this project.` (in a worktree: `… for this worktree, as main
is.` or `…; main is on.`) and `turn on`; when the status cannot be
read: `Familiar · status unavailable`.

**The pane** (`/familiar` or `details`): "you said" (only the prompts that became an ask,
each once; answers and notes stay in `status --json`), each ask with its parts and where each
stands, each text whole, its hold, its holder and the judge's word, open question cards (each
with its finding and choices; a choice runs nothing, takes no passphrase and shows the
exact `familiar-ai approve` command, or the unsigned one), running test runs, the CI line; each part's holder in words (`held by: opus, conversation
cc9af915`, `held by: the investigator (fable)`, `held by: a subagent (sonnet)`). Below
it, one section per other checkout of the repository: its folder and branch, what the
person said there, each ask with its parts and who holds each; an idle one is one line,
`<folder> · idle`. It re-reads `status --json` after each tool call (at most
once a second), every 5 s while open and 30 s while idle.

**Each part's journey** is a table on top, the rail when a part is opened, so the person
can track how the tests are aligning with the criterion proving them. The table has one
line per part: its number, its stage (`planning`, `waiting`, `testing`, `red`, `green`,
`verified`, `needs you`, `held`), a dot for each of the seven steps, and where it stands,
whole (it wraps, never cut). The steps are **plan** (the criterion is in the plan),
**place** (a place in the code is chosen, or new work with no spot), **test** (its test
files are written), **red** (sealed failing at a commit), **fix** (code landed on the
place after the seal), **green** (it passes at a commit) and **CI**. A dot is `●` done,
`○` not done, `·` not reached (a part not yet in play, past its next step); CI is `◐`
when locally verified and `●` only when hosted CI-proven. A part is opened when it is the
one in play with a holder (testing, red or green) or held: below the table it shows its
name and words, the rail `plan ── place ── test ── red ── fix ── green ── CI` with its
dots, and its `test` files, its `place`, and the commits it went `red` and `green` at, who
holds it on the last line. `status --json` carries each criterion's `journey`: `steps`
(seven `{name, done, at?}`, `done` true, false, null or `"half"`, `at` a short sha),
`tests`, `places` (`{file, symbol}`) and `opened`; the binary derives every step, the
pane only draws them. A status from an older binary, with no `journey`, draws seven `·`.
A part is drawn the same wherever it lives: another checkout's section shows its ask, then
this same table and, for each opened part, this same rail and detail lines.

**The status line** is `<folder> | Familiar | ON | <ask> · <map>`:

```
my-project | Familiar | ON | your 12:36 ask: fix the greeting · 1 green · 2 needs you
```

The map names parts by number, and consecutive parts in the same state fold
(`3–9 wait`). Switched off it ends `| OFF`; with no `.familiar`, `| not set up`; outside
a repository it is empty. In a worktree the switch says whose it is: `OFF · as main`,
`ON · as main`, `ON · here`, `ON · here · main OFF`. After this checkout's own map comes
one entry per other checkout with a run open, by its short name (its folder less main's
and a dash): `· hook-words: 1 needs you 3 green 1 testing`, `needs you` first; it shows
while this checkout is off too. It writes nothing.

`status --json` carries `checkouts`: one entry per other checkout whose project is set
up, main first, each read with the same wait and listed `busy` when its record stays in
use: `folder`, `name`, `branch`, its switch (`off`, `switch_from`, `main_off`), `mode`,
`said`, `asks` (each with its `parts` as `criteria` gives them, `held_by` included),
`runs`, `cards` and `counts.needs_you`. The band counts those cards with its own and
names each `part <n> in <folder>`, under the off line too.

Each surface has one job, and there is no toast. All three read only
`familiar-ai status --json`, which reads only the record, never the network.

**Parts by number**: a person never reads a criterion id; a criterion is `part <n>`, or
`part <n> of your <h:mm> ask` once there are several asks. The parts beside the ask are
the person's check on the agent's reading of it: whether a criterion is strong enough for
what was asked is the person's call, and the only one no machine makes. `status --json` carries, per
criterion, `name`, `part` and `ask` (its time, its words whole, and the prompt it
answers), and each card and run carries its `name`. **The "you said" list** is in the
pane, in `status --said` (`· ask 1`, `· answer` or `· note` per prompt) and in
`status --json` as `said`.

**The agent's lines** begin `familiar-ai:`, name a criterion by its text and say what
comes next. The states it reads, derived from the record and never set, are `unplanned`,
`waiting`, `testing`, `sealed`, `green`, `proven` and `held`.

## How a prompt is read

The record keeps every prompt the person types in their own session as `person_said`
(task notifications, subagents and launched sessions are not the person). No model
classifies a prompt; its label is derived each time from what the agent later claims:

| label | when |
| --- | --- |
| ask | an `intent_submitted` or a `criterion` names it with `for` (an intent without one takes the latest prompt) |
| answer | a `person_said_answered --how question\|no_work` names it |
| note | neither |

"Note" is the default, not a judgment of the text; a note stays in the person's words and
can still steer the next ask. A prompt typed after the plan is confirmed holds the stop
(never an edit) until a criterion or `person_said_answered` names it.

## Tidy

`familiar-ai tidy` rewrites only the unpushed commits Familiar saw the agent make. Each
criterion ends up as one seal commit, its tests failing, and one fix commit, its tests
passing; failed attempts are dropped. Before the branch moves, tidy re-checks the seal and
every criterion's state, and on any mismatch nothing moves. The old head is kept at
`refs/familiar-ai/pre-tidy/<time>`.

## Verify

`familiar-ai verify` reads only a fresh clone and the committed `.familiar/plan`, never
the laptop's record. Per criterion it checks that the tests fail at the tests-first
commit, that the sealed tests are unchanged at the head, that they pass at the head, that
the tests the change touches still pass, and that they pass the revert check. It exits 0 when every
criterion is proven or released, 1 when any is not, and 2 when proof cannot be
established (shallow history, no committed plan, no test command, a test that could not
run); it never prints "proven" in that case. It writes `.familiar/verify-report.json`,
which the action uploads and `familiar-ai ci fetch` records as `ci_report_seen`.

- **Translations.** Every run `verify` makes is reserved (by `pipeline`), ended and
  translated as a laptop run is, with the same labels and families (see
  [Outcomes](#outcomes)), and its results name it. A pipeline result with no `run_id` after
  the first translation is unknown, never red or green; a run that did not run is neither.
- **Answers.** A card's answer counts only from a committed approval that verifies against
  CI's `FAMILIAR_AI_ALLOWED_SIGNERS` (see [Signed answers](#signed-answers)); an
  unverifiable one leaves its part "approval not verified" (exit 1). A plan line's
  `hold` and `choice` on a `place_chosen` (the person's `keep_going`) are dropped unless a
  verified approval gives that answer; a pipeline stamp in the plan is dropped.
- **Dependencies.** CI re-earns a dependency's part from its own crash at the tests-first
  commit, the dependency's green at the stamp's commit, the re-earn conditions and the
  verified approvals since (see [Dependencies](#dependencies)).
- **Readings** [draft]. `verify` runs its own dry run at HEAD per runner, uses its own
  choice, and prints one line when the committed `.familiar/adapter` differs; the exit code
  never changes. After the teeth it re-checks each seal under the setup files changed since
  it (see [Readers and the adapter](#readers-and-the-adapter-draft)). A can't-tell answer
  counts only where its own run at that commit reads can't-tell with the same label and
  deciding line.

## The seed

`familiar-ai setup` starts a fresh record from the committed plan, not empty: the plan's
lines, then `verify`'s recheck of the history (its facts, and each of its runs but the
teeth as this checkout's laptop evidence), then `record_seeded {commit, plan_lines}`, all in
one write, so a seed that does not finish leaves the record empty. The plan file stays the
committed one: the seed's walk is never mirrored into it.

- **`touched_here`.** A part is touched in this checkout when a fact after the latest
  `record_seeded` is an agent's claim about it (`criterion`, `criterion_assigned`,
  `place_chosen`, `test_planned`, ...) or a laptop result the runner wrote. With no mark
  every part is. The seed's own facts, the binary's and the person's `hold_answered` touch
  nothing. A lane's stop reads only the parts touched here.
- **Relay.** A card in another checkout reaches this session only from main (every card) or
  from an alive checkout (a live agent holds its lock) and then only on parts touched
  there.
- **Readings** [draft]. The seed runs its own dry run for each runner, as `verify` does.

**The evidence labels.** "Proven" is never written bare. Each claim says which machine
computed it:

    locally red / sealed       the hook on a laptop, at the tests-first commit
    locally green              the hook on a laptop, at the fix
    locally verified           `verify` in a fresh clone on the same machine
    hosted CI-proven at <sha>, run <id>
                               `familiar-verify` on GitHub, from the release binary

## Removing it

`familiar-ai off` pauses one project. To take Familiar out of a project for good:
uninstall the plugin (`claude plugin uninstall familiar-ai@familiar-ai`), then delete
`.familiar/`, the `familiar-ai statusline` entry in `.claude/settings.json`, the
`familiar-verify` job if you added one, and the project's marker under
`~/.local/share/familiar-ai/projects/` (or `$XDG_DATA_HOME/familiar-ai/projects/`). The
plugin goes first: while it is installed, a project whose folder is gone but whose marker
remains refuses every tool call until setup runs again.

## Terms

| term | meaning |
| --- | --- |
| ask | a request of the person's that the agent claimed as an intent; its parts are the criteria claimed under it |
| criterion, part | one testable part of an ask; the person sees it as a part, named by its number in that ask |
| light path, full path | a criterion's size: light for a bug fix or a small change, full for new behavior, which also needs plan line 3 |
| anchor | the lines of code the plan names for a criterion; its fix lands inside them |
| string | the order criteria are worked in; a criterion is its own head, or a link that names the head of the string whose code its test needs; the links take their turns in the order claimed |
| lane | the criteria one agent conversation holds; several lanes share the one record |
| stamp | the fingerprint, a hash of a criterion's test files, recorded at its seal |
| sealed | its tests are committed alone, seen failing, and stamped; they change only after an announcement |
| green | its tests pass on this laptop under the same stamp; the person reads "locally green" |
| proven | CI proved it from a fresh clone, the revert check included |
| revert check | the tests run with the anchor's lines put back as they were; they must fail |
| held | a hold stops it until a clearing move clears it, or the person answers its question card |
| question card | a held criterion's question to the person, shown in Claude Code's own picker |
| rung | one step on the model ladder haiku, sonnet, opus, fable |
| judge | the model asked at each seal whether the tests answer the person's words |
| investigator | the model the binary launches to take over a criterion after two failed tries |
| family | what a run's label means for the proof; each family has its route (see [Outcomes](#outcomes)) |
| can't tell | the reading `unclassified`: the output does not say, so the person is asked (`run_unclassified`), never a pass and never a red |
| reader | what turns a run's output into a label: Familiar's reporter, the runner's report files, a pinned console format or the regex tables [draft] |
| adapter | `.familiar/adapter`: which reader each runner is read by, chosen by a dry run and committed with the work [draft] |
| dry run | probe tests of known truth, run in a scratch copy to prove a reader in this project [draft] |
| setup part | the reserved project-level part `setup`, which carries a gap in a reading as a card [draft] |
| approval | the person's signed answer to a card, in `.familiar/approvals/`; an answer reads verified only by one that verifies |
