//! Lane AKjs (plan of record 2026-10-19, A): mocha, as plain data [draft]. The vocabulary is
//! `node`'s (types, placeholders, the agent-mode unsets).
//!
//! `--require <kit>/mocha/load.cjs` beside `--reporter`: mocha loads the spec files before it builds
//! a reporter, so a file that fails to load (P4) reaches only the load hook.

use super::node::{join, EnvOp, Probe, Reader, AGENT_MODE_UNSET};

/// The runner as the kit's session line names it.
pub const RUNNER: &str = "mocha";

/// The kit; `node/common.cjs` is the writer shared with node's.
pub const KIT_FILES: &[(&str, &str)] = &[
    ("mocha/load.cjs", include_str!("../../../kit/mocha/load.cjs")),
    ("mocha/reporter.cjs", include_str!("../../../kit/mocha/reporter.cjs")),
    ("node/common.cjs", include_str!("../../../kit/node/common.cjs")),
];

const REPORTER_ENV: [EnvOp; 1] = [("FAMILIAR_AI_REPORT", "set", "{REPORT_DIR}/results.jsonl", "")];

const REPORTER: [EnvOp; 17] = join(REPORTER_ENV, AGENT_MODE_UNSET);

/// Best first: Familiar's reporter added in Familiar's own run, the console pinned to the runner's
/// default, then the regex tables on whatever the person's setup prints.
pub const READERS: &[Reader] = &[
    ("mocha:reporter", "reporter", &REPORTER, &["--require", "{KIT}/mocha/load.cjs", "--reporter", "{KIT}/mocha/reporter.cjs"], &[]),
    ("mocha:pinned", "pinned", &AGENT_MODE_UNSET, &["--reporter", "spec"], &[]),
    ("mocha:tables", "tables", &AGENT_MODE_UNSET, &[], &[]),
];

/// What `re_aim` drops from a claimed mocha command (and the value of those that take one).
pub const SELECTION_FLAGS: &[(&str, bool)] = &[
    ("--grep", true),
    ("-g", true),
    ("--fgrep", true),
    ("-f", true),
    ("--invert", false),
    ("-i", false),
    ("--spec", true),
    ("--ignore", true),
    ("--exclude", true),
];

/// The setup files whose change owes a new dry run.
pub const WATCHED: &[&str] = &[
    "**/package.json",
    "**/.mocharc*",
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
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

it('adds', () => {
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
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

it('adds', () => {
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
const assert = require('node:assert');
const { mul } = require('./familiar_probe_calc.cjs');

it('multiplies', () => {
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
const assert = require('node:assert');
const { mul } = require('./familiar_probe_calc.cjs');

it('multiplies', () => {
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
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');
const { helper } = require('./familiar_probe_nosuch.cjs');

it('adds', () => {
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
const assert = require('node:assert');
const { add } = require('./familiar_probe_calc.cjs');

it('adds', () => {
  assert.strictEqual(add(2, 2), 4);
});

it.skip('later', () => {
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
const assert = require('node:assert');
const { parse } = require('./familiar_probe_calc.cjs');

it('parses', () => {
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
