import assert from 'node:assert/strict'
import test from 'node:test'
import { createSinglePlayerSelection, isValidGameNickname, mapSinglePlayerDecks, resolveLocalSide } from './singlePlayerSetup.ts'
import { timeControlLabel } from './timeControls.ts'

test('singleplayer fixed and random sides resolve exactly once', () => {
  assert.equal(resolveLocalSide('white', 1), 'white')
  assert.equal(resolveLocalSide('black', 0), 'black')
  assert.equal(resolveLocalSide('random', 2), 'white')
  assert.equal(resolveLocalSide('random', 3), 'black')
})

test('history time-control labels cover every supported clock', () => {
  assert.deepEqual(['five_zero', 'ten_zero', 'five_three', 'ten_five', 'fifteen_ten', 'unlimited'].map(id => timeControlLabel(id as Parameters<typeof timeControlLabel>[0])), ['5 + 0', '10 + 0', '5 + 3', '10 + 5', '15 + 10', '무제한'])
})

test('singleplayer nickname validation and payload preserve per-game overrides', () => {
  assert.equal(isValidGameNickname('Player A'), true)
  assert.equal(isValidGameNickname('   '), false)
  assert.equal(isValidGameNickname('bad\nname'), false)
  assert.equal(isValidGameNickname('x'.repeat(31)), false)
  assert.deepEqual(createSinglePlayerSelection({ localDeckId: 'mine', opponentDeckId: 'guest', localSide: 'white', localNickname: '  Match Name ', guestNickname: ' Guest ', timeControl: 'ten_five' }), {
    localDeckId: 'mine', opponentDeckId: 'guest', localSide: 'white', localNickname: 'Match Name', guestNickname: 'Guest', timeControl: 'ten_five',
  })
})

test('local and opponent decks follow the resolved side', () => {
  assert.deepEqual(mapSinglePlayerDecks('white', 'mine', 'guest'), { white: 'mine', black: 'guest' })
  assert.deepEqual(mapSinglePlayerDecks('black', 'mine', 'guest'), { white: 'guest', black: 'mine' })
})
