//! `read_all` stays fast when every run carries its full evidence (plan
//! Assumption 31, lane S3): 30 runs, each with a 2,000 + 6,000 character cut of
//! both streams.

use std::time::{Duration, Instant};

use familiar_ai_facts::{Record, RunEnded, RunReserved, RunStarted, Stream, Truncated};

fn stream(run: usize, name: &str) -> String {
    let line = |i: usize| format!("{name} {run:02} line {i:05} ........................................\n");
    let head: String = (0..30).map(line).collect::<String>().chars().take(2_000).collect();
    let tail: String = (100_000..100_100).map(line).collect::<String>().chars().take(6_000).collect();
    format!("{head}[familiar: 123456 chars dropped]\n{tail}")
}

#[test]
fn read_all_of_thirty_runs_with_full_tails_is_timed() {
    let dir = std::env::temp_dir().join(format!("familiar-ai-read-all-tails-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let record = Record::open(&dir.join("record")).expect("record");
    for run in 0..30 {
        let id = format!("r-{run:012x}");
        let at = run as f64;
        let reserved = RunReserved {
            run_id: id.clone(),
            criterion_id: format!("c{run}"),
            criterion_rev: "rev".into(),
            commit: format!("{run:040x}"),
            command: "pytest -q".into(),
            resources: vec!["tests".into()],
            wait: "1800".into(),
            by: "binary".into(),
            after: None,
        };
        record.append(&reserved, at, "t").expect("reserved");
        record.append(&RunStarted { run_id: id.clone(), pgid: "100".into(), pgroup_start: "1".into() }, at, "t").expect("started");
        let ended = RunEnded {
            run_id: id,
            outcome: "failed".into(),
            end: "exited".into(),
            exit_code: Some(1),
            signal: None,
            stdout_tail: Some(stream(run, "out")),
            stderr_tail: Some(stream(run, "err")),
            truncated: Some(vec![
                Truncated { stream: Stream::Stdout, total_bytes: 9_000_000, dropped_bytes: 600_000 },
                Truncated { stream: Stream::Stderr, total_bytes: 9_000_000, dropped_bytes: 600_000 },
            ]),
        };
        record.append(&ended, at, "t").expect("ended");
    }
    let size = std::fs::metadata(dir.join("record")).map(|m| m.len()).unwrap_or(0);
    let mut best = Duration::MAX;
    for _ in 0..5 {
        let started = Instant::now();
        let log = record.read_all().expect("read_all");
        best = best.min(started.elapsed());
        assert_eq!(log.len(), 90);
    }
    eprintln!("read_all: 30 runs with full tails, 90 facts, record {size} bytes: best of 5 {best:?}");
    assert!(best < Duration::from_secs(1), "read_all took {best:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
