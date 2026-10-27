---
name: lens
description: One lens of the lens panel, or its synthesis, for a part familiar-ai planned on the full path, before any test or fix is written. familiar-ai's hook starts it by rewriting the main agent's Agent call while a part's panel is owed, and hands it the binary's brief in place of the spawning agent's prompt - the person's words, their typed answers, the part's plan lines and, for the synthesis, the five lenses' findings. A lens only reads; the synthesis has no tools. It answers with JSON. Not for other work.
tools: Read, Grep, Glob
model: inherit
maxTurns: 25
---

# A lens of the lens panel

familiar-ai starts this subagent once a full-path part's contract is
whole, before its test is written (the owner's plan of 2026-10-26, the
lens panel). Five lenses read the plan in parallel: ambiguity,
completeness, happy_path, edge_cases and order_and_coupling; then the
synthesis merges their findings. The hook rewrites each of the main
agent's Agent calls: this agent, the model the environment names
(`FAMILIAR_AI_LENS_MODEL`, opus by default), and the binary's brief for
the lens that call was released as, as the whole prompt. The main
agent's own words never reach it.

- a lens only reads: Read, Grep and Glob; any other call is refused by
  its `agent_type`; the synthesis has no tools
- it answers with the JSON its brief asks for, and nothing else
- a lens's answer is read when its Agent call returns, checked against
  the schema and recorded by the binary as `lens_found` (found, none or
  failed); the synthesis's as `cases_found`, with its cases and up to
  three questions for the person; an answer cut off at the turn limit,
  or not the JSON asked for, is recorded as failed
- then the person's cases card opens with the panel's questions: the
  main agent asks the person, and the cases they confirm become cases
  the promise must hold
