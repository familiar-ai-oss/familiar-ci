---
name: setup
description: Use when the person runs /familiar-ai:setup, when familiar-ai is not installed on this machine and hooks are refusing work, or when a session starts with "this project is not set up". Installs and verifies the familiar-ai binary with the person's consent, offers to wire the pipeline check, then sets the project up with `familiar-ai setup`.
---

# Setup familiar-ai

`familiar-ai` is a separate command on this machine; this plugin only carries
the hooks. These steps install it. **The download is never silent**: show the
exact command, say what it fetches and from where, and run it only after the
person agrees.

The version this plugin is built for is `0.1.5-prod`. It is the `<version>`
in the steps below.

## Steps

First run `familiar-ai --version`.

- If it answers with the version this plugin is built for, the binary is
  already installed: go straight to step 8 (the project is set up per
  project).
- If it answers with another version, tell the person in one sentence:
  "familiar-ai <installed> is installed; this plugin is built for
  <version>." Then offer the update: steps 1 to 4, the same download,
  checksum verified, unpacked over the file in `~/.local/bin`,
  run only after they agree. If they decline, go to step 8 with the
  installed one.
  After the update, go on to step 6, where the binary must answer with
  `<version>`.
- If it does not answer, install it: start at step 1.

1. **Work out the machine.** Run `uname -s` and `uname -m`, then map to a
   release target:

   | uname -s | uname -m | target                        |
   |----------|----------|-------------------------------|
   | Linux    | x86_64   | `x86_64-unknown-linux-gnu`    |
   | Linux    | aarch64  | `aarch64-unknown-linux-gnu`   |
   | Darwin   | arm64    | `aarch64-apple-darwin`        |
   | Darwin   | x86_64   | `x86_64-apple-darwin`         |

   Anything else: stop, say there is no prebuilt target for it, and offer
   step 5 (`cargo install`) instead.

2. **Show the person the exact install command** and what it does, in plain
   words: it downloads `familiar-ai-<target>.tar.gz` and its
   `familiar-ai-<target>.tar.gz.sha256` from
   `https://github.com/familiar-ai-oss/familiar-ci/releases/download/v<version>/`,
   verifies the archive against the checksum, and unpacks the single
   `familiar-ai` binary into `~/.local/bin`. The command, with `<version>`
   (the version this plugin is built for, the release's tag without the
   `v`) and `<target>` from step 1 filled in, is exactly:

   ```sh
   curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v<version>/familiar-ai-<target>.tar.gz -o /tmp/familiar-ai-<target>.tar.gz && curl -fsSL https://github.com/familiar-ai-oss/familiar-ci/releases/download/v<version>/familiar-ai-<target>.tar.gz.sha256 -o /tmp/familiar-ai-<target>.tar.gz.sha256 && cd /tmp && sha256sum -c familiar-ai-<target>.tar.gz.sha256 && mkdir -p "$HOME/.local/bin" && tar xzf familiar-ai-<target>.tar.gz -C "$HOME/.local/bin" && chmod +x "$HOME/.local/bin/familiar-ai"
   ```

   (On macOS, where `sha256sum` may be absent, use
   `shasum -a 256 -c familiar-ai-<target>.tar.gz.sha256` in its place and say
   so in the shown command.)

   Ask the person: "Run this?" **Run it only after they agree.** If they
   decline, stop; offer step 5.

3. **The checksum is verified inside the command itself**
   (`sha256sum -c` aborts the chain on mismatch, so the binary is never
   installed unverified). If the chain stopped at the checksum step, say the
   download did not match its `.sha256` and do not retry silently.

4. **Put the binary on PATH.** It lands in `~/.local/bin`. Check whether that
   directory is on this shell's PATH; if it is not, tell the person plainly
   ("`~/.local/bin` is not on your PATH") and show the line for their shell
   profile.

5. **The other path.** If no release fits, or the person prefers building:
   the crate is on crates.io, so run `cargo install familiar-ai` (needs a
   Rust toolchain). Same consent rule: show it, then run it after they agree.

6. **Check the binary answers.** Run `familiar-ai --version`. If it does not
   answer, or after an update answers with a version other than
   `<version>`, stop and say so; do not proceed.

7. **Offer — do not just do — the pipeline step.** Ask the person whether to
   add the verify step to the project's pipeline file (e.g.
   `.github/workflows/`). The step is:

   ```yaml
   - uses: familiar-ai-oss/familiar-ci/action@v<version>
     with:
       version: <version>
   ```

   with `<version>` the installed one, in both places. Add it only if they
   agree.

