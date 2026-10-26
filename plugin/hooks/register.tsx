import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { FamiliarReading } from '../types'
import { approving, cardRows } from './approve'
import { bandTree, hintTail } from './band'
import { paneTree } from './pane'
import { IDLE_MS, TICK_MS, firstLine, parse, said, unavailable } from './status'
import { PANE } from './words'

// The familiar mod's wiring: when the status is read, the person's switch, the
// /familiar command, and the hooks that hand the band and the pane their status.

const reading = atom({ plugin: 'familiar-ai', key: 'reading' } as const, null)
const refreshedAt = atom({ plugin: 'familiar-ai', key: 'refreshedAt' } as const, 0)
// what a failed `familiar-ai on|off` said, shown on the band until a read succeeds
const switchFailed = atom({ plugin: 'familiar-ai', key: 'switchFailed' } as const, null)

// the person's off switch: the button runs the binary as the person (never an
// agent's shell), then reads the status again; a failed switch changed
// nothing, so its own words stay on the band until a read succeeds
const switchTo = async ($: EngineInterface, to: 'off' | 'on') => {
  let failed: string | undefined
  try {
    const ran = await $.process.run(['familiar-ai', to])
    if (ran.exitCode !== 0) failed = firstLine(ran.stderr, `familiar-ai ${to}: exit ${ran.exitCode}`)
  } catch (e) {
    failed = said(e)
  }
  if (failed !== undefined) {
    const words = failed
    await update($, switchFailed, () => words)
    return
  }
  await refresh($)
}

// one read at a time
let queue: Promise<void> = Promise.resolve()
let reads = 0

async function readStatus($: EngineInterface) {
  const now = await $.clock.now()
  await update($, refreshedAt, () => now)
  let next: FamiliarReading
  try {
    const ran = await $.process.run(['familiar-ai', 'status', '--json'])
    next = ran.exitCode === 0 ? parse(ran.stdout) : unavailable(firstLine(ran.stderr, `familiar-ai status --json: exit ${ran.exitCode}`))
  } catch (e) {
    next = unavailable(said(e))
  }
  await update($, reading, () => next)
  if ('status' in next) await update($, switchFailed, () => null)
}

function refresh($: EngineInterface) {
  reads += 1
  queue = queue
    .then(() => readStatus($))
    // a read that could not write the state leaves the last reading, and the next tick reads again
    .catch(() => undefined)
    .finally(() => {
      reads -= 1
    })
  return queue
}

// the clock's tick: a read every 5 s while proving, every 30 s while idle,
// never while one is in flight
async function tick($: EngineInterface) {
  if (reads > 0) return
  const r = await read($, reading)
  const idle = r === null || 'unavailable' in r || r.status.mode === 'idle'
  if (idle && (await $.clock.now()) - (await read($, refreshedAt)) < IDLE_MS) return
  await refresh($)
}

// after a tool's result, at most once a second, off the call's own path
function refreshSoon($: EngineInterface) {
  $.clock.after(0, () => {
    void (async () => {
      const now = await $.clock.now()
      if (now - (await read($, refreshedAt)) < 1000) return
      await update($, refreshedAt, () => now)
      await refresh($)
      // the clock's next tick reads again if this one fails
    })().catch(() => undefined)
  })
}

// a hot reload or a new session starts the clock again
let ticking: { cancel: () => void } | undefined

function startClock($: EngineInterface) {
  try {
    ticking?.cancel()
  } catch {
    // the old session's clock is gone with it
  }
  ticking = $.clock.every(TICK_MS, () => {
    // a tick that fails is the next tick's to repeat
    void tick($).catch(() => undefined)
  })
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    await $.command.register({ name: 'familiar', description: 'Show the tracked work in a pane' })
    await refresh($)
    startClock($)

    return next(e)
  })

  on('command.run', { command: 'familiar' }, async $ => {
    await $.ui.open(PANE)

    return { text: 'Familiar: the tracked work is in the pane.' }
  })

  on('tool.call', async ($, e, next) => {
    const ran = await next(e)
    refreshSoon($)

    return ran
    // a failed refresh never holds or fails the tool's own call
  }).catch(($, e, next) => next(e))

  on('turn.complete', async ($, e, next) => {
    const done = await next(e)
    await refresh($)

    return done
  })

  on('ui.render', { component: 'PromptHint' }, async ($, e, next) => {
    const mine = hintTail(await read($, reading))
    if (mine === undefined) return next(e)

    return next({ ...e, props: { ...e.props, tail: e.props.tail ? `${e.props.tail} ${mine}` : mine } })
  })

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const r = await read($, reading)
    if (e.props.hasSurvey || r === null || 'unavailable' in r) return next(e)

    return bandTree($.ui.resolve(e), r.status, await read($, switchFailed), {
      details: () => $.ui.open(PANE),
      turnOn: () => switchTo($, 'on'),
      turnOff: () => switchTo($, 'off'),
    })
  })

  on('ui.render', { component: 'Pane', requestId: PANE.id }, async ($, e) => {
    const ui = $.ui.resolve(e)
    const done = async () => {
      await refresh($)
    }
    return paneTree(ui, await read($, reading), cardRows($, ui, await approving($), done))
  })
}
