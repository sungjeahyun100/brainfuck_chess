import assert from 'node:assert/strict'
import test from 'node:test'
import { formatNotation, fullMoveNumber, groupNotation } from './replayNotation.ts'
import type { RecordedNotationAction } from './types/gameRecord.ts'

function notation(side: 'white' | 'black', moveNumber = 1, overrides: Partial<RecordedNotationAction> = {}): RecordedNotationAction {
  return {
    turn_number: moveNumber * 2 - (side === 'white' ? 1 : 0), move_number: moveNumber, side,
    actor: { piece_id: 'p', piece_type_id: 'tank', piece_name: 'tank', from: { file: 4, rank: 2 }, layer: 'ground', state: {} },
    kind: 'move', from: { file: 4, rank: 2 }, to: { file: 4, rank: 4 }, ability_events: [], ...overrides,
  }
}

function entries(sides: Array<'white' | 'black'>, moveNumber = 1) {
  return sides.map((side, index) => ({ notation: notation(side, moveNumber), ply: index + 1 }))
}

test('formats normal, ability and drop actions without changing notation rules', () => {
  assert.equal(formatNotation(notation('white')), 'tank - e3 - e5')
  assert.equal(formatNotation(notation('white', 1, { kind: 'ability', ability_name: '포격' })), 'tank - 포격 - e3 - e5')
  assert.equal(formatNotation(notation('white', 1, { kind: 'drop' })), 'tank - 착수 - e5')
})

test('derives human full-move numbers from authoritative engine player-turn numbers', () => {
  assert.deepEqual([1, 2, 3, 4, 5].map(fullMoveNumber), [1, 1, 2, 2, 3])
})

test('groups ordinary white/black alternation in recorded order', () => {
  const rows = groupNotation(entries(['white', 'black']))
  assert.deepEqual(rows.map(row => row.entries.map(entry => entry.notation.side)), [['white', 'black']])
})

test('preserves white, white, black without overwriting either white action', () => {
  const rows = groupNotation(entries(['white', 'white', 'black']))
  assert.deepEqual(rows[0].entries.map(entry => entry.ply), [1, 2, 3])
  assert.deepEqual(rows[0].entries.map(entry => entry.notation.side), ['white', 'white', 'black'])
})

test('preserves three same-side actions and does not create a missing-side placeholder', () => {
  const first = entries(['white', 'white', 'white', 'black'])
  const last = { notation: notation('white', 2), ply: 5 }
  const rows = groupNotation([...first, last])
  assert.deepEqual(rows[0].entries.map(entry => entry.ply), [1, 2, 3, 4])
  assert.deepEqual(rows[1].entries.map(entry => entry.ply), [5])
})

test('keeps bomber move and forced landing as separate clickable ply entries', () => {
  const bomber = { piece_id: 'b', piece_type_id: 'bomber', piece_name: 'bomber', from: { file: 4, rank: 10 }, layer: 'air' as const, state: {} }
  const move = { notation: notation('white', 1, { actor: bomber, from: { file: 4, rank: 10 }, to: { file: 4, rank: 3 } }), ply: 1 }
  const landing = { notation: notation('white', 1, { actor: { ...bomber, from: { file: 4, rank: 3 } }, kind: 'ability', ability_id: 'forced-landing', ability_name: '강제 착륙', from: { file: 4, rank: 3 }, to: { file: 10, rank: 3 } }), ply: 2 }
  const black = { notation: notation('black', 1), ply: 3 }
  const row = groupNotation([move, landing, black])[0]
  assert.equal(formatNotation(row.entries[0].notation), 'bomber - e11 - e4')
  assert.equal(formatNotation(row.entries[1].notation), 'bomber - 강제 착륙 - e4 - k4')
  assert.deepEqual(row.entries.map(entry => entry.ply), [1, 2, 3])
})
