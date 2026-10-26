import type { FamiliarReading, FamiliarStatus } from '../types'

// The status the mod draws: how often it is read, and the check that
// `familiar-ai status --json` carries every field the band and the pane read.
// The binary decides what the status means; the mod renders it.

// the clock's tick while proving, and the gap between reads while idle
export const TICK_MS = 5000
export const IDLE_MS = 30000

export const unavailable = (reason: string): FamiliarReading => ({ unavailable: true, reason })

// the first line a command wrote to stderr, else what is known of how it ended
export const firstLine = (text: string, otherwise: string) => text.split('\n').find(l => l.trim() !== '')?.trim() ?? otherwise
export const said = (e: unknown) => (e instanceof Error ? e.message : String(e))

type Kind = 'text' | 'number' | 'flag' | 'list'
type Spec = Record<string, Kind | `${Kind}?`>

const fits = (v: unknown, kind: string) => {
  if (kind.endsWith('?') && (v === undefined || v === null)) return true
  switch (kind.replace('?', '')) {
    case 'text':
      return typeof v === 'string'
    case 'number':
      return typeof v === 'number'
    case 'flag':
      return typeof v === 'boolean'
    default:
      return Array.isArray(v)
  }
}

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v)

// the first field of `o` that is not of its kind, as a reason
const misfit = (o: unknown, where: string, spec: Spec): string | undefined => {
  if (!isObject(o)) return `${where} is not an object`
  for (const [k, kind] of Object.entries(spec)) {
    if (!fits(o[k], kind)) return `${where}.${k} is not ${kind.replace('?', '')}`
  }
  return undefined
}

const each = (list: unknown, where: string, spec: Spec, more?: (o: Record<string, unknown>, at: string) => string | undefined) => {
  if (!Array.isArray(list)) return undefined
  for (const [i, o] of list.entries()) {
    const at = `${where}[${i}]`
    const bad = misfit(o, at, spec) ?? more?.(o as Record<string, unknown>, at)
    if (bad) return bad
  }
  return undefined
}

// a part's journey: its steps, each done true, false, null or "half"
const journeyMisfit = (j: unknown, where: string) =>
  misfit(j, where, { steps: 'list', tests: 'list?', places: 'list?', opened: 'flag?' }) ??
  each((j as Record<string, unknown>).steps, `${where}.steps`, { name: 'text', at: 'text?' }, (s, at) =>
    s.done === true || s.done === false || s.done === null || s.done === 'half' ? undefined : `${at}.done is not true, false, null or half`,
  )

const STATE_NAMES =['unplanned', 'waiting', 'testing', 'sealed', 'green', 'proven', 'held']
const BECAME = ['ask', 'answer', 'note']

// every field the band and the pane read, checked once: the status, or the
// one-line reason it cannot be drawn
export const check = (v: unknown): FamiliarStatus | string => {
  const bad =
    misfit(v, 'status', { mode: 'text', criteria: 'list', cards: 'list', runs: 'list?', said: 'list?', off: 'flag?', switch_from: 'text?', main_off: 'flag?', ci: 'text?', reading: 'text?', checkouts: 'list?' }) ??
    misfit((v as Record<string, unknown>).counts, 'counts', { running: 'number', waiting: 'number', needs_you: 'number' }) ??
    each(
      (v as Record<string, unknown>).criteria,
      'criteria',
      { id: 'text', name: 'text?', part: 'number?', text: 'text?', state: 'text', state_name: 'text?', hold: 'text?', holder: 'text?', judge: 'text?', after: 'text?' },
      (c, at) =>
        (c.state_name != null && !STATE_NAMES.includes(c.state_name as string) ? `${at}.state_name is not a state` : undefined) ??
        (c.ask == null ? undefined : misfit(c.ask, `${at}.ask`, { at: 'text?', text: 'text' })) ??
        (c.journey == null ? undefined : journeyMisfit(c.journey, `${at}.journey`)) ??
        each(c.answers, `${at}.answers`, { hold: 'text', choice: 'text', approval: 'text?', verified: 'flag' }),
    ) ??
    each((v as Record<string, unknown>).cards, 'cards', { criterion_id: 'text', name: 'text?', question: 'text', summary: 'list?' }) ??
    each((v as Record<string, unknown>).runs, 'runs', { run_id: 'text', name: 'text?', commit: 'text', in_flight: 'flag' }) ??
    each((v as Record<string, unknown>).checkouts, 'checkouts', { folder: 'text', branch: 'text?', busy: 'flag?', mode: 'text?', said: 'list?', asks: 'list?', cards: 'list?' }, (o, at) =>
      each(o.asks, `${at}.asks`, { text: 'text', parts: 'list?' }, (a, aAt) => each(a.parts, `${aAt}.parts`, { id: 'text', state: 'text', held_by: 'text?' })) ??
      each(o.cards, `${at}.cards`, { criterion_id: 'text', name: 'text?', question: 'text', summary: 'list?' }),
    ) ??
    each((v as Record<string, unknown>).said, 'said', { for: 'number', at: 'text?', text: 'text', ask: 'number?' }, (p, at) =>
      BECAME.includes(p.became as string) ? undefined : `${at}.became is not ask, answer or note`,
    )
  if (bad) return bad
  const intent = (v as Record<string, unknown>).intent
  if (!(intent == null || typeof intent === 'string' || misfit(intent, 'intent', { text: 'text?' }) === undefined)) return 'intent is not text'
  return v as FamiliarStatus
}

export const parse = (stdout: string): FamiliarReading => {
  let v: unknown
  try {
    v = JSON.parse(stdout)
  } catch {
    return unavailable('status --json is not JSON')
  }
  const s = check(v)
  return typeof s === 'string' ? unavailable(s) : { status: s }
}
