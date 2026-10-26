import { expect, mock, test } from 'claude-code/testing'
import type { Engine } from 'claude-code/testing'
import type { On, RenderElement } from 'claude-code'

// A card in the pane (plan §6.9 B; the owner, 2026-10-17): the pane shows the
// card, its finding and its choices; a choice runs nothing and takes no
// passphrase, it shows the exact command to sign in a terminal of the
// person's own; "answer unsigned" runs approve --unsigned. A fake familiar-ai
// records each run's argv, stdin, env and cwd.

const PLUGIN = 'familiar-ai'
const SCROLL = { offset: 0, bodyRows: 10 }
const PANE = {
  plugin: PLUGIN,
  surface: 'terminal',
  component: 'Pane',
  requestId: 'familiar',
  props: { title: 'Familiar', isFocused: true, bodyColumns: 120, placement: 'dock', scroll: SCROLL, view: {} },
} as const

const FINDING = {
  category: 'wrong_test', choice: 'try_again', model: 'opus', reason: 'the test checks the log line, not the greeting',
  quotes: [{ text: 'assert "logged" in out' }, { text: 'greet says hello' }],
}
const CARD = { criterion_id: 'c2', name: 'part 2', hold: 'two_tries_not_red', question: 'part 2 never went red. What now?', choices: ['try_again', 're_plan', 'release'], finding: FINDING }
const LANE_CARD = { criterion_id: 'c9', name: 'part 1', hold: 'two_failures', question: 'two tries failed. What now?', choices: ['try_again', 'release'], finding: null }
const STATUS = {
  mode: 'proving',
  intent: { text: 'greet says hello' },
  criteria: [{ id: 'c2', name: 'part 2', part: 2, text: 'bye says goodbye', state: 'never red', state_name: 'held' }],
  cards: [CARD],
  counts: { running: 0, waiting: 0, needs_you: 2 },
  runs: [],
  off: false,
  checkouts: [
    {
      folder: 'familiar-ai-oss-lane', name: 'lane', branch: 'lane-x', root: '/work/familiar-ai-oss-lane', busy: false, mode: 'proving', said: [], runs: [],
      asks: [{ text: 'wave', parts: [{ id: 'c9', state: 'two tries failed' }] }], cards: [LANE_CARD],
    },
  ],
}

type Ran = { argv: string[]; stdin?: string; env?: Record<string, string>; cwd?: string }
type Answer = { exitCode: number; stdout?: string; stderr?: string }

