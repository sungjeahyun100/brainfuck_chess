import { computed, ref, watch, type Ref } from 'vue'
import { api } from '../api/gameApi'
import type {
  AiAction,
  BotDifficulty,
  BotTurnStats,
  DropAction,
  GameState,
  MoveAction,
  PlayerId,
  Square,
} from '../types/game'

interface UseBotReplayOptions {
  getState: () => GameState
  getBotPlayer: () => PlayerId | null | undefined
  getBotDifficulty: () => BotDifficulty | undefined
  isBotTurn: () => boolean
  selectedPieceId: Ref<string | null>
  movableSquares: Ref<Square[]>
  attackSquares: Ref<Square[]>
  dropSquares: Ref<Square[]>
  abilityMode: Ref<boolean>
  clearSelection: () => void
  emitStateUpdate: (state: GameState) => void
}

const BOT_ACTION_PREVIEW_MS = 520
const BOT_ACTION_SETTLE_MS = 340

function wait(ms: number): Promise<void> {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

function cloneGameState(state: GameState): GameState {
  return JSON.parse(JSON.stringify(state)) as GameState
}

function squareId(square: Square): string {
  return `${square.file}_${square.rank}`
}

function otherPlayer(player: PlayerId): PlayerId {
  return player === 'white' ? 'black' : 'white'
}

function removePieceFromBoard(state: GameState, pieceId: string) {
  for (const [id, occupant] of Object.entries(state.board.squares)) {
    if (occupant === pieceId) state.board.squares[id] = null
  }
}

function applyMoveForReplay(state: GameState, action: MoveAction): GameState {
  const next = cloneGameState(state)
  next.turn_state.mode = 'move'
  const movedPiece = next.pieces[action.piece_id]
  const isCastling = movedPiece?.type_id === 'king'
    && Math.abs(action.to.file - action.from.file) === 2
    && action.to.rank === action.from.rank
  next.board.squares[squareId(action.from)] = null

  const capturedPieceId = action.captured_piece_id ?? next.board.squares[squareId(action.to)] ?? undefined
  if (capturedPieceId) {
    removePieceFromBoard(next, capturedPieceId)
    const capturedPiece = next.pieces[capturedPieceId]
    if (capturedPiece) {
      capturedPiece.captured = true
      capturedPiece.current_square = undefined
    }
    const opponent = next.players[otherPlayer(action.player_id)]
    if (opponent && !opponent.captured_pieces.includes(capturedPieceId)) {
      opponent.captured_pieces.push(capturedPieceId)
    }
  }

  if (isCastling) {
    const direction = Math.sign(action.to.file - action.from.file)
    let rookFile = action.from.file + direction
    while (rookFile >= 0 && rookFile < next.board.size) {
      const rookSquare = { file: rookFile, rank: action.from.rank }
      const rookId = next.board.squares[squareId(rookSquare)]
      const rook = rookId ? next.pieces[rookId] : null
      if (rookId && rook) {
        if (rook.owner === action.player_id && rook.type_id === 'rook' && rook.current_square) {
          const rookTo = { file: action.from.file + direction, rank: action.from.rank }
          next.board.squares[squareId(rookSquare)] = null
          next.board.squares[squareId(rookTo)] = rookId
          rook.current_square = rookTo
          rook.has_moved = true
        }
        break
      }
      rookFile += direction
    }
  }

  next.board.squares[squareId(action.to)] = action.piece_id
  if (movedPiece) {
    movedPiece.current_square = action.to
    movedPiece.has_moved = true
    if (action.promotion) {
      movedPiece.type_id = action.promotion
    }
  }

  next.turn_state.actions.push(action)

  const capturedTypeId = capturedPieceId ? next.pieces[capturedPieceId]?.type_id : undefined
  if (capturedTypeId && next.piece_definitions[capturedTypeId]?.is_king) {
    next.phase = 'ended'
    next.result = { winner: action.player_id, reason: 'king_capture' }
  }

  return next
}

function applyDropForReplay(state: GameState, action: DropAction): GameState {
  const next = cloneGameState(state)
  next.turn_state.mode = 'drop'

  const player = next.players[action.player_id]
  if (player) {
    player.deck.pocket_pieces = player.deck.pocket_pieces.filter(id => id !== action.piece_id)
  }

  const piece = next.pieces[action.piece_id]
  if (piece) {
    piece.in_pocket = false
    piece.current_square = action.to
  }
  next.board.squares[squareId(action.to)] = action.piece_id
  next.turn_state.actions.push(action)

  return next
}

function applyEndTurnForReplay(state: GameState): GameState {
  if (state.turn_state.actions.length === 0) return state

  const next = cloneGameState(state)
  next.current_player = otherPlayer(next.current_player)
  next.turn_number += 1
  next.turn_state = {
    mode: 'undecided',
    actions: [],
  }

  return next
}

function applyActionForReplay(state: GameState, action: AiAction): GameState {
  if (action.type === 'move') return applyMoveForReplay(state, action)
  if (action.type === 'drop') return applyDropForReplay(state, action)
  return applyEndTurnForReplay(state)
}

export function useBotReplay(options: UseBotReplayOptions) {
  const botError = ref<string | null>(null)
  const botThinking = ref(false)
  const botReplaying = ref(false)
  const botReplayMessage = ref<string | null>(null)
  const lastBotStats = ref<BotTurnStats | null>(null)
  const botPreviewSelectedPieceId = ref<string | null>(null)
  const botPreviewMovableSquares = ref<Square[]>([])
  const botPreviewAttackSquares = ref<Square[]>([])
  const botPreviewDropSquares = ref<Square[]>([])
  const botReplayState = ref<GameState | null>(null)
  let botRunSerial = 0

  const viewState = computed(() => botReplayState.value ?? options.getState())
  const visibleSelectedPieceId = computed(() => (
    botReplaying.value ? botPreviewSelectedPieceId.value : options.selectedPieceId.value
  ))
  const visibleMovableSquares = computed(() => (
    botReplaying.value ? botPreviewMovableSquares.value : options.movableSquares.value
  ))
  const visibleAttackSquares = computed(() => (
    botReplaying.value ? botPreviewAttackSquares.value : options.attackSquares.value
  ))
  const visibleDropSquares = computed(() => (
    botReplaying.value ? botPreviewDropSquares.value : options.dropSquares.value
  ))
  const visibleAbilityMode = computed(() => !botReplaying.value && options.abilityMode.value)

  function clearBotPreview() {
    botPreviewSelectedPieceId.value = null
    botPreviewMovableSquares.value = []
    botPreviewAttackSquares.value = []
    botPreviewDropSquares.value = []
  }

  function clearBotReplay() {
    botReplayState.value = null
    clearBotPreview()
  }

  function actionLabel(action: AiAction): string {
    if (action.type === 'end_turn') return '턴 종료'

    const state = options.getState()
    const piece = state.pieces[action.piece_id]
    const pieceName = state.piece_definitions[piece?.type_id ?? '']?.name ?? action.piece_id
    if (action.type === 'drop') {
      return `${pieceName} 포켓 기물 놓기: ${action.to.file + 1}, ${action.to.rank + 1}`
    }

    const captureText = action.captured_piece_id ? ' 포획' : ' 이동'
    return `${pieceName}${captureText}: ${action.from.file + 1}, ${action.from.rank + 1} -> ${action.to.file + 1}, ${action.to.rank + 1}`
  }

  function previewBotAction(action: AiAction) {
    clearBotPreview()
    if (action.type === 'move') {
      botPreviewSelectedPieceId.value = action.piece_id
      if (action.captured_piece_id) {
        botPreviewAttackSquares.value = [action.to]
      } else {
        botPreviewMovableSquares.value = [action.to]
      }
    } else if (action.type === 'drop') {
      botPreviewDropSquares.value = [action.to]
    }
  }

  async function replayBotTurn(actions: AiAction[], finalState: GameState, runId: number) {
    if (actions.length === 0) {
      options.emitStateUpdate(finalState)
      return
    }

    botReplaying.value = true
    let nextReplayState = cloneGameState(options.getState())
    for (let index = 0; index < actions.length; index++) {
      if (runId !== botRunSerial) return

      const action = actions[index]
      botReplayMessage.value = `${index + 1}/${actions.length} ${actionLabel(action)}`
      previewBotAction(action)
      await wait(BOT_ACTION_PREVIEW_MS)
      if (runId !== botRunSerial) return

      nextReplayState = applyActionForReplay(nextReplayState, action)
      botReplayState.value = nextReplayState
      clearBotPreview()
      await wait(BOT_ACTION_SETTLE_MS)
    }

    if (runId !== botRunSerial) return
    botReplayState.value = null
    options.emitStateUpdate(finalState)
  }

  async function runBotTurn() {
    const botPlayer = options.getBotPlayer()
    if (!botPlayer || !options.isBotTurn() || botThinking.value) return

    const runId = ++botRunSerial
    botThinking.value = true
    botReplaying.value = false
    botReplayMessage.value = null
    botError.value = null
    options.clearSelection()
    clearBotReplay()
    try {
      const state = options.getState()
      const response = await api.botTurn(
        state.id,
        botPlayer,
        options.getBotDifficulty() ?? 'normal',
      )
      if (runId !== botRunSerial) return
      lastBotStats.value = response.stats
      await replayBotTurn(response.actions, response.game_state, runId)
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e)
      if (message.includes('현재 턴 플레이어와 bot_player_id가 일치하지 않습니다.')) {
        try {
          const syncedState = await api.getGame(options.getState().id)
          options.emitStateUpdate(syncedState)
          botError.value = null
        } catch {
          botError.value = message
        }
      } else {
        botError.value = message
      }
    } finally {
      if (runId === botRunSerial) {
        botThinking.value = false
        botReplaying.value = false
        botReplayMessage.value = null
        clearBotReplay()
      }
    }
  }

  watch(
    () => {
      const state = options.getState()
      return [
        state.id,
        state.current_player,
        state.turn_number,
        state.phase,
        options.getBotPlayer(),
        options.getBotDifficulty(),
      ]
    },
    () => {
      if (options.isBotTurn()) void runBotTurn()
    },
    { immediate: true },
  )

  return {
    botError,
    botThinking,
    botReplaying,
    botReplayMessage,
    lastBotStats,
    viewState,
    visibleSelectedPieceId,
    visibleMovableSquares,
    visibleAttackSquares,
    visibleDropSquares,
    visibleAbilityMode,
    runBotTurn,
  }
}
