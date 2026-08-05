<template>
  <div class="game-screen">
    <!-- Header -->
    <div class="header">
      <h2>덱체스 <small class="title-en">Deck Chess</small></h2>
      <div class="turn-info">
        <span class="player-badge" :class="`player-${viewState.current_player}`">
          {{ viewState.current_player === 'white' ? '⬜ White' : '⬛ Black' }}
        </span>
        <span v-if="localPlayer" class="local-badge" :class="{ waiting: !isMyTurn }">
          {{ isBotTurn ? '봇 턴' : isMyTurn ? '내 턴' : '상대 턴' }}
        </span>
        <span v-if="botPlayer" class="bot-badge">🤖 {{ botDifficultyLabel }}</span>
        <span class="turn-badge">Turn {{ viewState.turn_number }}</span>
      </div>
    </div>

    <div v-if="botPlayer" class="bot-status" :class="{ thinking: botThinking || botReplaying, failed: Boolean(botError) }" aria-live="polite">
      <div>
        <strong>{{ botStatusTitle }}</strong>
        <small v-if="botReplayMessage">{{ botReplayMessage }}</small>
        <small v-if="lastBotStats">
          최근 탐색 {{ lastBotStats.searched_nodes.toLocaleString() }}노드 · 깊이 {{ lastBotStats.depth_reached }} · {{ lastBotStats.elapsed_ms }}ms
        </small>
        <small v-else-if="!botReplayMessage">{{ playerName(botPlayer) }} 봇 · {{ botDifficultyLabel }}</small>
      </div>
      <button v-if="botError && isBotTurn && !botThinking && !botReplaying" @click="runBotTurn">다시 시도</button>
    </div>

    <!-- Promotion picker overlay -->
    <div v-if="promotionRequest" class="promotion-overlay">
      <div class="promotion-box">
        <h3>기물 승격</h3>
        <p>Pawn이 도착할 기물을 선택하세요.</p>
        <div class="promotion-choices">
          <button
            v-for="choice in promotionRequest.options"
            :key="choice"
            class="promotion-choice"
            type="button"
            @click="choosePromotion(choice)"
          >
            <img
              v-if="pieceAsset(choice, promotionRequest.owner)"
              class="promotion-choice-image"
              :src="pieceAsset(choice, promotionRequest.owner)"
              :alt="promotionPieceLabel(choice)"
            />
            <span v-else>{{ pieceSymbol(choice) }}</span>
            <small>{{ promotionPieceLabel(choice) }}</small>
          </button>
        </div>
      </div>
    </div>

    <!-- Game over overlay -->
    <div v-if="viewState.phase === 'ended'" class="game-over-overlay">
      <div class="game-over-box">
        <h2>Game Over</h2>
        <p v-if="viewState.result?.winner">
          {{ viewState.result.winner === 'white' ? '⬜ White' : '⬛ Black' }} wins!
          <br><small>({{ viewState.result.reason }})</small>
        </p>
        <p v-else>Draw</p>
        <button @click="$emit('restart')">New Game</button>
      </div>
    </div>

    <div class="main-layout" :class="{ locked: botThinking || botReplaying || isBotTurn }">
      <!-- Left: Pocket (White) -->
      <div class="pocket">
        <h4>⬜ White Pocket</h4>
        <div class="pocket-pieces">
          <div
            v-for="group in whitePocketGroups"
            :key="group.typeId"
            class="pocket-piece-row"
            :class="{ selected: selectedPocketPieceId ? group.pieceIds.includes(selectedPocketPieceId) : false }"
            draggable="true"
            @click="onPocketClick(group.representativeId)"
            @dragstart="onPocketDragStart($event, group.representativeId)"
            @dragend="onPocketDragEnd"
          >
            <img
              v-if="pieceImage(group.representativeId)"
              class="pocket-piece-image"
              :src="pieceImage(group.representativeId)"
              :alt="pieceAlt(group.representativeId)"
              draggable="false"
            />
            <span v-else class="pocket-piece-symbol">{{ pieceSymbol(group.typeId) }}</span>
            <span class="pocket-piece-meta">
              <strong>{{ group.name }}</strong>
              <span class="pocket-count-bar">
                <span :style="{ width: pocketGroupFillWidth(group.count, maxWhitePocketCount) }"></span>
              </span>
            </span>
            <span class="pocket-piece-count">{{ group.count }}</span>
          </div>
        </div>
        <div class="score-info" v-if="whiteDeck">
          <span>{{ whiteDeck.total_score }} / {{ whiteDeck.score_limit }} pts</span>
        </div>
      </div>

      <!-- Center: Board -->
      <div class="board-column">
        <Board
          :board="viewState.board"
          :pieces="viewState.pieces"
          :definitions="viewState.piece_definitions"
          :selected-piece-id="visibleSelectedPieceId"
          :movable-squares="visibleMovableSquares"
          :attack-squares="visibleAttackSquares"
          :drop-squares="visibleDropSquares"
          :last-move="lastMove"
          :orientation="boardOrientation"
          :ability-mode="visibleAbilityMode"
          @square-click="onSquareClick"
          @piece-drag-start="onBoardPieceDragStart"
          @square-drop="onSquareDrop"
        />

        <div v-if="selectedPieceId && selectedPieceDefinition" class="selected-piece-panel" :class="{ active: abilityMode }">
          <div>
            <strong>{{ selectedPieceDefinition.name }}</strong>
            <small v-if="abilityMode && selectedAbility">특수 능력 모드 · {{ selectedAbility.name }}</small>
            <small v-else-if="selectedPieceAbilities.length">특수 능력 사용 가능</small>
            <small v-else>일반 이동 모드</small>
          </div>
          <div v-if="selectedPieceAbilities.length" class="ability-actions">
            <button
              v-for="ability in selectedPieceAbilities"
              :key="ability.id"
              class="ability-button"
              :class="{ active: abilityMode && activeAbilityId === ability.id }"
              type="button"
              :disabled="Boolean(abilityUnavailableReason(ability))"
              :title="abilityUnavailableReason(ability) || ability.description"
              @click="toggleAbilityMode(ability.id)"
            >
              {{ ability.name || '특수 능력' }}
            </button>
          </div>
          <small v-if="selectedPieceAbilities.length && selectedAbilityHelpText" class="ability-help">
            {{ selectedAbilityHelpText }}
          </small>
        </div>
      </div>

      <!-- Right: Pocket (Black) -->
      <div class="pocket">
        <h4>⬛ Black Pocket</h4>
        <div class="pocket-pieces">
          <div
            v-for="group in blackPocketGroups"
            :key="group.typeId"
            class="pocket-piece-row"
            :class="{ selected: selectedPocketPieceId ? group.pieceIds.includes(selectedPocketPieceId) : false }"
            draggable="true"
            @click="onPocketClick(group.representativeId)"
            @dragstart="onPocketDragStart($event, group.representativeId)"
            @dragend="onPocketDragEnd"
          >
            <img
              v-if="pieceImage(group.representativeId)"
              class="pocket-piece-image"
              :src="pieceImage(group.representativeId)"
              :alt="pieceAlt(group.representativeId)"
              draggable="false"
            />
            <span v-else class="pocket-piece-symbol">{{ pieceSymbol(group.typeId) }}</span>
            <span class="pocket-piece-meta">
              <strong>{{ group.name }}</strong>
              <span class="pocket-count-bar">
                <span :style="{ width: pocketGroupFillWidth(group.count, maxBlackPocketCount) }"></span>
              </span>
            </span>
            <span class="pocket-piece-count">{{ group.count }}</span>
          </div>
        </div>
        <div class="score-info" v-if="blackDeck">
          <span>{{ blackDeck.total_score }} / {{ blackDeck.score_limit }} pts</span>
        </div>
      </div>
    </div>

    <!-- Footer: actions -->
    <div class="footer">
      <button
        class="btn btn-resign"
        :disabled="botThinking || botReplaying || viewState.phase === 'ended' || (Boolean(roomId) && !localPlayer)"
        @click="onResign"
      >
        기권
      </button>
    </div>

    <div v-if="error || botError" class="error-banner">{{ error || botError }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type {
  AiAction,
  ActionTimelineFrame,
  BotDifficulty,
  BotTurnStats,
  DropAction,
  GameState,
  MoveAction,
  MoveOptionDefinition,
  PlayerId,
  Square,
  SubmitDropAction,
  SubmitMoveAction,
} from '../types/game'
import { api } from '../api/gameApi'
import { pieceAsset, renderedPieceAsset } from '../pieceAssets'
import Board from './Board.vue'
import { applyTimelineFrame } from '../composables/useActionTimeline'

const props = defineProps<{
  state: GameState
  localPlayer?: PlayerId | null
  roomId?: string | null
  botPlayer?: PlayerId | null
  botDifficulty?: BotDifficulty
}>()
const emit = defineEmits<{
  stateUpdate: [state: GameState]
  restart: []
}>()

const selectedPieceId = ref<string | null>(null)
const selectedPocketPieceId = ref<string | null>(null)
const abilityMode = ref(false)
const activeAbilityId = ref<string | null>(null)
const legalTargetSquares = ref<Square[]>([])
const movableSquares = ref<Square[]>([])
const attackSquares = ref<Square[]>([])
const dropSquares = ref<Square[]>([])
const error = ref<string | null>(null)
const botError = ref<string | null>(null)
const botThinking = ref(false)
const botReplaying = ref(false)
const botReplayMessage = ref<string | null>(null)
const lastBotStats = ref<BotTurnStats | null>(null)
const draggedPocketPieceId = ref<string | null>(null)
const promotionRequest = ref<{ pieceId: string; to: Square; owner: PlayerId; options: string[] } | null>(null)
let promotionResolve: ((choice: string | null) => void) | null = null
const botPreviewSelectedPieceId = ref<string | null>(null)
const botPreviewMovableSquares = ref<Square[]>([])
const botPreviewAttackSquares = ref<Square[]>([])
const botPreviewDropSquares = ref<Square[]>([])
const botReplayState = ref<GameState | null>(null)
let botRunSerial = 0

const BOT_ACTION_PREVIEW_MS = 520
const BOT_ACTION_SETTLE_MS = 340

interface PocketGroup {
  typeId: string
  name: string
  representativeId: string
  pieceIds: string[]
  count: number
}

interface LegalPieceOptions {
  abilityId: string | null
  legalTargets: Square[]
  movable: Square[]
  captures: Square[]
  moves: MoveAction[]
}

const pieceOptionsCache = new Map<string, LegalPieceOptions>()
const pieceOptionsRequests = new Map<string, Promise<LegalPieceOptions>>()
const dropOptionsCache = new Map<string, DropAction[]>()
const dropOptionsRequests = new Map<string, Promise<DropAction[]>>()

const viewState = computed(() => botReplayState.value ?? props.state)
const whitePocket = computed(() =>
  viewState.value.players['white']?.deck.pocket_pieces ?? []
)
const blackPocket = computed(() =>
  viewState.value.players['black']?.deck.pocket_pieces ?? []
)
const whitePocketGroups = computed(() => groupPocketPieces(whitePocket.value))
const blackPocketGroups = computed(() => groupPocketPieces(blackPocket.value))
const maxWhitePocketCount = computed(() => Math.max(1, ...whitePocketGroups.value.map(group => group.count)))
const maxBlackPocketCount = computed(() => Math.max(1, ...blackPocketGroups.value.map(group => group.count)))
const whiteDeck = computed(() => viewState.value.players['white']?.deck)
const blackDeck = computed(() => viewState.value.players['black']?.deck)
const isMyTurn = computed(() => !props.localPlayer || props.state.current_player === props.localPlayer)
const isBotTurn = computed(() => Boolean(
  props.botPlayer
  && props.state.current_player === props.botPlayer
  && props.state.phase === 'playing',
))
const canUsePlayerControls = computed(() => isMyTurn.value && !botThinking.value && !botReplaying.value && !promotionRequest.value)
const visibleSelectedPieceId = computed(() => (
  botReplaying.value ? botPreviewSelectedPieceId.value : selectedPieceId.value
))
const visibleMovableSquares = computed(() => (
  botReplaying.value ? botPreviewMovableSquares.value : movableSquares.value
))
const visibleAttackSquares = computed(() => (
  botReplaying.value ? botPreviewAttackSquares.value : attackSquares.value
))
const visibleDropSquares = computed(() => (
  botReplaying.value ? botPreviewDropSquares.value : dropSquares.value
))
const visibleAbilityMode = computed(() => !botReplaying.value && abilityMode.value)
const lastMove = computed(() => {
  const history = viewState.value.history
  const action = history[history.length - 1]?.action
  return action?.type === 'move' ? { from: action.from, to: action.to } : null
})
const boardOrientation = computed(() => props.localPlayer ?? viewState.value.current_player)
const selectedPiece = computed(() => (
  selectedPieceId.value ? props.state.pieces[selectedPieceId.value] ?? null : null
))
const selectedPieceDefinition = computed(() => (
  selectedPiece.value ? props.state.piece_definitions[selectedPiece.value.type_id] ?? null : null
))
const selectedPieceAbilities = computed<MoveOptionDefinition[]>(() => (
  selectedPieceDefinition.value?.move_options?.filter(option => option.kind === 'ability') ?? []
))
const selectedAbility = computed(() => (
  selectedPieceAbilities.value.find(ability => ability.id === activeAbilityId.value) ?? null
))
const selectedAbilityHelpText = computed(() => {
  const unavailable = selectedPieceAbilities.value
    .map(abilityUnavailableReason)
    .find(reason => reason.length > 0)
  return unavailable ?? ''
})

function abilityUnavailableReason(ability: MoveOptionDefinition): string {
  if (!selectedPiece.value) return '선택한 기물이 없습니다.'
  if (selectedPieceAbilities.value.length === 0) return '선택한 기물은 특수 능력이 없습니다.'
  if (selectedPiece.value.owner !== props.state.current_player) return '현재 턴의 기물이 아닙니다.'
  if (ability.execution_mode !== 'move_modifier') return '독립 실행 옵션은 아직 지원되지 않습니다.'
  const remaining = selectedPiece.value.move_option_cooldowns?.[ability.id]?.remaining ?? 0
  if (remaining > 0) {
    const clockLabel = ability.cooldown?.clock === 'global_turns'
      ? '전체 행동/턴'
      : '소유자 턴'
    return `${remaining}번의 ${clockLabel}을 마친 뒤 다시 사용할 수 있습니다.`
  }
  return ''
}
const botDifficultyLabel = computed(() => {
  const labels: Record<BotDifficulty, string> = {
    easy: 'Easy',
    normal: 'Normal',
    hard: 'Hard',
  }
  return labels[props.botDifficulty ?? 'normal']
})
const botStatusTitle = computed(() => {
  if (botThinking.value && !botReplaying.value) return '봇이 수를 계산하고 있습니다...'
  if (botReplaying.value) return '봇이 수를 두고 있습니다'
  if (botError.value) return '봇 턴 실행 실패'
  return '봇 대전'
})

function playerName(player: PlayerId): string {
  return player === 'white' ? 'White' : 'Black'
}

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
  const piece = props.state.pieces[action.piece_id]
  const pieceName = props.state.piece_definitions[piece?.type_id ?? '']?.name ?? action.piece_id
  if (action.type === 'drop') {
    return `${pieceName} 포켓 기물 놓기: ${action.to.file + 1}, ${action.to.rank + 1}`
  }

  const captureText = action.captured_piece_id ? ' 포획' : ' 이동'
  return `${pieceName}${captureText}: ${action.from.file + 1}, ${action.from.rank + 1} -> ${action.to.file + 1}, ${action.to.rank + 1}`
}