// the fake familiar-ai: status --json prints STATUS; approve answers `approve`
const world = (on: On, approve: Answer[], status: unknown = STATUS) => {
  const runs: Ran[] = []
  let approvals = 0
  mock.clock(on, { now: 1_000_000 })
  on('process.run', async ($, e) => {
    runs.push({ argv: [...e.argv], stdin: e.init?.stdin, env: e.init?.env, cwd: e.init?.cwd })
    const a: Answer = e.argv[1] === 'approve' ? approve[Math.min(approvals++, approve.length - 1)] : { exitCode: 0, stdout: JSON.stringify(status) }
    return { value: { exitCode: a.exitCode, stdout: a.stdout ?? '', stderr: a.stderr ?? '', isStdoutTruncated: false, isStderrTruncated: false } }
  })
  on('command.register', ($, e) => ({ value: { command: e.name } }))
  on('session.start', ($, e) => ({ cwd: e.cwd }))
  on('turn.complete', () => ({ text: '' }))
  on('ui.focus', () => ({}))
  on('ui.close', () => undefined)
  on('ui.render', { component: 'Pane' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
  on('ui.render', { component: 'AbovePrompt' }, ($, e) => h($.ui.resolve(e).Box, { key: 'engine' }) as RenderElement)
  on('ui.render', { component: 'PromptHint' }, ($, e) => h($.ui.resolve(e).Box, { key: 'hint' }) as RenderElement)
  return runs
}

const UNSIGNED = { exitCode: 0, stdout: 'familiar-ai: answered unsigned: CI reads it as approval not verified\n' }

const start = ($: Engine) => $.session.start({ cwd: '/project', surface: 'terminal', isInteractive: true })
const approves = (runs: Ran[]) => runs.filter(r => r.argv[1] === 'approve')
const SIGN_HERE = 'Sign it in a terminal of your own; it asks for your approval key\'s passphrase there:'

test('the pane shows each open card, its finding and proof, and its choices as buttons, this checkout\'s and another\'s', async ($, on) => {
  world(on, [UNSIGNED])
  await start($)
  const ui = await $.ui.mount(PANE)
  const text = async (key: string) => (await ui.find({ key }))?.text
  expect(await text('question-c2')).toBe('part 2 never went red. What now?')
  expect(await text('found-c2')).toBe('found: the test checks the log line, not the greeting')
  expect(await text('proof-c2')).toBe('proof: "assert "logged" in out" · "greet says hello"')
  for (const choice of ['try_again', 're_plan', 'release', 'typed']) expect(await ui.find({ key: `choice-c2-${choice}` })).toBeDefined()
  expect(await text('checkout-0-question-c9')).toBe('two tries failed. What now?')
  expect(await ui.find({ key: 'choice-checkout-0-c9-release' })).toBeDefined()
  expect(await ui.find({ key: 'sign-c2' })).toBeUndefined()
})

test('a choice runs nothing and takes no passphrase: it shows the exact command to sign in a terminal of the person\'s own', async ($, on) => {
  const runs = world(on, [UNSIGNED])
  await start($)
  const before = runs.length
  const ui = await $.ui.mount(PANE)
  await ui.press({ key: 'choice-c2-re_plan' })
  expect(runs.length).toBe(before)
  expect(await ui.findAll({ type: 'Input' })).toHaveLength(0)
  expect((await ui.find({ key: 'sign-c2' }))?.text).toBe(SIGN_HERE)
  expect((await ui.find({ key: 'command-c2' }))?.text).toBe('familiar-ai approve --criterion_id c2 --hold two_tries_not_red --choice re_plan')
})

test("another checkout's card: the command starts in that checkout's root", async ($, on) => {
  world(on, [UNSIGNED])
  await start($)
  const ui = await $.ui.mount(PANE)
  await ui.press({ key: 'choice-checkout-0-c9-release' })
  expect((await ui.find({ key: 'command-checkout-0-c9' }))?.text).toBe(
    "cd '/work/familiar-ai-oss-lane' && familiar-ai approve --criterion_id c9 --hold two_failures --choice release",
  )
})

test("something else: the person's words go in the command as --typed, quoted for the shell", async ($, on) => {
  world(on, [UNSIGNED])
  await start($)
  const ui = await $.ui.mount(PANE)
  await ui.press({ key: 'choice-c2-typed' })
  await ui.input({ key: 'words', text: "keep the old greeting, it's fine", kind: 'change' })
  expect((await ui.find({ key: 'command-c2' }))?.text).toBe(
    "familiar-ai approve --criterion_id c2 --hold two_tries_not_red --choice typed --typed 'keep the old greeting, it'\\''s fine'",
  )
})

test('answer unsigned runs approve --unsigned with nothing on stdin, in the card\'s checkout, and says what it did', async ($, on) => {
  const runs = world(on, [UNSIGNED])
  await start($)
  const ui = await $.ui.mount(PANE)
  await ui.press({ key: 'choice-checkout-0-c9-release' })
  await ui.press({ key: 'unsigned-checkout-0-c9' })
  const [ran] = approves(runs)
  expect(ran.argv).toEqual(['familiar-ai', 'approve', '--criterion_id', 'c9', '--hold', 'two_failures', '--choice', 'release', '--unsigned'])
  expect(ran.stdin).toBe('')
  expect(ran.cwd).toBe('/work/familiar-ai-oss-lane')
  expect(ran.env).toEqual({ CLAUDE_CODE_SESSION_ID: '', CLAUDE_PROJECT_DIR: '' })
  expect((await ui.find({ key: 'said-checkout-0-c9' }))?.text).toBe('familiar-ai: answered unsigned: CI reads it as approval not verified')
  expect(runs.at(-1)?.argv).toEqual(['familiar-ai', 'status', '--json'])
})

// H-4 (the owner, 2026-10-19): the can't-tell card's run in short sentences, after its question
const SUMMARY = ['It ran: pytest -q', 'Counts: 1 failed, 3 passed.', "Familiar could not tell because the output's summary wasn't recognised.", 'The full output: .familiar/runs/ab12.log']

test("the can't-tell card shows its run's sentences after its question, each on its own line and escaped, here and in another checkout", async ($, on) => {
  const card = { ...CARD, hold: 'run_unclassified', choices: ['try_again', 'its_the_red', 're_plan', 'release'], finding: null, summary: [...SUMMARY, 'bell\x07 line'] }
  const lane = { ...STATUS.checkouts[0], cards: [{ ...LANE_CARD, hold: 'run_unclassified', summary: SUMMARY }] }
  world(on, [UNSIGNED], { ...STATUS, cards: [card], checkouts: [lane] })
  await start($)
  const ui = await $.ui.mount(PANE)
  for (const [i, line] of SUMMARY.entries()) {
    expect((await ui.find({ key: `summary-c2-${i}` }))?.text).toBe(line)
    expect((await ui.find({ key: `checkout-0-summary-c9-${i}` }))?.text).toBe(line)
  }
  expect((await ui.find({ key: `summary-c2-${SUMMARY.length}` }))?.text).toBe('bell\\x07 line')
  expect(await ui.find({ key: 'choice-c2-its_the_red' })).toBeDefined()
  // a card with no summary (an older binary, another card) has no such row
  expect(await ui.find({ key: 'summary-c9-0' })).toBeUndefined()
})

// control characters (ANSI ESC, \r, BEL and the rest of C0, DEL) never reach the terminal from a card
const NASTY = { ...CARD, finding: { ...FINDING, reason: 'ok\x1b[2J\rhidden', quotes: [{ text: 'bell\x07 and del\x7f' }] } }

test("a finding's control characters are drawn escaped, never sent to the terminal", async ($, on) => {
  world(on, [UNSIGNED], { ...STATUS, cards: [NASTY] })
  await start($)
  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ key: 'found-c2' }))?.text).toBe('found: ok\\x1b[2J\\x0dhidden')
  expect((await ui.find({ key: 'proof-c2' }))?.text).toBe('proof: "bell\\x07 and del\\x7f"')
})

