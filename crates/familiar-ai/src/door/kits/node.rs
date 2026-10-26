//! Lane AKjs (plan of record 2026-10-19, A): node --test, as plain data [draft].
//!
//! Everything here is `const` and std-only, so the reader table, the kit cache and the dry run
//! (lane A-py's `readers.rs`, `reported.rs` and friends) build from it without a shared type. The
//! four JS runners (this module, `jest`, `vitest`, `mocha`) share one layout and one vocabulary:
//!
//! ```text
//!   KIT_FILES   (path under the kit directory, contents)      written once per version to
//!                                                              <temp>/familiar-ai-kit-<version>/,
//!                                                              the layout kept (../node/common.cjs)
//!   READERS     (id, kind, env ops, added flags, report globs) best first; the dry run proves each
//!   PROBES      (probe, truth label, truth kind, files)        the first file is the test the
//!                                                              command names; the rest is its code
//!   SELECTION_FLAGS (flag, takes a value)                      what `re_aim` drops
//!   WATCHED     globs of the setup files whose change owes a new dry run
//! ```
//!
//! An env op is `(variable, op, value, separator)`: `op` is `set`, `append`, `prepend` or `unset`;
//! `append` and `prepend` join to an existing value with `separator` (the person's own
//! `NODE_OPTIONS` is kept), and an `unset` carries no value. Values and flags hold three
//! placeholders: `{KIT}` the kit directory, `{REPORT_DIR}` a fresh directory outside the work tree
//! made for the run (the kit writes `{REPORT_DIR}/results.jsonl`, the file `FAMILIAR_AI_REPORT`
//! names), and for node `{NODE_DEFAULT_REPORTER}` (see [`default_reporter`]).
//!
//! Every reader unsets the agent-mode variables ([`AGENT_MODE_UNSET`]), so the laptop and CI print
//! alike: vitest 5 switches its console to an agent reporter under `CLAUDECODE=1`.

