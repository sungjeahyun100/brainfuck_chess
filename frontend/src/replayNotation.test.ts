import assert from 'node:assert/strict'
import test from 'node:test'
import { formatNotation, groupNotation } from './replayNotation.ts'
import type { RecordedNotationAction } from './types/gameRecord.ts'

function notation(kind: RecordedNotationAction['kind'], side: 'white' | 'black', ability?: string): RecordedNotationAction {
  return { move_number: 1, side, actor: { piece_id: 'p', piece_type_id: 'tank', piece_name: 'tank', from: { file: 4, rank: 2 }, layer: 'ground', state: {} }, kind, ability_name: ability, from: { file: 4, rank: 2 }, to: { file: 4, rank: 4 }, ability_events: [] }
}

test('formats normal, ability and drop actions from structured notation', () => {
  assert.equal(formatNotation(notation('move', 'white')), 'tank - e3 - e5')
  assert.equal(formatNotation(notation('ability', 'white', '포격')), 'tank - 포격 - e3 - e5')
  assert.equal(formatNotation(notation('drop', 'white')), 'tank - 착수 - e5')
})

test('groups two plies into a full move without losing either side', () => {
  const white = { notation: notation('move', 'white'), ply: 1 }
  const black = { notation: notation('move', 'black'), ply: 2 }
  const rows = groupNotation([white, black])
  assert.equal(rows.length, 1); assert.equal(rows[0].white?.ply, 1); assert.equal(rows[0].black?.ply, 2)
})
