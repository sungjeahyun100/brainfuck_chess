import assert from 'node:assert/strict'
import test from 'node:test'
import { mapSinglePlayerDecks, resolveLocalSide } from './singlePlayerSetup.ts'

test('singleplayer fixed and random sides resolve exactly once', () => {
  assert.equal(resolveLocalSide('white', 1), 'white')
  assert.equal(resolveLocalSide('black', 0), 'black')
  assert.equal(resolveLocalSide('random', 2), 'white')
  assert.equal(resolveLocalSide('random', 3), 'black')
})

test('local and opponent decks follow the resolved side', () => {
  assert.deepEqual(mapSinglePlayerDecks('white', 'mine', 'guest'), { white: 'mine', black: 'guest' })
  assert.deepEqual(mapSinglePlayerDecks('black', 'mine', 'guest'), { white: 'guest', black: 'mine' })
})
