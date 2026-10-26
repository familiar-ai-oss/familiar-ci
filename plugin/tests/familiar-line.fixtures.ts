// Hand-built objects in the shape of `familiar-ai status --json`, typed as `FamiliarStatus` so the contract holds their shape.
// Each scenario carries one line saying what it holds.

import type { FamiliarAsk, FamiliarCriterion, FamiliarRun, FamiliarStateName, FamiliarStatus } from '../types'

/** The status plus the fields the binary prints that the mod does not read. */
type Printed = FamiliarStatus & { plan: string | null; missing_line: string | null; next: string | null }

const HOLDER = '37f2ca69-c458-4c99-9159-08b55ebd6bac'

const TEXTS: Record<string, string> = {
  c1: 'greet says hello',
  c2: 'bye says goodbye',
  c3: 'the help text is kept as it is',
  c4: 'the title reads familiar',
  c5: 'the footer names the year',
}

/** The 12:36 ask every criterion in PROVING was claimed for. */
export const FIRST_ASK = { at: '12:36', text: 'keep the person informed in the TUI\nand allow an idle mode', for: 3 } satisfies FamiliarAsk

/** The 12:50 ask c4 and c5 belong to in TWO_ASKS. */
const SECOND_ASK = { at: '12:50', text: 'and make the title and the footer right', for: 9 } satisfies FamiliarAsk

/** A criterion of FIRST_ASK in the given state. */
const criterion = (id: string, state_name: FamiliarStateName, state: string, more: Partial<FamiliarCriterion> = {}): FamiliarCriterion => ({
  id,
  name: `part ${id.slice(1)}`,
  part: Number(id.slice(1)),
  ask: FIRST_ASK,
  text: TEXTS[id],
  state,
  state_name,
  holds: [],
  hold: null,
  holder: HOLDER,
  judge: 'not judged',
  path: 'full',
  after: null,
  ...more,
})

/** A run of a criterion's test; in flight while it has no outcome. */
const run = (run_id: string, criterion_id: string, commit: string, outcome: FamiliarRun['outcome'] = null): FamiliarRun => ({
  run_id,
  criterion_id,
  name: `part ${criterion_id.slice(1)}`,
  commit,
  in_flight: outcome === null,
  outcome,
})

export const QUESTION =
  '"the footer names the year" is held: two tries failed; this needs you. What now? plan it again, release it, or take it over yourself'

/** The intent as the binary prints it, with fields the contract does not name. */
const INTENT = { t: 1, text: FIRST_ASK.text, sha: 'a1b2c3d' }

const CLAIM = "familiar-ai claim hold_answered --criterion_id c5 --hold two_failures --choice try_again --typed ''"

// No intent, no criteria, nothing running.
export const IDLE: Printed = {
  mode: 'idle',
  intent: null,
  plan: null,
  missing_line: null,
  criteria: [],
  cards: [],
  next: null,
  waiting_on_you: 0,
  counts: { running: 0, waiting: 0, needs_you: 0 },
  runs: [],
}

// c1 testing and c2 sealed with runs in flight, c3 waiting on c2, c4 green, c5 held with a card.
export const PROVING: Printed = {
  mode: 'proving',
  intent: INTENT,
  plan: 'confirmed',
  missing_line: null,
  criteria: [
    criterion('c1', 'testing', 'the test comes first'),
    criterion('c2', 'sealed', 'the test fails, as it should before the fix · the code comes next'),
    criterion('c3', 'waiting', 'waits for "bye says goodbye"', { after: 'c2' }),
    criterion('c4', 'green', 'locally green'),
    criterion('c5', 'held', 'two tries failed; this needs you', { hold: 'two tries failed; this needs you', holds: ['two_failures'] }),
  ],
  cards: [
    { criterion_id: 'c5', name: 'part 5', hold: 'two_failures', question: QUESTION, choices: ['try_again', 'take_over', 'release'], claim: CLAIM },
  ],
  next: 'c2: the code comes next',
  ci: 'CI pending',
  waiting_on_you: 1,
  counts: { running: 2, waiting: 1, needs_you: 1 },
  runs: [run('r0', 'c4', '0f0f0f0', 'passed'), run('r1', 'c1', 'a1b2c3d'), run('r2', 'c2', 'e4f5a6b')],
}

/** PROVING's criteria with one criterion's fields replaced. */
const changed = (id: string, more: Partial<FamiliarCriterion>) => PROVING.criteria.map(c => (c.id === id ? { ...c, ...more } : c))

// PROVING with c2 green and its run passed.
export const C2_GREEN: Printed = {
  ...PROVING,
  criteria: changed('c2', { state_name: 'green', state: 'locally green' }),
  counts: { running: 1, waiting: 1, needs_you: 1 },
  runs: [run('r0', 'c4', '0f0f0f0', 'passed'), run('r1', 'c1', 'a1b2c3d'), run('r2', 'c2', 'e4f5a6b', 'passed')],
}

// PROVING with c1 sealed and its run failed.
export const C1_SEALED: Printed = {
  ...PROVING,
  criteria: changed('c1', { state_name: 'sealed', state: 'the test fails, as it should before the fix · the code comes next' }),
  counts: { running: 1, waiting: 1, needs_you: 1 },
  runs: [run('r0', 'c4', '0f0f0f0', 'passed'), run('r1', 'c1', 'a1b2c3d', 'failed'), run('r2', 'c2', 'e4f5a6b')],
}

