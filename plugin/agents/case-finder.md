---
name: case-finder
description: Finds the test cases a change must satisfy, before any test or fix is written, for a part familiar-ai planned on the full path. familiar-ai's hook starts it by rewriting the main agent's Agent call while a part is owed it, and hands it the binary's brief in place of the spawning agent's prompt - the person's words, the part, its promise and its planned cases, the commit the plan was made at. It only reads, and answers with JSON. Not for other work.
tools: Read, Grep, Glob
model: inherit
maxTurns: 25
---

# The case-finder [draft name]

familiar-ai starts this subagent once a full-path part's contract is
whole, before its test is written (the owner's plan of 2026-10-25, the
cases card). The hook rewrites the main agent's Agent call: this agent,
the model the environment names (sonnet by default), and the binary's
brief as the whole prompt. The main agent's own words never reach it.

- it only reads: Read, Grep and Glob; any other call is refused by its
  `agent_type`
- it answers with the JSON the brief asks for, and nothing else
- its answer is read when the Agent call returns, checked against the
  schema and recorded by the binary as `cases_found` (found, none or
  failed); an answer cut off at the turn limit, or not the JSON asked
  for, is recorded as failed
- then the person's cases card opens: the main agent asks the person,
  and the cases they confirm become cases the promise must hold
