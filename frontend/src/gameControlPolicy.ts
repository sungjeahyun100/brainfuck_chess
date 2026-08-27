import type { PlayerId } from './types/game'

export type PlayMode = 'single' | 'bot' | 'challenge' | 'multiplayer'

interface TurnControlContext {
  playMode: PlayMode
  currentPlayer: PlayerId
  localPlayer?: PlayerId | null
  botPlayer?: PlayerId | null
}

export function canControlCurrentTurn(context: TurnControlContext): boolean {
  if (context.playMode === 'single') return true
  if (!context.localPlayer || context.currentPlayer !== context.localPlayer) return false
  return !['bot', 'challenge'].includes(context.playMode) || context.currentPlayer !== context.botPlayer
}

export function turnControlLabel(context: TurnControlContext): string {
  if (context.playMode === 'single') {
    return context.currentPlayer === 'white' ? 'White 턴' : 'Black 턴'
  }
  if (['bot', 'challenge'].includes(context.playMode) && context.currentPlayer === context.botPlayer) return '봇 턴'
  return context.currentPlayer === context.localPlayer ? '내 턴' : '상대 턴'
}

export function blockedControlMessage(context: TurnControlContext): string {
  return ['bot', 'challenge'].includes(context.playMode) && context.currentPlayer === context.botPlayer
    ? '봇 턴입니다.'
    : '상대 턴입니다.'
}

export function resigningPlayer(context: TurnControlContext): PlayerId {
  return context.playMode === 'single'
    ? context.currentPlayer
    : context.localPlayer ?? context.currentPlayer
}
