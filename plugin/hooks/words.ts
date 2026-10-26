// The words the band and the pane share, and the pane's own id.

export const PANE = { id: 'familiar', title: 'Familiar' } as const

export const OFF_LINE = 'Familiar is turned off for this project.'
// a worktree's off line: as main is, or while main is on
export const OFF_AS_MAIN = 'Familiar is turned off for this worktree, as main is.'
export const OFF_MAIN_ON = 'Familiar is turned off for this worktree; main is on.'
// `main_off`: main's switch, null in main itself or when main is not set up
export const offLine = (mainOff: boolean | null | undefined) => (mainOff == null ? OFF_LINE : mainOff ? OFF_AS_MAIN : OFF_MAIN_ON)
export const IDLE_LINE = 'Familiar · idle'
export const UNAVAILABLE_LINE = 'Familiar · status unavailable'

// the line above a card's text, on the band and in the pane
const ASKS_HERE = 'the agent will ask you here'
export const cardLine = (name: string | null | undefined) => `${name ?? 'a part'} needs you · ${ASKS_HERE}`

// C0 controls (ESC, \r, BEL ...) and DEL: never drawn as themselves, so a card's
// text cannot move the cursor, retitle the terminal or hide what it shows
const CONTROL = /[\x00-\x1f\x7f]/g
export const visible = (s: string) => s.replace(CONTROL, c => `\\x${c.charCodeAt(0).toString(16).padStart(2, '0')}`)
