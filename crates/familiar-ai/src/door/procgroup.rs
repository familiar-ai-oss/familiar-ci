//! A run's process group, named by its number and its leader's start time so a
//! reused number is never taken for the run's group.

use std::time::{Duration, Instant};

/// Fields of `/proc/<pid>/stat` after the command name: (state, pgrp, starttime).
fn stat(pid: i32) -> Option<(char, i32, String)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = text.get(text.rfind(')')? + 1..)?;
    let f: Vec<&str> = rest.split_whitespace().collect();
    Some((f.first()?.chars().next()?, f.get(2)?.parse().ok()?, f.get(19)?.to_string()))
}

/// When process `pid` started (clock ticks since boot), as text.
pub fn start_of(pid: i32) -> Option<String> {
    stat(pid).map(|s| s.2)
}

/// Whether the group `pgid` that began at `start` still has a live member.
pub fn alive(pgid: i32, start: &str) -> bool {
    if let Some((_, _, s)) = stat(pgid) {
        if s != start {
            return false; // the number was reused: not our group
        }
    }
    let Ok(dir) = std::fs::read_dir("/proc") else { return false };
    dir.flatten().filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok()).any(|pid| {
        stat(pid).is_some_and(|(state, pg, _)| pg == pgid && state != 'Z' && state != 'X')
    })
}

/// Kill the group and wait up to `patience`; true when it is confirmed gone or was never ours.
pub fn kill_and_confirm(pgid: i32, start: &str, patience: Duration) -> bool {
    if pgid <= 1 {
        return true;
    }
    let until = Instant::now() + patience;
    loop {
        if !alive(pgid, start) {
            return true;
        }
        // Soundness: kill(2) with a negated pgid signals that group only; ESRCH is harmless.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
        if Instant::now() >= until {
            return !alive(pgid, start);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    #[test]
    fn a_group_is_killed_and_confirmed_gone_and_a_reused_number_is_not_ours() {
        let mut c = std::process::Command::new("sh").arg("-c").arg("sleep 30 & sleep 30").process_group(0).spawn().expect("spawn");
        let pgid = c.id() as i32;
        let start = start_of(pgid).expect("start time");
        assert!(alive(pgid, &start));
        assert!(!alive(pgid, "0"), "a different start is a different group");
        assert!(kill_and_confirm(pgid, &start, Duration::from_secs(5)));
        let _ = c.wait();
        assert!(!alive(pgid, &start));
    }
}