function removePieceFromBoard(state: GameState, pieceId: string) {
  for (const [id, occupant] of Object.entries(state.board.squares)) {
    if (occupant === pieceId) state.board.squares[id] = null
  }
}

function applyMoveForReplay(state: GameState, action: MoveAction): GameState {
  const next = cloneGameState(state)
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
      const promotedDefinition = next.piece_definitions[action.promotion]
      movedPiece.state = Object.fromEntries(
        (promotedDefinition?.state_schema ?? []).map(entry => [entry.key, entry.default_value]),
      )
      movedPiece.move_option_cooldowns = {}
    }
  }
  for (const update of action.effects.piece_state_updates) {
    const target = next.pieces[update.piece_id]
    if (target) target.state = { ...(target.state ?? {}), [update.key]: update.value }
  }
  for (const update of action.effects.global_state_updates) {
    next.global_state = { ...(next.global_state ?? {}), [update.key]: update.value }
  }
  for (const update of action.effects.cooldown_updates) {
    const target = next.pieces[update.piece_id]
    const option = target
      ? next.piece_definitions[target.type_id]?.move_options.find(option => option.id === update.move_option_id)
      : undefined
    if (target && option?.cooldown) {
      target.move_option_cooldowns = {
        ...(target.move_option_cooldowns ?? {}),
        [update.move_option_id]: { remaining: update.remaining },
      }
    }
  }


  const capturedTypeId = capturedPieceId ? next.pieces[capturedPieceId]?.type_id : undefined
  if (capturedTypeId && next.piece_definitions[capturedTypeId]?.is_king) {
    next.phase = 'ended'
    next.result = { winner: action.player_id, reason: 'king_capture' }
  }

  return next
}

