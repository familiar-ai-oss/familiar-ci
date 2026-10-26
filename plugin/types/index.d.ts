// The familiar-ai mod's $.state contract: what `familiar-ai status --json`
// printed last, and when it was last read.

// the rules' state, by name, beside the words `state` prints
export type FamiliarStateName = 'unplanned' | 'waiting' | 'testing' | 'sealed' | 'green' | 'proven' | 'held'

// the person's prompt an intent was claimed for: its time (h:mm), its
// words whole, its turn
export type FamiliarAsk = { at?: string | null; text: string; for?: number | null }

// one of a part's seven steps (plan · place · test · red · fix · green · CI):
// `done` true ●, false ○ (not done), null · (not reached), "half" ◐ (CI:
// locally verified, not hosted); `at` the short sha it was done at
export type FamiliarStep = {
  name: 'plan' | 'place' | 'test' | 'red' | 'fix' | 'green' | 'CI'
  done: boolean | null | 'half'
  at?: string
}

// a part's journey, as the binary derives it: the seven steps in order, its
// test files, its places, and whether the pane opens it
export type FamiliarJourney = {
  steps: FamiliarStep[]
  tests?: string[]
  places?: { file: string | null; symbol: string | null }[]
  opened?: boolean
}

// `id` is the agent's and the binary's; a person reads `name` ("part 2",
// "part 2 of your 12:36 ask")
export type FamiliarCriterion = {
  id: string
  name?: string | null
  part?: number | null
  ask?: FamiliarAsk | null
  text?: string
  state: string
  state_name?: FamiliarStateName
  holds?: string[]
  hold?: string | null
  holder?: string | null
  // who holds it, in words: "held by: opus, conversation cc9af915"
  held_by?: string | null
  judge?: string | null
  path?: string | null
  after?: string | null
  // absent from an older binary
  journey?: FamiliarJourney
  // each answer to one of its cards; `verified` only when the binary verified
  // its approval's signature (absent from an older binary)
  answers?: FamiliarAnswer[]
}

export type FamiliarAnswer = {
  hold: string
  choice: string
  approval?: string | null
  verified: boolean
  words?: string
}

export type FamiliarCard = {
  criterion_id: string
  name?: string | null
  hold?: string | null
  question: string
  choices?: string[]
  claim?: string | null
  // what a stronger model found on a two_tries_not_red card, with its proof
  finding?: FamiliarFinding | null
  // the can't-tell card's run in short sentences (H-4); empty on other cards,
  // absent from an older binary
  summary?: string[]
}

export type FamiliarFinding = {
  category?: string
  choice?: string | null
  model?: string
  reason?: string
  quotes?: { text?: string }[]
}

// the card the person picked a choice of in the pane; `cwd` the checkout's
// root for another checkout's card, null for this one
export type FamiliarPicking = { key: string; criterion_id: string; hold: string; choice: string; cwd: string | null }

// what the pane says after an unsigned answer: the binary's line, or why it was not recorded
export type FamiliarApproveSaid = { key: string; text: string }

// the counts the binary derives, once, for every surface; `needs_you` is every
// open card and everything waiting on the person
export type FamiliarCounts = { running: number; waiting: number; needs_you: number }

export type FamiliarRun = {
  run_id: string
  criterion_id: string
  name?: string | null
  commit: string
  in_flight: boolean
  outcome: 'passed' | 'failed' | 'did_not_run' | 'cancelled' | null
}

// a prompt the person typed and what it became: an ask (its number), an
// answer, or a note
export type FamiliarSaid = {
  for: number
  at?: string | null
  text: string
  became: 'ask' | 'answer' | 'note'
  ask?: number | null
}

// an ask of another checkout with its parts in order
export type FamiliarCheckoutAsk = FamiliarAsk & { parts?: FamiliarCriterion[] }

// another checkout of the repository, as its own status --json gives it;
// `busy` alone when its record was in use
export type FamiliarCheckout = {
  folder: string
  name?: string
  branch?: string | null
  busy?: boolean
  unreadable?: string
  off?: boolean
  switch_from?: 'main' | 'here'
  main_off?: boolean | null
  mode?: string
  said?: FamiliarSaid[]
  asks?: FamiliarCheckoutAsk[]
  runs?: FamiliarRun[]
  cards?: FamiliarCard[]
  counts?: { needs_you: number }
  // the checkout's root, where its binary runs
  root?: string
}

export type FamiliarStatus = {
  mode: string
  intent?: { text?: string } | string | null
  criteria: FamiliarCriterion[]
  cards: FamiliarCard[]
  waiting_on_you?: number
  counts: FamiliarCounts
  runs?: FamiliarRun[]
  // the CI words to show, whenever a part is green or beyond
  ci?: string | null
  // how each runner's runs are read (plan A): the reader chosen and the
  // outcomes that ask, in words; null before any choice, absent from an older binary
  reading?: string | null
  // the person's off switch as this checkout obeys it: every hook lets everything through
  off?: boolean
  // whose switch `off` is: this checkout's own, or main's, which a worktree
  // with no switch of its own follows
  switch_from?: 'main' | 'here'
  // main's switch; null in main itself or when main is not set up
  main_off?: boolean | null
  // every prompt the person typed, oldest first
  said?: FamiliarSaid[]
  // every other checkout of the repository
  checkouts?: FamiliarCheckout[]
}

// the status as read, or `unavailable` with the one-line reason it could not
// run, parse or be checked
export type FamiliarReading = { status: FamiliarStatus } | { unavailable: true; reason?: string }

declare module 'claude-code' {
  interface PluginState {
    'familiar-ai': {
      reading: FamiliarReading | null
      refreshedAt: number
      switchFailed: string | null
      picking: FamiliarPicking | null
      typedWords: string
      approveSaid: FamiliarApproveSaid | null
    }
  }
}
