import type { LobbyPlayer, SingleDeckSelection } from './types/deck'
import type { TimeControlId } from './types/game'

export function isValidGameNickname(value: string): boolean {
  const normalized = value.trim()
  return normalized.length > 0 && [...normalized].length <= 30 && ![...normalized].some(character => /\p{Cc}/u.test(character))
}

export function createSinglePlayerSelection(input: Omit<SingleDeckSelection, 'localNickname' | 'guestNickname'> & { localNickname: string; guestNickname: string; timeControl: TimeControlId }): SingleDeckSelection {
  return { ...input, localNickname: input.localNickname.trim(), guestNickname: input.guestNickname.trim() }
}

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
