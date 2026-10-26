//! The person's signed answer to a card, the witness CI trusts (plan §6.9,
//! path B; docs/decisions.md#the-passphrase-is-the-presence). On the laptop
//! `familiar-ai approve` decrypts the person's ed25519 key with the passphrase
//! read on stdin, signs the card's payload as SSHSIG in the namespace
//! `familiar-ai-approval`, writes `.familiar/approvals/<t>.json` and `.sig`
//! beside the plan, and records `hold_answered` naming it. In CI, `verify`
//! reads those files at HEAD and checks each against the allowed signers of
//! CI's own configuration; only a verified one becomes a `hold_answered`.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, PublicKey, SshSig};

use crate::door::claim_cmd::{claim_one, OneClaim};
use crate::door::project::{open_project, plain};
use crate::door::{setup, FAMILIAR_FOLDER, PLAN_FILE};
use crate::input::Surroundings;
use crate::teach::words::LEAD;
use crate::Outcome;

/// The SSHSIG namespace of an approval, so no other signature of the key
/// (a commit's, a file's) reads as one.
pub const NAMESPACE: &str = "familiar-ai-approval";
/// The folder beside the plan that holds the approvals.
pub const FOLDER: &str = "approvals";
/// CI's variable that names the allowed signers, one per line, in OpenSSH's
/// allowed_signers form (`<principal> <key type> <base64>`) or a bare public key.
pub const SIGNERS_VAR: &str = "FAMILIAR_AI_ALLOWED_SIGNERS";
/// The approval key's default place, in the person's home.
pub const DEFAULT_KEY: &str = ".ssh/familiar_ai_approve";

/// The refusal of `approve` from an agent's shell or session.
pub const REFUSAL: &str = "familiar-ai: not approved · familiar-ai approve is the person's: the person signs by running familiar-ai approve in a terminal of their own (Familiar's pane shows the command)";

/// The card being answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub criterion_id: String,
    pub hold: String,
    pub choice: String,
    pub run_id: String,
    /// The person's own words with the choice `typed`; empty otherwise.
    pub typed: String,
}

/// What an approval binds: the repository (its root commit), the card, its
/// run, the commit it was answered at and its `t`, a unique name only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Payload {
    pub repo: String,
    pub criterion_id: String,
    pub hold: String,
    pub choice: String,
    /// The person's own words; absent from an approval signed before the pane.
    #[serde(default)]
    pub typed: String,
    pub run_id: String,
    pub commit: String,
    pub t: u64,
    /// H-4: the label and deciding line of the run an `its_the_red` / `its_the_pass` confirms;
    /// absent from every other approval.
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub evidence: String,
}

/// What an approval of `its_the_red` / `its_the_pass` binds besides the card (H-4): the
/// can't-tell card's run, its label and its deciding line, read from the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmed {
    pub run_id: String,
    pub label: String,
    pub evidence: String,
}

/// The binding of a confirming choice on `c`'s can't-tell card in `log`: `Ok(None)` for any other
/// choice; `Err` when the card does not offer it there (`criterion::confirmable`).
pub fn confirm_binding(log: &familiar_ai_facts::Log, c: &str, hold: &str, choice: &str) -> Result<Option<Confirmed>, String> {
    use familiar_ai_rules::criterion::{confirmable, CONFIRMS};
    if hold != "run_unclassified" || !CONFIRMS.contains(&choice) {
        return Ok(None);
    }
    let v = familiar_ai_rules::View::new(log);
    let run_id = confirmable(&v, c, choice).ok_or_else(|| format!("{c}'s can't-tell card does not offer {choice} now: its_the_red is offered before the seal, on a run at the tests-first commit; its_the_pass after the seal"))?;
    let t = familiar_ai_rules::outcome::translation_of(&v, run_id).ok_or("the card's run has no translation")?;
    let said = |k: &str| t.fields.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(Some(Confirmed { run_id: run_id.to_string(), label: said("label"), evidence: deciding_line(&said("evidence")) }))
}

