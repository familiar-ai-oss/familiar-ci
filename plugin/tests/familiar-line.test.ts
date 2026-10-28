import { expect, mock, test } from 'claude-code/testing'
import type { Engine } from 'claude-code/testing'
import type { On, ProcessRunResult, RenderElement } from 'claude-code'

import {
  C1_SEALED,
  C2_GREEN,
  C4_PROVEN,
  C4_REWORDED,
  COUNTS_DISAGREE,
  FIRST_ASK,
  IDLE,
  MAIN_WITH_A_LANE,
  NEXT_CHANGED,
  NO_GREEN,
  OFF_IDLE,
  OFF_WITH_A_LANE_CARD,
  OFF_PROVING,
  PROVING,
  PROVING_WITH_A_LANE_CARD,
  QUESTION,
  SAID,
  TWO_ASKS,
  WITH_CI,
  WORKTREE_AS_MAIN,
  WORKTREE_MAIN_ON,
  json,
} from './familiar-line.fixtures'

// ---- helpers ----

/** The plugin id every mount names. */
const PLUGIN = 'familiar-ai'
/** A scroll position at the top with ten body rows. */
const SCROLL = { offset: 0, bodyRows: 10 }
/** Mount props for the band above the prompt. */
const BAND = {
  plugin: PLUGIN,
  surface: 'terminal',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 200, scroll: SCROLL, view: {} },
} as const
/** Mount props for the docked Familiar pane. */
const PANE = {
  plugin: PLUGIN,
  surface: 'terminal',
  component: 'Pane',
  requestId: 'familiar',
  props: {
    title: 'Familiar',
    isFocused: false,
    bodyColumns: 120,
    placement: 'dock',
    scroll: SCROLL,
    view: {},
  },
} as const

/** The line the band and pane show while off. */
const OFF_LINE = 'Familiar is turned off for this project.'
/** The Familiar mark the pane opens with. */
const MARK = [
  '    ┌─────┐',
  '  ╭─┼─╮   │    F A M I L I A R',
  '  │ └─┼───┘    a familiar ai.',
  '  ╰───╯',
]

/** One mocked answer to a `familiar-ai` process run. */
type Answer = string | { exitCode: number; stdout: string } | { slowMs: number; stdout: string } | 'no binary'

/** A process result with the given stdout and exit code. */
const ran = (stdout: string, exitCode = 0): ProcessRunResult => ({
  exitCode,
  stdout,
  stderr: '',
  isStdoutTruncated: false,
  isStderrTruncated: false,
})

