//! Plan the rewrite `familiar-ai tidy` makes of the branch's unpushed commits,
//! before anything is written. Each criterion's agent commits, from after the
//! previous criterion's green up to its own, become one seal and one fix; every
//! other commit is kept.

use std::collections::BTreeSet;

use crate::door::git_program::CommitInfo;

pub use crate::door::setup::SETUP_MESSAGE;

/// One unpushed commit, as the plan reads it.
#[derive(Debug, Clone)]
pub struct Unpushed {
    pub info: CommitInfo,
    /// The paths it changed against its parent.
    pub paths: Vec<String>,
    /// Seen with `by: agent`, landing during the agent's own Bash call.
    pub by_agent: bool,
}

impl Unpushed {
    /// The agent's commit, other than setup's own.
    pub fn agents(&self) -> bool {
        self.by_agent && self.info.subject() != SETUP_MESSAGE
    }
}

/// A criterion whose seal and green both landed among the unpushed commits,
/// as indexes into them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tidied {
    pub criterion_id: String,
    pub seal: usize,
    pub green: usize,
}

/// One step of the rewrite, in the branch's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The commit at this index, as it was.
    Keep(usize),
    /// Criterion `crit`'s seal: `paths` as the commit at `from` holds them.
    Seal { crit: usize, from: usize, paths: Vec<String> },
    /// Criterion `crit`'s fix: every file its group's agent commits changed,
    /// as the commit at `from` holds them.
    Fix { crit: usize, from: usize, paths: Vec<String> },
    /// A failed attempt or an earlier seal of the agent's, left out.
    Drop(usize),
}

/// The steps, and for each unpushed commit the criterion whose group it is
/// in, `None` after the last tidied green.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    pub steps: Vec<Step>,
    pub group_of: Vec<Option<usize>>,
    pub crits: Vec<Tidied>,
}

impl Rewrite {
    pub fn dropped(&self) -> usize {
        self.steps.iter().filter(|s| matches!(s, Step::Drop(_))).count()
    }

    pub fn kept(&self) -> usize {
        self.steps.iter().filter(|s| matches!(s, Step::Keep(_))).count()
    }
}