/// A run's deciding line as an approval binds it and CI compares it (H-4): the scratch copies'
/// paths (`…/familiar-ai-run-<pid>-<n>/`, `…/familiar-ai-verify-<pid>-<n>/`) made
/// project-relative, the digits of durations (`in 0.002s` → `in <t>s`, `(12ms)`) and of clock
/// times masked, whitespace collapsed. Everything else, the failing test and its line among it,
/// is compared as it is; the label is compared exactly.
pub fn deciding_line(s: &str) -> String {
    use std::sync::OnceLock;
    static COPY: OnceLock<regex::Regex> = OnceLock::new();
    static CLOCK: OnceLock<regex::Regex> = OnceLock::new();
    static DURATION: OnceLock<regex::Regex> = OnceLock::new();
    let copy = COPY.get_or_init(|| regex::Regex::new(r#"(?:file://)?[^\s'"`(]*familiar-ai-(?:run|verify)-\d+-\d+/"#).expect("the copy pattern"));
    let clock = CLOCK.get_or_init(|| {
        regex::Regex::new(r"\b\d{4}-\d{2}-\d{2}[T ]\d{1,2}:\d{2}(?::\d{2}(?:[.,]\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?|\b\d{1,2}:\d{2}:\d{2}(?:[.,]\d+)?\b").expect("the clock pattern")
    });
    let duration = DURATION.get_or_init(|| regex::Regex::new(r"\b\d+(?:[.,]\d+)?(\s?)(ns|µs|us|ms|s|sec|secs|seconds|min|mins|m|h)\b").expect("the duration pattern"));
    let s = copy.replace_all(s, "");
    let s = clock.replace_all(&s, "<time>");
    let s = duration.replace_all(&s, "<t>$1$2");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The body `approve` signs: the repository, the card, its run, the commit and `t`; for a
/// confirming choice also the run's label and deciding line, which CI reads again (H-4).
pub fn body(repo: &str, card: &Card, head: &str, t: u64, bound: Option<&Confirmed>) -> Value {
    let mut v = json!({
        "repo": repo, "criterion_id": card.criterion_id, "hold": card.hold, "choice": card.choice, "typed": card.typed,
        "run_id": bound.map_or(card.run_id.as_str(), |b| b.run_id.as_str()), "commit": head, "t": t,
    });
    if let Some(b) = bound {
        v["label"] = json!(b.label);
        v["evidence"] = json!(b.evidence);
    }
    v
}

impl Payload {
    /// The `hold_answered` this approval stands for.
    pub fn answer(&self) -> serde_json::Map<String, Value> {
        let v = json!({
            "criterion_id": self.criterion_id, "hold": self.hold, "choice": self.choice, "typed": self.typed,
            "run_id": self.run_id, "commit": self.commit, "approval": self.t.to_string(),
        });
        v.as_object().cloned().unwrap_or_default()
    }
}

fn refused(why: &str) -> Outcome {
    plain(1, String::new(), format!("{LEAD}not approved · {why}\n"))
}

/// `familiar-ai approve`: the passphrase is the first line of stdin.
pub fn run_approve(card: &Card, key: Option<&Path>, input: &str, around: &Surroundings) -> Outcome {
    if around.session_id.is_some() {
        return plain(1, String::new(), format!("{REFUSAL}\n"));
    }
    if !crate::teach::card::is_card_hold(&card.hold) {
        return refused(&format!("{} is not a card hold; the card holds are {}", card.hold, crate::teach::card::card_holds_words()));
    }
    let passphrase = input.lines().next().unwrap_or("");
    let key_path = match key {
        Some(k) => k.to_path_buf(),
        None => match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
            Some(home) => PathBuf::from(home).join(DEFAULT_KEY),
            None => return refused("no key given and no home to find ~/.ssh/familiar_ai_approve in: pass --key <file>"),
        },
    };
    let signing = match open_key(&key_path, passphrase) {
        Ok(k) => k,
        Err(why) => return refused(&why),
    };
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let Some(worktree) = familiar_ai_git::worktree_root(&root).ok().flatten() else {
        return refused("the project is not inside a git repository");
    };
    let Some(head) = familiar_ai_git::head_commit(&worktree).ok().flatten() else {
        return refused("the repository has no commit to bind the answer to");
    };
    let repo = match familiar_ai_git::root_commit(&worktree, &head) {
        Ok(r) => r,
        Err(e) => return refused(&format!("its first commit cannot be read: {e}")),
    };
    let folder = root.join(FAMILIAR_FOLDER).join(FOLDER);
    let mut t = match record.len() {
        Ok(n) => n,
        Err(e) => return refused(&crate::door::cannot("read its record", &e)),
    };
    // H-4: a confirming choice binds the card's run, its label and its deciding line
    let bound = match record.read_all().map_err(|e| crate::door::cannot("read its record", &e)).and_then(|log| confirm_binding(&log, &card.criterion_id, &card.hold, &card.choice)) {
        Ok(b) => b,
        Err(why) => return refused(&why),
    };
    // The record is opened again to append the answer.
    drop(record);
    while folder.join(format!("{t}.json")).exists() {
        t += 1;
    }
    let mut body = match serde_json::to_vec(&body(&repo, card, &head, t, bound.as_ref())) {
        Ok(b) => b,
        Err(e) => return refused(&e.to_string()),
    };
    body.push(b'\n');
    let sig = match signing.sign(NAMESPACE, HashAlg::Sha512, &body).and_then(|s| s.to_pem(LineEnding::LF)) {
        Ok(s) => s,
        Err(e) => return refused(&format!("the payload could not be signed: {e}")),
    };
    let (json_file, sig_file) = (folder.join(format!("{t}.json")), folder.join(format!("{t}.sig")));
    let written = std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&json_file, &body)).and_then(|()| std::fs::write(&sig_file, sig));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&json_file);
        return refused(&crate::door::cannot("write the approval", &e));
    }
    let (label, evidence) = bound.as_ref().map(|b| (b.label.clone(), b.evidence.clone())).unwrap_or_default();
    let run_id = bound.as_ref().map_or_else(|| card.run_id.clone(), |b| b.run_id.clone());
    let payload = Payload { repo, criterion_id: card.criterion_id.clone(), hold: card.hold.clone(), choice: card.choice.clone(), typed: card.typed.clone(), run_id, commit: head, t, label, evidence };
    let said = format!("{LEAD}approved and signed: {} (commit {} with the plan)\n", rel_to(&root, &json_file), FAMILIAR_FOLDER);
    answer(card, payload.answer(), &said, around, || {
        let _ = std::fs::remove_file(&json_file);
        let _ = std::fs::remove_file(&sig_file);
    })
}