/** Mocks the engine: process runs answer from `answers` in order (the last repeats), and runs, toasts, opened panes, commands and hint tails are recorded. */
const world = (on: On, answers: Answer[]) => {
  const seen = { runs: [] as string[][], toasts: [] as string[], opened: [] as string[], commands: [] as string[], tails: [] as (string | undefined)[] }
  const clock = mock.clock(on, { now: 1_000_000 })
  on('process.run', async ($, e) => {
    seen.runs.push([...e.argv])
    const a = answers[Math.min(seen.runs.length - 1, answers.length - 1)]
    if (a === undefined || a === 'no binary') return { deny: 'familiar-ai: not found' }
    if (typeof a === 'string') return { value: ran(a) }
    if ('slowMs' in a) {
      await clock.sleep(a.slowMs)
      return { value: ran(a.stdout) }
    }
    return { value: ran(a.stdout, a.exitCode) }
  })
  on('ui.toast', ($, e) => {
    seen.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.open', ($, e) => {
    seen.opened.push(e.id)
    return { value: { isPlaced: true } }
  })
  on('command.register', ($, e) => {
    seen.commands.push(e.name)
    return { value: { command: e.name } }
  })
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('turn.complete', () => ({ text: '' }))
  on('tool.call', () => ({ result: { stdout: '', stderr: '' } as never }))
  on('ui.render', { component: 'PromptHint' }, ($, e) => {
    seen.tails.push(e.props.tail)
    const { Box, Text } = $.ui.resolve(e)
    const shown = `${e.props.hint}${e.props.tail === undefined ? '' : ` ${e.props.tail}`}`
    return h(Box, { key: 'hint' }, h(Text, null, shown)) as RenderElement
  })
  // The engine's own band and pane render whenever the plugin passes.
  on('ui.render', { component: 'AbovePrompt' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
  on('ui.render', { component: 'Pane' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
  return { seen, clock }
}

/** Starts an interactive terminal session in /project. */
const start = ($: Engine) => $.session.start({ cwd: '/project', surface: 'terminal', isInteractive: true })

/** Completes one turn. */
const turnEnds = ($: Engine) =>
  $.turn.complete({ answer: '', durationMs: 1, isAborted: false, turnId: 't', reason: 'answer' })

/** The prompt hint's text, engine hint and plugin tail together. */
const hint = async ($: Engine) => {
  const ui = await $.ui.mount({
    plugin: PLUGIN,
    surface: 'terminal',
    component: 'PromptHint',
    props: { isDraft: false, isWorking: false, hint: '? for shortcuts' },
  })
  const text = (await ui.find({ key: 'hint' }))?.text
  await ui.unmount()
  return text
}

/** Mounts the band at the given width and returns its line, card and buttons (undefined when the plugin draws nothing). */
const band = async ($: Engine, bodyColumns = 200) => {
  const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, bodyColumns } })
  const tree = await ui.drawn().then(
    () => true,
    () => false,
  )
  return {
    ui,
    line: tree ? (await ui.find({ key: 'line' }))?.text : undefined,
    card: tree ? (await ui.find({ key: 'card' }))?.text : undefined,
    details: tree ? await ui.find({ key: 'details' }) : undefined,
    turnOff: tree ? await ui.find({ key: 'turn-off' }) : undefined,
    turnOn: tree ? await ui.find({ key: 'turn-on' }) : undefined,
    clear: tree ? await ui.find({ key: 'clear' }) : undefined,
  }
}

/** Mounts the pane and returns it with a lookup of a row's text by key. */
const paneRows = async ($: Engine) => {
  const pane = await $.ui.mount(PANE)
  const row = async (key: string) => (await pane.find({ key }))?.text
  return { pane, row }
}

/** Every text the person can read: the band at 40 columns, the hint and the pane. */
const texts = async ($: Engine) => {
  const said: string[] = []
  const b = await band($, 40)
  said.push(b.line ?? '', b.card ?? '')
  said.push((await hint($)) ?? '')
  const pane = await $.ui.mount(PANE)
  for (const t of await pane.findAll({ type: 'Text' })) said.push(t.text ?? '')
  return said
}

// ---- the band ----

// Idle draws the band too, so the off switch is always one key away.
test('idle: session start reads status --json; the band reads "Familiar · idle" with details and turn off, no card, no toast, and the hint is unchanged', async ($, on) => {
  const { seen } = world(on, [json(IDLE)])
  await start($)
  expect(seen.runs[0]).toEqual(['familiar-ai', 'status', '--json'])
  expect(await hint($)).toBe('? for shortcuts')
  const { line, card, details, turnOff, turnOn } = await band($)
  expect(line).toBe('Familiar · idle')
  expect(card).toBeUndefined()
  expect(details?.props.label).toBe('details')
  expect(turnOff?.type).toBe('Button')
  expect(turnOff?.props.label).toBe('turn off')
  expect(turnOn).toBeUndefined()
  expect(seen.toasts).toEqual([])
})

test('proving: the band line is "Familiar · 1 needs you · CI pending", the card names the held part, details is a button, the hint is unchanged and nothing is toasted', async ($, on) => {
  const { seen } = world(on, [json(PROVING)])
  await start($)
  const { line, card, details } = await band($)
  expect(line).toBe('Familiar · 1 needs you · CI pending')
  expect(card).toBe('part 5 needs you · the agent will ask you here')
  expect(details?.type).toBe('Button')
  expect(details?.props.label).toBe('details')
  expect(await hint($)).toBe('? for shortcuts')
  expect(seen.toasts).toEqual([])
})

// The renderer wraps long text, so the band never truncates it.
test('the card text is whole at 20 columns', async ($, on) => {
  world(on, [json(PROVING)])
  await start($)
  const { card } = await band($, 20)
  expect(card).toBe('part 5 needs you · the agent will ask you here')
})

test('the band line shows the ci field verbatim', async ($, on) => {
  world(on, [json(WITH_CI)])
  await start($)
  const { line } = await band($)
  expect(line).toBe('Familiar · 1 needs you · CI: no run for a1b2c3')
})

test('with no green criterion and no ci field, the band line has no CI segment', async ($, on) => {
  world(on, [json(NO_GREEN)])
  await start($)
  const { line } = await band($)
  expect(line).toBe('Familiar · 1 needs you')
})

// The binary derives the counts once for every surface, so the band must not recount.
test('the band line takes its needs-you number from counts, not from the cards', async ($, on) => {
  world(on, [json(COUNTS_DISAGREE)])
  await start($)
  expect((await band($)).line).toBe('Familiar · 2 need you · CI pending')
})

for (const [name, status] of [
  ['idle', IDLE],
  ['proving', PROVING],
  ['with ci', WITH_CI],
  ['two asks', TWO_ASKS],
  ['with said', SAID],
] as const) {
  test(`band, hint and pane text contain none of ⛔ ◆ ▸ ✓ … + and no criterion id (${name})`, async ($, on) => {
    world(on, [json(status)])
    await start($)
    const said = await texts($)
    expect(said.length).toBeGreaterThan(1)
    for (const t of said) {
      for (const bad of ['⛔', '◆', '▸', '✓', '…', '+']) expect(t.includes(bad)).toBe(false)
      expect(/\bc\d\b/.test(t)).toBe(false)
    }
  })
}

// ---- the pane ----

test('details and /familiar each open the pane, whose rows show the ask, each part and its state, the card question, in-flight runs and CI, and not the choices, claim, ended runs or intent', async ($, on) => {
  const { seen } = world(on, [json(WITH_CI)])
  await start($)
  expect(seen.commands).toEqual(['familiar'])
  const { ui } = await band($)
  await ui.press({ key: 'details' })
  expect(seen.opened).toEqual(['familiar'])
  const said = await $.command.run({
    command: 'familiar',
    args: '',
    origin: { kind: 'composer' },
    presentation: { isFullscreen: true, columns: 120 },
  })
  expect(seen.opened).toEqual(['familiar', 'familiar'])
  expect(said.text).toBe('Familiar: the tracked work is in the pane.')

  const { pane, row } = await paneRows($)
  expect(await row('ask-0')).toBe('your 12:36 ask')
  expect(await row('ask-words-0')).toBe('keep the person informed in the TUI\nand allow an idle mode')
  // one table line per part; this status carries no journey, so every step reads ·
  expect(await row('table-head-0')).toBe('part  stage      plan place test red fix green CI')
  expect(await row('table-c1')).toBe('1     testing     ·    ·     ·    ·   ·   ·     ·   the test comes first · judged: not judged')
  expect(await row('table-c3')).toBe('3     waiting     ·    ·     ·    ·   ·   ·     ·   waits for part 2')
  expect(await row('table-c4')).toBe('4     green       ·    ·     ·    ·   ·   ·     ·   locally green · judged: not judged')
  expect(await row('table-c5')).toBe('5     needs you   ·    ·     ·    ·   ·   ·     ·   two tries failed; this needs you')
  expect(await row('card-c5')).toBe('part 5 needs you · the agent will ask you here')
  expect(await row('question-c5')).toBe(QUESTION)
  // Definition change (B-6, S9e; the owner 2026-10-15): the pane draws the card's choices as
  // buttons, to answer it here; the claim line stays the agent's and is not drawn.
  expect(await row('choices-c5')).toBe('try againtake overreleasesomething else')
  expect(await row('claim-c5')).toBeUndefined()
  expect(await row('run-r1')).toBe('runs    part 1 · running its test at a1b2c3d')
  expect(await row('run-r2')).toBe('        part 2 · running its test at e4f5a6b')
  expect(await row('run-r0')).toBeUndefined()
  expect(await row('ci')).toBe('CI      no run for a1b2c3')
  expect(await row('intent')).toBeUndefined()
  for (const c of ['c1', 'c2', 'c3', 'c4', 'c5']) expect(await row(`table-${c}`)).toBeDefined()
  // no part of a status without a journey is opened
  expect(await row('part-c1')).toBeUndefined()
  expect(await pane.find({ key: 'rail-c2' })).toBeUndefined()
})

test('with two asks, the pane heads each with its time and numbers parts within it, and the card and run names carry the ask time', async ($, on) => {
  world(on, [json(TWO_ASKS)])
  await start($)
  expect((await band($)).card).toBe('part 2 of your 12:50 ask needs you · the agent will ask you here')
  const { row } = await paneRows($)
  expect(await row('ask-0')).toBe('your 12:36 ask')
  expect(await row('ask-1')).toBe('your 12:50 ask')
  expect(await row('ask-words-1')).toBe('and make the title and the footer right')
  expect(await row('table-head-1')).toBe('part  stage      plan place test red fix green CI')
  expect(await row('table-c3')).toBe('3     waiting     ·    ·     ·    ·   ·   ·     ·   waits for part 2 of your 12:36 ask')
  expect(await row('table-c4')).toBe('1     green       ·    ·     ·    ·   ·   ·     ·   locally green · judged: not judged')
  expect(await row('table-c5')).toBe('2     needs you   ·    ·     ·    ·   ·   ·     ·   two tries failed; this needs you')
  expect(await row('card-c5')).toBe('part 2 of your 12:50 ask needs you · the agent will ask you here')
  expect(await row('run-r1')).toBe('runs    part 1 of your 12:36 ask · running its test at a1b2c3d')
})

for (const [name, status] of [['idle', IDLE], ['off', OFF_IDLE]] as const) {
  test(`the pane's first four texts are the Familiar mark, then a blank line (${name})`, async ($, on) => {
    world(on, [json(status)])
    await start($)
    const { pane } = await paneRows($)
    const texts = (await pane.findAll({ type: 'Text' })).map(t => t.text)
    expect(texts.slice(0, 4)).toEqual(MARK)
    expect(texts[4]?.trim()).toBe('')
    expect(await pane.find({ key: 'mark' })).toBeDefined()
  })
}

// ---- the status line ----

for (const [why, answer] of [
  ['no binary', 'no binary'],
  ['a non-zero exit', { exitCode: 2, stdout: '' }],
  ['bad JSON', '{ not json'],
] as const) {
  test(`when status --json fails (${why}), the hint ends "Familiar · status unavailable", the band has no line or details, and nothing is toasted`, async ($, on) => {
    const { seen } = world(on, [answer])
    await start($)
    expect(await hint($)).toBe('? for shortcuts Familiar · status unavailable')
    const { line, details } = await band($)
    expect(line).toBeUndefined()
    expect(details).toBeUndefined()
    expect(seen.toasts).toEqual([])
  })
}

// ---- polling ----

test('each turn end re-reads status, and a status differing only in next leaves the band line unchanged with no toast', async ($, on) => {
  const { seen } = world(on, [json(PROVING), json(PROVING), json(NEXT_CHANGED)])
  await start($)
  const before = (await band($)).line
  await turnEnds($)
  await turnEnds($)
  expect(seen.runs).toHaveLength(3)
  expect((await band($)).line).toBe(before)
  expect(seen.toasts).toEqual([])
})

// The band is the one surface that changes, so state transitions never toast.
test('reads through idle, proving, sealed, green, reworded and proven raise no toast, and the band after a proven part still says CI pending', async ($, on) => {
  const { seen } = world(on, [json(IDLE), json(PROVING), json(C1_SEALED), json(C2_GREEN), json(C4_REWORDED), json(C4_PROVEN)])
  await start($)
  for (let i = 0; i < 5; i += 1) await turnEnds($)
  expect(seen.runs).toHaveLength(6)
  expect(seen.toasts).toEqual([])
  expect((await band($)).line).toBe('Familiar · 1 needs you · CI pending')
})

test('a tool call refreshes status after its result, at most once per second, without denying the call', async ($, on) => {
  const { seen, clock } = world(on, [json(IDLE), json(PROVING)])
  await start($)
  expect(seen.runs).toHaveLength(1)
  await clock.advance(1500)
  const result = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(result.deny).toBeUndefined()
  await clock.settle()
  expect(seen.runs).toHaveLength(2)
  await $.tool.call({ tool: 'Bash', command: 'ls' })
  await clock.settle()
  expect(seen.runs).toHaveLength(2)
  await clock.advance(1500)
  await $.tool.call({ tool: 'Bash', command: 'ls' })
  await clock.settle()
  expect(seen.runs).toHaveLength(3)
  expect((await band($)).line).toBe('Familiar · 1 needs you · CI pending')
})

test('polling refreshes every 5 s while proving and every 30 s while idle, with no toast', async ($, on) => {
  const { seen, clock } = world(on, [json(PROVING), json(PROVING), json(C1_SEALED), json(IDLE), json(IDLE)])
  await start($)
  expect(seen.runs).toHaveLength(1)
  await clock.advance(4000)
  expect(seen.runs).toHaveLength(1)
  await clock.advance(1000)
  expect(seen.runs).toHaveLength(2)
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(3)
  expect(seen.toasts).toEqual([])
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(4)
  // The fourth read is idle, so the next poll is 30 s after it.
  await clock.advance(25000)
  expect(seen.runs).toHaveLength(4)
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(5)
})

test('polling skips refresh while a request is pending', async ($, on) => {
  const { seen, clock } = world(on, [json(PROVING), { slowMs: 12000, stdout: json(PROVING) }, json(PROVING)])
  await start($)
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(2)
  // The read started at 5 s ends at 17 s, so the 10 s and 15 s ticks are skipped.
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(2)
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(2)
  await clock.advance(5000)
  expect(seen.runs).toHaveLength(3)
})

// ---- the off switch ----

test('while on, the band shows details and a turn off button and no turn on', async ($, on) => {
  world(on, [json(PROVING)])
  await start($)
  const { line, details, turnOff, turnOn } = await band($)
  expect(line).toBe('Familiar · 1 needs you · CI pending')
  expect(details?.props.label).toBe('details')
  expect(turnOff?.type).toBe('Button')
  expect(turnOff?.props.label).toBe('turn off')
  expect(turnOn).toBeUndefined()
})

test('while off, the band shows the off line and a turn on button and no card, details or turn off', async ($, on) => {
  world(on, [json(OFF_PROVING)])
  await start($)
  const { line, card, details, turnOff, turnOn } = await band($)
  expect(line).toBe(OFF_LINE)
  expect(card).toBeUndefined()
  expect(details).toBeUndefined()
  expect(turnOff).toBeUndefined()
  expect(turnOn?.type).toBe('Button')
  expect(turnOn?.props.label).toBe('turn on')
})

test('while off and idle, the band shows the off line and turn on', async ($, on) => {
  world(on, [json(OFF_IDLE)])
  await start($)
  const { line, turnOn } = await band($)
  expect(line).toBe(OFF_LINE)
  expect(turnOn?.props.label).toBe('turn on')
})

// ---- clearing the parts (the owner, 2026-10-27) ----

test('while on, the band shows a clear button beside turn off', async ($, on) => {
  world(on, [json(PROVING)])
  await start($)
  const { clear } = await band($)
  expect(clear?.type).toBe('Button')
  expect(clear?.props.label).toBe('clear')
})

test('while off, the band shows a clear button beside turn on', async ($, on) => {
  world(on, [json(OFF_IDLE)])
  await start($)
  const { clear } = await band($)
  expect(clear?.type).toBe('Button')
  expect(clear?.props.label).toBe('clear')
})

test('pressing clear runs familiar-ai clear and reads the status again', async ($, on) => {
  const { seen } = world(on, [json(PROVING), 'Familiar cleared 1 part of this project.\n', json(IDLE)])
  await start($)
  const { ui } = await band($)
  await ui.press({ key: 'clear' })
  expect(seen.runs[1]).toEqual(['familiar-ai', 'clear'])
  expect(seen.runs[2]).toEqual(['familiar-ai', 'status', '--json'])
})

test('pressing turn off while idle runs familiar-ai off and the band then shows the off line', async ($, on) => {
  const { seen } = world(on, [json(IDLE), 'Familiar is turned off for this project.\n', json(OFF_IDLE)])
  await start($)
  const { ui } = await band($)
  await ui.press({ key: 'turn-off' })
  expect(seen.runs[1]).toEqual(['familiar-ai', 'off'])
  expect((await band($)).line).toBe(OFF_LINE)
})

test('pressing turn off while proving runs familiar-ai off, re-reads status, and the band shows the off line', async ($, on) => {
  const { seen } = world(on, [json(PROVING), 'Familiar is turned off for this project.\n', json(OFF_PROVING)])
  await start($)
  const { ui } = await band($)
  await ui.press({ key: 'turn-off' })
  expect(seen.runs).toEqual([
    ['familiar-ai', 'status', '--json'],
    ['familiar-ai', 'off'],
    ['familiar-ai', 'status', '--json'],
  ])
  expect((await band($)).line).toBe(OFF_LINE)
})

test('pressing turn on runs familiar-ai on, re-reads status, and the band shows the proving line', async ($, on) => {
  const { seen } = world(on, [json(OFF_PROVING), 'Familiar is turned on for this project.\n', json(PROVING)])
  await start($)
  const { ui } = await band($)
  await ui.press({ key: 'turn-on' })
  expect(seen.runs).toEqual([
    ['familiar-ai', 'status', '--json'],
    ['familiar-ai', 'on'],
    ['familiar-ai', 'status', '--json'],
  ])
  expect((await band($)).line).toBe('Familiar · 1 needs you · CI pending')
})

test('in a worktree following main, the band and the pane say it is off as main is', async ($, on) => {
  world(on, [json(WORKTREE_AS_MAIN)])
  await start($)
  const { line, turnOn } = await band($)
  expect(line).toBe('Familiar is turned off for this worktree, as main is.')
  expect(turnOn?.props.label).toBe('turn on')
  const { row } = await paneRows($)
  expect(await row('off')).toBe('Familiar is turned off for this worktree, as main is.')
})

test('in a worktree switched off itself while main is on, the band says main is on', async ($, on) => {
  world(on, [json(WORKTREE_MAIN_ON)])
  await start($)
  const { line, turnOn } = await band($)
  expect(line).toBe('Familiar is turned off for this worktree; main is on.')
  expect(turnOn?.props.label).toBe('turn on')
})

// The mark and its blank line take the first five texts, so the off line is the sixth.
test('while off, the pane shows the off line right after the mark and still shows the asks and parts', async ($, on) => {
  world(on, [json(OFF_PROVING)])
  await start($)
  const { pane, row } = await paneRows($)
  expect(await row('off')).toBe(OFF_LINE)
  expect(await row('ask-0')).toBe('your 12:36 ask')
  expect(await row('table-c1')).toBe('1     testing     ·    ·     ·    ·   ·   ·     ·   the test comes first · judged: not judged')
  const first = (await pane.findAll({ type: 'Text' }))[5]
  expect(first?.text).toBe(OFF_LINE)
})

test('while on, the pane has no off line', async ($, on) => {
  world(on, [json(PROVING)])
  await start($)
  const { row } = await paneRows($)
  expect(await row('off')).toBeUndefined()
})

// ---- what the person said ----

// Each prompt is marked by what it became so the person can tell an ask from a question or a note.
test('the pane lists each prompt with its time, words and what it became, oldest first and above the asks, and the band line is unchanged', async ($, on) => {
  world(on, [json(SAID)])
  await start($)
  const { pane, row } = await paneRows($)
  expect(await row('said')).toBe('you said')
  expect(await row('said-at-3')).toBe('12:36')
  expect(await row('said-words-3')).toBe(FIRST_ASK.text)
  expect(await row('said-became-3')).toBe('ask 1')
  // definition change (the owner, 2026-10-12): prompts that became an answer or a note are not in the pane
  for (const key of ['said-at-5', 'said-words-5', 'said-became-5', 'said-at-7', 'said-words-7', 'said-became-7']) expect(await row(key)).toBeUndefined()
  const order = (await pane.findAll({ type: 'Text' })).map(t => t.text)
  const at = (t: string) => order.indexOf(t)
  expect(at('you said')).toBeGreaterThanOrEqual(0)
  expect(at('you said')).toBeLessThan(at('12:36'))
  expect(at('12:36')).toBeLessThan(at('your 12:36 ask'))
  expect(order.includes('12:41') || order.includes('12:44')).toBe(false)
  expect((await band($)).line).toBe('Familiar · 1 needs you · CI pending')
})

test('before any prompt, the pane shows "you said" and "nothing yet"', async ($, on) => {
  world(on, [json(IDLE)])
  await start($)
  const { row } = await paneRows($)
  expect(await row('said')).toBe('you said')
  expect(await row('said-none')).toBe('nothing yet')
})

// ---- the other checkouts ----

test('the pane shows each other checkout below its own picture: folder and branch, what was said, the ask, each part and who holds it; an idle checkout is one line', async ($, on) => {
  world(on, [json(MAIN_WITH_A_LANE)])
  await start($)
  const { pane, row } = await paneRows($)
  expect(await row('checkout-0')).toBe('familiar-ai-oss-hook-words · lane-hook-words')
  expect(await row('checkout-0-said-0')).toBe('14:24  say why at each hook  · ask 1')
  expect(await row('checkout-0-ask-0')).toBe('your 14:24 ask: say why at each hook')
  expect(await row('checkout-0-table-head-0')).toBe('part  stage      plan place test red fix green CI')
  expect(await row('checkout-0-table-c1')).toBe('1     testing     ·    ·     ·    ·   ·   ·     ·   the test comes first · held by: opus, conversation cc9af915 · judged: not judged')
  expect(await row('checkout-0-table-c2')).toBe('2     waiting     ·    ·     ·    ·   ·   ·     ·   waits for part 1 · held by: a subagent (sonnet) · judged: not judged')
  expect(await row('checkout-1')).toBe('familiar-ai-oss-docs · idle')
  const order = (await pane.findAll({ type: 'Text' })).map(t => t.text)
  expect(order.indexOf('familiar-ai-oss-hook-words · lane-hook-words')).toBeGreaterThan(order.indexOf('Familiar is turned off for this project.'))
})

test("another checkout's green part reads locally green, never proven on this laptop", async ($, on) => {
  const [lane, idle] = MAIN_WITH_A_LANE.checkouts ?? []
  const ask = lane.asks[0]
  const green = { ...ask.parts[1], state_name: 'green' as const, state: 'locally green' }
  world(on, [json({ ...MAIN_WITH_A_LANE, checkouts: [{ ...lane, asks: [{ ...ask, parts: [ask.parts[0], green] }] }, idle] })])
  await start($)
  const { pane, row } = await paneRows($)
  expect(await row('checkout-0-table-c2')).toBe('2     green       ·    ·     ·    ·   ·   ·     ·   locally green · held by: a subagent (sonnet) · judged: not judged')
  const words = (await pane.findAll({ type: 'Text' })).map(t => t.text).join('\n')
  expect(words.includes('proven on this laptop')).toBe(false)
})

test("the band counts another checkout's card with its own and names it with the checkout's folder", async ($, on) => {
  world(on, [json(PROVING_WITH_A_LANE_CARD)])
  await start($)
  const { ui, line, card } = await band($)
  expect(line).toBe('Familiar · 2 need you · CI pending')
  expect(card).toBe('part 5 needs you · the agent will ask you here')
  expect((await ui.find({ key: 'card-1' }))?.text).toBe('part 1 in familiar-ai-oss-hook-words needs you · the agent will ask you here')
})

test("while off, the band still shows another checkout's card under the off line", async ($, on) => {
  world(on, [json(OFF_WITH_A_LANE_CARD)])
  await start($)
  const { line, card, turnOn } = await band($)
  expect(line).toBe(OFF_LINE)
  expect(turnOn?.props.label).toBe('turn on')
  expect(card).toBe('part 1 in familiar-ai-oss-hook-words needs you · the agent will ask you here')
})
