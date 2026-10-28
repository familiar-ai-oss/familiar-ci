# Contributing

The referee that must be green, the two-commit shape of a change, the rules
a change must keep, and how a release is cut.

Change one thing at a time, and keep the referee green before and after. If
a change would move a rule's decision, stop and say so instead of landing
it. The reasons behind the rules below are in
[docs/decisions.md](docs/decisions.md).

## The referee

Every one of these must pass, from a clean build:

    cargo clean
    PATH=$PWD/target/debug:$PATH cargo test --locked --workspace --no-fail-fast
    cargo clippy --workspace --all-targets --locked -- -D warnings
    scripts/check-no-c.sh

    # the real binary through the real hook protocol
    FAMILIAR_AI_JUDGE=off FAMILIAR_AI_INVESTIGATOR=off sh plugin/tests/test-run.sh
    FAMILIAR_AI_JUDGE=off FAMILIAR_AI_INVESTIGATOR=off sh plugin/tests/test-agent.sh
    FAMILIAR_AI_JUDGE=off FAMILIAR_AI_INVESTIGATOR=off sh plugin/tests/test-ci.sh

    check                         what it covers
    ───────────────────────────── ─────────────────────────────────────────
    cargo test --workspace        the crates
    cargo clippy -D warnings      no warnings
    scripts/check-no-c.sh         no C in any dependency, on 4 targets
    plugin/tests/test-run.sh      the real binary runs a test and reads its
                                  result through the hook
    plugin/tests/test-agent.sh    a scripted agent through the real hooks:
                                  plan, seal, fix, holds, cards
    plugin/tests/test-ci.sh       a project walked to green, then `verify`
                                  in a fresh clone exits 0; with the fix
                                  undone, 1

`PATH` carries `target/debug` so the tests that run the real `familiar-ai`
find the binary you just built. Each walk ends `ALL PASS` or names the
first check that failed. The judge and
the investigator are off because they call a model on your login and cost
money; their own tests fake the model.

CI (`.github/workflows/ci.yml`) runs the same steps on every push. A change
merges only with all of it green and no intentional red outstanding: a
sealed, failing test that belongs to work still in flight is not a pass.

A change to the rules crate is also tested against an exhaustive reference
before release. That reference is not in this repository, so a maintainer
runs it; say in your pull request that your change touches the rules.

## Tests first, and the commit shape

Write the test first and watch it fail. A change lands as two commits:

1. `tests: …` holds the tests alone, no code, and they fail (red).
2. `fix: …` holds the code, and the same tests pass.

Never mix the two in one commit, and never edit a test after it is sealed
without saying why in the commit. Name integration tests after the
behaviour they check. familiar-ai's own `verify` holds this repository to
the same rule, from its committed `.familiar/plan`. Why the order matters:
a test that never failed proves nothing about the change
([tests first](docs/decisions.md#red-green-revert)).

## When a definition changes

A test that encodes a definition is updated when the definition changes,
never loosened to pass. The update goes in the `tests:` commit, and the
commit message names each changed assertion as a definition change, with
the file and line. A wrong expectation that was never the definition is
named as that instead. A reader of the history must be able to tell a
definition change from a weakened test
([why](docs/decisions.md#definition-changes-are-named)).

## Signing your commits

The maintainers end their own commits with a `Co-Authored-By:` trailer for
Familiar AI LLC; that is their convention, and an outside contributor signs
commits as themselves.

## The rules

- **One concern per crate.** `facts` holds the fact kinds and the record,
  `git` the git reads, `plan` the plan file, `rules` the states and holds,
  and `run` the test command. The binary holds the hook and the commands. It
  calls the others and never derives a state itself. A crate doing two jobs
  should become two crates.
- **Facts in, states derived.** Nothing stores a state. Every state, hold
  and line of teaching is read from the log when needed. No field is named
  `state` or `status`. Nothing reads a clock; time is `at`, passed in.
- **Short files and short functions.** Aim for files under 500 lines and
  functions under 60, except a flat table; no script enforces it, and a
  few files run longer. Split a long function into named steps, and do not
  make a long file longer.
- **No C.** Git is read through `gix` and never through `git2`; the `git`
  crate never runs the `git` program. The binary runs it for exactly two
  writes, the commit `setup` makes and the rewrite `tidy` makes
  (`crates/familiar-ai/src/door/git_program.rs`). `scripts/check-no-c.sh`
  enforces the rest.
- **Fail closed.** A hook that gets bad input or cannot read the record
  refuses. It never lets the action through and never panics.
- **The words are pinned.** Tests check the refusal, card and status
  strings, so they are behaviour, not prose. Change one only on purpose,
  together with its tests. Words the person reads, and the names of fact
  kinds, states, holds, fields and commands, are the maintainers' to
  choose: propose a new one in an issue before you build on it
  ([why](docs/decisions.md#which-kinds-the-agent-may-write)).

## Adding a fact kind

1. Add the struct to `familiar-ai-facts`, in the file under `src/forms/`
   for its area. Register it in `form_table.rs` and `KINDS`.
2. Keep it flat: `t`, `at`, `kind`, `tenant`, then its fields, nothing
   nested. The typed `append` and `append_raw` must write the same bytes.
3. Add a sample to `samples()` in `tests/common/mod.rs`, with a round trip, a
   refusal and a reader test.
4. If the binary writes the kind itself, add it to `BINARY_KINDS` in
   `crates/familiar-ai/src/door/claim.rs`, so no claim can write it.
5. If the kind changes a state or a hold, it is a rules change: tests
   first in the rules crate, and say so in the pull request.

## Releasing

A maintainer cuts a release. The version is the workspace's, in
`Cargo.toml`; `plugin/.claude-plugin/plugin.json`,
`plugin/skills/setup/SKILL.md` and `action/examples/familiar-verify.yml`
carry the same version and change with it. Push the tag `v<version>`:
`.github/workflows/release.yml` refuses a tag that differs from the crate's
version, builds the binary for the four targets
(`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
`aarch64-apple-darwin`, `x86_64-apple-darwin`), runs
`scripts/check-no-c.sh`, runs `.github/workflows/ci.yml` on the tag, and
only once the builds and CI pass publishes each
`familiar-ai-<target>.tar.gz` with its `.sha256` as the GitHub release. A
red CI on the tag publishes nothing. `/familiar-ai:setup` downloads
from that release, and the action installs from it. Then publish the
crates to crates.io in dependency order (`familiar-ai-facts`,
`familiar-ai-git`, `familiar-ai-run`, `familiar-ai-rules`,
`familiar-ai-plan`, then `familiar-ai`), which is what
`cargo install familiar-ai` builds from.

## Documentation

Docs explain why; the code shows how. A doc carries the intent, the
constraints and the options turned down, and does not narrate what a
function does.

- **Record a decision where it is made.** A choice that shapes the product
  gets a block in [docs/decisions.md](docs/decisions.md): what was
  decided, what it turned down and what that cost, why, where it lives,
  and what proves it. Link to that block from the README the choice
  touches, rather than restating it.
- **Verify every claim against the code at this commit.** Every command,
  flag, kind name, file path and number in a doc must exist in the tree.
  `grep` for it before you write it. No invented commands or flags.
- **Keep the forms apart.** The README is a front door: what it is, the
  fast start, then links. Reference, how-to and explanation stay separate.
- **Plain words.** Short sentences, a technical word explained the first
  time, no emojis, no marketing. Diagrams are Unicode box text no wider
  than 72 columns.
- **Docs change with the code**, in the same pull request. A doc that
  describes something that no longer exists is a bug.