/// `familiar-ai approve --unsigned`: the person's answer with no key yet,
/// recorded with no approval, so CI reads it as approval not verified.
pub fn run_unsigned(card: &Card, around: &Surroundings) -> Outcome {
    if around.session_id.is_some() {
        return plain(1, String::new(), format!("{REFUSAL}\n"));
    }
    if !crate::teach::card::is_card_hold(&card.hold) {
        return refused(&format!("{} is not a card hold; the card holds are {}", card.hold, crate::teach::card::card_holds_words()));
    }
    let mut fields = json!({ "criterion_id": card.criterion_id, "hold": card.hold, "choice": card.choice, "typed": card.typed });
    // H-4: a confirming choice is bound to the card's run, signed or not
    let bound = match open_project(around) {
        Ok((_, record)) => match record.read_all().map_err(|e| crate::door::cannot("read its record", &e)).and_then(|log| confirm_binding(&log, &card.criterion_id, &card.hold, &card.choice)) {
            Ok(b) => b,
            Err(why) => return refused(&why),
        },
        Err(out) => return out,
    };
    if let Some(b) = bound {
        fields["run_id"] = json!(b.run_id);
    }
    let said = format!("{LEAD}answered unsigned: CI reads it as approval not verified\n");
    answer(card, fields.as_object().cloned().unwrap_or_default(), &said, around, || ())
}

/// Record the person's answer as the picker does: `hold_answered`, then its
/// follow-ups (`follow_ups`); `undo` runs when nothing was recorded.
fn answer(card: &Card, fields: serde_json::Map<String, Value>, said: &str, around: &Surroundings, undo: impl FnOnce()) -> Outcome {
    let (root, record) = match open_project(around) {
        Ok(x) => x,
        Err(out) => return out,
    };
    let before = match record.read_all() {
        Ok(l) => l,
        Err(e) => return refused(&crate::door::cannot("read its record", &e)),
    };
    let follow = follow_ups(&before, &card.criterion_id, &card.hold, &card.choice);
    drop(record);
    match claim_one("hold_answered", &[], &Value::Object(fields).to_string(), around) {
        OneClaim::Appended { fact, tail } => {
            let said = format!("{said}{LEAD}claimed {}\n", fact.to_json());
            if !follow.is_empty() {
                if let Err(e) = append_follow_ups(&root, &follow, around) {
                    return plain(1, said, format!("{LEAD}{e}\n"));
                }
            }
            match tail.exit_code {
                0 => plain(0, format!("{said}{}", tail.stdout_text), String::new()),
                code => plain(code, said, tail.stderr_text),
            }
        }
        OneClaim::Refused(out) => {
            undo();
            out
        }
    }
}

