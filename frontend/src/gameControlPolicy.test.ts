import assert from 'node:assert/strict'
import test from 'node:test'
import {
  blockedControlMessage,
  canControlCurrentTurn,
  resigningPlayer,
  turnControlLabel,
  type PlayMode,
} from './gameControlPolicy.ts'
import type { PlayerId } from './types/game.ts'

function context(
  playMode: PlayMode,
  currentPlayer: PlayerId,
  localPlayer: PlayerId = 'white',
  botPlayer: PlayerId | null = null,
) {
  return { playMode, currentPlayer, localPlayer, botPlayer }
}

test('single hot-seat controls both sides regardless of the oriented local side', () => {
  assert.equal(canControlCurrentTurn(context('single', 'white', 'white')), true)
  assert.equal(canControlCurrentTurn(context('single', 'black', 'white')), true)
  assert.equal(canControlCurrentTurn(context('single', 'white', 'black')), true)
  assert.equal(canControlCurrentTurn(context('single', 'black', 'black')), true)
  assert.equal(turnControlLabel(context('single', 'white')), 'White 턴')
  assert.equal(turnControlLabel(context('single', 'black')), 'Black 턴')
})

test('single keeps control for a same-player forced follow-up action', () => {
  const forcedLanding = context('single', 'white', 'black')
  assert.equal(canControlCurrentTurn(forcedLanding), true)
  assert.equal(canControlCurrentTurn(forcedLanding), true)
})

test('bot mode permits only the human side and keeps bot messaging', () => {
  assert.equal(canControlCurrentTurn(context('bot', 'white', 'white', 'black')), true)
  assert.equal(canControlCurrentTurn(context('bot', 'black', 'white', 'black')), false)
  assert.equal(turnControlLabel(context('bot', 'black', 'white', 'black')), '봇 턴')
  assert.equal(blockedControlMessage(context('bot', 'black', 'white', 'black')), '봇 턴입니다.')
})

test('multiplayer permits only the local side', () => {
  assert.equal(canControlCurrentTurn(context('multiplayer', 'black', 'black')), true)
  assert.equal(canControlCurrentTurn(context('multiplayer', 'white', 'black')), false)
  assert.equal(turnControlLabel(context('multiplayer', 'white', 'black')), '상대 턴')
})

test('resignation follows the active side in single and the human side otherwise', () => {
  assert.equal(resigningPlayer(context('single', 'black', 'white')), 'black')
  assert.equal(resigningPlayer(context('bot', 'black', 'white', 'black')), 'white')
  assert.equal(resigningPlayer(context('multiplayer', 'white', 'black')), 'black')
})
