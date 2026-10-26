# Architecture

The shape of the system: the crates, one hook call, a criterion's states
and holds, and the proof chain, each linked to the decision behind it.

One rule everywhere: **facts in, states derived.** The only stored thing is
an append-only log of facts. Every state, every hold, every line shown to the
agent or the person is computed from that log when it is needed. Nothing is
stored as a `state` or `status` field, so nothing can go stale. Nothing reads
a clock either: time is `at`, passed in by the caller, so the same log always
gives the same answer.

This file is the shape. Each shape choice links to its block in
[docs/decisions.md](docs/decisions.md), which says what was turned down,
what it cost, and why.

    the person ──▶ Claude Code ──▶ plugin/hooks ──▶ familiar-ai (binary)
                        ▲                               │
                        │ allow · deny · block          ▼
                        └───────────────────── .familiar/record
                                                        │
    the band, the pane ◀── status --json ◀──────────────┤
                                                        ▼
    pull request ◀── CI: familiar-ai verify ◀── .familiar/plan

## The crates

    ┌──────────────────────────────────────────────┐
    │            familiar-ai  (the binary)         │
    │       the hook and the plain commands        │
    └──────────────────────────────────────────────┘
         │            │            │            │
         ▼            ▼            ▼            ▼
     ┌───────┐    ┌───────┐    ┌───────┐    ┌───────┐
     │  run  │    │ plan  │    │ rules │    │  git  │
     └───────┘    └───────┘    └───────┘    └───────┘
                      │            │            │
                      ▼            ▼            ▼
                 ┌──────────────────────────────────┐
                 │        familiar-ai-facts         │
                 │  the fact kinds and the record   │
                 └──────────────────────────────────┘

    facts  the fact kinds (KINDS), their forms, the record on redb
    rules  pure functions: planning, states, holds, hook decisions
    git    git reads through gix; never runs the git program
    plan   reads and writes .familiar/plan (claims, never output)
    run    runs the test command; anything but exit 0 is failed

