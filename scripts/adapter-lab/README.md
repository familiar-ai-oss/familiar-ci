# adapter lab [draft]

Hand-made projects, one per setup the plugin research found in the wild, walked through a
given `familiar-ai` binary the way an agent walks them. It prints, per project: the adapter's
choice and gaps, whether the red sealed honestly, whether the green went green, and any card
nobody expected. It changes no product code: everything is built in a scratch folder.

It runs against any binary. Against a binary without the adapter (today's main) every reading
goes through the tables and the output is the **before**; run it again at the milestone with
`--label after` and diff the two summaries.

    sh scripts/adapter-lab/run-lab.sh --bin <familiar-ai> --label before            # all
    sh scripts/adapter-lab/run-lab.sh --bin <familiar-ai> 'pytest-*' go-goflags-json  # some
    sh scripts/adapter-lab/run-lab.sh --list                                         # what each is for
    diff <lab>/out/before/summary.txt <lab>/out/after/summary.txt

`--truth` checks the projects themselves, without familiar-ai: the tests-first tree must fail and the
fixed tree must pass (the attacks are skipped: their exit codes lie by design). Run it after adding or
changing a project.

Needs `python3` and `git`; each runner's own toolchain is built on first use into a cache
(a pip venv, an npm folder, a composer vendor folder) and kept. A runner that is not installed
here runs from its docker image through a shim, so the claimed command stays the project's own
(`go test ./...`); no docker and no tool means that project is skipped, and says why.
`ADAPTER_LAB_DOCKER=0` never uses docker. The lab folder defaults to
`~/.cache/familiar-ai-adapter-lab` (`--dir`, `ADAPTER_LAB_DIR`); `/tmp` is not used because a
shared `/tmp` can run out of quota mid-lab.

## One project's walk

    ┌─────────────────────────────────────────────────────────────────┐
    │ build   common/<kind>/base + projects/<name>/base, git init,    │
    │         familiar-ai setup, the base commit                      │
    └───────────────────────────────┬─────────────────────────────────┘
                                    ▼
    plan      intent · behavior · promise · criterion · place · tests ·
              stack and command (the project's own `test_command`)
                                    ▼
    stage 1   red layer ─ commit ─ PostToolUse hook ─ wait for the run
              ─ `status --json` ──▶ sealed | asks (a card) | refused
                                    ▼  only after a seal
    stage 2   fix layer ─ commit ─ PostToolUse hook ─ wait for the run
              ─ `status --json` ──▶ green | asks | refused
                                    ▼
    adapter   `.familiar/adapter` (when the binary writes one) or `-`

Every claim and hook goes through `plugin/hooks/run.sh` and `familiar-ai claim`, as the walks in
`plugin/tests/` do, under `env -i` (a lane session's variables must not reach the hooks).
Per project the log is `<lab>/out/<label>/<project>/log.txt`, with each `status --json`.

## Reading the table

    project       adapter (gaps)                 red: want / got  green: want / got  cards  verdict
    pytest-plain  tables (want pytest:reporter)  sealed / sealed  green / green      -      ok

- **adapter**: the reader `.familiar/adapter` names for the runner and its gaps; `tables` when
  the binary has no adapter (or wrote none); `(want X)` is the project's `EXPECT_READER`.
- **want / got**: `sealed`/`green` the true outcome; `not_sealed`/`not_green` anything but a
  seal or green (a refusal or a card); `asks` a card. Got is `sealed`, `green`, `asks` (a card is
  open), `refused` (a hold with no card), or `none:<state>`.
- **verdict**: `ok`; `WRONG` a seal or green that is not true (false security); `FALSE-ALARM` a
  true red or green refused; `CARD` a card where none was expected; `DIFF` any other difference.
- **cards**: every card seen; `!` marks one the project did not list in `EXPECT_CARDS`.

What is wanted is what an honest, calibrated adapter should say, so a row that differs today is
the work left. Stage 2 runs only after a seal.

## The baseline

`baseline/before-8c2f45e.txt` is this lab against main 8c2f45e, which has no adapter, so every
reading went through the tables. Of the 24 projects:

- 10 are as wanted: the seven plain pytest variants (pretty, `-n2`, rerunfailures, `-qq`,
  `--tb=short`, `python -I`), `node --test` on Node 22, `vitest related`, and a skipped planned test
  (it asks).
- 3 are **WRONG**, all false security: a typo in the planned test under `--tb=short` seals, a
  conftest's forged red seals, and a conftest added with the fix hides the failure and goes green.
- 8 are **CARD**: a setup the tables cannot read asks on the honest red (dotnet `de-DE`, go
  `GOFLAGS=-json`, jest-junit only, Maven `printSummary=false`, mocha json, nextest
  `status-level none`, PHPUnit compact) or on the green (`| tparse`).
- 2 are **FALSE-ALARM**: Node 24's spec reporter and mocha-junit-reporter's crash get an honest red
  refused as a broken test or command.
- 1 is **DIFF**: a conftest that hides a real failure is refused as "tests not red", not asked.

## The projects

| project | setup | wanted |
|---|---|---|
| pytest-plain | plain pytest | seal, green (`pytest:reporter`) |
| pytest-pretty | pytest-pretty's Results block | seal, green |
| pytest-xdist | `-n2` | seal, green |
| pytest-rerunfailures | `--reruns 2` | seal, green |
| pytest-qq | `-qq` | seal, green |
| pytest-python-isolated | `python -I -m pytest`: isolated mode ignores `PYTHONPATH` | seal, green (any reader but the reporter) |
| pytest-tbshort-addopts | `addopts = --tb=short` | seal, green |
| pytest-tbshort-typo | `--tb=short`, a typo in the planned test | no seal (a broken test) |
| pytest-conftest-flip-hides-red | a conftest turns a real failure into a pass | asks |
| pytest-conftest-flip-fake-red | the same conftest over a test that passes: a forged red | asks, no seal |
| pytest-conftest-skip-planned | the fix commit adds a conftest skipping the planned test | seal, then asks |
| pytest-conftest-flip-after-seal | the fix commit adds a flipping conftest, code unfixed | seal, then asks |
| jest-junit-only | `reporters: ["jest-junit"]` only | seal, green (`jest:reporter`) |
| vitest-related | `vitest related src/app.js --run` | seal, green |
| node-test-tap | `node --test` on Node 22 (tap piped) | seal, green |
| node-test-24-spec | `node --test` on Node 24 (spec, docker) | seal, green |
| mocha-json-reporter | `.mocharc.json` `{reporter: json}` | seal, green |
| mocha-junit-reporter-crash | mocha 12 + mocha-junit-reporter 2.2.1, which crashes | seal, green |
| go-goflags-json | `GOFLAGS=-json` in the environment (docker) | seal, green |
| go-tparse | `go test -json ./... \| tparse` (docker) | seal, green |
| nextest-status-level-none | nextest `status-level`/`final-status-level` none | seal, green |
| phpunit-compact-output | `PHPUNIT_COMPACT_OUTPUT=1` (docker) | seal, green |
| maven-print-summary-false | surefire `printSummary=false` (docker) | seal, green |
| dotnet-de-DE | `DOTNET_CLI_UI_LANGUAGE=de-DE` (docker) | seal, green |

Each project is the same small story: `greet()` returns `"hi"`, the planned test wants
`"hello"`, the fix returns `"hello"`. Not in the lab yet: Pest, NUnit, MSTest, Gradle,
`dotnet` with Microsoft.Testing.Platform, C and C++ (A-7 covers them last).

## A project

    projects/<name>/project.conf     shell variables, below
    projects/<name>/{base,red,fix}/  files laid over the project at each step
    common/<kind>/{base,red,fix}/    the same, shared by projects with COMMON=<kind>
    prepare/<script>.sh              builds a toolchain into the cache (PREPARE="<script> args")

`base` is committed first; `red` is the planned test, committed alone; `fix` is the next
commit. The project's own layer is laid over the common one, so a project can replace any file
(a `fix` that leaves the code wrong makes an attack).

    DESC          one line: what this setup is
    COMMON        the shared layer's name
    PREPARE       a script from prepare/ and its arguments (pytest, npm, composer, tparse)
    SHIMS         tool=docker-image pairs, run through docker with the project's folder and environment
    PINNED        shimmed tools whose version is the point: never replaced by a local install
    ENV_VARS      K=V pairs exported for the whole walk (the person's own environment)
    STACK         the test_stack claimed          TEST_COMMAND  the command claimed
    TRUTH_COMMAND what `--truth` runs when the claimed command cannot tell the truth by itself (a reporter that crashes)
    PLANNED_TEST  the planned test's path         PLACE  "file symbol first-line last-line"
    EXPECT_RED    sealed | not_sealed | asks      EXPECT_GREEN  green | not_green | asks | -
    EXPECT_READER the reader id the adapter should choose     EXPECT_CARDS  hold names expected

## When the adapter lands

The lab reads `.familiar/adapter` as the plan drafts it (JSON Lines: `root`, `runner`, `reader`,
`gaps[]`, per runner) and prints the reader and gaps, else any status key naming a reader or an
adapter. When setup or a claim says a dry run started (`the dry run for ... started`) or what
it reads (`reading: pytest by ...`), the lab waits (up to `ADAPTER_LAB_DRY_WAIT` seconds, default
900) for `.familiar/adapter`, and always for any run in flight, so the tests-first commit is read
under the adapter.

Exercised twice: with `stand-in-adapter.sh`, a wrapper that runs the real binary, says a dry run
started and writes a fixed adapter file three seconds later, and with lane A-py's in-progress build
of 2026-10-19 16:05, which prints `reading: pytest by Familiar's own reporter` at the test
command's claim and writes `{"root":"","runner":"pytest","reader":"pytest:reporter","familiar":
"0.1.0-dev","gaps":[]}`. The column then reads `pytest=pytest:reporter`, `pytest=none` for a
conftest that flips outcomes, `pytest=pytest:tables gaps: pytest:P5` under `python -I`:

    ADAPTER_LAB_STAND_IN_REAL=<familiar-ai> sh scripts/adapter-lab/run-lab.sh \
        --bin scripts/adapter-lab/stand-in-adapter.sh pytest-plain

(a stand-in's verdicts mean nothing). Check the real adapter's words and file shape again when A lands.

## Limits

- One lab run per `--dir` at a time (a run starts by clearing the folder's `work/`), and no edits
  to the scripts while one runs.
- Stage 2 is reached only after a seal, so today the fix layers of the setups the tables cannot read
  (jest-junit, mocha json, go `-json`, nextest, PHPUnit, Maven, dotnet) have run only under `--truth`,
  which shows them right: each fails with the tests first and passes once fixed.
- The attack projects prove only what the binary does with that attack; a harness that forges
  outcomes only for the planned test is the plan's known limit.
- The docker images are used as found on the machine (`go` 1.25, `node` 24, `maven` 3.9 with
  Java 21, `dotnet` SDK 8, `composer` 2 for PHP); a missing image is pulled once.
- A run longer than `ADAPTER_LAB_RUN_WAIT` seconds is left as it is and noted in the row.
