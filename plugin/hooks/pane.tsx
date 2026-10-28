import type { ElementTable } from 'claude-code'

import type { FamiliarAsk, FamiliarCard, FamiliarCheckout, FamiliarCriterion, FamiliarReading, FamiliarSaid, FamiliarStatus, FamiliarStep } from '../types'
import { IDLE_LINE, UNAVAILABLE_LINE, cardLine, offLine, visible } from './words'

// The pane: the mark, the off line, what the person said, and each ask with
// its parts as a table of seven steps and the rail of each opened part, then
// the cards, the runs, CI and how the test runs are read. A person reads a part by its name,
// never its id, and every text whole.

// the pane's label column
const LABEL = 8
const pad = (label: string) => label.padEnd(LABEL)

// what the person said, and what it became: an ask, an answer or a note
const YOU_SAID = 'you said'
const NOTHING_YET = 'nothing yet'
const became = (p: FamiliarSaid) => (p.ask ? `${p.became} ${p.ask}` : p.became)

// the pane's "you said" shows only the prompts that became an ask; answers and
// notes stay in status --json
const asked = (said: FamiliarSaid[] | undefined) => (said ?? []).filter(p => p.became === 'ask')

const heading =(a: FamiliarAsk | null) => (a?.at ? `your ${a.at} ask` : 'your ask')

// the asks in the order their first part comes, each with its parts; an
// intent with no part yet is its own ask
const groups = (s: FamiliarStatus) => {
  const out: { ask: FamiliarAsk | null; criteria: FamiliarCriterion[] }[] = []
  for (const c of s.criteria) {
    const key = JSON.stringify(c.ask ?? null)
    const found = out.find(g => JSON.stringify(g.ask) === key)
    if (found) found.criteria.push(c)
    else out.push({ ask: c.ask ?? null, criteria: [c] })
  }
  const text = typeof s.intent === 'string' ? s.intent : s.intent?.text
  if (out.length === 0 && text) out.push({ ask: { text }, criteria: [] })
  return out
}

// a part's standing words: what holds it, else where it stands and the judge
const standing = (s: FamiliarStatus, c: FamiliarCriterion) => {
  if (s.cards.some(k => k.criterion_id === c.id)) return c.hold ?? c.state
  const before = c.state_name === 'waiting' && c.after ? s.criteria.find(x => x.id === c.after)?.name : undefined
  if (before) return `waits for ${before}`
  const said = [c.state]
  if (c.held_by) said.push(c.held_by)
  else if (c.holder?.includes('/')) said.push('with a subagent')
  if (c.judge) said.push(`judged: ${c.judge}`)
  said.push(...answerFlags(c))
  return said.join(' · ')
}

// [draft names, the owner's word owed] a cleared hold's flag: verified only
// by the binary's `verified`, never by the words or an approval's name
const ANSWERED_VERIFIED = 'answered, verified'
const ANSWERED_NOT_VERIFIED = 'answered, not verified'
const answerFlags = (c: FamiliarCriterion) =>
  (c.answers ?? []).map(a => (a.verified === true && a.approval ? `${ANSWERED_VERIFIED} ${visible(a.approval)}` : ANSWERED_NOT_VERIFIED))

// ---- the journey: a table on top, the rail when a part is opened ----

// the seven steps, by the names the person reads
const STEPS = ['plan', 'place', 'test', 'red', 'fix', 'green', 'CI'] as const
// the table's part and stage columns
const PART_W = 6
const STAGE_W = 11
// the detail lines' label column, under the rail
const DETAIL_W = 7
const RAIL_JOIN = ' ── '

// a step's dot: ● done, ○ not done, · not reached, ◐ CI locally verified
const dot = (done: FamiliarStep['done'] | undefined) => (done === true ? '●' : done === false ? '○' : done === 'half' ? '◐' : '·')
// each step's done, in order; · for every step of a status with no journey
const dones = (c: FamiliarCriterion) => STEPS.map(n => c.journey?.steps.find(x => x.name === n)?.done ?? null)

