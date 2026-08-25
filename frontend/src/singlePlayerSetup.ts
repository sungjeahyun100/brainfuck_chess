import type { LobbyPlayer } from './types/deck'

export function resolveLocalSide(choice: LobbyPlayer | 'random', randomByte?: number): LobbyPlayer {
  if (choice !== 'random') return choice
  const value = randomByte ?? crypto.getRandomValues(new Uint8Array(1))[0]
  return value % 2 === 0 ? 'white' : 'black'
}

export function mapSinglePlayerDecks<T>(localSide: LobbyPlayer, localDeck: T, opponentDeck: T): { white: T; black: T } {
  return localSide === 'white'
    ? { white: localDeck, black: opponentDeck }
    : { white: opponentDeck, black: localDeck }
}
