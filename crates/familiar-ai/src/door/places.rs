//! The place lister [draft] (the owner's plan of 2026-10-21, step 3): after a claim, once the plan
//! is confirmed and a part's place checked for planning, the binary searches that commit for the sites that name
//! what the place names (`door::siblings::list_place`) and writes them as `places_listed`, once
//! per place chosen, before any fix. Every test is left out: the plan's own and touching tests,
//! any file under a test folder or named as a test; so are the other parts' places, which their
//! own parts plan. The rules read the listing
//! (`familiar_ai_rules::places`).

use std::path::Path;

use familiar_ai_facts::{Fact, ListedSite, Log, PlacesListed, Record, RecordError};
use familiar_ai_rules::{places, plan, View};

use crate::door::siblings::{list_place, Planned};

/// The files a listing never searches beside the test folders: the plan's test files and its
/// touching tests.
fn plan_tests(v: &View) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in plan::criteria(v) {
        out.extend(familiar_ai_rules::criterion::test_files(v, &c));
        out.extend(v.of("touching_test_declared", &c).chain(v.of("touching_test_derived", &c)).filter_map(|f| f.fields.get("file_path").and_then(|x| x.as_str())).map(str::to_string));
    }
    out
}

/// The planned places of `c`'s anchor exit.
fn planned(v: &View, c: &str) -> Vec<Planned> {
    let Some(a) = places::exit_of(v, c) else {
        return Vec::new();
    };
    let n = |x: Option<i64>| x.and_then(|x| u64::try_from(x).ok()).unwrap_or(0);
    plan::places_of(a)
        .into_iter()
        .map(|p| Planned { file: p.file_path.to_string(), symbol: p.symbol.to_string(), start: n(p.line_start), end: n(p.line_end) })
        .collect()
}

/// Write each listing owed, in plan order.
pub fn write_owed(record: &Record, root: &Path, log: &mut Log, at: f64, tenant: &str) -> Result<Vec<Fact>, RecordError> {
    let mut written = Vec::new();
    loop {
        let v = View::new(log);
        let Some((c, sha)) = plan::criteria(&v).into_iter().find_map(|c| places::owed(&v, &c).map(|sha| (c, sha))) else {
            break;
        };
        let tests = plan_tests(&v);
        let skip = |p: &str| crate::door::existing::is_test_file(p) || tests.iter().any(|t| t == p);
        let others: Vec<Planned> = plan::criteria(&v).into_iter().filter(|x| *x != c).flat_map(|x| planned(&v, &x)).collect();
        let sites = list_place(root, &sha, &planned(&v, &c), &others, &skip)
            .into_iter()
            .map(|s| ListedSite { file: s.file, line: s.line, def: s.def.map(|(_, d)| d.trim().to_string()).unwrap_or_default(), names: s.names })
            .collect();
        let fact = record.append(&PlacesListed { criterion_id: c, sites }, at, tenant)?;
        log.push(fact.clone());
        written.push(fact);
    }
    Ok(written)
}
