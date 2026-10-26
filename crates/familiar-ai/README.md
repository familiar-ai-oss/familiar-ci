# familiar-ai (the binary)

The program behind the hooks. Claude Code runs it on every event, the agent
runs it to claim a line of the plan, and the person runs it for `status`,
`verify` and the switch.

It exists because a hook has one call to decide whether an edit, a commit or
a stop goes through, and that decision needs git, the record and the
project's test run. Logic in the shell scripts would be untestable and
duplicated, and logic in the TypeScript mod only exists where Claude Code's
interface does, so one program decides and the scripts and the mod relay its
answer ([the relay](../../docs/decisions.md#every-event-goes-through-the-binary)).

Each hook call does four things:

1. Open `.familiar/record` and append the commits that landed since the
   last call, with each criterion's test fingerprint.
2. After a commit, run the project's test command and record the exit
   code. The output is never read, so any language and runner works
   ([exit codes](../../docs/decisions.md#exit-codes-not-output)).
3. Hand the log to the rules crate. The rules derive every state and the
   binary derives none, so any past state can be recomputed
   ([pure rules](../../docs/decisions.md#pure-rules-in-their-own-crate)).
4. Answer in one line: allow, or refuse and name the claim that clears it.

The result: every requirement has a test that was red before the code and
is green after it, and `verify` proves it again in CI from a clean clone.
The reasoning behind each rule is in [docs/decisions.md](../../docs/decisions.md).

## Commands

All of them are in [docs/reference.md](../../docs/reference.md#commands).
The first claim in a project that has just run `setup`:

    $ familiar-ai claim intent_submitted --text "let a person log in and reset a forgotten password"
    familiar-ai: claimed {"t":0,...,"kind":"intent_submitted",...}
    familiar-ai: the plan is still being written
    familiar-ai: the plan comes first · next: write the criterion
      claim it: familiar-ai claim criterion --criterion_id c1 --text 'greet says hello' --path light

## Prompts

Every prompt the person types in their own session is a `person_said`
fact. The first one is also the `intent_submitted`. Prompts from a task
notification, a subagent or a session the binary launched are not recorded.

Once the plan is confirmed, a new prompt holds the stop until the plan
accounts for it: a criterion claimed with `--for <t>`, or a
`person_said_answered`. A request cannot be dropped on the floor. Whether a
prompt was an ask, an answer or a note is read from those facts, not
guessed by a model
([why](../../docs/decisions.md#asks-answers-and-notes)).

The person never sees a criterion id. A criterion is "part 2", or "part 2
of your 12:36 ask" when there is more than one ask. `status --json` carries
that as `name`, `part` and `ask` on each criterion, card and run.

## Cards

Some holds need a person: the anchor moved, two tries failed, the fix is
elsewhere, there is no test stack. Each is a card. The agent asks with
Claude Code's AskUserQuestion tool, the hook rewrites that call into the
card with the choices the binary fixed, and the person's pick becomes
`hold_answered`. The agent cannot claim `hold_answered`. A person at a
terminal can type the claim the card shows
([why](../../docs/decisions.md#holds-are-the-persons)).

## Off

`familiar-ai off` from the person's terminal appends
`familiar_switch {to: off}`. Every hook then allows, says nothing and
writes nothing, and the session starts with one line instead of the
rulebook. `familiar-ai on` turns it back. Both are refused from an agent's
shell, which is any shell with `CLAUDE_CODE_SESSION_ID` set or a session
the binary launched ([why](../../docs/decisions.md#the-off-switch)).

## The judge and the investigator

Two calls to `claude -p` on the person's login.

- At the seal, a judge one model up from the agent's, never below Opus, is
  asked whether the tests answer the person's words. While the verdict is
  pending or "no", code for that criterion is refused
  ([why](../../docs/decisions.md#a-judge-one-rung-up)).
- After two failed tries at the fix, an investigator one model up takes
  the criterion over. Two investigators without a green and it becomes a
  card.

Models, caps, logs and the off switches for each are in
[docs/reference.md](../../docs/reference.md#model-calls).

## Failing closed

Bad hook input, an unknown event or command, and a record that cannot be
read all exit 2, which Claude Code treats as a refusal. A guard that turns
off when it breaks is not a guard
([why](../../docs/decisions.md#the-hook-fails-closed)).

## Tests

`tests/` drives the real binary against a real git repository and a real
record. Files are named subject first, then behaviour (`hook_fails_closed`,
`walk_criterion_lifecycle`), and each test function's name is its claim.
Unit tests sit beside the code. The referee in
[CONTRIBUTING.md](../../CONTRIBUTING.md) must be green before a change lands.
