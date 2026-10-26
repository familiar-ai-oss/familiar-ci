# familiar-ai-plan

Reads and writes the plan file, `.familiar/plan`.

The plan is committed, so every clone carries it, and CI's `verify` reads
it there. It holds claims only. These are the same facts as the log, minus
`t`, `at` and `tenant`, and never a test's output. Of the person's answers
it holds only the one on the cases card, beside the case-finder's finding,
so a record rebuilt from the plan (a fresh clone, a new lane, CI's
`verify`) never asks again (the owner, 2026-10-25); any other card's answer
is refused.

The format is JSON Lines: one claim per line, `kind` first, then the fields
in their form's order. The same claims always give the same bytes, so a
pull request shows one changed line per changed claim. Only `src/format.rs`
knows the format.

Why it is this way: CI must recompute every result from git and this file
alone, so the plan is committed with the work and holds claims, never a
test's output
([the plan is committed](../../docs/decisions.md#the-plan-is-committed),
[the proof is CI's](../../docs/decisions.md#ci-proves-the-laptop-guides)). Every decision
is in [docs/decisions.md](../../docs/decisions.md).

## Use

```rust
use familiar_ai_facts::{append, Log, Path};
use familiar_ai_plan::{claims_from_log, read, write};

let log: Log = append(
    &Vec::new(),
    &familiar_ai_facts::Criterion {
        criterion_id: "c1".into(),
        text: "greet says hello".into(),
        path: Path::Light,
        behavior_id: None,
        promise_id: None,
        for_t: None,
    },
    0.0,
    "t",
)?;
let plan = std::env::temp_dir().join("familiar-ai-plan-readme-example");

let claims = claims_from_log(&log); // the claim kinds, in log order
write(&plan, &claims)?;
assert_eq!(read(&plan)?, claims);
# std::fs::remove_file(&plan).ok();
# Ok::<(), Box<dyn std::error::Error>>(())
```

`read` and `write` take a full path; the crate does not decide where the
file lives. A missing file is `PlanError::NotFound`, and an empty file is
an empty plan. A kind that is not a claim, such as a test result, is
refused.

## Tests

`cargo test -p familiar-ai-plan --locked`:

- `write_read.rs`: every allowed kind round-trips, the bytes are stable,
  and no claim can carry `output`, `state` or `status`.
- `refusals.rs`: each refusal names its line.
- `from_log.rs`: only claim kinds cross from the log, in order, without
  `t`, `at` or `tenant`.
- `the_cases_card_is_mirrored.rs`: the case-finder's finding and every
  answer on the cases card cross, in order, as recorded; another card's
  answer, or a choice the cases card does not offer, is refused on read
  and on write.