// the stage word the table shows for a part's state
const stage = (s: FamiliarStatus, c: FamiliarCriterion) => {
  switch (c.state_name) {
    case 'unplanned':
      return 'planning'
    case 'sealed':
      return 'red'
    case 'proven':
      return 'verified'
    case 'held':
      return s.cards.some(k => k.criterion_id === c.id) ? 'needs you' : 'held'
    default:
      return c.state_name ?? ''
  }
}

const tableHead = (Box: Ui['Box'], Text: Ui['Text'], key: string) => (
  <Box key={key}>
    <Text>{'part'.padEnd(PART_W) + 'stage'.padEnd(STAGE_W) + STEPS.join(' ')}</Text>
  </Box>
)

// two lines per part: its number, its stage and the seven dots (each under
// the second letter of its step); then, under the part column, its words and
// its standing, which wrap (the owner, 2026-10-28: a narrow pane left the
// words no room beside the dots)
const tableRow = (Box: Ui['Box'], Text: Ui['Text'], s: FamiliarStatus, c: FamiliarCriterion, k: string) => {
  const d = dones(c)
  return [
    <Box key={`${k}table-${c.id}`}>
      <Text>{String(c.part ?? '').padEnd(PART_W) + stage(s, c).padEnd(STAGE_W)}</Text>
      {STEPS.map((n, i) => (
        <Text key={n} dimColor={d[i] === null}>
          {` ${dot(d[i])}`.padEnd(n.length + 1)}
        </Text>
      ))}
    </Box>,
    <Box key={`${k}words-${c.id}`}>
      <Box key={`${k}words-gap-${c.id}`} flexShrink={0}>
        <Text>{''.padEnd(PART_W)}</Text>
      </Box>
      <Box key={`${k}words-text-${c.id}`} flexGrow={1} flexShrink={1}>
        <Text>{[c.text, standing(s, c)].filter(Boolean).join(' · ')}</Text>
      </Box>
    </Box>,
  ]
}

// an opened part: its name and words, the rail with a dot under each step,
// then where its test, place, seal and green are; who holds it ends the last
const opened = (Box: Ui['Box'], Text: Ui['Text'], c: FamiliarCriterion, k: string) => {
  const j = c.journey
  if (!j) return []
  const d = dones(c)
  const at = (n: string) => j.steps.find(x => x.name === n)?.at
  let rail = ''
  const dots = STEPS.map((n, i) => {
    const start = STEPS.slice(0, i).reduce((w, x) => w + x.length + RAIL_JOIN.length, 0)
    const gap = ' '.repeat(Math.max(start + 1 - rail.length, 0))
    rail += gap + dot(d[i])
    return { gap, n, i }
  })
  const details: [string, string][] = []
  if ((j.tests ?? []).length > 0) details.push(['test', (j.tests ?? []).join(', ')])
  if ((j.places ?? []).length > 0) details.push(['place', (j.places ?? []).map(p => [p.file, p.symbol].filter(Boolean).join(' ')).join(' · ')])
  for (const n of ['red', 'green']) {
    const sha = at(n)
    if (sha) details.push([n, sha])
  }
  if (c.held_by && details.length > 0) details[details.length - 1][1] += `   ${c.held_by}`
  return [
    <Box key={`${k}part-${c.id}`}>
      <Text>{pad(c.name ?? 'a part')}</Text>
      <Text>{c.text ?? ''}</Text>
    </Box>,
    <Box key={`${k}rail-${c.id}`}>
      <Text>{pad('') + STEPS.join(RAIL_JOIN)}</Text>
    </Box>,
    <Box key={`${k}rail-dots-${c.id}`}>
      <Text>{pad('')}</Text>
      {dots.map(({ gap, n, i }) => (
        <Text key={n} dimColor={d[i] === null}>
          {gap + dot(d[i])}
        </Text>
      ))}
    </Box>,
    ...details.map(([label, words]) => (
      <Box key={`${k}detail-${c.id}-${label}`}>
        <Box key={`${k}detail-label-${c.id}-${label}`} flexShrink={0}>
          <Text>{pad('') + label.padEnd(DETAIL_W)}</Text>
        </Box>
        <Box key={`${k}detail-words-${c.id}-${label}`} flexGrow={1} flexShrink={1}>
          <Text>{words}</Text>
        </Box>
      </Box>
    )),
  ]
}

