# familiar-ai-run

Runs the project's test command and reports `Passed` or `Failed`.

`Passed` means exit code 0 within the timeout. Anything else is `Failed`:
a non-zero exit, a signal, a timeout, a missing command or directory, or an
empty command. Output is kept for the local record and never parsed, so a
test that prints "passed" and exits 1 has failed. This keeps Familiar
language-agnostic: any test runner works if its exit code is honest.

Why it is this way: parsing output would tie familiar-ai to known runners
and their formats; the exit code works for any language, at the cost that a
runner which exits 0 on failure fools it. A command that could not start is
never read as a failure that seals a test: the binary records it as
`did_not_run`
([exit codes](../../docs/decisions.md#exit-codes-not-output)).
Every decision is in [docs/decisions.md](../../docs/decisions.md).

## Use

```rust
use std::path::Path;
use std::time::Duration;
use familiar_ai_run::{run_test_command, Verdict};

let r = run_test_command("cargo test", Path::new("."), Duration::from_secs(60));
assert!(matches!(r.verdict, Verdict::Passed | Verdict::Failed));
println!("{}", r.stdout);
```

The command runs as `sh -c <command>` in its own process group. On timeout
the whole group is killed, so the call always returns. Both pipes are
drained while it runs, so megabytes of output never block it. Each stream
keeps its first 4 MiB and its last 4 MiB (`CAPTURE_HEAD`, `CAPTURE_TAIL`);
the bytes between are counted and dropped, and the kept text says how many
(`[familiar: N bytes dropped]`).

Some runners print a failure's details only when asked, or into report
files. `RUNNER_DETAILS` has a row per runner a command can name and what
each needs (CTest an env variable, Maven and Gradle their report files).
`details_for(command)` finds the row from the planned command: leading
`NAME=value` words and wrappers (`npx`, `pnpm exec`, `yarn`, `bunx`,
`uv run`, `poetry run`, `pipenv run`) are skipped, `python -m` takes the
module, the first remaining word picks the row by its basename, and each
`&&` / `;` segment is read with the last match winning; `make test` names
none. `run_test_command_with` sets the env on the child. The command itself
is never changed.

## Tests

`cargo test -p familiar-ai-run --locked` covers pass, fail, missing command
or directory, empty command, signals, timeouts that kill children and keep
earlier output, megabytes of output, a 20 MiB output kept to its head and
tail with its dropped bytes counted, non-UTF-8 output, background processes,
a process that leaves the group, and "passed" in the output deciding
nothing.
