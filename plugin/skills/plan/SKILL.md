---
name: plan
description: What to do when familiar-ai refuses or tells you something: the plan in one `familiar-ai plan` call (every planning refusal shows its eleven lines), the planned test you are writing, a sealed test that must change, a judge's no, tests that did not go red, a failed try, an edit outside the anchor, a card (anchor_moved, two_failures, two_tries_not_red, fix_elsewhere, anchor_untouched, no_test_stack, cases_unconfirmed) the person answers through AskUserQuestion, a refused stop. Use when a hook refuses an edit, commit or stop, when a hook speaks before an edit or after a commit, when a card is open, or before the first edit in a project with a .familiar folder.
---

# When familiar-ai refuses, or tells you something

The rulebook is in your context from SessionStart, and again after compaction and resume. Every line
beginning `familiar-ai:` is the binary's fact: what holds, and what comes next. This is what to do at
the moments the rulebook does not spell out.

## 1. The plan, in one call
`familiar-ai plan` takes the lines as a JSON array on stdin (a heredoc); with nothing on stdin it prints the array's
form, right for a code anchor (`"path":"full"`; `"light"` is for a documentation anchor only). Fill your own values
and choose the fix's `"category"`: this_repository, dependency, environment or request (empty is refused). The status
comes back with it. It stops at the first refusal (`familiar-ai: line N of M, <kind>: ...`); the lines before it stay.
One criterion per part of what the person asked for; several may name the same prompt with `--for`. The person checks
the parts. A promise names its `cases`, a test function each: `normal`, the behavior asked for; `opposite`, the same
call with the trigger absent (or `opposite_none`: why none); `edge`, a boundary the person names. Classify each site
the binary lists (`place_classified`): `applies`, with its own test, or `not_affected`, with a reason. Asked how the
words read (`reading_unclear`): `one_reading` and why, or `reading_alternatives` for the person to pick.

## 2. The fields
A value that reads as JSON (`10`, `true`, `[...]`) is JSON; anything else is text. The fields of one
claim may instead be one JSON object on stdin. `project_root`: `.` is the whole project, `src/` only the
anchors under it; `codebase_id` left out is the folder's name. `familiar-ai claim <kind>` with no fields
prints that kind's form and one complete example; copy a line only when every value in it is yours.

## 3. Refused while planning
A planning refusal, at the first edit too, shows the eleven lines as a checklist: `✓` and the value for a line that
holds; for each line not complete, the exact `claim it:` command with what the record knows filled in. Claim them in
order with your own values, or all at once with `familiar-ai plan`; nothing is edited until the plan is confirmed.

## 4. A sealed test must change
Announce it first: `familiar-ai claim test_change_announced --criterion_id c1 --reason 'the assertion
named the wrong file'`. The refusal carries the criterion's "asked for", "the agent's reading" and the
judge's word, so the test is updated to the person's words, never loosened. A judge's "no" is that
announcement, with the judge's reason as its own. The changed test is committed alone; it must fail first.

## 5. A card is the person's
The rulebook says it: ask the person with AskUserQuestion; the hook turns that call into the question
card and records their pick; never claim hold_answered yourself. For no_test_stack, detect the stack
yourself (`test_stack_detected`); never wait on it. The cases card (`cases_unconfirmed`): write its lines
word for word, then claim the promise again with the cases the person asked for (the hook names them).

## 6. Subagents, and green everywhere
A whole full-path promise owes the case-finder before its test: spawn the Agent tool; it runs as
familiar-ai:case-finder on the binary's brief, and its answer opens the cases card. On two_failures a
spawn by the holder runs as familiar-ai:investigator, one rung up; the criterion moves to it, the stop
waits for green, and green sends it back. When every criterion is green the stop's facts say whether the
branch is pushed; ask the person, and on a yes push and open the pull request as `plugin/commands/prove.md` says.

## 7. The test you are writing
Before an edit of a planned test, the hook names the criterion it proves, with "asked for" (the person's words) and
"the agent's reading", which decide what the test covers. It must fail on the code as it is, committed alone, before
any fix; one that passed did not go red (the hook says so at that commit): write one that fails now. Reverting a fix
to get red does not count. Under pytest the test comes before the place: write it, run `familiar-ai try` (nothing
committed), and pick the place from the functions the failing test ran, best match first; off it, `off_list_reason`.

## 8. A try failed
After a code commit whose tests still fail, the hook says `try 1 of 2`, the place the code goes, and the last lines
of the run. Read them; the code goes on the anchor and the test stays as it is. Two in a row hold the criterion.

## 9. Outside the anchor
An edit outside the place the plan named goes ahead, and the hook names the place. If the fix belongs elsewhere:
ask the person (AskUserQuestion, a plain question), and on a yes claim the new place, said first, with the
`place_chosen` line the hook gives; the proof of that criterion starts over there. Never claim place_chosen to get past a hold.

A lane starts with `familiar-ai launch_lane --brief <file>`, never `claude -p`. Builders' notes (the
kinds the binary writes, the printed words, the judge and the investigator): `docs/reference.md`.
