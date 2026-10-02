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

test('the same piece moves from air to ground and back without death or drop', () => {
  const airborne = boardOccupancy({ size: 12, squares: {}, air_squares: { '4_10': 'b1' } })
  const landed = boardOccupancy({ size: 12, squares: { '10_3': 'b1' }, air_squares: {} })
  assert.deepEqual(presentationEvents(airborne, landed), [
    { type: 'move', pieceId: 'b1', from: { file: 4, rank: 10 }, to: { file: 10, rank: 3 } },
  ])
  assert.deepEqual(presentationEvents(landed, airborne), [
    { type: 'move', pieceId: 'b1', from: { file: 10, rank: 3 }, to: { file: 4, rank: 10 } },
  ])
})

test('capture checks destination layer while tracking the mover by piece ID', () => {
  const before = boardOccupancy({ size: 12, squares: { '0_0': 'a', '1_1': 'victim' }, air_squares: { '1_1': 'air' } })
  const after = boardOccupancy({ size: 12, squares: { '1_1': 'a' }, air_squares: { '1_1': 'air' } })
  assert.deepEqual(presentationEvents(before, after), [
    { type: 'capture', pieceId: 'a', from: { file: 0, rank: 0 }, to: { file: 1, rank: 1 } },
  ])

  const otherLayer = boardOccupancy({ size: 12, squares: { '0_0': 'a' }, air_squares: { '1_1': 'air' } })
  assert.deepEqual(presentationEvents(otherLayer, after), [
    { type: 'move', pieceId: 'a', from: { file: 0, rank: 0 }, to: { file: 1, rank: 1 } },
  ])
})

test('a layer transition captures only a piece removed from its destination layer', () => {
  const before = boardOccupancy({ size: 12, squares: { '1_1': 'victim' }, air_squares: { '0_0': 'flyer' } })
  const after = boardOccupancy({ size: 12, squares: { '1_1': 'flyer' }, air_squares: {} })
  assert.deepEqual(presentationEvents(before, after), [
    { type: 'capture', pieceId: 'flyer', from: { file: 0, rank: 0 }, to: { file: 1, rank: 1 } },
  ])
})