// one ask's parts, drawn the same wherever they live: the head, a table line
// per part, then each opened part's rail and detail lines. `k` prefixes every
// key ('' here, 'checkout-<i>-' for another checkout); `head` names the head's key.
const journeyRows = (Box: Ui['Box'], Text: Ui['Text'], s: FamiliarStatus, parts: FamiliarCriterion[], k: string, head: string) => [
  ...(parts.length > 0 ? [tableHead(Box, Text, `${k}${head}`)] : []),
  ...parts.flatMap(c => tableRow(Box, Text, s, c, k)),
  ...parts.filter(c => c.journey?.opened).flatMap(c => opened(Box, Text, c, k)),
]

// another checkout's heading: its folder and branch, and idle or busy when
// that is all there is to say
const checkoutHeading = (o: FamiliarCheckout) => {
  const head = o.branch ? `${o.folder} · ${o.branch}` : o.folder
  if (o.busy) return `${o.folder} · busy`
  if (o.unreadable) return `${o.folder} · its record cannot be read`
  if (o.mode === 'idle' || o.mode == null) return `${o.folder} · idle`
  return head
}

// one section per other checkout: what the person said there, then each ask
// with its parts and who holds each
const checkoutRows = (Box: Ui['Box'], Text: Ui['Text'], o: FamiliarCheckout, i: number, more: CardMore) => {
  const rows = [
    <Box key={`checkout-${i}`}>
      <Text bold>{checkoutHeading(o)}</Text>
    </Box>,
  ]
  if (o.busy || o.unreadable || o.mode === 'idle' || o.mode == null) return rows
  // that checkout's own parts and cards, for the stage and standing words
  const own = { cards: o.cards ?? [], criteria: (o.asks ?? []).flatMap(a => a.parts ?? []) } as unknown as FamiliarStatus
  for (const p of asked(o.said)) {
    rows.push(
      <Box key={`checkout-${i}-said-${p.for}`} paddingLeft={2}>
        <Text>{`${p.at ?? ''}  ${p.text}  · ${became(p)}`}</Text>
      </Box>,
    )
  }
  for (const [j, a] of (o.asks ?? []).entries()) {
    rows.push(
      <Box key={`checkout-${i}-ask-${j}`} paddingLeft={2}>
        <Text>{`${heading(a)}: ${a.text}`}</Text>
      </Box>,
    )
    rows.push(...journeyRows(Box, Text, own, a.parts ?? [], `checkout-${i}-`, `table-head-${j}`))
  }
  for (const c of o.cards ?? []) {
    const k = `checkout-${i}-${c.criterion_id}`
    rows.push(
      <Box key={`checkout-${i}-card-${c.criterion_id}`} paddingLeft={2}>
        <Text bold>{cardLine(c.name == null ? c.name : visible(c.name))}</Text>
      </Box>,
      <Box key={`checkout-${i}-question-${c.criterion_id}`} paddingLeft={2}>
        <Text>{visible(c.question)}</Text>
      </Box>,
      ...summaryRows(Box, Text, c, `checkout-${i}-`, 4),
      ...more(c, k, o.root ?? null),
    )
  }
  return rows
}

type Ui = Pick<ElementTable, 'Box' | 'Text'>

// H-4: the can't-tell card's run in short sentences, one row each after its
// question, escaped; none from an older binary or another card
const summaryRows = (Box: Ui['Box'], Text: Ui['Text'], c: FamiliarCard, prefix: string, indent: number) =>
  (c.summary ?? []).map((line, j) => (
    <Box key={`${prefix}summary-${c.criterion_id}-${j}`} paddingLeft={indent}>
      <Text>{visible(String(line))}</Text>
    </Box>
  ))

