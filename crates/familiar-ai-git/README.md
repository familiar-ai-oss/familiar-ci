# familiar-ai-git

The git reads, in pure Rust through `gix`.

When a commit lands, the binary appends a `commit_seen` fact. This crate
computes its fields: the changed files, whether tests or code were touched,
the overlap with the anchor, and the tests' hash. It also reads a file or a
line range at a commit, maps a line range across a diff, says whether git
ignores a path, and writes the scratch copy for the revert check.

It keeps no state and reads no clock. The same commit content gives the
same answers, so a rebase changes nothing.

Three choices worth knowing:

- No C and no `git` process for reads. The binary runs the `git` program
  for its two writes, so a commit familiar-ai makes carries the person's
  hooks and signature
  ([two writes](../../docs/decisions.md#git-reads-in-pure-rust-two-writes-through-git)).
- `map_range` follows a line range across a diff, so lines that only moved
  down the file are not read as changed
  ([a pure shift](../../docs/decisions.md#the-anchor-is-a-whole-function-a-shift-is-not-a-move)).
- The scratch copy is where `verify` puts the old code back and runs the
  tests again, to see that they bite
  ([the revert check](../../docs/decisions.md#red-green-revert)).

## Use

Every function takes the repository path and explicit arguments, and
returns `Result<_, GitError>`.

```rust
let files = familiar_ai_git::changed(std::path::Path::new("."), "HEAD")?;
for file in &files {
    println!("{} ({} hunks)", file.path, file.hunks.len());
}
# Ok::<(), familiar_ai_git::GitError>(())
```

`src/lib.rs` lists every public item, and each doc comment says when to
call it.

## Tests

`cargo test -p familiar-ai-git` builds small repositories with the real
`git` program. Tests may run it; the library never does. Each read is
compared with git's own answer: `git show`, `git diff --name-only`,
`git rev-list` and `git check-ignore`. The line arithmetic in `overlap`,
`map_range` and the put-back is checked against hand-computed cases,
including edits at a range's edges and ranges deleted outright.