Edges point one way. No crate depends on the binary, and `rules` depends on
`facts` alone, so every decision can be tested without a disk or a repo
([the rules are pure](docs/decisions.md#pure-rules-in-their-own-crate)).

The `git` crate never runs the git program. The binary runs it for exactly
two writes, the commit `setup` makes and the rewrite `tidy` makes, so that
a commit familiar-ai makes carries your hooks and signature
([two writes](docs/decisions.md#git-reads-in-pure-rust-two-writes-through-git)).
Every read the rules see goes through `gix`.

## A hook call

    Claude Code fires an event: one JSON object on stdin
         │
         ▼
    ┌───────────────────────────────────────────────────┐
    │ input   parse the event                           │
    │         bad input: refuse, exit 2 (fail closed)   │
    └───────────────────────────────────────────────────┘
         │
         ▼
    ┌───────────────────────────────────────────────────┐
    │ door    open .familiar/record                     │
    │         switched off: allow, say nothing, stop    │
    │         append the commits that have landed       │
    │         append the facts a commit owes            │
    │         check the anchors that are due a check    │
    └───────────────────────────────────────────────────┘
         │  the log, now current
         ▼
    ┌───────────────────────────────────────────────────┐
    │ decide  rules::decide(log, event, now)            │
    └───────────────────────────────────────────────────┘
         │  a decision
         ▼
    ┌───────────────────────────────────────────────────┐
    │ teach   put the decision in plain words           │
    └───────────────────────────────────────────────────┘
         │
         ▼
    JSON on stdout: allow, deny with a reason, or block.
    systemMessage is for the person; the rest is for the agent.

Each box is a folder of `crates/familiar-ai/src/`. `input` parses what
Claude Code sent; `door` is everything that writes to the record or the
repository, and the part that touches the disk, git, test runs and
models; `decide` asks the rules crate; `teach` puts the answer in words.
A topic that crosses layers keeps one file per folder under the same name:
`decide/investigator.rs` rewrites the spawn, `door/investigator.rs` writes
its facts, `teach/investigator.rs` words it.
The binary is the one place that decides, and Claude Code enforces what it
answers
([the relay](docs/decisions.md#every-event-goes-through-the-binary)).

A hook that gets bad input or cannot read the record refuses rather than
let the action through ([fail closed](docs/decisions.md#the-hook-fails-closed)). In Claude
Code's plan mode, and while the person has run `familiar-ai off`, every
hook allows and says nothing
([plan mode](docs/decisions.md#plan-mode-is-never-blocked),
[the off switch](docs/decisions.md#the-off-switch)).

The rules crate's own words are fixed and tested. The binary re-words them
in `teach/`: for the agent, every line begins `familiar-ai:` and names a
criterion by its text; for the person, a criterion is a part of their ask,
never an id.
Session start gives the agent the rulebook once, and every refusal names
the missing line, its command and its form
([the rulebook](docs/decisions.md#a-short-rulebook-and-every-refusal-names-its-way-out)).

## A criterion's states

Each state is a predicate over the log. They are checked in a fixed order and
the first true one wins, so exactly one is ever true
([seven states](docs/decisions.md#seven-states-in-a-fixed-order)).

    unplanned ──▶ waiting ──▶ testing ──▶ sealed ──▶ green ──▶ proven

    held       any hold below is true; it beats every other state
    proven     sealed, green and the revert check passed where CI ran
    green      its sealed tests pass here at a later commit
    sealed     its tests failed first and their fingerprint is stamped
               (its own, over its own files: one tests_observed per commit)
    testing    its turn, no seal yet
    waiting    planned, but the criterion before it is not yet green
    unplanned  the plan is not confirmed

Criteria are strung in order. A criterion's turn comes when the one before
it is green, proven or released, and one is strung behind another only when
its test cannot be set up without the other's code
([strung](docs/decisions.md#strings-only-where-a-test-needs-anothers-code)).
Green means the sealed tests pass unchanged, under the same stamp (the
fingerprint taken at the seal), at a later commit
([green](docs/decisions.md#red-green-revert)).

## The holds

A criterion is held while any of these is true. Each names the move that
clears it. The order is the order in `crates/familiar-ai-rules/src/holds.rs`.

    released              the person let it go, for good
    planning_incomplete   plan lines are missing
    anchor_not_found      the planned lines were not found
    no_test_stack         no test command covers its code
    tests_not_red         its tests passed at the tests-first commit
    not_stamped           code on its anchor before its tests were sealed
    tests_and_code_mixed  tests and code in one commit
    proof_tampered        a sealed test changed and was not seen red again
    command_changed       the test command changed since the seal
    anchor_untouched      green without touching its anchor        *
    regression            a touching test that passed now fails
    anchor_moved          its lines changed after planning         *
    two_failures          two tries to go green failed             *
    two_tries_not_red     two tries to go red failed               *
    no_teeth              its tests pass with the change taken out
    fix_elsewhere         the fix is claimed to live outside       *
                          this repository

    * pausing: the criterion's commits stop until it is cleared

`two_failures` and `two_tries_not_red` go to an investigator one step up
the model ladder (haiku, sonnet, opus, fable), at most twice per criterion, and otherwise to the person as a card
([the investigator](docs/decisions.md#two-failures-send-an-investigator)).
An investigator that ends without handing the criterion back writes
`investigator_unfinished`, and the hold shows again.

A card is the person's. When one is open the agent asks the person with
Claude Code's question tool; the hook turns that call into the question
card and writes `hold_answered` only from the person's pick
([the person's holds](docs/decisions.md#holds-are-the-persons)).

## Lanes

Several criteria can be worked at once, each by its own agent conversation,
because nothing in the record is shared except the one log. A lane is the
set of criteria one conversation holds.

- **One record.** There is one append-only log for the project; a lane is
  not a second record
  ([one record](docs/decisions.md#one-record-per-project)).
- **A criterion per conversation.** `criterion_assigned` names the
  conversation that holds a criterion (the main conversation's
  `CLAUDE_CODE_SESSION_ID`, or `<session_id>/<agent_id>` for a subagent).
  `criterion_transfer` hands it on: the investigator takes a held criterion
  over and gives it back when its tests are green.
- **Runnable criteria.** Each criterion carries its own test command, and
  the binary runs it on each commit. A run is facts, `run_reserved`,
  `run_started`, `run_ended` and `run_cancel_requested`: one reservation per
  work item, decided inside the write transaction, so two writers racing
  cannot both win. The run is detached and writes its own result
  ([detached runs](docs/decisions.md#runs)).
  `familiar-ai run` catches up and lists the runs; `familiar-ai cancel`
  ends a run's process group, and a cancelled run is no result.
- **A commit is charged only to the criterion it touches.** A commit belongs
  to the criterion whose anchor it overlaps (or whose new-work file it
  names). It is never charged to another criterion, and another criterion's red
  is not read as this criterion's.
- **The derived command, and the unscoped rule.** When a criterion claims no
  command of its own, the binary derives one from the stack's runner and the
  criterion's own test files. The table is `runner_form` in
  `crates/familiar-ai-rules/src/criterion.rs`: pytest and jest append the
  files, vitest appends `run` and the files, cargo appends `--test <name>`
  for each file under a `tests/` folder, go appends the files' folders. An
  explicit command wins. A runner not in the table, or a file it cannot
  express, leaves the criterion **unscoped**: `status` ends its line with
  `unscoped · scope the command to prove in parallel`, and its seal does not
  stand while another criterion of the same stack has an outstanding red
  ([the command](docs/decisions.md#the-command-is-claimed-derived-or-unscoped)).
- **Holds are per criterion.** A hold on one criterion pauses that
  criterion's commits and no other's. A held criterion does not stop any
  other criterion, and a card is about one criterion, named by its words.

## The proof chain

Thirteen links from the laptop to the check on the pull request. Each is
recomputable from git and the committed plan alone.

1. The tests were committed before the code, alone.
2. Those tests failed at that commit.
3. The tests were not changed after they were sealed.
4. The tests bite: they fail with the change taken back out.
5. The change landed where the plan said it would.
6. The tests pass at the commit being shown.
7. The red and the green belong to this criterion, not another criterion.
8. The tests are about what the person actually asked.
9. Nothing in the run was fabricated.
10. The record is facts only, and every state is derived.
11. A neutral machine recomputes all of it from git and the plan.
12. The green check on the pull request is that computation.
13. The history a reviewer reads is honest.

How each is established: 1 by refusing a commit that mixes tests and code
(`tests_and_code_mixed`)
([tests first](docs/decisions.md#red-green-revert)); 2 by running the command in a real checkout of the
tests-first commit, where a command that could not run is never a red; 3 by
a fingerprint of the test files at the red run, with an edit refused unless
announced; 4 by `verify` putting the anchor's lines back to their pre-fix
text in a scratch copy and running the command again
([the revert check](docs/decisions.md#red-green-revert)); 5 by the anchor's
hash at the planning sha; 6 by running the command at every tip; 7 by each
criterion's own command and the touch rule above; 8 by the judge, a model
one step up the ladder; 9 by one reservation per run; 10 by the facts' closed forms and
the pure rules crate; 11 by `familiar-ai verify` in a fresh clone, exit 0, 1
or 2; 12 by the `familiar-verify` job; 13 by `tidy`.

## The plugin and the TUI

`plugin/hooks/hooks.json` sends every Claude Code event to
`plugin/hooks/run.sh`, which hands it to the binary; the shell decides
nothing ([the relay](docs/decisions.md#every-event-goes-through-the-binary)).
The `plan` and `setup` skills and the `/familiar-ai:setup` and
`/familiar-ai:prove` commands sit beside them.
[docs/reference.md](docs/reference.md) has every command, hook and surface
in full.

`plugin/hooks/band.tsx` and `pane.tsx` draw the band above the prompt, the
hint and the details pane from `familiar-ai status --json` alone. That command reads
only the record, never the network, and carries everything the mod shows:
names, parts, the ask, counts, runs, choices and `off`. The mod counts
nothing
([one source](docs/decisions.md#one-source-for-the-tui)).
Each surface has one job: the band what needs you and CI, the status line
your ask and a map of its parts, the pane every text whole; there is no
toast.

## CI

`familiar-ai verify` runs in a fresh clone and reads only the clone and the
committed `.familiar/plan`. It exits 0 when every criterion is proven or
released, 1 when any is not, and 2 when proof cannot be established. The
`familiar-verify` job in [action/](action/README.md) runs it on the pull
request. `verify` also writes a JSON report that CI uploads, and
`familiar-ai ci fetch` appends what the run said as `ci_report_seen`, so
`status` reads CI from the record
([CI's word is a fact](docs/decisions.md#ci-proves-the-laptop-guides)).
The laptop's checks guide; the proof is CI's
([the proof is CI's](docs/decisions.md#ci-proves-the-laptop-guides)).
