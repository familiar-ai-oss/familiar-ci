---
description: The agent's recipe for the call to action — push the branch and open the pull request whose body shows what each requirement was proven by. Not something the person needs to type.
---

This is the AGENT's recipe. The person is never asked
to type it: when every criterion is green or proven on this laptop, the
hooks tell the agent the facts (`familiar-ai: every criterion is proven on
this laptop. CI proves it on the pull request. The branch is not pushed.
The person has not been asked whether to push and open the pull request.
The push begins with familiar-ai tidy, which keeps one seal commit and one
fix commit per criterion and drops the failed attempts.`), and the agent
asks the person in its own words. Only after the person says yes:

1. Run `familiar-ai status --pr`; its output is the pull request body.
2. Tidy, then push the current branch, as one command:
   `familiar-ai tidy && git push -u origin HEAD` (when the branch is
   already pushed, skip this step). `familiar-ai tidy` rewrites only the
   branch's unpushed commits that Familiar saw the agent make: per
   criterion, one seal commit (the tests as sealed) and one fix commit (the
   code as at green); the failed attempts are dropped; pushed commits, the
   `familiar-ai setup` commit and commits Familiar never saw (made by hand
   outside a session) stay as they were.
   It re-checks the proof before the branch moves and keeps the original
   head at `refs/familiar-ai/pre-tidy/<time>`. When it exits 1, nothing was
   changed and nothing is pushed: show the person its lines and stop.
3. Open the pull request with `gh pr create`. Its title is the request in
   a few words; its body is exactly the output of `familiar-ai status --pr`,
   pasted whole, with nothing added before or after it (pass it with
   `--body-file`). The page is Familiar's own, so it ends with Familiar's
   line: no generated-with footer.
   When `gh` is not installed, say so and give the person the compare URL
   that `git push` printed instead.
4. Report the pull request's URL to the person.

After the push, `familiar-ai status` and the next session start say
`CI is proving it`, then `proven ✓` or `CI found: <check> (see the PR)`.
