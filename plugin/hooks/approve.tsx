import { atom, read, update } from 'claude-code'
import type { ElementTable, EngineInterface } from 'claude-code'

import type { FamiliarApproveSaid, FamiliarCard, FamiliarPicking } from '../types'
import { visible } from './words'

// A card in the pane (plan §6.9 B; the owner, 2026-10-17): the pane shows the
// card, its finding and its choices. The passphrase is never typed in the pane
// (a typed character can draw before any mask, and typing outside a field
// reaches the model): a choice shows the exact command to sign in a terminal
// of the person's own, where `familiar-ai approve` asks for the passphrase with
// echo off. "answer unsigned" records the pick here with no approval.

const picking = atom({ plugin: 'familiar-ai', key: 'picking' } as const, null)
const typedWords = atom({ plugin: 'familiar-ai', key: 'typedWords' } as const, '')
const approveSaid = atom({ plugin: 'familiar-ai', key: 'approveSaid' } as const, null)

export const SIGN_HERE = "Sign it in a terminal of your own; it asks for your approval key's passphrase there:"
export const NO_TYPED_STACK = 'The agent asks you for the test command in its question card.'

export const CONTROL_WORDS = 'Your words hold a control character, so no command is shown: type them again.'

const hasControl = (s: string) => /[\x00-\x1f\x7f]/.test(s)

const firstLine = (text: string) => text.split('\n').find(l => l.trim() !== '')?.trim() ?? ''

// a word as the shell reads it back
const quote = (s: string) => `'${s.replace(/'/g, "'\\''")}'`

// the binary's argv for the picked card
export const approveArgv = (p: FamiliarPicking, words: string, unsigned: boolean) => {
  const argv = ['familiar-ai', 'approve', '--criterion_id', p.criterion_id, '--hold', p.hold, '--choice', p.choice]
  if (p.choice === 'typed') argv.push('--typed', words)
  if (unsigned) argv.push('--unsigned')
  return argv
}

// the command the person runs in a terminal of their own, in the card's checkout
export const signCommand = (p: FamiliarPicking, words: string) => {
  const argv = approveArgv(p, words, false).map(a => (/^[A-Za-z0-9_./:=-]+$/.test(a) ? a : quote(a)))
  return (p.cwd ? `cd ${quote(p.cwd)} && ` : '') + argv.join(' ')
}

// the child's environment over Claude Code's: the session's id and project
// blanked, so the binary reads this as the person's own process in the card's checkout
const ENV = { CLAUDE_CODE_SESSION_ID: '', CLAUDE_PROJECT_DIR: '' }

async function said($: EngineInterface, s: FamiliarApproveSaid | null) {
  await update($, approveSaid, () => s)
}

async function unsigned($: EngineInterface, done: () => Promise<void>) {
  const p = await read($, picking)
  if (p === null) return
  const words = await read($, typedWords)
  let ran
  try {
    ran = await $.process.run(approveArgv(p, words, true), { stdin: '', env: ENV, ...(p.cwd ? { cwd: p.cwd } : {}) })
  } catch (e) {
    await said($, { key: p.key, text: e instanceof Error ? e.message : String(e) })
    return
  }
  if (ran.exitCode !== 0) {
    await said($, { key: p.key, text: firstLine(ran.stderr) || `familiar-ai approve: exit ${ran.exitCode}` })
    return
  }
  await update($, picking, () => null)
  await update($, typedWords, () => '')
  await said($, { key: p.key, text: firstLine(ran.stdout) })
  await done()
}

type Ui = Pick<ElementTable, 'Box' | 'Button' | 'Text'> & { Input?: ElementTable<'terminal'>['Input'] }

// what the card rows read, once per drawing
export type Approving = { picking: FamiliarPicking | null; words: string; said: FamiliarApproveSaid | null }

export async function approving($: EngineInterface): Promise<Approving> {
  return {
    picking: await read($, picking),
    words: await read($, typedWords),
    said: await read($, approveSaid),
  }
}

const label = (choice: string) => choice.replace(/_/g, ' ')

// one card's finding, its choices as buttons, and under the picked one the
// command to sign it; `k` keys the card, `cwd` is its checkout (null: this one)
export function cardRows($: EngineInterface, ui: Ui, a: Approving, done: () => Promise<void>) {
  return (c: FamiliarCard, k: string, cwd: string | null) => {
    const { Box, Button, Input, Text } = ui
    const rows = []
    const f = c.finding
    if (f) {
      rows.push(
        <Box key={`found-${k}`}>
          <Text>{`found: ${visible(f.reason ?? f.category ?? '')}`}</Text>
        </Box>,
      )
      const quotes = (f.quotes ?? []).map(q => q.text).filter((t): t is string => typeof t === 'string' && t !== '')
      if (quotes.length > 0) {
        rows.push(
          <Box key={`proof-${k}`}>
            <Text>{`proof: ${quotes.map(q => `"${visible(q)}"`).join(' · ')}`}</Text>
          </Box>,
        )
      }
    }
    if (c.hold === 'no_test_stack') {
      rows.push(
        <Box key={`choices-${k}`}>
          <Text dimColor>{NO_TYPED_STACK}</Text>
        </Box>,
      )
      return rows
    }
    const hold = c.hold ?? ''
    const choices = c.choices ?? []
    rows.push(
      <Box key={`choices-${k}`}>
        {[...choices, ...(choices.includes('typed') ? [] : ['typed'])].map(choice => (
          <Button
            key={`choice-${k}-${choice}`}
            label={choice === 'typed' ? 'something else' : label(choice)}
            onPress={async () => {
              await said($, null)
              await update($, picking, () => ({ key: k, criterion_id: c.criterion_id, hold, choice, cwd }))
            }}
          />
        ))}
      </Box>,
    )
    const p = a.picking
    if (p !== null && p.key === k) {
      // the person's own words are no secret: an ordinary field, only for "something else"
      if (p.choice === 'typed' && Input) {
        rows.push(
          <Box key={`words-row-${k}`}>
            <Input key="words" label="your words" value={visible(a.words)} onInput={v => void update($, typedWords, () => v)} onSubmit={v => void update($, typedWords, () => v)} />
          </Box>,
        )
      }
      rows.push(
        <Box key={`sign-${k}`}>
          <Text>{SIGN_HERE}</Text>
        </Box>,
        <Box key={`command-${k}`}>
          <Text>{hasControl(a.words) || hasControl(p.cwd ?? '') ? CONTROL_WORDS : signCommand(p, a.words)}</Text>
        </Box>,
        <Box key={`unsigned-row-${k}`}>
          <Button key={`unsigned-${k}`} label="answer unsigned (CI reads: approval not verified)" onPress={() => unsigned($, done)} />
        </Box>,
      )
    }
    if (a.said && a.said.key === k) {
      rows.push(
        <Box key={`said-${k}`}>
          <Text>{a.said.text}</Text>
        </Box>,
      )
    }
    return rows
  }
}
