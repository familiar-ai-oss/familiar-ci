import type { ElementTable } from 'claude-code'

import type { FamiliarReading, FamiliarStatus } from '../types'
import { IDLE_LINE, UNAVAILABLE_LINE, cardLine, offLine } from './words'

// The band above the prompt: what needs the person now, CI, the way to the
// details, the switch and clearing the parts, each line as the binary words it.

type Ui = Pick<ElementTable, 'Box' | 'Button' | 'Text'>
type Press = { details: () => unknown; turnOn: () => unknown; turnOff: () => unknown; clear: () => unknown }

// the open cards of the other checkouts, each named with its checkout's folder
const elsewhere = (s: FamiliarStatus) =>
  (s.checkouts ?? []).flatMap(o => (o.cards ?? []).map(c => ({ ...c, name: `${c.name ?? 'a part'} in ${o.folder}` })))

const line = (s: FamiliarStatus) => {
  const n = s.counts.needs_you + elsewhere(s).length
  if (s.mode === 'idle' && n === 0) return IDLE_LINE
  const parts = ['Familiar']
  if (n > 0) parts.push(`${n} ${n === 1 ? 'needs' : 'need'} you`)
  if (s.ci) parts.push(s.ci)
  return parts.join(' · ')
}

// the hint's own words: only that the status is unavailable
export const hintTail = (r: FamiliarReading | null) => (r !== null && 'unavailable' in r ? UNAVAILABLE_LINE : undefined)

// `failed`: what a failed switch said, on its own line under the band
export const bandTree = ({ Box, Button, Text }: Ui, s: FamiliarStatus, failed: string | null, press: Press) => {
  const failedRow = failed ? (
    <Box key="switch-failed">
      <Text>{failed}</Text>
    </Box>
  ) : undefined
  // this checkout's first card, then each other checkout's, which reach this session too
  const cards = [...(s.mode === 'idle' || s.off ? [] : s.cards.slice(0, 1)), ...elsewhere(s)]
  const cardRows = cards.map((c, i) => (
    <Box key={i === 0 ? 'card' : `card-${i}`}>
      <Text>{cardLine(c.name)}</Text>
    </Box>
  ))
  if (s.off) {
    const row = (
      <Box key="switch">
        <Box key="line">
          <Text>{offLine(s.main_off)}</Text>
        </Box>
        <Text>{'   '}</Text>
        <Button key="turn-on" label="turn on" onPress={press.turnOn} />
        <Text> </Text>
        <Button key="clear" label="clear" onPress={press.clear} />
      </Box>
    )
    return failedRow || cardRows.length > 0 ? (
      <Box flexDirection="column">
        {row}
        {failedRow}
        {cardRows}
      </Box>
    ) : (
      row
    )
  }

  return (
    <Box flexDirection="column">
      <Box>
        <Box key="line">
          <Text>{line(s)}</Text>
        </Box>
        <Text> </Text>
        <Button key="details" label="details" hotkey="d" onPress={press.details} />
        <Text> </Text>
        <Button key="turn-off" label="turn off" onPress={press.turnOff} />
        <Text> </Text>
        <Button key="clear" label="clear" onPress={press.clear} />
      </Box>
      {failedRow}
      {cardRows}
    </Box>
  )
}
