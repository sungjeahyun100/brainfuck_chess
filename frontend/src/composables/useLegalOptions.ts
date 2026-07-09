import { api } from '../api/gameApi'
import type { DropAction, GameState, MoveAction, Square } from '../types/game'

export interface LegalPieceOptions {
  abilityId: string | null
  legalTargets: Square[]
  movable: Square[]
  captures: Square[]
  moves: MoveAction[]
}

function sameSquare(left: Square, right: Square): boolean {
  return left.file === right.file && left.rank === right.rank
}

export function useLegalOptions(getState: () => GameState) {
  const pieceOptionsCache = new Map<string, LegalPieceOptions>()
  const pieceOptionsRequests = new Map<string, Promise<LegalPieceOptions>>()
  const dropOptionsCache = new Map<string, DropAction[]>()
  const dropOptionsRequests = new Map<string, Promise<DropAction[]>>()

  function actionCacheKey(pieceId?: string, abilityId?: string | null): string {
    const state = getState()
    return [
      state.id,
      state.current_player,
      state.turn_number,
      state.turn_state.mode,
      state.turn_state.actions.length,
      pieceId ?? '',
      abilityId ?? '',
    ].join(':')
  }

  async function loadPieceOptions(
    pieceId: string,
    abilityId: string | null = null,
  ): Promise<LegalPieceOptions> {
    const state = getState()
    const key = actionCacheKey(pieceId, abilityId)
    const cached = pieceOptionsCache.get(key)
    if (cached) return cached

    const pending = pieceOptionsRequests.get(key)
    if (pending) return pending

    const request = api.getPieceOptions(state.id, pieceId, abilityId).then(({ moves }) => {
      const options: LegalPieceOptions = {
        abilityId,
        legalTargets: moves.map(move => move.to),
        movable: moves.filter(move => !move.captured_piece_id).map(move => move.to),
        captures: moves.filter(move => Boolean(move.captured_piece_id)).map(move => move.to),
        moves,
      }
      pieceOptionsCache.set(key, options)
      pieceOptionsRequests.delete(key)
      return options
    }).catch(error => {
      pieceOptionsRequests.delete(key)
      throw error
    })

    pieceOptionsRequests.set(key, request)
    return request
  }

  async function loadDropOptions(): Promise<DropAction[]> {
    const state = getState()
    const key = actionCacheKey('drops')
    const cached = dropOptionsCache.get(key)
    if (cached) return cached

    const pending = dropOptionsRequests.get(key)
    if (pending) return pending

    const request = api.getLegalDrops(state.id).then(({ drops }) => {
      dropOptionsCache.set(key, drops)
      dropOptionsRequests.delete(key)
      return drops
    }).catch(error => {
      dropOptionsRequests.delete(key)
      throw error
    })

    dropOptionsRequests.set(key, request)
    return request
  }

  function isLegalSquare(square: Square, legalSquares: Square[]): boolean {
    return legalSquares.some(target => sameSquare(target, square))
  }

  function clearLegalOptionsCache() {
    pieceOptionsCache.clear()
    pieceOptionsRequests.clear()
    dropOptionsCache.clear()
    dropOptionsRequests.clear()
  }

  return {
    loadPieceOptions,
    loadDropOptions,
    isLegalSquare,
    clearLegalOptionsCache,
  }
}