fn append_follow_ups(root: &Path, follow: &[(String, Value)], around: &Surroundings) -> Result<(), String> {
    let record = crate::door::project::open_record(root, around.record_wait)
        .map_err(|e| crate::door::cannot("open its record", &e))?
        .ok_or_else(|| "Familiar cannot open its record".to_string())?;
    for (kind, fields) in follow {
        let fields = fields.as_object().cloned().unwrap_or_default();
        record.append_raw(kind, fields, around.now, crate::door::TENANT).map_err(|e| crate::door::cannot("write its record", &e))?;
    }
    let log = record.read_all().map_err(|e| crate::door::cannot("read its record", &e))?;
    crate::door::project_plan(root, &log)
}

/// The facts the picker appends after a card's answer, read on the record
/// before it (door::picker::write): a miscoped part answered
/// covered_by_dependency is closed as covered by the dependency it was.
pub fn follow_ups(before: &familiar_ai_facts::Log, criterion_id: &str, hold: &str, choice: &str) -> Vec<(String, Value)> {
    if hold != "miscoped" || choice != "covered_by_dependency" {
        return Vec::new();
    }
    familiar_ai_rules::dependency::miscoped_by(&familiar_ai_rules::View::new(before), criterion_id)
        .map(|dep| vec![("criterion_covered".to_string(), json!({ "criterion_id": criterion_id, "by_criterion": dep }))])
        .unwrap_or_default()
}

/// What the terminal gave for the passphrase.
pub enum Asked {
    /// The line typed, wiped when dropped.
    Typed(zeroize::Zeroizing<String>),
    /// ^C or ^D at the prompt.
    Cancelled,
    /// stdin is no terminal that can be set to not echo.
    NoTerminal,
}

/// The terminal as it was, put back on every way out of the prompt.
struct Restore(libc::termios);

impl Drop for Restore {
    fn drop(&mut self) {
        // SAFETY: fd 0's settings, as read before the prompt.
        unsafe { libc::tcsetattr(0, libc::TCSAFLUSH, &self.0) };
    }
}

/// The passphrase typed at the terminal on stdin, read a byte at a time with
/// echo and the terminal's own signals off: ^C and ^D cancel, backspace takes
/// the last character off, Enter ends it. The terminal is put back on every
/// path, a panic's unwinding included (`Restore`).
pub fn ask_passphrase() -> Asked {
    use std::io::Write;
    use zeroize::Zeroizing;
    // SAFETY: termios is plain data, read for fd 0 only.
    let mut was: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(0, &mut was) } != 0 {
        return Asked::NoTerminal;
    }
    let mut quiet = was;
    quiet.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG);
    quiet.c_cc[libc::VMIN] = 1;
    quiet.c_cc[libc::VTIME] = 0;
    eprint!("{LEAD}the passphrase of your approval key: ");
    let _ = std::io::stderr().flush();
    // SAFETY: as above.
    if unsafe { libc::tcsetattr(0, libc::TCSAFLUSH, &quiet) } != 0 {
        return Asked::NoTerminal;
    }
    let restore = Restore(was);
    let mut typed: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
    let cancelled = loop {
        let mut b = 0u8;
        // SAFETY: one byte into a local.
        let n = unsafe { libc::read(0, (&mut b as *mut u8).cast(), 1) };
        if n != 1 {
            break true;
        }
        match b {
            b'\r' | b'\n' => break false,
            3 | 4 => break true,
            8 | 127 => {
                typed.pop();
            }
            _ => typed.push(b),
        }
    };
    drop(restore);
    eprintln!();
    if cancelled {
        return Asked::Cancelled;
    }
    Asked::Typed(Zeroizing::new(String::from_utf8_lossy(&typed).into_owned()))
}

/// ^C or ^D at the passphrase prompt: nothing signed, nothing written.
pub fn cancelled() -> Outcome {
    refused("cancelled at the passphrase prompt; nothing was written")
}

