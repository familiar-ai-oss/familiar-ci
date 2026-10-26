import { expect, mock, test } from 'claude-code/testing'
import type { Engine } from 'claude-code/testing'
import type { On, RenderElement } from 'claude-code'

import { check } from '../hooks/status'

const PLUGIN = 'familiar-ai'
const SCROLL = { offset: 0, bodyRows: 10 }
const BAND = {
  plugin: PLUGIN,
  surface: 'terminal',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 200, scroll: SCROLL, view: {} },
} as const
const PANE = {
  plugin: PLUGIN,
  surface: 'terminal',
  component: 'Pane',
  requestId: 'familiar',
  props: { title: 'Familiar', isFocused: false, bodyColumns: 120, placement: 'dock', scroll: SCROLL, view: {} },
} as const

// one part green, one card: what `status --json` prints for it
const STATUS = {
  mode: 'proving',
  intent: { text: 'greet says hello' },
  criteria: [
    { id: 'c1', name: 'part 1', part: 1, ask: { at: '12:36', text: 'greet says hello' }, text: 'greet says hello', state: 'locally green', state_name: 'green' },
    { id: 'c2', name: 'part 2', part: 2, ask: { at: '12:36', text: 'greet says hello' }, text: 'bye says goodbye', state: 'two tries failed', state_name: 'held' },
  ],
  cards: [{ criterion_id: 'c2', name: 'part 2', question: 'What now?' }],
  counts: { running: 0, waiting: 0, needs_you: 1 },
  runs: [],
  ci: 'CI pending',
  off: false,
}
const json = (s: unknown) => JSON.stringify(s)

type Answer = { exitCode: number; stdout?: string; stderr?: string } | { deny: string }