/// The rewrite of `commits` for the criteria `crits`, or why there is none.
pub fn plan(commits: &[Unpushed], crits: &[Tidied]) -> Result<Rewrite, String> {
    let mut crits = crits.to_vec();
    crits.sort_by_key(|c| c.seal);
    let mut lo = 0usize;
    for c in &crits {
        if c.seal >= c.green || c.green >= commits.len() {
            return Err(format!("{}'s seal does not come before its green", c.criterion_id));
        }
        if c.seal < lo {
            return Err("two criteria's commits are interleaved".to_string());
        }
        if !commits[c.seal].agents() || !commits[c.green].agents() {
            return Err(format!("{}'s seal or green is not a commit of the agent's", c.criterion_id));
        }
        lo = c.green + 1;
    }
    let mut group_of: Vec<Option<usize>> = vec![None; commits.len()];
    let mut fix_paths: Vec<Vec<String>> = Vec::with_capacity(crits.len());
    let mut lo = 0usize;
    for (i, c) in crits.iter().enumerate() {
        let mut paths: BTreeSet<String> = BTreeSet::new();
        for (idx, u) in commits.iter().enumerate().take(c.green + 1).skip(lo) {
            group_of[idx] = Some(i);
            if u.agents() {
                paths.extend(u.paths.iter().cloned());
            }
        }
        fix_paths.push(paths.into_iter().collect());
        lo = c.green + 1;
    }
    let mut steps = Vec::with_capacity(commits.len());
    for (idx, u) in commits.iter().enumerate() {
        let step = if let Some(i) = crits.iter().position(|c| c.seal == idx) {
            Step::Seal { crit: i, from: idx, paths: u.paths.clone() }
        } else if let Some(i) = crits.iter().position(|c| c.green == idx) {
            Step::Fix { crit: i, from: idx, paths: fix_paths[i].clone() }
        } else if u.agents() && group_of[idx].is_some() {
            Step::Drop(idx)
        } else {
            Step::Keep(idx)
        };
        steps.push(step);
    }
    Ok(Rewrite { steps, group_of, crits })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(id: &str, subject: &str, paths: &[&str], by_agent: bool) -> Unpushed {
        Unpushed {
            info: CommitInfo {
                id: id.into(),
                tree: "t".into(),
                parents: vec!["p".into()],
                author: "a <a@x> 1 +0000".into(),
                committer: "a <a@x> 1 +0000".into(),
                message: format!("{subject}\n"),
            },
            paths: paths.iter().map(|p| p.to_string()).collect(),
            by_agent,
        }
    }

    #[test]
    fn the_failed_attempts_are_dropped_and_the_hand_made_and_setup_commits_kept() {
        let commits = vec![
            u("S", SETUP_MESSAGE, &[".gitignore"], true),
            u("T1", "tests", &["tests/t.sh"], true),
            u("T2", "tests again", &["tests/t.sh", ".familiar/plan"], true),
            u("F1", "try", &["src/a.sh"], true),
            u("H", "readme", &["README"], false),
            u("F2", "fix", &["src/a.sh", "src/b.sh"], true),
            u("P", "plan", &[".familiar/plan"], true),
        ];
        let r = plan(&commits, &[Tidied { criterion_id: "c1".into(), seal: 2, green: 5 }]).expect("plan");
        assert_eq!(
            r.steps,
            vec![
                Step::Keep(0),
                Step::Drop(1),
                Step::Seal { crit: 0, from: 2, paths: vec!["tests/t.sh".into(), ".familiar/plan".into()] },
                Step::Drop(3),
                Step::Keep(4),
                Step::Fix {
                    crit: 0,
                    from: 5,
                    paths: vec![".familiar/plan".into(), "src/a.sh".into(), "src/b.sh".into(), "tests/t.sh".into()]
                },
                Step::Keep(6),
            ]
        );
        assert_eq!(r.group_of, vec![Some(0), Some(0), Some(0), Some(0), Some(0), Some(0), None]);
        assert_eq!((r.dropped(), r.kept()), (2, 3));
    }

    #[test]
    fn two_criteria_in_turn_each_get_their_own_seal_and_fix() {
        let commits = vec![
            u("T1", "t1", &["tests/1"], true),
            u("F1", "f1", &["src/a"], true),
            u("T2", "t2", &["tests/2"], true),
            u("X", "x", &["src/b"], true),
            u("F2", "f2", &["src/b"], true),
        ];
        let crits = [Tidied { criterion_id: "c2".into(), seal: 2, green: 4 }, Tidied { criterion_id: "c1".into(), seal: 0, green: 1 }];
        let r = plan(&commits, &crits).expect("plan");
        assert_eq!(r.crits[0].criterion_id, "c1", "in the branch's order");
        assert_eq!(r.steps[3], Step::Drop(3));
        assert_eq!(r.steps[4], Step::Fix { crit: 1, from: 4, paths: vec!["src/b".into(), "tests/2".into()] });
    }

    #[test]
    fn interleaved_or_backwards_or_unseen_is_no_rewrite() {
        let commits = vec![u("T1", "t", &["t"], true), u("T2", "t", &["t"], true), u("F1", "f", &["a"], true), u("F2", "f", &["b"], false)];
        let a = Tidied { criterion_id: "c1".into(), seal: 0, green: 2 };
        let b = Tidied { criterion_id: "c2".into(), seal: 1, green: 2 };
        assert!(plan(&commits, &[a.clone(), b]).is_err());
        assert!(plan(&commits, &[Tidied { criterion_id: "c1".into(), seal: 2, green: 0 }]).is_err());
        assert!(plan(&commits, &[Tidied { criterion_id: "c1".into(), seal: 0, green: 3 }]).is_err(), "a green not the agent's");
        assert!(plan(&commits, &[a]).is_ok());
    }
}
