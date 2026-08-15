import assert from 'node:assert/strict'
import test from 'node:test'
import {
  activeCooldownRemaining,
  isImmediateAbilityAction,
  moveOptionTargets,
  usesMoveSubmission,
} from './moveOptionUi.ts'
import type { AbilityAction, MoveAction } from './types/game.ts'

const effectlessMove = (overrides: Partial<MoveAction>): MoveAction => ({
  type: 'move',
  player_id: 'white',
  piece_id: 'cannon-rook',
  from: { file: 3, rank: 3 },
  to: { file: 3, rank: 5 },
  move_option_id: 'cannon_move',
  source_layer_ids: ['cannon_move'],
  effects: {
    global_state_updates: [],
    piece_state_updates: [],
    cooldown_updates: [],
  },
  ...overrides,
})

test('move-modifier abilities are submitted as moves', () => {
  assert.equal(usesMoveSubmission({ execution_mode: 'move_modifier' }), true)
  assert.equal(usesMoveSubmission({ execution_mode: 'standalone_action' }), false)
  assert.equal(usesMoveSubmission(null), false)
})

test('piece cooldown badge uses the largest active cooldown', () => {
  assert.equal(activeCooldownRemaining(undefined), 0)
  assert.equal(activeCooldownRemaining({ barrage: { remaining: 2 } }), 2)
  assert.equal(activeCooldownRemaining({ first: { remaining: 1 }, second: { remaining: 3 } }), 3)
})

test('cannon move targets retain normal move and capture highlights', () => {
  const moves = [
    effectlessMove({}),
    effectlessMove({
      to: { file: 3, rank: 6 },
      captured_piece_id: 'enemy',
    }),
  ]

  assert.deepEqual(moveOptionTargets(moves, []), {
    legalTargets: [{ file: 3, rank: 5 }, { file: 3, rank: 6 }],
    movable: [{ file: 3, rank: 5 }],
    captures: [{ file: 3, rank: 6 }],
  })
})

test('standalone ability targets remain selectable', () => {
  const actions = [{
    type: 'ability',
    player_id: 'white',
    piece_id: 'actor',
    ability_id: 'recall',
    to: { file: 4, rank: 4 },
    deployments: [],
  }] satisfies AbilityAction[]

  assert.deepEqual(moveOptionTargets([], actions), {
    legalTargets: [{ file: 4, rank: 4 }],
    movable: [{ file: 4, rank: 4 }],
    captures: [],
  })
})

test('targetless standalone abilities are recognized as immediate actions', () => {
  const action = {
    type: 'ability',
    player_id: 'white',
    piece_id: 'actor',
    ability_id: 'mortar-barrage',
    deployments: [],
  } satisfies AbilityAction

  assert.equal(isImmediateAbilityAction(action), true)
  assert.equal(isImmediateAbilityAction({ ...action, to: { file: 3, rank: 3 } }), false)
})