// a card's rows after its question (approve.tsx: its finding, choices and the
// command to sign it in a terminal); `k` keys the card, `cwd` is its checkout's root
type CardMore = (c: FamiliarCard, k: string, cwd: string | null) => JSX.Element[]
const NO_MORE: CardMore = () => []

export const paneTree = ({ Box, Text }: Ui, r: FamiliarReading | null, more: CardMore = NO_MORE) => {
  if (r === null || 'unavailable' in r) {
    if (r === null || !r.reason) return <Text dimColor>{UNAVAILABLE_LINE}</Text>
    return (
      <Box flexDirection="column">
        <Text dimColor>{UNAVAILABLE_LINE}</Text>
        <Box key="reason">
          <Text dimColor>{r.reason}</Text>
        </Box>
      </Box>
    )
  }

  const s = r.status
  const running = (s.runs ?? []).filter(run => run.in_flight)

  return (
    <Box flexDirection="column">
      <Box flexDirection="column" key="mark">
        <Text>{'    ┌─────┐'}</Text>
        <Text>{'  ╭─┼─╮   │    F A M I L I A R'}</Text>
        <Text>{'  │ └─┼───┘    a familiar ai.'}</Text>
        <Text>{'  ╰───╯'}</Text>
      </Box>
      <Text> </Text>
      {s.off && (
        <Box key="off">
          <Text>{offLine(s.main_off)}</Text>
        </Box>
      )}
      {s.mode === 'idle' && <Text dimColor>{IDLE_LINE}</Text>}
      <Box key="said">
        <Text bold>{YOU_SAID}</Text>
      </Box>
      {asked(s.said).length === 0 ? (
        <Box key="said-none" paddingLeft={2}>
          <Text dimColor>{NOTHING_YET}</Text>
        </Box>
      ) : (
        asked(s.said).map(p => (
          <Box key={`said-${p.for}`}>
            <Box key={`said-at-${p.for}`} width={LABEL} flexShrink={0}>
              <Text>{p.at ?? ''}</Text>
            </Box>
            <Box key={`said-words-${p.for}`} flexGrow={1} flexShrink={1}>
              <Text>{p.text}</Text>
            </Box>
            <Box key={`said-became-${p.for}`} flexShrink={0} paddingLeft={2}>
              <Text>{became(p)}</Text>
            </Box>
          </Box>
        ))
      )}
      {groups(s).flatMap((g, i) => [
        <Box key={`ask-${i}`}>
          <Text bold>{heading(g.ask)}</Text>
        </Box>,
        ...(g.ask?.text
          ? [
              <Box key={`ask-words-${i}`} paddingLeft={2}>
                <Text>{g.ask.text}</Text>
              </Box>,
            ]
          : []),
        ...journeyRows(Box, Text, s, g.criteria, '', `table-head-${i}`),
      ])}
      {s.cards.flatMap(c => [
        <Box key={`card-${c.criterion_id}`}>
          <Text bold>{cardLine(c.name == null ? c.name : visible(c.name))}</Text>
        </Box>,
        <Box key={`question-${c.criterion_id}`}>
          <Text>{visible(c.question)}</Text>
        </Box>,
        ...summaryRows(Box, Text, c, '', 2),
        ...more(c, c.criterion_id, null),
      ])}
      {running.map((run, i) => (
        <Box key={`run-${run.run_id}`}>
          <Text>{pad(i === 0 ? 'runs' : '')}</Text>
          <Text>{`${run.name ?? 'a part'} · running its test at ${run.commit}`}</Text>
        </Box>
      ))}
      {s.ci && (
        <Box key="ci">
          <Text>{pad('CI')}</Text>
          <Text>{s.ci.replace(/^CI:? /, '')}</Text>
        </Box>
      )}
      {s.reading && (
        <Box key="reading">
          <Text>{pad('reading')}</Text>
          <Text>{visible(s.reading.replace(/^reading:? /, ''))}</Text>
        </Box>
      )}
      {(s.checkouts ?? []).flatMap((o, i) => checkoutRows(Box, Text, o, i, more))}
    </Box>
  )
}