// `familiar-ai` answers from `answers`, one per run, the last repeated
const world = (on: On, answers: Answer[]) => {
  const runs: string[][] = []
  mock.clock(on, { now: 1_000_000 })
  on('process.run', async ($, e) => {
    runs.push([...e.argv])
    const a = answers[Math.min(runs.length - 1, answers.length - 1)]
    if ('deny' in a) return { deny: a.deny }
    return { value: { exitCode: a.exitCode, stdout: a.stdout ?? '', stderr: a.stderr ?? '', isStdoutTruncated: false, isStderrTruncated: false } }
  })
  on('command.register', ($, e) => ({ value: { command: e.name } }))
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('turn.complete', () => ({ text: '' }))
  on('ui.render', { component: 'PromptHint' }, ($, e) => {
    const { Box, Text } = $.ui.resolve(e)
    return h(Box, { key: 'hint' }, h(Text, null, `${e.props.hint}${e.props.tail === undefined ? '' : ` ${e.props.tail}`}`)) as RenderElement
  })
  on('ui.render', { component: 'AbovePrompt' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
  on('ui.render', { component: 'Pane' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
  return runs
}

const ok = (s: unknown) => ({ exitCode: 0, stdout: json(s) })
const start = ($: Engine) => $.session.start({ cwd: '/project', surface: 'terminal', isInteractive: true })
const turnEnds = ($: Engine) => $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't', reason: 'answer' })
const row = async ($: Engine, mount: typeof BAND | typeof PANE, key: string) => {
  const ui = await $.ui.mount(mount)
  const found = await ui.find({ key })
  return { ui, text: found?.text }
}
const hint = async ($: Engine) => {
  const ui = await $.ui.mount({ plugin: PLUGIN, surface: 'terminal', component: 'PromptHint', props: { isDraft: false, isWorking: false, hint: '?' } })
  return (await ui.find({ key: 'hint' }))?.text
}

test('the band reads the binary count and CI words as they are', async ($, on) => {
  world(on, [ok({ ...STATUS, counts: { running: 0, waiting: 0, needs_you: 3 }, ci: 'CI: no run for a1b2c3d' })])
  await start($)
  expect((await row($, BAND, 'line')).text).toBe('Familiar · 3 need you · CI: no run for a1b2c3d')
})

test('no ci words from the binary: the band shows none, a green part or not', async ($, on) => {
  world(on, [ok({ ...STATUS, ci: null })])
  await start($)
  expect((await row($, BAND, 'line')).text).toBe('Familiar · 1 needs you')
})

test('a failed turn off shows the command\'s own words on the band until a read succeeds', async ($, on) => {
  const runs = world(on, [ok(STATUS), { exitCode: 1, stderr: 'familiar-ai: the switch could not be written\n' }, ok(STATUS)])
  await start($)
  const { ui } = await row($, BAND, 'line')
  await ui.press({ key: 'turn-off' })
  expect(runs).toEqual([
    ['familiar-ai', 'status', '--json'],
    ['familiar-ai', 'off'],
  ])
  expect((await row($, BAND, 'switch-failed')).text).toBe('familiar-ai: the switch could not be written')
  expect((await row($, BAND, 'line')).text).toBe('Familiar · 1 needs you · CI pending')
  await turnEnds($)
  expect((await row($, BAND, 'switch-failed')).text).toBeUndefined()
})

test('a turn on that cannot run shows why on the band', async ($, on) => {
  world(on, [ok({ ...STATUS, off: true }), { deny: 'familiar-ai: not found' }])
  await start($)
  const { ui } = await row($, BAND, 'turn-on')
  await ui.press({ key: 'turn-on' })
  expect((await row($, BAND, 'switch-failed')).text).toContain('familiar-ai: not found')
  expect((await row($, BAND, 'line')).text).toBe('Familiar is turned off for this project.')
})

test('a status the renderer cannot read is unavailable, with its reason in the pane', async ($, on) => {
  const broken = { ...STATUS, criteria: [{ ...STATUS.criteria[0], name: 2 }] }
  world(on, [ok(broken)])
  await start($)
  expect(await hint($)).toBe('? Familiar · status unavailable')
  expect((await row($, BAND, 'line')).text).toBeUndefined()
  expect((await row($, PANE, 'reason')).text).toBe('criteria[0].name is not text')
})

test('a status --json that fails says its own words in the pane', async ($, on) => {
  world(on, [{ exitCode: 1, stderr: 'familiar-ai: no record here\n' }])
  await start($)
  expect((await row($, PANE, 'reason')).text).toBe('familiar-ai: no record here')
})

test('check names the first field the band or the pane cannot read', () => {
  expect(check(STATUS)).toEqual(STATUS)
  expect(check({ mode: 'idle', intent: null, criteria: [], cards: [], counts: { running: 0, waiting: 0, needs_you: 0 } })).toEqual(
    expect.objectContaining({ mode: 'idle' }),
  )
  expect(check([])).toBe('status is not an object')
  expect(check({ ...STATUS, mode: 1 })).toBe('status.mode is not text')
  expect(check({ ...STATUS, counts: undefined })).toBe('counts is not an object')
  expect(check({ ...STATUS, counts: { running: 0, waiting: 0, needs_you: '1' } })).toBe('counts.needs_you is not number')
  expect(check({ ...STATUS, ci: 3 })).toBe('status.ci is not text')
  expect(check({ ...STATUS, off: 'no' })).toBe('status.off is not flag')
  expect(check({ ...STATUS, criteria: [{ ...STATUS.criteria[0], part: '1' }] })).toBe('criteria[0].part is not number')
  expect(check({ ...STATUS, criteria: [{ ...STATUS.criteria[0], state_name: 'done' }] })).toBe('criteria[0].state_name is not a state')
  expect(check({ ...STATUS, criteria: [{ ...STATUS.criteria[0], ask: { at: '12:36' } }] })).toBe('criteria[0].ask.text is not text')
  expect(check({ ...STATUS, cards: [{ name: 'part 2', question: 'What now?' }] })).toBe('cards[0].criterion_id is not text')
  expect(check({ ...STATUS, runs: [{ run_id: 'r1', commit: 'a1b2c3d' }] })).toBe('runs[0].in_flight is not flag')
  expect(check({ ...STATUS, said: [{ for: 1, text: 'hi', became: 'shout' }] })).toBe('said[0].became is not ask, answer or note')
  expect(check({ ...STATUS, intent: 4 })).toBe('intent is not text')
})

// ---- the journey: a table on top, the rail when a part is opened ----

const step = (name: string, done: boolean | null | 'half', at?: string) => (at ? { name, done, at } : { name, done })
const steps = (done: (boolean | null | 'half')[], at: Record<string, string> = {}) =>
  ['plan', 'place', 'test', 'red', 'fix', 'green', 'CI'].map((n, i) => step(n, done[i], at[n]))
const T = true
const F = false
const N = null
// the pane mounted once, and a lookup of a row's text by key
const paneText = async ($: Engine) => {
  const ui = await $.ui.mount(PANE)
  return async (key: string) => (await ui.find({ key }))?.text
}

// part 1 red and in play, part 2 waiting on it, part 3 held with a card, part 4 locally verified
const JOURNEY = {
  ...STATUS,
  criteria: [
    {
      id: 'c1', name: 'part 1', part: 1, ask: { at: '12:36', text: 'greet says hello' }, text: 'greet says hello',
      state: 'the test fails, as it should before the fix · the code comes next', state_name: 'sealed', held_by: 'held by: conversation 16976e81',
      journey: { steps: steps([T, T, T, T, F, F, F], { red: 'a1b2c3d' }), tests: ['tests/test_c1.sh'], places: [{ file: 'src/app.sh', symbol: 'greet' }], opened: true },
    },
    {
      id: 'c2', name: 'part 2', part: 2, ask: { at: '12:36', text: 'greet says hello' }, text: 'bye says goodbye',
      state: 'waits for "greet says hello"', state_name: 'waiting', after: 'c1',
      journey: { steps: steps([T, T, F, N, N, N, N]), tests: ['tests/test_c2.sh'], places: [{ file: 'src/app.sh', symbol: 'bye' }], opened: false },
    },
    {
      id: 'c3', name: 'part 3', part: 3, ask: { at: '12:36', text: 'greet says hello' }, text: 'the title reads familiar',
      state: 'two tries failed; this needs you', state_name: 'held', hold: 'two tries failed; this needs you',
      journey: { steps: steps([T, T, T, T, T, F, F], { red: '0f0f0f0', fix: 'e4f5a6b' }), tests: ['tests/test_c3.sh'], places: [{ file: 'src/title.sh', symbol: 'title' }], opened: true },
    },
    {
      id: 'c4', name: 'part 4', part: 4, ask: { at: '12:36', text: 'greet says hello' }, text: 'the footer names the year',
      state: 'locally verified', state_name: 'proven',
      journey: { steps: steps([T, T, T, T, T, T, 'half'], { red: '1a2b3c4', fix: '5d6e7f8', green: '5d6e7f8' }), tests: ['tests/test_c4.sh'], places: [], opened: false },
    },
  ],
  cards: [{ criterion_id: 'c3', name: 'part 3', question: 'What now?' }],
}

test('the pane draws one table line per part: its number, its stage, the seven steps and its standing words', async ($, on) => {
  world(on, [ok(JOURNEY)])
  await start($)
  const text = await paneText($)
  expect(await text('table-head-0')).toBe('part  stage      plan place test red fix green CI')
  expect(await text('table-c1')).toBe('1     red         ●    ●     ●    ●   ○   ○     ○   the test fails, as it should before the fix · the code comes next · held by: conversation 16976e81')
  expect(await text('table-c2')).toBe('2     waiting     ●    ●     ○    ·   ·   ·     ·   waits for part 1')
  expect(await text('table-c3')).toBe('3     needs you   ●    ●     ●    ●   ●   ○     ○   two tries failed; this needs you')
  expect(await text('table-c4')).toBe('4     verified    ●    ●     ●    ●   ●   ●     ◐   locally verified')
})

test('an opened part shows its words, the rail and where its test, place and seal are; a part not opened shows only its table line', async ($, on) => {
  world(on, [ok(JOURNEY)])
  await start($)
  const text = await paneText($)
  expect(await text('part-c1')).toBe('part 1  greet says hello')
  expect(await text('rail-c1')).toBe('        plan ── place ── test ── red ── fix ── green ── CI')
  expect(await text('rail-dots-c1')).toBe('         ●       ●        ●       ●      ○      ○        ○')
  expect(await text('detail-c1-test')).toBe('        test   tests/test_c1.sh')
  expect(await text('detail-c1-place')).toBe('        place  src/app.sh greet')
  expect(await text('detail-c1-red')).toBe('        red    a1b2c3d   held by: conversation 16976e81')
  expect(await text('detail-c1-green')).toBeUndefined()
  expect(await text('part-c3')).toBe('part 3  the title reads familiar')
  expect(await text('rail-dots-c3')).toBe('         ●       ●        ●       ●      ●      ○        ○')
  expect(await text('detail-c3-red')).toBe('        red    0f0f0f0')
  for (const key of ['part-c2', 'rail-c2', 'detail-c2-test', 'part-c4', 'rail-c4']) expect(await text(key)).toBeUndefined()
  // the card, the runs and CI follow as before
  expect(await text('card-c3')).toBe('part 3 needs you · the agent will ask you here')
  expect(await text('ci')).toBe('CI      pending')
})

test('a status without a journey, from an older binary, still draws the table with seven · and opens no part', async ($, on) => {
  world(on, [ok(STATUS)])
  await start($)
  const text = await paneText($)
  expect(await text('table-c1')).toBe('1     green       ·    ·     ·    ·   ·   ·     ·   locally green')
  expect(await text('table-c2')).toBe('2     needs you   ·    ·     ·    ·   ·   ·     ·   two tries failed')
  expect(await text('rail-c1')).toBeUndefined()
  expect(await text('part-c2')).toBeUndefined()
})

// another checkout: its ask, then the same table and rail as this checkout's, keys prefixed
const LANE = (parts: unknown[]) => ({
  folder: 'familiar-ai-oss-lane', name: 'lane', branch: 'lane-x', busy: false, mode: 'proving', said: [], runs: [], cards: [],
  asks: [{ at: '14:24', text: 'say why at each hook', for: 0, parts }],
})

test("another checkout's parts draw the same table and rail under its heading, keys prefixed by the checkout", async ($, on) => {
  const [c1, c2] = JOURNEY.criteria
  world(on, [ok({ ...STATUS, checkouts: [LANE([{ ...c1, ask: null }, { ...c2, ask: null }])] })])
  await start($)
  const text = await paneText($)
  expect(await text('checkout-0')).toBe('familiar-ai-oss-lane · lane-x')
  expect(await text('checkout-0-ask-0')).toBe('your 14:24 ask: say why at each hook')
  expect(await text('checkout-0-table-head-0')).toBe('part  stage      plan place test red fix green CI')
  expect(await text('checkout-0-table-c1')).toBe('1     red         ●    ●     ●    ●   ○   ○     ○   the test fails, as it should before the fix · the code comes next · held by: conversation 16976e81')
  expect(await text('checkout-0-table-c2')).toBe('2     waiting     ●    ●     ○    ·   ·   ·     ·   waits for part 1')
  expect(await text('checkout-0-part-c1')).toBe('part 1  greet says hello')
  expect(await text('checkout-0-rail-c1')).toBe('        plan ── place ── test ── red ── fix ── green ── CI')
  expect(await text('checkout-0-rail-dots-c1')).toBe('         ●       ●        ●       ●      ○      ○        ○')
  expect(await text('checkout-0-detail-c1-test')).toBe('        test   tests/test_c1.sh')
  expect(await text('checkout-0-detail-c1-red')).toBe('        red    a1b2c3d   held by: conversation 16976e81')
  for (const key of ['checkout-0-part-c2', 'checkout-0-rail-c2', 'checkout-0-row-c1', 'checkout-0-state-c1']) expect(await text(key)).toBeUndefined()
})

test("another checkout's parts from an older binary, with no journey, still draw the table with seven ·", async ($, on) => {
  const [c1] = STATUS.criteria
  world(on, [ok({ ...STATUS, checkouts: [LANE([{ ...c1, ask: null }])] })])
  await start($)
  const text = await paneText($)
  expect(await text('checkout-0-table-c1')).toBe('1     green       ·    ·     ·    ·   ·   ·     ·   locally green')
  expect(await text('checkout-0-rail-c1')).toBeUndefined()
})

test('the pane shows only the prompts that became an ask: 45 said lines, 2 asks, 2 lines; with none, nothing yet', async ($, on) => {
  const said = Array.from({ length: 45 }, (_, n) => ({
    for: n, at: `12:${String(n).padStart(2, '0')}`, text: `prompt ${n}`, became: n === 3 || n === 40 ? 'ask' : n % 2 ? 'answer' : 'note', ask: n === 3 || n === 40 ? 1 : null,
  }))
  world(on, [ok({ ...STATUS, said, checkouts: [{ ...LANE([]), said }] })])
  await start($)
  const ui = await $.ui.mount(PANE)
  const keys = (await ui.findAll({ type: 'Box' })).map(b => String(b.key))
  expect(keys.filter(k => /^said-\d+$/.test(k))).toEqual(['said-3', 'said-40'])
  expect(keys.filter(k => /^checkout-0-said-\d+$/.test(k))).toEqual(['checkout-0-said-3', 'checkout-0-said-40'])
})

test('the pane with only answers and notes said shows nothing yet', async ($, on) => {
  const said = [{ for: 1, at: '12:41', text: 'what?', became: 'answer', ask: null }]
  world(on, [ok({ ...STATUS, said })])
  await start($)
  const text = await paneText($)
  expect(await text('said-none')).toBe('nothing yet')
  expect(await text('said-1')).toBeUndefined()
})

test('check names a journey the pane cannot read', () => {
  expect(check(JOURNEY)).toEqual(JOURNEY)
  const c = JOURNEY.criteria[0]
  expect(check({ ...JOURNEY, criteria: [{ ...c, journey: 'red' }] })).toBe('criteria[0].journey is not an object')
  expect(check({ ...JOURNEY, criteria: [{ ...c, journey: { ...c.journey, steps: 7 } }] })).toBe('criteria[0].journey.steps is not list')
  expect(check({ ...JOURNEY, criteria: [{ ...c, journey: { ...c.journey, steps: [{ name: 'plan', done: 'yes' }] } }] })).toBe(
    'criteria[0].journey.steps[0].done is not true, false, null or half',
  )
})

// Unsigned answers flagged (the owner, 2026-10-17/18): a part cleared by an
// answer reads "answered, not verified" unless the binary verified its
// approval's signature; the pane reads only `verified`, never the words an
// agent could have written, as the person's verified approval.
const answered = (answers: unknown[]) => ({ ...STATUS, criteria: [{ ...STATUS.criteria[0], answers }, STATUS.criteria[1]] })

test('a part cleared with no verified approval is marked answered, not verified in the pane', async ($, on) => {
  world(on, [ok(answered([{ hold: 'two_failures', choice: 'release', approval: null, verified: false, words: 'answered, not verified' }]))])
  await start($)
  expect((await row($, PANE, 'table-c1')).text).toContain('answered, not verified')
})

test('a part cleared by a verified approval reads answered, verified <t>', async ($, on) => {
  world(on, [ok(answered([{ hold: 'two_failures', choice: 'release', approval: '7', verified: true, words: 'answered, verified 7' }]))])
  await start($)
  const text = (await row($, PANE, 'table-c1')).text
  expect(text).toContain('answered, verified 7')
  expect(text).not.toContain('not verified')
})

test('words that claim verified never make an unverified answer the person\'s verified approval', async ($, on) => {
  world(on, [ok(answered([{ hold: 'two_failures', choice: 'release', approval: '7', verified: false, words: 'answered, verified 7' }]))])
  await start($)
  const text = (await row($, PANE, 'table-c1')).text
  expect(text).toContain('answered, not verified')
  expect(text).not.toContain('verified 7')
})

test('check names an answer the pane cannot read', () => {
  expect(check(answered([{ hold: 'two_failures', choice: 'release', verified: 'yes' }]))).toBe('criteria[0].answers[0].verified is not flag')
})
