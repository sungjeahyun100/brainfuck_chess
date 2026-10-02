import assert from 'node:assert/strict'
import test from 'node:test'
import { boardOccupancy, presentationEvents } from './presentationEvents.ts'

test('committed occupancy yields movement, capture, drop and simultaneous deaths without changing state', () => {
  const before = { 'ground:a': '0_0', 'ground:b': '1_1', 'ground:c': '2_2' }
  const after = { 'ground:a': '1_1', 'ground:d': '3_3' }
  assert.deepEqual(presentationEvents(before, after), [
    { type: 'capture', pieceId: 'a', from: { file: 0, rank: 0 }, to: { file: 1, rank: 1 } },
    { type: 'drop', pieceId: 'd', to: { file: 3, rank: 3 } },
    { type: 'death', at: { file: 2, rank: 2 } },
  ])
  assert.equal(before['ground:a'], '0_0')
})

test('air and ground occupancy remain independent and successive movement is derived independently', () => {
  const first = boardOccupancy({ size: 4, squares: { '0_0': 'a' }, air_squares: { '0_0': 'b' } })
  const second = boardOccupancy({ size: 4, squares: { '1_0': 'a' }, air_squares: { '0_0': 'b' } })
  const third = boardOccupancy({ size: 4, squares: { '2_0': 'a' }, air_squares: { '0_0': 'b' } })
  assert.deepEqual(presentationEvents(first, second), [{ type: 'move', pieceId: 'a', from: { file: 0, rank: 0 }, to: { file: 1, rank: 0 } }])
  assert.deepEqual(presentationEvents(second, third), [{ type: 'move', pieceId: 'a', from: { file: 1, rank: 0 }, to: { file: 2, rank: 0 } }])
})