// PROVING with c4 proven by the pipeline, so no criterion is green.
export const C4_PROVEN: Printed = { ...PROVING, criteria: changed('c4', { state_name: 'proven', state: 'proven by the pipeline' }) }

// PROVING with c4's state text changed and its state_name still green.
export const C4_REWORDED: Printed = { ...PROVING, criteria: changed('c4', { state: 'locally green, again' }) }

// PROVING without c4, so no criterion is green and the ci field is null.
export const NO_GREEN: Printed = { ...PROVING, ci: null, criteria: PROVING.criteria.filter(c => c.id !== 'c4') }

// PROVING with counts that disagree with its criteria and its one card.
export const COUNTS_DISAGREE: Printed = { ...PROVING, counts: { running: 0, waiting: 3, needs_you: 2 }, runs: [] }

// PROVING with only `next` changed, a field the mod does not draw.
export const NEXT_CHANGED: Printed = { ...PROVING, next: 'c2: the code comes next (the plan names line 41)' }

// PROVING with a ci field.
export const WITH_CI: Printed = { ...PROVING, ci: 'CI: no run for a1b2c3' }

/** Moves c4 and c5 to SECOND_ASK as its parts 1 and 2. */
const second = (c: FamiliarCriterion) => (Number(c.id.slice(1)) > 3 ? { ...c, ask: SECOND_ASK, part: Number(c.id.slice(1)) - 3 } : c)
/** Names a criterion by its part and its ask's time. */
const named = (c: FamiliarCriterion) => ({ ...c, name: `part ${c.part} of your ${c.ask?.at} ask` })

// PROVING split across two asks, c1 to c3 from FIRST_ASK and c4 to c5 from SECOND_ASK, with names carrying the ask time.
export const TWO_ASKS: Printed = {
  ...PROVING,
  criteria: PROVING.criteria.map(second).map(named),
  cards: PROVING.cards.map(c => ({ ...c, name: 'part 2 of your 12:50 ask' })),
  runs: (PROVING.runs ?? []).map(r => ({ ...r, name: `part ${r.criterion_id.slice(1)} of your 12:36 ask` })),
}

// PROVING with three prompts the person typed: one that became an ask, one an answer, one a note.
export const SAID: Printed = {
  ...PROVING,
  said: [
    { for: 3, at: '12:36', text: FIRST_ASK.text, became: 'ask', ask: 1 },
    { for: 5, at: '12:41', text: 'what does the help text say now?', became: 'answer', ask: null },
    { for: 7, at: '12:44', text: 'keep the words short, please', became: 'note', ask: null },
  ],
}

// IDLE with the off switch set.
export const OFF_IDLE: Printed = { ...IDLE, off: true }

// PROVING with the off switch set.
export const OFF_PROVING: Printed = { ...PROVING, off: true }

// main switched off, idle, with a lane at work: its words, its one ask with
// two parts and who holds each; and a second lane idle
export const MAIN_WITH_A_LANE: Printed = {
  ...IDLE,
  off: true,
  switch_from: 'here',
  main_off: null,
  checkouts: [
    {
      folder: 'familiar-ai-oss-hook-words',
      name: 'hook-words',
      branch: 'lane-hook-words',
      busy: false,
      off: false,
      switch_from: 'here',
      main_off: true,
      mode: 'proving',
      said: [{ for: 0, at: '14:24', text: 'say why at each hook', became: 'ask', ask: 1 }],
      asks: [
        {
          at: '14:24',
          text: 'say why at each hook',
          for: 0,
          parts: [
            criterion('c1', 'testing', 'the test comes first', { ask: null, held_by: 'held by: opus, conversation cc9af915' }),
            criterion('c2', 'waiting', 'waits for part 1', { ask: null, held_by: 'held by: a subagent (sonnet)' }),
          ],
        },
      ],
      runs: [],
      cards: [],
      counts: { needs_you: 0 },
    },
    { folder: 'familiar-ai-oss-docs', name: 'docs', branch: 'lane-docs', busy: false, mode: 'idle', said: [], asks: [], runs: [], cards: [] },
  ],
}

// a lane's card, as main's status --json lists it under the lane's checkout
const LANE_WITH_A_CARD = {
  folder: 'familiar-ai-oss-hook-words',
  name: 'hook-words',
  branch: 'lane-hook-words',
  busy: false,
  off: false,
  mode: 'proving',
  said: [],
  asks: [],
  runs: [],
  cards: [{ criterion_id: 'c1', name: 'part 1', hold: 'anchor_moved', question: '"greet says hello": the code at src/app.sh greet changed after the plan was made. What now?' }],
  counts: { needs_you: 1 },
}

// main switched off and idle, a lane's card open
export const OFF_WITH_A_LANE_CARD: Printed = { ...IDLE, off: true, checkouts: [LANE_WITH_A_CARD] }

// main proving with its own card, and a lane's card open
export const PROVING_WITH_A_LANE_CARD: Printed = { ...PROVING, checkouts: [LANE_WITH_A_CARD] }

// a worktree with no switch of its own, off as main is
export const WORKTREE_AS_MAIN: Printed = { ...PROVING, off: true, switch_from: 'main', main_off: true }

// a worktree switched off itself while main is on
export const WORKTREE_MAIN_ON: Printed = { ...PROVING, off: true, switch_from: 'here', main_off: false }

/** A status as the binary prints it. */
export const json = (status: unknown) => JSON.stringify(status, null, 2)
