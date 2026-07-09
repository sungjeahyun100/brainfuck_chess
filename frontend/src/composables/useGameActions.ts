import type { Ref } from 'vue'
import { api } from '../api/gameApi'
import type { DropAction, GameState, PlayerId, Square } from '../types/game'
import type { LegalPieceOptions } from './useLegalOptions'

interface UseGameActionsOptions {
  getState: () => GameState
  getRoomId: () => string | null | undefined
  getLocalPlayer: () => PlayerId | null | undefined
  selectedPieceId: Ref<string | null>
  selectedPocketPieceId: Ref<string | null>
  abilityMode: Ref<boolean>
  activeAbilityId: Ref<string | null>
  legalTargetSquares: Ref<Square[]>
  dropSquares: Ref<Square[]>
  loadPieceOptions: (pieceId: string, abilityId?: string | null) => Promise<LegalPieceOptions>
  loadDropOptions: () => Promise<DropAction[]>
  selectBoardPiece: (pieceId: string) => Promise<LegalPieceOptions | null>
  selectPocketPiece: (pieceId: string) => Promise<Square[]>
  isLegalSquare: (square: Square, legalSquares: Square[]) => boolean
  requestPromotionChoice: (
    pieceId: string,
    to: Square,
    owner: PlayerId,
    choices: string[],
  ) => Promise<string | null>
  clearSelection: () => void
  setError: (message: string | null) => void
  emitStateUpdate: (state: GameState) => void
  confirmResign?: () => boolean
}

function sameSquare(left: Square, right: Square): boolean {
  return left.file === right.file && left.rank === right.rank
}

export function useGameActions(options: UseGameActionsOptions) {
  async function submitMove(pieceId: string, to: Square) {
    const state = options.getState()
    const fromPiece = state.pieces[pieceId]
    if (!fromPiece?.current_square || sameSquare(fromPiece.current_square, to)) {
      options.clearSelection()
      return
    }

    const moveAbilityId = options.selectedPieceId.value === pieceId && options.abilityMode.value
      ? options.activeAbilityId.value
      : null
    const legalOptions = options.selectedPieceId.value === pieceId
      && options.legalTargetSquares.value.length > 0
      ? await options.loadPieceOptions(pieceId, moveAbilityId)
      : await options.selectBoardPiece(pieceId)
    if (!legalOptions || !options.isLegalSquare(to, legalOptions.legalTargets)) {
      options.clearSelection()
      return
    }

    const promotionChoices = legalOptions.moves
      .filter(move => move.piece_id === pieceId && sameSquare(move.to, to) && move.promotion)
      .map(move => move.promotion as string)

    let promotion: string | undefined
    if (promotionChoices.length > 0) {
      const chosen = await options.requestPromotionChoice(pieceId, to, fromPiece.owner, promotionChoices)
      if (!chosen) {
        options.clearSelection()
        return
      }
      promotion = chosen
    }

    const selectedMove = legalOptions.moves.find(move =>
      move.piece_id === pieceId
      && sameSquare(move.to, to)
      && (move.promotion ?? undefined) === promotion
      && (move.ability_id ?? null) === (moveAbilityId ?? null)
    )
    if (!selectedMove) {
      options.clearSelection()
      return
    }

    try {
      const newState = await api.submitAction(state.id, selectedMove)
      options.emitStateUpdate(newState)
    } catch (e: unknown) {
      options.setError(e instanceof Error ? e.message : String(e))
    } finally {
      options.clearSelection()
    }
  }

  async function submitDrop(pieceId: string, to: Square) {
    const targets = options.selectedPocketPieceId.value === pieceId && options.dropSquares.value.length > 0
      ? options.dropSquares.value
      : await options.selectPocketPiece(pieceId)
    if (!options.isLegalSquare(to, targets)) {
      options.clearSelection()
      return
    }

    const drops = await options.loadDropOptions()
    const selectedDrop = drops.find(drop => drop.piece_id === pieceId && sameSquare(drop.to, to))
    if (!selectedDrop) {
      options.clearSelection()
      return
    }

    try {
      const state = options.getState()
      const newState = await api.submitAction(state.id, selectedDrop)
      options.emitStateUpdate(newState)
    } catch (e: unknown) {
      options.setError(e instanceof Error ? e.message : String(e))
    } finally {
      options.clearSelection()
    }
  }

  async function resign() {
    options.setError(null)
    const state = options.getState()
    if (state.phase === 'ended') return

    const resigningPlayer = options.getLocalPlayer() ?? state.current_player
    if (!(options.confirmResign?.() ?? true)) return

    try {
      const roomId = options.getRoomId()
      const newState = roomId
        ? await api.resignRoom(roomId, resigningPlayer)
        : await api.resignGame(state.id, resigningPlayer)
      options.clearSelection()
      options.emitStateUpdate(newState)
    } catch (e: unknown) {
      options.setError(e instanceof Error ? e.message : String(e))
    }
  }

  return {
    submitMove,
    submitDrop,
    resign,
  }
}
