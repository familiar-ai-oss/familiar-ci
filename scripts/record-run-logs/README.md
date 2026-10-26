# Recording the run logs

The fixtures under `crates/familiar-ai/tests/fixtures/run_logs/<runner>/<fixture>/` are real runs
of 19 test runners (plan S0): `cmd`, `exit` (N, or -N when `sh` itself died of signal N),
`version`, `stdout`, `stderr`, and `report/` (the runner's report files, where it writes them).
The classifier's per-runner tables (`crates/familiar-ai/src/door/runner_tables.rs`) are tested
against every one of them (`crates/familiar-ai/tests/classify_run_logs.rs`).

To re-record: lay these files out as a scratch `run_logs/` tree (`capture.py` at
`run_logs/_bin/capture.py`, `_gen/` at `run_logs/_gen/`, `record-all.sh` at `run_logs/`), set
`R` in `_gen/lib.sh` to that tree, run `sh _gen/<runner>.sh` to write a runner's fixture projects
and record them (or `./record-all.sh [runner…]` to re-record what is there; its header lists the
toolchains and docker images each runner needs), then copy each fixture's `cmd`, `exit`,
`version`, `stdout`, `stderr` and `report/` back here; never the project sources or `.cache`.

The one exception (S9b): `crates/familiar-ai/tests/fixtures/run_sources/<runner>/<fixture>/` holds
the source files (by extension, no build, cache, `vendor` or `node_modules` folder) of every fixture
whose name starts `tests_failed_exception`, `missing_target_symbol`, `collection_error` or
`killed_`, as the recorder wrote them: `tests/location_per_runner.rs` reads them in place of git's
files at the run's commit, to check each location and the function above it.
