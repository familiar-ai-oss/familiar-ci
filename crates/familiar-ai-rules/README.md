# familiar-ai-rules

The rules: what state each criterion is in, which holds are on it, and
whether an edit, a commit or a stop goes through.

Every rule is a function of the log. It takes the facts and a time and
returns an answer. The crate does no input or output, reads no clock and no
git, and stores nothing. A state such as `sealed` or `green` is not kept
anywhere; it is computed from the facts each time it is asked for.

That has three consequences:

- The laptop hooks and CI's `verify` compute the same answer from the same
  facts, because there is nothing else to differ on.
- Any past state can be recomputed by handing in the log up to that point.
- The rules can be tested without a repository, a record or a process. The
  crate is the model: a private checks crate walks every situation reachable
  from its fixtures up to a depth, judges the rails and runs the mutants
  ([pure rules](../../docs/decisions.md#pure-rules-in-their-own-crate)).

A criterion has seven states: `held`, `unplanned`, `waiting`, `testing`,
`sealed`, `green`, `proven`. Each is a predicate, checked in that order,
and the first true one is the state. So a hold beats everything, exactly
one state applies, and every surface names the same one
([seven states](../../docs/decisions.md#seven-states-in-a-fixed-order)).

## Use

Hand in a log and an event, and get a decision:

```rust
use familiar_ai_facts::Log;
use familiar_ai_rules::{decide, Event, Decision};

let log: Log = Vec::new();
let d = decide(&log, &Event::FirstEditOrCommit, 0.0);
// nothing submitted yet, so planning is incomplete
assert!(matches!(d, Decision::Refuse(_)));
```

| module | holds |
|---|---|
| `plan` | planning's lines and when planning is complete |
| `criterion` | one criterion's reads up to the seal |
| `lifecycle` | green, proven, and the seven states |
| `holds` | the holds and what clears each |
| `ladder` | which model an investigator runs on: haiku, sonnet, opus, fable, then a second fable |
| `hooks` | the stops and the referee, as one `decide` |
| `action` | CI's recheck of a pull request head |
| `terminal` | state lines, refusals, cards, the whole picture |
| `reader` | the `View` over a log and its commit index |
| `sample` | sample logs for tests, and the moves allowed in them |
| `moves` | a criterion's moves: commits, rebase, transfers |

## Tests

`cargo test -p familiar-ai-rules` runs the behaviour tests in `tests/`,
each named for the rule it checks, such as
`each_criterion_is_proved_on_its_own` and
`a_legacy_fingerprint_is_read_but_never_stamped`.
