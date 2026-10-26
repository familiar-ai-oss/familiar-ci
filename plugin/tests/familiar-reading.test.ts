import { expect, mock, test } from 'claude-code/testing'
import type { Engine } from 'claude-code/testing'
import type { On, RenderElement } from 'claude-code'

import { check } from '../hooks/status'

// The reading (plan A, lane a-surf): how each runner's runs are read, a row in
// the pane in the CI row's shape, the binary's words as they are.

const PLUGIN = 'familiar-ai'
const SCROLL = { offset: 0, bodyRows: 10 }
const PANE = {
  plugin: PLUGIN,
  surface: 'terminal',
  component: 'Pane',
  requestId: 'familiar',
  props: { title: 'Familiar', isFocused: false, bodyColumns: 160, placement: 'dock', scroll: SCROLL, view: {} },
} as const

const STATUS = {
  mode: 'proving',
  intent: { text: 'greet says hello' },
  criteria: [{ id: 'c1', name: 'part 1', part: 1, ask: { at: '12:36', text: 'greet says hello' }, text: 'greet says hello', state: 'locally green', state_name: 'green' }],
  cards: [],
  counts: { running: 0, waiting: 0, needs_you: 0 },
  runs: [],
  ci: 'CI pending',
  reading: 'reading: pytest by its output tables; asks you on: a pass beside a skipped test',
  off: false,
}
const SETUP_CARD = {
  criterion_id: 'setup',
  name: 'the test setup',
  hold: 'reading_gap',
  question: 'the test setup: Familiar cannot read some outcomes of pytest at the project root: a pass beside a skipped test. Runs that end that way will ask you. What now?',
  choices: ['keep_going', 'try_again'],
}

const world = (on: On, stdout: string) => {
  mock.clock(on, { now: 1_000_000 })
  on('process.run', async () => ({ value: { exitCode: 0, stdout, stderr: '', isStdoutTruncated: false, isStderrTruncated: false } }))
  on('command.register', ($, e) => ({ value: { command: e.name } }))
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('ui.render', { component: 'Pane' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
}
const start = ($: Engine) => $.session.start({ cwd: '/project', surface: 'terminal', isInteractive: true })
const paneText = async ($: Engine) => {
  const ui = await $.ui.mount(PANE)
  return async (key: string) => (await ui.find({ key }))?.text
}

test('the pane shows the reading as a row in the CI row\'s shape, after CI', async ($, on) => {
  world(on, JSON.stringify(STATUS))
  await start($)
  const text = await paneText($)
  expect(await text('ci')).toBe('CI      pending')
  expect(await text('reading')).toBe('reading pytest by its output tables; asks you on: a pass beside a skipped test')
})

test('no reading from the binary, or an older binary: no row', async ($, on) => {
  const { reading: _gone, ...older } = STATUS
  world(on, JSON.stringify({ ...older, ci: null }))
  await start($)
  const text = await paneText($)
  expect(await text('reading')).toBeUndefined()
  expect(await text('ci')).toBeUndefined()
})

test('the setup card is the test setup needing the person, by its name', async ($, on) => {
  world(on, JSON.stringify({ ...STATUS, cards: [SETUP_CARD], counts: { running: 0, waiting: 0, needs_you: 1 } }))
  await start($)
  const text = await paneText($)
  expect(await text('card-setup')).toBe('the test setup needs you · the agent will ask you here')
  expect(await text('question-setup')).toBe(SETUP_CARD.question)
})

test('check names a reading the pane cannot read', () => {
  expect(check(STATUS)).toEqual(STATUS)
  expect(check({ ...STATUS, reading: null })).toEqual(expect.objectContaining({ reading: null }))
  expect(check({ ...STATUS, reading: 3 })).toBe('status.reading is not text')
})