fn rel_to(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

/// The person's key, decrypted: an encrypted ed25519 key only, since a key
/// with no passphrase is no proof that the person was there.
fn open_key(path: &Path, passphrase: &str) -> Result<PrivateKey, String> {
    let text = std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!(
            "no approval key at {}: make one in the setup step (/familiar-ai:setup), or answer unsigned (recorded, but CI reads it as approval not verified)",
            path.display()
        ),
        _ => format!("the key at {} cannot be read: {e}", path.display()),
    })?;
    let key = PrivateKey::from_openssh(&text).map_err(|e| format!("the key at {} is not an OpenSSH private key: {e}", path.display()))?;
    if key.algorithm() != Algorithm::Ed25519 {
        return Err(format!("the key at {} is not an ed25519 key", path.display()));
    }
    if !key.is_encrypted() {
        return Err(format!(
            "the key at {} has no passphrase, so it is no proof that you were there: make one with ssh-keygen -t ed25519 -f ~/.ssh/familiar_ai_approve in a terminal of your own",
            path.display()
        ));
    }
    if passphrase.is_empty() {
        return Err("no passphrase came on stdin: the passphrase is read on stdin only, never from the command line or the environment".into());
    }
    key.decrypt(passphrase).map_err(|_| format!("the passphrase does not open the key at {}", path.display()))
}

/// An approval at HEAD that a signer CI allows signed, bound to this repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub payload: Payload,
}

/// The approvals at HEAD: those verified, and for each criterion named by one
/// that is not, why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Witnessed {
    pub verified: Vec<Verified>,
    pub unverified: Vec<(String, String)>,
}

/// The allowed signers' public keys that may sign an approval now, read as
/// `ssh-keygen -Y verify` reads the file: `<principals> [options] <key type>
/// <base64>`, comments and blank lines skipped. A `cert-authority` line, a
/// key outside its `valid-after`/`valid-before` window, one whose
/// `namespaces=` leaves out `familiar-ai-approval`, or a line with an option
/// this cannot read signs nothing (fail closed).
pub fn allowed_keys(text: &str) -> Vec<PublicKey> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    allowed_keys_at(text, now)
}

/// `allowed_keys` at `now`, seconds since the epoch.
pub fn allowed_keys_at(text: &str, now: i64) -> Vec<PublicKey> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let words = split_outside_quotes(line, |c| c.is_whitespace());
        let Some(i) = words.iter().position(|w| w.starts_with("ssh-") || w.starts_with("ecdsa-") || w.starts_with("sk-")) else {
            continue;
        };
        let options: Vec<String> = words.get(1..i).unwrap_or_default().iter().flat_map(|w| split_outside_quotes(w, |c| c == ',')).collect();
        if !options.iter().all(|o| option_allows(o, now)) {
            continue;
        }
        if let Some(b64) = words.get(i + 1) {
            if let Ok(k) = PublicKey::from_openssh(&format!("{} {b64}", words[i])) {
                out.push(k);
            }
        }
    }
    out
}