8. **Set the project up.** familiar-ai keeps everything of a project under
   one folder of its own, `.familiar`, at the project's root; the
   hooks do nothing in a project without it, and a session started there is
   told so. Run, at the project's root, exactly:

   ```sh
   familiar-ai setup
   ```

   It makes the `.familiar` folder and, unless git already ignores it, adds
   the line `.familiar/record` to the project's `.gitignore` (the record
   stays on this machine and is never committed; the plan file beside it,
   `.familiar/plan`, IS committed, and so is `.familiar/adapter`, so the
   whole folder is never ignored).
   In a checkout whose committed plan has claims (a fresh worktree or
   clone, or a folder that was removed), it brings the plan back from git
   and starts the record from it: the plan's claims and the history,
   rechecked as `familiar-ai verify` does, its tests run; it says
   `.familiar/record starts from the committed plan: N parts, M commits
   rechecked`. With an empty committed plan the record starts empty.
   It also adds the always-visible status line to the project's
   `.claude/settings.json` when no `statusLine` is set there:
   `{"statusLine":{"type":"command","command":"familiar-ai statusline"}}`
   (a status line of the person's own is left as it is; the person's other
   settings keep their order). It adds the familiar-ai lines to the end of
   the project's `CLAUDE.md`, under the heading `## familiar-ai`, after one
   blank line, creating the file with those lines alone when there is none:
   what the plugin is, that the rulebook arrives at the session's start and
   again after compaction and resume, that `familiar-ai status` is the only
   picture, and that the switch and the question cards are the person's.
   Nothing the person wrote above is read or changed; a file that already
   has the heading is left alone (`CLAUDE.md already carries the familiar-ai
   lines`). Then it commits exactly its own edits
   (`.gitignore`, `.claude/settings.json`, `CLAUDE.md`) as one commit with the message
   `familiar-ai setup`, by the repository's configured user, so setup never
   rides along with work. It prints one line per thing done or
   already done, each beginning `familiar-ai:`, and is safe to run again:
   with nothing left to commit it says `already set up`.
   Show the person those lines. If it refuses, say so in its words and
   stop; it changed nothing. It refuses when the folder is not inside a git
   repository, when `.familiar/plan` would be ignored, when the index holds
   staged changes (it stashes nothing: the person commits or unstages
   them), when git does not know who the person is (`user.name`,
   `user.email`), or when `.gitignore`, `.claude/settings.json` or
   `CLAUDE.md` has an uncommitted change of the person's own.

   **How the test runs are read.** Once the project's test command is known
   (from the committed plan, or when the agent claims it), Familiar runs a
   dry run: small probe tests of its own through that runner, to choose how
   its runs are read. It says `familiar-ai: the dry run for <runner> started
   (run d-…); its choice of how the runner is read goes in .familiar/adapter`;
   show the person that line. `.familiar/adapter` is committed with the work,
   like the plan: the binary writes it, nobody edits it by hand, and it is
   never ignored. When the dry run cannot read some outcomes, the test setup
   gets one card naming the runner and the outcomes that will ask.

   **Sharing gap recordings, once.** Ask the person once whether the
   recordings of what a dry run could not read may be sent to Familiar's
   makers, so a later release reads them too. A recording is Familiar's own
   probe output, never the person's code; until an endpoint is named they
   wait in `.familiar/runs/gaps`. The yes or no is theirs, for this machine,
   typed in a terminal of their own: `familiar-ai share on` (after it,
   recordings are sent without asking again) or `familiar-ai share off`;
   never run `familiar-ai share on` yourself (an agent's shell is refused, as
   for `familiar-ai on` and `off`). Without a clear yes nothing is shared.

9. **Done.** Say the hooks will now hand every event to the binary, and that
   until the plan is complete the binary itself will refuse edits and name
   the line that is missing with the `familiar-ai claim` command that fills
   it — that is the tool working, not a setup failure. The `plan` skill says
   what to do when it refuses.

## Optional: the approval key

Offer this after step 9, once; it is the person's choice. A card the person
answers (a release, carrying on after the code moved) is recorded on this
machine, but CI never reads the record: in CI an answer counts only when the
person signed it with a key CI knows. Tell the person, in plain words:

- **What it buys:** CI honors their card answers. Each answer is signed with
  their key when they type its passphrase in Familiar's pane
  (`familiar-ai approve`), the signature is committed beside the plan in
  `.familiar/approvals/`, and CI checks it against the public key they gave
  it.
- **Without it:** a part whose card they answered fails closed in CI, as
  "approval not verified", and is not counted proven. Nothing else changes.
- **The two lines**, which they run themselves, **in a terminal of their own**
  — never through you, never through `!` in this session, never with `-N`
  (the passphrase would sit on a command line), and the key is never added to `ssh-agent`
  (the passphrase typed at each answer is what shows they were
  there):

  ```sh
  ssh-keygen -t ed25519 -f ~/.ssh/familiar_ai_approve
  ```

  ssh-keygen asks for a passphrase; a strong one, since the agent can read
  the encrypted file and the passphrase is all that stands between them.
  Then they add the public key, `~/.ssh/familiar_ai_approve.pub`, as an
  allowed signer in the repository's CI configuration: a secret or variable
  named `FAMILIAR_AI_ALLOWED_SIGNERS` (one line per signer,
  `<their email> <the .pub file's contents>`), mapped into the verify job's
  environment. Never a file in the repository: anything in the tree the
  agent could write.

Ask the person whether they want it. If they do, show the two lines and wait;
never run `ssh-keygen` yourself, never ask for or handle the passphrase, and
never edit `.github/` or the repository's settings (those are the person's).
If they decline, say that answered parts will read "approval not verified"
in CI, and go on.
