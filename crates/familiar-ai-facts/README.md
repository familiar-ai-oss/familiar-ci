# familiar-ai-facts

The fact kinds, the append that validates them, and the record that stores
them.

A fact is flat JSON, `{t, at, kind, ...fields, tenant}`. `KINDS` lists the
kinds and `FORM` gives each one's fields. The caller passes `at`; the crate
reads no clock. It holds nothing but the record.

Why it is this way:

- The record is append-only, with no update and no delete, because every
  state is derived from it and nothing may be quietly rewritten
  ([facts in, states derived](../../docs/decisions.md#append-only-facts)).
- Each kind has a fixed set of fields and values, and the kinds that record
  what happened are the binary's alone
  ([which kinds the agent may write](../../docs/decisions.md#which-kinds-the-agent-may-write)).
- Only the record stamps `t`, `at`, `kind` and `tenant`, because a fact's
  position is what "before" means everywhere in the rules
  ([the stamped fields](../../docs/decisions.md#the-binary-stamps-position-time-kind-and-tenant)).
- It is one `redb` file, not a text log, so several processes can take
  turns with atomic writes
  ([a database file](../../docs/decisions.md#the-record-is-a-database-file)).

## Use

One typed struct per kind. `append` returns a new log and leaves the one it
was given as it was.

```rust
use familiar_ai_facts::{append, Criterion, Path, Log};

let log: Log = Vec::new();
let log = append(
    &log,
    &Criterion {
        criterion_id: "c1".to_string(),
        text: "a right password opens a session".to_string(),
        path: Path::Full,
        behavior_id: Some("b1".to_string()),
        promise_id: Some("p1".to_string()),
        for_t: None,
    },
    1_700_000_000.0, // at
    "my_tenant",
)?;
assert_eq!(log[0].t, 0); // t is the position; only append sets it
# Ok::<(), familiar_ai_facts::AppendError>(())
```

JSON arrives through `append_raw`; a bad fact is an `AppendError` that names
the problem. `Record` is the file; `Record::open_waiting` lets processes take
turns.

## Tests

- `tests/append.rs`: every kind round-trips through append and JSON, typed
  and raw appends agree, append is pure, `t` is the position, and the
  readers find by kind and criterion.
- `tests/validation.rs`: the refusals: an unknown kind, an unregistered or
  stamped key, a missing field, a word outside a closed set, a value of the
  wrong shape, a timestamp that is not a finite number.
- `tests/forms.rs`: each kind's shape and closed values, and that no form
  registers `state` or `status`.
- `tests/record_persistence.rs`: the record keeps facts in order across a
  reopen and two handles, reads by kind, and writes nothing it refuses.
- `tests/record_concurrency.rs`: many processes append at once, and every
  fact lands in order with no gap or repeat.
- `tests/append_if.rs`: an append that lands only if a predicate holds on
  the whole log, read and written in one transaction, so of two writers
  racing for the same work exactly one wins.
- `tests/commit_seen_by.rs`: the optional `by` on `commit_seen`.
- `tests/common/mod.rs`: one sample of each kind, shared by the files above.
