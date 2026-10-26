//! Lane AKjs (plan of record 2026-10-19, A): jest, as plain data [draft]. The vocabulary is
//! `node`'s (types, placeholders, the agent-mode unsets).
//!
//! `--reporters=default --reporters=<kit>` replaces the config's reporters in Familiar's own run only
//! (the person's `jest-junit` file is not written there; their own runs are untouched).

use super::node::{join, EnvOp, Probe, Reader, AGENT_MODE_UNSET};

/// The runner as the kit's session line names it.
pub const RUNNER: &str = "jest";

/// The kit; `node/common.cjs` is the writer shared with node's.
pub const KIT_FILES: &[(&str, &str)] = &[
    ("jest/reporter.cjs", include_str!("../../../kit/jest/reporter.cjs")),
    ("node/common.cjs", include_str!("../../../kit/node/common.cjs")),
];

const REPORTER_ENV: [EnvOp; 1] = [("FAMILIAR_AI_REPORT", "set", "{REPORT_DIR}/results.jsonl", "")];

const REPORTER: [EnvOp; 17] = join(REPORTER_ENV, AGENT_MODE_UNSET);

/// Best first: Familiar's reporter added in Familiar's own run, the console pinned to the runner's
/// default, then the regex tables on whatever the person's setup prints.
pub const READERS: &[Reader] = &[
    ("jest:reporter", "reporter", &REPORTER, &["--reporters=default", "--reporters={KIT}/jest/reporter.cjs"], &[]),
    ("jest:pinned", "pinned", &AGENT_MODE_UNSET, &["--reporters=default"], &[]),
    ("jest:tables", "tables", &AGENT_MODE_UNSET, &[], &[]),
];

/// What `re_aim` drops from a claimed jest command (and the value of those that take one;
/// `--findRelatedTests` takes the files that follow it).
pub const SELECTION_FLAGS: &[(&str, bool)] = &[
    ("-t", true),
    ("--testNamePattern", true),
    ("--testPathPattern", true),
    ("--testPathPatterns", true),
    ("--testPathIgnorePatterns", true),
    ("--findRelatedTests", true),
    ("-o", false),
    ("--onlyChanged", false),
    ("-f", false),
    ("--onlyFailures", false),
    ("--changedSince", true),
    ("--lastCommit", false),
    ("--changedFilesWithAncestor", false),
    ("--shard", true),
    ("--selectProjects", true),
    ("--ignoreProjects", true),
];

/// The setup files whose change owes a new dry run.
pub const WATCHED: &[&str] = &[
    "**/package.json",
    "**/jest.config.*",
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
            ("familiar_probe_p1.test.js", r#"'use strict';
const { add } = require('./familiar_probe_calc.js');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p2.test.js", r#"'use strict';
const { add } = require('./familiar_probe_calc.js');

test('adds', () => {
  expect(add(2, 2)).toBe(5);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p3.test.js", r#"'use strict';
const { mul } = require('./familiar_probe_calc.js');

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p3.test.js", r#"'use strict';
const { mul } = require('./familiar_probe_calc.js');

test('multiplies', () => {
  expect(mul(2, 3)).toBe(6);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p4.test.js", r#"'use strict';
const { add } = require('./familiar_probe_calc.js');
const { helper } = require('./familiar_probe_nosuch.js');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p5.test.js", r#"'use strict';
const { add } = require('./familiar_probe_calc.js');

test('adds', () => {
  expect(add(2, 2)).toBe(4);
});

test.skip('later', () => {
  expect(add(1, 1)).toBe(2);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p6.test.js", r#"'use strict';
const { add } = require('./familiar_probe_calc.js');

function helper() {
  return add(1, 1);
}
void helper;
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
            ("familiar_probe_p7.test.js", r#"'use strict';
const { parse } = require('./familiar_probe_calc.js');

test('parses', () => {
  expect(parse('x1')).toBe(1);
});
"#),
            ("familiar_probe_calc.js", r#"'use strict';

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