function applyDropForReplay(state: GameState, action: DropAction): GameState {
  const next = cloneGameState(state)

  const player = next.players[action.player_id]
  if (player) {
    player.deck.pocket_pieces = player.deck.pocket_pieces.filter(id => id !== action.piece_id)
  }

  const piece = next.pieces[action.piece_id]
  if (action.captured_piece_id) {
    const captured = next.pieces[action.captured_piece_id]
    if (captured) {
      captured.captured = true
      captured.current_square = undefined
    }
    if (player) player.captured_pieces.push(action.captured_piece_id)
  }
  if (piece) {
    piece.in_pocket = false
    piece.current_square = action.to
  }
  next.board.squares[squareId(action.to)] = action.piece_id

  return next
}

function applyActionForReplay(state: GameState, action: AiAction): GameState {
  if (action.type === 'move') return applyMoveForReplay(state, action)
  return applyDropForReplay(state, action)
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

async function replayBotTurn(
  actions: AiAction[],
  finalState: GameState,
  runId: number,
  timeline?: ActionTimelineFrame[],
) {
  if (actions.length === 0) {
    emit('stateUpdate', finalState)
    return
  }

  botReplaying.value = true
  let nextReplayState = cloneGameState(props.state)
  for (let index = 0; index < actions.length; index++) {
    if (runId !== botRunSerial) return

    const action = actions[index]
    botReplayMessage.value = `${index + 1}/${actions.length} ${actionLabel(action)}`
    previewBotAction(action)
    await wait(BOT_ACTION_PREVIEW_MS)
    if (runId !== botRunSerial) return

    // New servers provide authoritative snapshots. The local action applier is
    // retained as a compatibility fallback for older responses.
    nextReplayState = timeline?.[index]
      ? applyTimelineFrame(nextReplayState, timeline[index])
      : applyActionForReplay(nextReplayState, action)
    botReplayState.value = nextReplayState
    clearBotPreview()
    await wait(BOT_ACTION_SETTLE_MS)
  }

  if (runId !== botRunSerial) return
  botReplayState.value = null
  emit('stateUpdate', finalState)
}

async function runBotTurn() {
  if (!props.botPlayer || !isBotTurn.value || botThinking.value) return

  const runId = ++botRunSerial
  botThinking.value = true
  botReplaying.value = false
  botReplayMessage.value = null
  botError.value = null
  clearSelection()
  clearBotReplay()
  try {
    const response = await api.botTurn(
      props.state.id,
      props.botPlayer,
      props.botDifficulty ?? 'normal',
    )
    if (runId !== botRunSerial) return
    lastBotStats.value = response.stats
    await replayBotTurn(response.actions, response.game_state, runId, response.timeline)
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e)
    if (message.includes('현재 턴 플레이어와 bot_player_id가 일치하지 않습니다.')) {
      try {
        const syncedState = await api.getGame(props.state.id)
        emit('stateUpdate', syncedState)
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
  () => [
    props.state.id,
    props.state.current_player,
    props.state.turn_number,
    props.state.phase,
    props.botPlayer,
    props.botDifficulty,
  ],
  () => {
    if (isBotTurn.value) void runBotTurn()
  },
  { immediate: true },
)

const PIECE_SYMBOLS: Record<string, string> = {
  king: '♔', queen: '♕', rook: '♖', bishop: '♗', knight: '♘',
  amazon: 'A', guhang: 'G', 'cannon-rook': 'C', 'tempest-queen': 'Q', 'tempest-rook': 'T', 'tempest-bishop': 'B', 'tempest-knight': 'N', 'bouncing-bishop': 'B', nightrider: 'N', windmill: 'W',
  'pawn-white': '♙', 'pawn-black': '♟', 'tempest-pawn-white': '♙', 'tempest-pawn-black': '♟', 'dozer-white': 'D', 'dozer-black': 'D',
}
function pieceSymbol(typeId: string): string {
  return PIECE_SYMBOLS[typeId] ?? '?'
}

function pieceImage(pieceId: string): string | undefined {
  const piece = viewState.value.pieces[pieceId]
  return piece
    ? renderedPieceAsset(piece, viewState.value.piece_definitions[piece.type_id])
    : undefined
}

function pieceAlt(pieceId: string): string {
  const piece = viewState.value.pieces[pieceId]
  return piece ? `${piece.owner} ${piece.type_id}` : pieceId
}

function groupPocketPieces(pieceIds: string[]): PocketGroup[] {
  const groups = new Map<string, PocketGroup>()

  for (const pieceId of pieceIds) {
    const piece = viewState.value.pieces[pieceId]
    if (!piece) continue

    const existing = groups.get(piece.type_id)
    if (existing) {
      existing.pieceIds.push(pieceId)
      existing.count += 1
      continue
    }

    groups.set(piece.type_id, {
      typeId: piece.type_id,
      name: viewState.value.piece_definitions[piece.type_id]?.name ?? piece.type_id,
      representativeId: pieceId,
      pieceIds: [pieceId],
      count: 1,
    })
  }

  return Array.from(groups.values())
}

function pocketGroupFillWidth(count: number, maxCount: number): string {
  return `${Math.round((count / Math.max(1, maxCount)) * 100)}%`
}

const PROMOTION_ORDER = [
  'queen',
  'rook',
  'bishop',
  'knight',
  'tempest-queen',
  'tempest-rook',
  'tempest-bishop',
  'tempest-knight',
]

function promotionPieceLabel(pieceType: string): string {
  return viewState.value.piece_definitions[pieceType]?.name ?? pieceType
}

function requestPromotionChoice(
  pieceId: string,
  to: Square,
  owner: PlayerId,
  choices: string[],
): Promise<string | null> {
  cancelPromotion()
  const options = [...choices].sort(
    (a, b) => PROMOTION_ORDER.indexOf(a) - PROMOTION_ORDER.indexOf(b),
  )
  promotionRequest.value = { pieceId, to, owner, options }
  return new Promise(resolve => {
    promotionResolve = resolve
  })
}

function choosePromotion(pieceType: string) {
  const resolve = promotionResolve
  promotionRequest.value = null
  promotionResolve = null
  resolve?.(pieceType)
}

function cancelPromotion() {
  const resolve = promotionResolve
  promotionRequest.value = null
  promotionResolve = null
  resolve?.(null)
}

function clearSelection() {
  selectedPieceId.value = null
  selectedPocketPieceId.value = null
  draggedPocketPieceId.value = null
  abilityMode.value = false
  activeAbilityId.value = null
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []
}

function actionCacheKey(pieceId?: string, abilityId?: string | null): string {
  return [
    props.state.id,
    props.state.current_player,
    props.state.turn_number,
    pieceId ?? '',
    abilityId ?? '',
  ].join(':')
}

function sameSquare(left: Square, right: Square): boolean {
  return left.file === right.file && left.rank === right.rank
}

function isLegalSquare(square: Square, legalSquares: Square[]): boolean {
  return legalSquares.some(target => sameSquare(target, square))
}

async function loadPieceOptions(pieceId: string, abilityId: string | null = null): Promise<LegalPieceOptions> {
  const key = actionCacheKey(pieceId, abilityId)
  const cached = pieceOptionsCache.get(key)
  if (cached) return cached

  const pending = pieceOptionsRequests.get(key)
  if (pending) return pending

  const request = api.getPieceOptions(props.state.id, pieceId, abilityId).then(({ moves }) => {
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

async function selectBoardPiece(pieceId: string): Promise<LegalPieceOptions | null> {
  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player) {
    clearSelection()
    return null
  }

  selectedPieceId.value = pieceId
  selectedPocketPieceId.value = null
  abilityMode.value = false
  activeAbilityId.value = null
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []

  try {
    const options = await loadPieceOptions(pieceId)
    if (selectedPieceId.value !== pieceId || abilityMode.value) return options

    legalTargetSquares.value = options.legalTargets
    movableSquares.value = options.movable
    attackSquares.value = options.captures
    return options
  } catch {
    if (selectedPieceId.value === pieceId) {
      legalTargetSquares.value = []
      movableSquares.value = []
      attackSquares.value = []
    }
    return null
  }
}

async function toggleAbilityMode(abilityId: string) {
  const ability = selectedPieceAbilities.value.find(ability => ability.id === abilityId)
  if (!selectedPieceId.value || !ability || abilityUnavailableReason(ability)) return

  if (abilityMode.value && activeAbilityId.value === abilityId) {
    const pieceId = selectedPieceId.value
    abilityMode.value = false
    activeAbilityId.value = null
    const options = await loadPieceOptions(pieceId)
    if (!abilityMode.value && selectedPieceId.value === pieceId) {
      legalTargetSquares.value = options.legalTargets
      movableSquares.value = options.movable
      attackSquares.value = options.captures
    }
    return
  }

  abilityMode.value = true
  activeAbilityId.value = abilityId
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []

  const pieceId = selectedPieceId.value
  const options = await loadPieceOptions(pieceId, abilityId)
  if (selectedPieceId.value === pieceId && abilityMode.value && activeAbilityId.value === abilityId) {
    legalTargetSquares.value = options.legalTargets
    movableSquares.value = options.movable
    attackSquares.value = options.captures
  }
}

async function loadDropOptions(): Promise<DropAction[]> {
  const key = actionCacheKey('drops')
  const cached = dropOptionsCache.get(key)
  if (cached) return cached

  const pending = dropOptionsRequests.get(key)
  if (pending) return pending

  const request = api.getLegalDrops(props.state.id).then(({ drops }) => {
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

async function selectPocketPiece(pieceId: string): Promise<Square[]> {
  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player) {
    clearSelection()
    return []
  }

  selectedPieceId.value = null
  selectedPocketPieceId.value = pieceId
  abilityMode.value = false
  activeAbilityId.value = null
  legalTargetSquares.value = []
  movableSquares.value = []
  attackSquares.value = []
  dropSquares.value = []

  try {
    const drops = await loadDropOptions()
    const targets = drops.filter(drop => drop.piece_id === pieceId).map(drop => drop.to)
    if (selectedPocketPieceId.value === pieceId) {
      dropSquares.value = targets
    }
    return targets
  } catch {
    if (selectedPocketPieceId.value === pieceId) {
      dropSquares.value = []
    }
    return []
  }
}

async function submitMove(pieceId: string, to: Square) {
  const fromPiece = props.state.pieces[pieceId]
  if (!fromPiece?.current_square || sameSquare(fromPiece.current_square, to)) {
    clearSelection()
    return
  }

  const moveAbilityId = selectedPieceId.value === pieceId && abilityMode.value
    ? activeAbilityId.value
    : null
  const options = selectedPieceId.value === pieceId && legalTargetSquares.value.length > 0
    ? await loadPieceOptions(pieceId, moveAbilityId)
    : await selectBoardPiece(pieceId)
  if (!options || !isLegalSquare(to, options.legalTargets)) {
    clearSelection()
    return
  }

  const promotionChoices = options.moves
    .filter(move => move.piece_id === pieceId && sameSquare(move.to, to) && move.promotion)
    .map(move => move.promotion as string)

  let promotion: string | undefined
  if (promotionChoices.length > 0) {
    const chosen = await requestPromotionChoice(pieceId, to, fromPiece.owner, promotionChoices)
    if (!chosen) {
      clearSelection()
      return
    }
    promotion = chosen
  }

  try {
    const action: SubmitMoveAction = {
      type: 'move',
      piece_id: pieceId,
      to,
      promotion,
      move_option_id: moveAbilityId ?? undefined,
    }
    const newState = await api.submitAction(props.state.id, action)
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    clearSelection()
  }
}

async function submitDrop(pieceId: string, to: Square) {
  const targets = selectedPocketPieceId.value === pieceId && dropSquares.value.length > 0
    ? dropSquares.value
    : await selectPocketPiece(pieceId)
  if (!isLegalSquare(to, targets)) {
    clearSelection()
    return
  }

  try {
    const action: SubmitDropAction = {
      type: 'drop',
      piece_id: pieceId,
      to,
    }
    const newState = await api.submitAction(props.state.id, action)
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    clearSelection()
  }
}

async function onSquareClick(sq: Square) {
  error.value = null
  if (promotionRequest.value) return
  if (!canUsePlayerControls.value) {
    error.value = '상대 턴입니다.'
    clearSelection()
    return
  }

  const currentPlayer = props.state.current_player
  const sqId = `${sq.file}_${sq.rank}`
  const pieceId = props.state.board.squares[sqId] ?? null
  const piece = pieceId ? props.state.pieces[pieceId] : null

  // ── Drop mode: selected pocket piece → drop on target ──
  if (selectedPocketPieceId.value) {
    await submitDrop(selectedPocketPieceId.value, sq)
    return
  }

  // ── Move mode: selected piece → move to target ──
  if (selectedPieceId.value) {
    if (pieceId && piece && piece.owner === currentPlayer && pieceId !== selectedPieceId.value) {
      await selectBoardPiece(pieceId)
      return
    }
    await submitMove(selectedPieceId.value, sq)
    return
  }

  // ── Select own piece ──
  if (pieceId && piece && piece.owner === currentPlayer) {
    await selectBoardPiece(pieceId)
  } else {
    clearSelection()
  }
}

async function onPocketClick(pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value) {
    error.value = '상대 턴입니다.'
    clearSelection()
    return
  }

  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player) return

  await selectPocketPiece(pieceId)
}

function onBoardPieceDragStart(pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value) {
    clearSelection()
    return
  }

  void selectBoardPiece(pieceId)
}

async function onSquareDrop(sq: Square | null, pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value || !sq) {
    clearSelection()
    return
  }

  const piece = props.state.pieces[pieceId]
  if (!piece) {
    clearSelection()
    return
  }

  if (piece.in_pocket || draggedPocketPieceId.value === pieceId) {
    await submitDrop(pieceId, sq)
  } else {
    await submitMove(pieceId, sq)
  }
}

function onPocketDragStart(event: DragEvent, pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value) {
    event.preventDefault()
    clearSelection()
    return
  }

  draggedPocketPieceId.value = pieceId
  event.dataTransfer?.setData('application/x-brainfuck-chess-pocket-piece', pieceId)
  event.dataTransfer?.setData('text/plain', pieceId)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
  void selectPocketPiece(pieceId)
}

function onPocketDragEnd() {
  draggedPocketPieceId.value = null
}

async function onResign() {
  error.value = null
  if (props.state.phase === 'ended') return

  const resigningPlayer = props.localPlayer ?? props.state.current_player
  if (!window.confirm('정말 기권하시겠습니까?')) return

  try {
    const newState = props.roomId
      ? await api.resignRoom(props.roomId, resigningPlayer)
      : await api.resignGame(props.state.id, resigningPlayer)
    clearSelection()
    emit('stateUpdate', newState)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<style scoped>
.game-screen { display: flex; flex-direction: column; gap: 12px; padding: 16px; position: relative; }

.header { display: flex; align-items: center; gap: 16px; }
.title-en { font-size: 0.55em; font-weight: 400; opacity: 0.65; margin-left: 6px; }
.player-badge { padding: 4px 10px; border-radius: 6px; font-weight: bold; }
.player-badge.player-white { background: #eee; color: #333; }
.player-badge.player-black { background: #333; color: #eee; }
.turn-badge, .mode-badge { padding: 4px 8px; background: #ddd; color: #1f2933; border-radius: 6px; }
.local-badge {
  padding: 4px 8px;
  background: #e8f5e9;
  color: #256029;
  border-radius: 6px;
  font-weight: 700;
}
.local-badge.waiting {
  background: #fff3cd;
  color: #7a5a00;
}
.bot-badge {
  padding: 4px 8px;
  background: #342a18;
  color: #f4dfb0;
  border: 1px solid rgba(217, 164, 65, 0.38);
  border-radius: 6px;
  font-weight: 700;
}

.bot-status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 16px;
  border: 1px solid rgba(217, 164, 65, 0.28);
  border-radius: 10px;
  background: rgba(217, 164, 65, 0.08);
  color: #f4dfb0;
}
.bot-status > div { display: flex; flex-direction: column; gap: 3px; }
.bot-status small { color: #a8b1c2; }
.bot-status.thinking { animation: bot-pulse 1.3s ease-in-out infinite alternate; }
.bot-status.failed { border-color: rgba(255, 125, 125, 0.55); }
.bot-status button {
  padding: 7px 12px;
  border: none;
  border-radius: 6px;
  background: #d9a441;
  color: #221a0d;
  cursor: pointer;
  font-weight: 700;
}
@keyframes bot-pulse {
  from { background: rgba(217, 164, 65, 0.06); }
  to { background: rgba(217, 164, 65, 0.16); }
}

.main-layout { display: flex; gap: 16px; align-items: flex-start; justify-content: center; }
.main-layout.locked { pointer-events: none; opacity: 0.78; }

.board-column {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.selected-piece-panel {
  width: min(80vw, 80vh);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid rgba(82, 96, 109, 0.28);
  border-radius: 8px;
  background: #f8fafc;
  color: #1f2933;
}

.selected-piece-panel.active {
  border-color: rgba(20, 184, 166, 0.72);
  background: #ecfdf9;
}

.selected-piece-panel > div:first-child {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.selected-piece-panel small {
  color: #52606d;
}

.ability-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.ability-button {
  min-height: 42px;
  padding: 9px 14px;
  border: 1px solid #0f766e;
  border-radius: 8px;
  background: #ffffff;
  color: #0f766e;
  cursor: pointer;
  font-weight: 800;
}

.ability-button.active {
  background: #0f766e;
  color: white;
}

.ability-button:disabled {
  opacity: 0.48;
  cursor: not-allowed;
}

.ability-help {
  flex-basis: 100%;
  text-align: right;
}

.pocket { width: 170px; min-width: 150px; display: flex; flex-direction: column; gap: 8px; }
.pocket h4 { margin: 0; font-size: 14px; }
.pocket-pieces { display: grid; gap: 6px; }
.pocket-piece-row {
  min-height: 48px;
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) 24px;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border: 2px solid #bbb;
  border-radius: 6px;
  cursor: pointer;
  background: #f9f9f9;
  color: #1f2933;
  user-select: none;
}
.pocket-piece-row[draggable="true"] { cursor: grab; }
.pocket-piece-row[draggable="true"]:active { cursor: grabbing; }
.pocket-piece-row.selected { border-color: #4a8fff; background: #e0eeff; }
.pocket-piece-image {
  display: block;
  width: 30px;
  height: 30px;
  object-fit: contain;
}
.pocket-piece-symbol {
  font-size: 22px;
  text-align: center;
}
.pocket-piece-meta {
  min-width: 0;
  display: grid;
  gap: 5px;
}
.pocket-piece-meta strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  line-height: 1;
}
.pocket-count-bar {
  height: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(31, 41, 51, 0.18);
}
.pocket-count-bar > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: #4a8fff;
}
.pocket-piece-count {
  color: #1f2933;
  font-size: 12px;
  font-weight: 800;
  text-align: right;
}
.score-info { font-size: 12px; color: #666; }

.footer { display: flex; align-items: center; gap: 16px; }
.btn { padding: 8px 16px; border-radius: 6px; border: none; cursor: pointer; font-size: 14px; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.btn-resign { background: #c62828; color: white; }
.btn-resign:hover:not(:disabled) { background: #a61f1f; }

.error-banner {
  position: fixed; bottom: 16px; left: 50%; transform: translateX(-50%);
  background: #c62828; color: white; padding: 10px 20px; border-radius: 8px;
  font-size: 14px; z-index: 100;
}

.game-over-overlay {
  position: absolute; inset: 0; background: rgba(0,0,0,0.5);
  display: flex; align-items: center; justify-content: center; z-index: 50;
}
.game-over-box {
  background: white; padding: 32px 48px; border-radius: 12px; text-align: center;
  color: #1f2933;
}
.game-over-box button { margin-top: 16px; padding: 10px 24px; background: #1976d2; color: white; border: none; border-radius: 6px; cursor: pointer; font-size: 16px; }

.promotion-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.55);
  display: flex; align-items: center; justify-content: center; z-index: 60;
}
.promotion-box {
  background: white; padding: 24px 32px; border-radius: 12px; text-align: center;
  color: #1f2933; max-width: 320px;
}
.promotion-box h3 { margin: 0 0 4px; }
.promotion-box p { margin: 0 0 16px; color: #52606d; font-size: 14px; }
.promotion-choices { display: flex; gap: 12px; justify-content: center; }
.promotion-choice {
  display: flex; flex-direction: column; align-items: center; gap: 6px;
  background: #f0f4f8; border: 2px solid transparent; border-radius: 10px;
  padding: 10px 12px; cursor: pointer; font-size: 13px; color: #1f2933;
}
.promotion-choice:hover, .promotion-choice:focus-visible {
  border-color: #1976d2; background: #e3f2fd;
}
.promotion-choice-image { width: 48px; height: 48px; }

@media (max-width: 900px) {
  .game-screen { padding: 12px; }
  .header,
  .footer {
    flex-wrap: wrap;
  }
  .turn-info {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .main-layout {
    flex-wrap: wrap;
    align-items: stretch;
  }
  .pocket {
    order: 2;
    width: min(320px, 100%);
    flex: 1 1 220px;
  }
  .board-column,
  .selected-piece-panel {
    width: 100%;
  }
  .selected-piece-panel {
    align-items: stretch;
    flex-direction: column;
  }
  .ability-actions {
    justify-content: stretch;
  }
  .ability-button {
    flex: 1 1 160px;
  }
  .ability-help {
    text-align: left;
  }
}
</style>