/// An environment operation: (variable, op, value, separator).
pub type EnvOp = (&'static str, &'static str, &'static str, &'static str);
/// A reader: (id, kind `reporter`|`report`|`pinned`|`tables`, env ops, added flags, report globs).
pub type Reader = (&'static str, &'static str, &'static [EnvOp], &'static [&'static str], &'static [&'static str]);
/// A probe: (probe, truth label, truth kind or `""`, files (path relative to where the probe is
/// placed, contents); the first is the test the command names).
pub type Probe = (&'static str, &'static str, &'static str, &'static [(&'static str, &'static str)]);

/// The runner as the kit's session line names it.
pub const RUNNER: &str = "node-test";

/// The agent-mode variables of std-env's detection (what vitest 5 reads), minus the ones matched
/// by value (`PATH`, `EDITOR`, `TERM_PROGRAM`), which are not ours to unset.
pub const AGENT_MODE_UNSET: [EnvOp; 16] = [
    ("AI_AGENT", "unset", "", ""),
    ("CLAUDECODE", "unset", "", ""),
    ("CLAUDE_CODE", "unset", "", ""),
    ("CLAUDE_CODE_ENTRYPOINT", "unset", "", ""),
    ("REPL_ID", "unset", "", ""),
    ("GEMINI_CLI", "unset", "", ""),
    ("CODEX_SANDBOX", "unset", "", ""),
    ("CODEX_THREAD_ID", "unset", "", ""),
    ("OPENCODE", "unset", "", ""),
    ("AUGMENT_AGENT", "unset", "", ""),
    ("GOOSE_PROVIDER", "unset", "", ""),
    ("JUNIE_DATA", "unset", "", ""),
    ("JUNIE_SHIM_PATH", "unset", "", ""),
    ("COPILOT_AGENT", "unset", "", ""),
    ("COPILOT_CLI", "unset", "", ""),
    ("CURSOR_AGENT", "unset", "", ""),
];

/// `a` then `b` as one array: a reader's own ops, then the unsets (`N` is `A + B`).
pub const fn join<const A: usize, const B: usize, const N: usize>(a: [EnvOp; A], b: [EnvOp; B]) -> [EnvOp; N] {
    assert!(A + B == N);
    let mut out = [("", "", "", ""); N];
    let mut i = 0;
    while i < A {
        out[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < B {
        out[A + j] = b[j];
        j += 1;
    }
    out
}

/// Node's own reporter for a run whose stdout is not a terminal: `tap` before node 23, `spec` from
/// 23 (verified on 24.22). The kit's second reporter is the version's default, so the person's
/// console stays what node would have printed.
pub fn default_reporter(node_major: u32) -> &'static str {
    if node_major >= 23 {
        "spec"
    } else {
        "tap"
    }
}

/// A command naming either of these has its own reporter; the env form would pair our reporter with
/// the wrong destination (node throws when the counts differ), so `node:reporter-flags` is used.
pub const OWN_REPORTER_FLAGS: &[&str] = &["--test-reporter", "--test-reporter-destination"];

/// The kit: `common.cjs` is shared by jest, vitest and mocha.
pub const KIT_FILES: &[(&str, &str)] = &[
    ("node/common.cjs", include_str!("../../../kit/node/common.cjs")),
    ("node/reporter.mjs", include_str!("../../../kit/node/reporter.mjs")),
];

const REPORTER_ENV: [EnvOp; 2] = [
    ("FAMILIAR_AI_REPORT", "set", "{REPORT_DIR}/results.jsonl", ""),
    // The destination must be a real file: `/dev/null` makes node 22 crash at exit. The second
    // reporter is node's default for the version, to stdout; the counts of reporters and
    // destinations must match.
    ("NODE_OPTIONS", "append", "--test-reporter={KIT}/node/reporter.mjs --test-reporter-destination={REPORT_DIR}/node.out --test-reporter={NODE_DEFAULT_REPORTER} --test-reporter-destination=stdout", " "),
];
const REPORTER_FLAGS_ENV: [EnvOp; 1] = [("FAMILIAR_AI_REPORT", "set", "{REPORT_DIR}/results.jsonl", "")];
const PINNED_ENV: [EnvOp; 1] = [("NODE_OPTIONS", "append", "--test-reporter=tap --test-reporter-destination=stdout", " ")];
const REPORTER: [EnvOp; 18] = join(REPORTER_ENV, AGENT_MODE_UNSET);
const REPORTER_FLAGS: [EnvOp; 17] = join(REPORTER_FLAGS_ENV, AGENT_MODE_UNSET);
const PINNED: [EnvOp; 17] = join(PINNED_ENV, AGENT_MODE_UNSET);

/// Best first. `node:reporter` needs the command to name no reporter of its own, `node:pinned` too;
/// `node:reporter-flags` pairs the person's reporter with stdout and ours with a temp file (proven
/// with one `--test-reporter` and no destination). A junit `report` reader is not listed: the spike
/// never ran one for node.
pub const READERS: &[Reader] = &[
    ("node:reporter", "reporter", &REPORTER, &[], &[]),
    (
        "node:reporter-flags",
        "reporter",
        &REPORTER_FLAGS,
        &["--test-reporter-destination=stdout", "--test-reporter={KIT}/node/reporter.mjs", "--test-reporter-destination={REPORT_DIR}/node.out"],
        &[],
    ),
    ("node:pinned", "pinned", &PINNED, &[], &[]),
    ("node:tables", "tables", &AGENT_MODE_UNSET, &[], &[]),
];

/// What `re_aim` drops from a claimed `node --test` command (and the value of those that take one).
pub const SELECTION_FLAGS: &[(&str, bool)] = &[
    ("--test-name-pattern", true),
    ("--test-skip-pattern", true),
    ("--test-only", false),
    ("--test-shard", true),
];

/// The setup files whose change owes a new dry run (the node version moves the default reporter).
pub const WATCHED: &[&str] = &[
    "**/package.json",
    "**/.nvmrc",
    "**/.node-version",
    "**/.tool-versions",
    "**/package-lock.json",
    "**/npm-shrinkwrap.json",
    "**/yarn.lock",
    "**/pnpm-lock.yaml",
    "**/bun.lock",
    "**/bun.lockb",
];

/// One file of a probe is the test, the other its code; P3 and P3b run the same test, P3b with the
/// function in place.
pub const PROBES: &[Probe] = &[
    (
        "P1",
        "passed",
        "",
        &[
            ("familiar_probe_p1.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
    (
        "P2",
        "tests_failed",
        "assertion",
        &[
            ("familiar_probe_p2.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

test('adds', () => {
  assert.strictEqual(add(2, 2), 5);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
    (
        "P3",
        "missing_target_symbol",
        "",
        &[
            ("familiar_probe_p3.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { mul } = require('./familiar_probe_calc.cjs');

test('multiplies', () => {
  assert.strictEqual(mul(2, 3), 6);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
    (
        "P3b",
        "passed",
        "",
        &[
            ("familiar_probe_p3.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { mul } = require('./familiar_probe_calc.cjs');

test('multiplies', () => {
  assert.strictEqual(mul(2, 3), 6);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

function mul(a, b) {
  return a * b;
}

module.exports = { add, parse, mul };
"#),
        ],
    ),
    (
        "P4",
        "collection_error",
        "",
        &[
            ("familiar_probe_p4.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');
const { helper } = require('./familiar_probe_nosuch.cjs');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
    (
        "P5",
        "passed",
        "",
        &[
            ("familiar_probe_p5.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

test('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});

test.skip('later', () => {
  assert.strictEqual(add(1, 1), 2);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
    (
        "P6",
        "no_tests_collected",
        "",
        &[
            ("familiar_probe_p6.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

function helper() {
  return add(1, 1);
}
void helper;
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
    (
        "P7",
        "tests_failed",
        "exception",
        &[
            ("familiar_probe_p7.test.cjs", r#"'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const { parse } = require('./familiar_probe_calc.cjs');

test('parses', () => {
  assert.strictEqual(parse('x1'), 1);
});
"#),
            ("familiar_probe_calc.cjs", r#"'use strict';

function add(a, b) {
  return a + b;
}

function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

module.exports = { add, parse };
"#),
        ],
    ),
];