test('words with a control character get no command: the pane asks for them again', async ($, on) => {
  world(on, [UNSIGNED])
  await start($)
  const ui = await $.ui.mount(PANE)
  await ui.press({ key: 'choice-c2-typed' })
  await ui.input({ key: 'words', text: 'fine\x1b]0;pwned\x07', kind: 'change' })
  expect((await ui.find({ key: 'command-c2' }))?.text).toBe('Your words hold a control character, so no command is shown: type them again.')
})

test("a card's name and question lines are drawn with their control characters escaped, here and in another checkout", async ($, on) => {
  const card = { ...CARD, name: 'part\x1b[31m 2', question: 'what\r now?\x07' }
  const lane = { ...STATUS.checkouts[0], cards: [{ ...LANE_CARD, name: 'part\x00 1', question: 'two\x1b]0;x\x07 tries' }] }
  world(on, [UNSIGNED], { ...STATUS, cards: [card], checkouts: [lane] })
  await start($)
  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ key: 'card-c2' }))?.text).toBe('part\\x1b[31m 2 needs you · the agent will ask you here')
  expect((await ui.find({ key: 'question-c2' }))?.text).toBe('what\\x0d now?\\x07')
  expect((await ui.find({ key: 'checkout-0-card-c9' }))?.text).toBe('part\\x00 1 needs you · the agent will ask you here')
  expect((await ui.find({ key: 'checkout-0-question-c9' }))?.text).toBe('two\\x1b]0;x\\x07 tries')
})
