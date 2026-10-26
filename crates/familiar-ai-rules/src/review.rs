//! The reviewer at green [draft names] (the owner's plan of 2026-10-20, step 3). Once a part is
//! green the binary asks a reviewer whether the fix does what the person asked, and writes
//! `fix_verdict`: `covers` yes, no or not_judged. A "no" is written only when the binary read
//! every quote again at the green commit, so a "no" here is a verified one. The reviewer can only
//! block: green itself is read as before, and nothing here marks a part done.
//! - `standing_no`: the part's latest verdict is a "no" and the person has not kept the fix as
//!   it is; the binary refuses the stop while one stands. A later verdict replaces it, and a
//!   "not judged" never blocks.
//! - `HOLD`: two "no"s on one part are the person's card (`disputed`); after `try_again`, one
//!   more is.

use familiar_ai_facts::Fact;

use crate::criterion::mine;
use crate::reader::{last_of, t_of, text, View};

/// The kind the binary writes.
pub const KIND: &str = "fix_verdict";
/// The person's card after `NOS` verified "no"s [draft name].
pub const HOLD: &str = "review_disputed";
/// The verified "no"s on one part that make the person's card.
pub const NOS: usize = 2;
/// The person keeps the fix as it is.
pub const KEEP: &str = "keep_going";
/// One more review after another fix.
pub const AGAIN: &str = "try_again";

/// The person's latest answer to `HOLD` on `c` since the epoch, a release aside.
fn last_answer<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    last_of(
        mine(v, c, "hold_answered")
            .into_iter()
            .filter(|f| text(f, "hold") == Some(HOLD) && text(f, "choice") != Some("release") && !crate::dependency::ended(v, f))
            .collect(),
    )
}

/// Whether the person kept `c`'s fix as it is: their latest answer to `HOLD` is `KEEP`.
pub fn accepted(v: &View, c: &str) -> bool {
    last_answer(v, c).is_some_and(|a| text(a, "choice") == Some(KEEP))
}

/// `c`'s verdicts since the epoch, oldest first.
pub fn verdicts<'a>(v: &View<'a>, c: &str) -> Vec<&'a Fact> {
    mine(v, c, KIND)
}

/// The "no" that stands against `c`: its latest verdict, when that is a "no" and the person has
/// not kept the fix as it is.
pub fn standing_no<'a>(v: &View<'a>, c: &str) -> Option<&'a Fact> {
    if accepted(v, c) {
        return None;
    }
    last_of(verdicts(v, c)).filter(|f| text(f, "covers") == Some("no"))
}

/// Whether `c`'s "no"s make the person's card: `NOS` of them with no answer to `HOLD`, or one
/// since a `try_again`; never after `KEEP`.
pub fn disputed(v: &View, c: &str) -> bool {
    let answer = last_answer(v, c);
    if answer.is_some_and(|a| text(a, "choice") == Some(KEEP)) {
        return false;
    }
    let since = answer.map(t_of).unwrap_or(-1);
    let nos = verdicts(v, c).iter().filter(|f| t_of(f) > since && text(f, "covers") == Some("no")).count();
    nos >= if answer.is_some() { 1 } else { NOS }
}

/// The gap the latest "no" on `c` named, for the card.
pub fn gap<'a>(v: &View<'a>, c: &str) -> Option<&'a str> {
    verdicts(v, c).into_iter().rev().find(|f| text(f, "covers") == Some("no")).and_then(|f| text(f, "gap"))
}
