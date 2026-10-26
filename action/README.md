# familiar-ai action

The GitHub Action that runs `familiar-ai verify` on a pull request and uploads its
report. It installs the binary from the GitHub release of the version you give it,
checked against the release's checksum. `verify` reads only the clone and the
committed `.familiar/plan`: it checks out each commit the plan names, runs the
plan's test command there, and never reads a laptop's record.

## Usage

```yaml
jobs:
  familiar-verify:
    name: familiar-verify
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      # Install your project's test dependencies here.
      - uses: familiar-ai-oss/familiar-ci/action@v<version>
        with:
          version: <version>
```

Replace `<version>` in both places with the version you installed (`familiar-ai --version`),
without the leading `v`.
See [the complete workflow](examples/familiar-verify.yml).

- Keep the job named `familiar-verify` so Familiar can identify its check.
- Check out the PR's head commit with full history (`fetch-depth: 0`).
- Commit `.familiar/plan` and install your project's test dependencies before verification.
- Linux and macOS runners are supported on x86-64 and ARM64.

## Results

`verify` writes a job summary. The action uploads `.familiar/verify-report.json`
as the `familiar-verify-report` artifact, including on verification failure,
for `familiar-ai ci fetch` to retrieve. That fetch is the only way CI's result
reaches your machine: the band reads it from the record, never from GitHub,
and says `CI: no run for <sha>` until you run it.

| Exit code | Meaning |
| --- | --- |
| `0` | Every criterion is proven or released. |
| `1` | At least one criterion is not proven or released. |
| `2` | Verification could not complete: missing plan or test command, shallow history, or tests that could not run at the tests-first commit. |
