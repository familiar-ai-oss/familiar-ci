//! Lane AKjs (plan of record 2026-10-19, A): vitest, as plain data [draft]. The vocabulary is
//! `node`'s (types, placeholders, the agent-mode unsets).
//!
//! The kit is the vitest 3 and later reporter API (`onInit`, `onTestRunEnd`); verified on 5.0.5. An
//! older vitest (`onFinished`) writes a session line and no end line: no structured output.

use super::node::{join, EnvOp, Probe, Reader, AGENT_MODE_UNSET};

/// The runner as the kit's session line names it.
pub const RUNNER: &str = "vitest";

/// The kit; `node/common.cjs` is the writer shared with node's.
pub const KIT_FILES: &[(&str, &str)] = &[
    ("vitest/reporter.mjs", include_str!("../../../kit/vitest/reporter.mjs")),
    ("node/common.cjs", include_str!("../../../kit/node/common.cjs")),
];

const REPORTER_ENV: [EnvOp; 1] = [("FAMILIAR_AI_REPORT", "set", "{REPORT_DIR}/results.jsonl", "")];

const REPORTER: [EnvOp; 17] = join(REPORTER_ENV, AGENT_MODE_UNSET);

/// Best first: Familiar's reporter added in Familiar's own run, the console pinned to the runner's
/// default, then the regex tables on whatever the person's setup prints.
pub const READERS: &[Reader] = &[
    ("vitest:reporter", "reporter", &REPORTER, &["--reporter=default", "--reporter={KIT}/vitest/reporter.mjs"], &[]),
    ("vitest:pinned", "pinned", &AGENT_MODE_UNSET, &["--reporter=default"], &[]),
    ("vitest:tables", "tables", &AGENT_MODE_UNSET, &[], &[]),
];

/// What `re_aim` drops from a claimed vitest command (and the value of those that take one;
/// `related` is a subcommand, dropped with the files that follow it).
pub const SELECTION_FLAGS: &[(&str, bool)] = &[
    ("-t", true),
    ("--testNamePattern", true),
    ("related", true),
    ("--changed", false),
    ("--shard", true),
    ("--exclude", true),
    ("--project", true),
    ("--dir", true),
];

/// The setup files whose change owes a new dry run.
pub const WATCHED: &[&str] = &[
    "**/package.json",
    "**/vitest.config.*",
    "**/vitest.workspace.*",
    "**/vite.config.*",
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
            ("familiar_probe_p1.test.mjs", r#"import { test, expect } from 'vitest';
import { add } from './familiar_probe_calc.mjs';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
    (
        "P2",
        "tests_failed",
        "assertion",
        &[
            ("familiar_probe_p2.test.mjs", r#"import { test, expect } from 'vitest';
import { add } from './familiar_probe_calc.mjs';

test('adds', () => {
  expect(add(2, 2)).toBe(5);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
    (
        "P3",
        "missing_target_symbol",
        "",
        &[
            ("familiar_probe_p3.test.mjs", r#"import { test, expect } from 'vitest';
import { mul } from './familiar_probe_calc.mjs';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
    (
        "P3b",
        "passed",
        "",
        &[
            ("familiar_probe_p3.test.mjs", r#"import { test, expect } from 'vitest';
import { mul } from './familiar_probe_calc.mjs';

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}

export function mul(a, b) {
  return a * b;
}
"#),
        ],
    ),
    (
        "P4",
        "collection_error",
        "",
        &[
            ("familiar_probe_p4.test.mjs", r#"import { test, expect } from 'vitest';
import { add } from './familiar_probe_calc.mjs';
import { helper } from './familiar_probe_nosuch.mjs';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
    (
        "P5",
        "passed",
        "",
        &[
            ("familiar_probe_p5.test.mjs", r#"import { test, expect } from 'vitest';
import { add } from './familiar_probe_calc.mjs';

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});

test.skip('later', () => {
  expect(add(1, 1)).toBe(2);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
    (
        "P6",
        "no_tests_collected",
        "",
        &[
            ("familiar_probe_p6.test.mjs", r#"import { test, expect } from 'vitest';
import { add } from './familiar_probe_calc.mjs';

function helper() {
  return add(1, 1);
}
void helper;
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
    (
        "P7",
        "tests_failed",
        "exception",
        &[
            ("familiar_probe_p7.test.mjs", r#"import { test, expect } from 'vitest';
import { parse } from './familiar_probe_calc.mjs';

test('parses', () => {
  expect(parse('x1')).toBe(1);
});
"#),
            ("familiar_probe_calc.mjs", r#"export function add(a, b) {
  return a + b;
}

export function parse(s) {
  if (!/^\d+$/.test(s)) {
    throw new Error('not a number: ' + s);
  }
  return Number(s);
}
"#),
        ],
    ),
];