/// The text split at `sep`, never inside double quotes; empty parts dropped.
fn split_outside_quotes(text: &str, sep: impl Fn(char) -> bool) -> Vec<String> {
    let (mut out, mut word, mut quoted) = (Vec::new(), String::new(), false);
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
            word.push(c);
        } else if !quoted && sep(c) {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Whether one allowed_signers option lets its key sign an approval at `now`.
fn option_allows(option: &str, now: i64) -> bool {
    let (name, value) = match option.split_once('=') {
        Some((n, v)) => (n.to_ascii_lowercase(), Some(v.trim_matches('"'))),
        None => (option.to_ascii_lowercase(), None),
    };
    match (name.as_str(), value) {
        ("namespaces", Some(list)) => {
            let patterns: Vec<&str> = list.split(',').map(str::trim).collect();
            !patterns.iter().any(|p| p.strip_prefix('!').is_some_and(|p| glob(p, NAMESPACE))) && patterns.iter().any(|p| !p.starts_with('!') && glob(p, NAMESPACE))
        }
        ("valid-after", Some(t)) => signers_time(t).is_some_and(|t| now >= t),
        ("valid-before", Some(t)) => signers_time(t).is_some_and(|t| now <= t),
        // cert-authority and anything unknown: no direct signer of an approval.
        _ => false,
    }
}

/// `pattern` with `*` and `?` matches all of `text`.
fn glob(pattern: &str, text: &str) -> bool {
    fn go(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some(b'?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), text.as_bytes())
}

/// An allowed_signers time, `YYYYMMDD[HHMM[SS]][Z]`, as seconds since the
/// epoch, read as UTC with or without the `Z`.
fn signers_time(t: &str) -> Option<i64> {
    let t = t.strip_suffix('Z').or_else(|| t.strip_suffix('z')).unwrap_or(t);
    if !matches!(t.len(), 8 | 12 | 14) || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize, b: usize| t.get(a..b).and_then(|s| s.parse::<i64>().ok());
    let (y, m, d) = (n(0, 4)?, n(4, 6)?, n(6, 8)?);
    let (hh, mm, ss) = (n(8, 10).unwrap_or(0), n(10, 12).unwrap_or(0), n(12, 14).unwrap_or(0));
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3_600 + mm * 60 + ss)
}

/// Read and check every approval committed at `head` beside the plan.
/// `signers` is CI's configuration; with none, nothing is verified.
pub fn witnessed(worktree: &Path, root: &Path, head: &str, signers: Option<&str>) -> Witnessed {
    let plan = setup::paths_in_repository(root, worktree).map(|(_, plan)| plan).unwrap_or_else(|_| format!("{FAMILIAR_FOLDER}/{PLAN_FILE}"));
    let dir = match plan.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{FOLDER}"),
        None => FOLDER.to_string(),
    };
    let names = familiar_ai_git::files_in(worktree, head, &dir).unwrap_or_default();
    let allowed = signers.map(allowed_keys).unwrap_or_default();
    let repo = familiar_ai_git::root_commit(worktree, head).ok();
    let mut out = Witnessed::default();
    for name in &names {
        let Some(stem) = name.strip_suffix(".json") else {
            continue;
        };
        let Ok(body) = familiar_ai_git::file_at(worktree, head, &format!("{dir}/{name}")) else {
            continue;
        };
        let criterion = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v.get("criterion_id").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        let sig = familiar_ai_git::file_at(worktree, head, &format!("{dir}/{stem}.sig")).ok();
        match check(&body, stem, sig.as_deref(), &allowed, repo.as_deref()) {
            Ok(payload) => out.verified.push(Verified { payload }),
            Err(why) => out.unverified.push((criterion, format!("{dir}/{name}: {why}"))),
        }
    }
    out
}

/// The local counterpart of [`witnessed`] for `status`, the status line and
/// the pane: the approvals committed at HEAD beside the plan that verify,
/// by the same check CI uses, against the allowed signers in the person's
/// own environment (`FAMILIAR_AI_ALLOWED_SIGNERS`), never a file in the
/// tree. With none configured, or no HEAD, nothing is verified.
pub fn approved_here(root: &Path) -> Vec<familiar_ai_rules::holds::Approved> {
    let signers = std::env::var(SIGNERS_VAR).ok().filter(|s| !s.trim().is_empty());
    let Some(signers) = signers else {
        return Vec::new();
    };
    let Some(worktree) = familiar_ai_git::worktree_root(root).ok().flatten() else {
        return Vec::new();
    };
    let Some(head) = familiar_ai_git::head_commit(&worktree).ok().flatten() else {
        return Vec::new();
    };
    witnessed(&worktree, root, &head, Some(&signers))
        .verified
        .into_iter()
        .map(|v| familiar_ai_rules::holds::Approved { t: v.payload.t.to_string(), criterion_id: v.payload.criterion_id, hold: v.payload.hold, choice: v.payload.choice })
        .collect()
}

fn check(body: &[u8], stem: &str, sig: Option<&[u8]>, allowed: &[PublicKey], repo: Option<&str>) -> Result<Payload, String> {
    let sig = sig.ok_or("no signature beside it")?;
    if allowed.is_empty() {
        return Err(format!("CI's configuration names no allowed signer ({SIGNERS_VAR})"));
    }
    let sig = SshSig::from_pem(sig).map_err(|e| format!("its signature cannot be read: {e}"))?;
    if sig.namespace() != NAMESPACE {
        return Err(format!("signed in the namespace {}, not {NAMESPACE}", sig.namespace()));
    }
    let signer = allowed.iter().find(|k| k.key_data() == sig.public_key()).ok_or("its signer is not one CI allows")?;
    signer.verify(NAMESPACE, body, &sig).map_err(|_| "its signature does not match it")?;
    let payload: Payload = serde_json::from_slice(body).map_err(|e| format!("it is not an approval: {e}"))?;
    if payload.t.to_string() != stem {
        return Err(format!("it names t {} in a file named {stem}", payload.t));
    }
    if Some(payload.repo.as_str()) != repo {
        return Err("it was signed for another repository".into());
    }
    if !crate::teach::card::is_card_hold(&payload.hold) {
        return Err(format!("{} is not a card hold", payload.hold));
    }
    Ok(payload)
}
