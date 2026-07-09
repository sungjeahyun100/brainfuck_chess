<template>
  <div class="game-screen">
    <GameHeader
      :state="viewState"
      :local-player="localPlayer"
      :bot-player="botPlayer"
      :bot-difficulty="botDifficulty"
      :is-my-turn="isMyTurn"
      :is-bot-turn="isBotTurn"
    />

    <BotStatus
      v-if="botPlayer"
      :bot-player="botPlayer"
      :bot-difficulty-label="botDifficultyLabel"
      :bot-thinking="botThinking"
      :bot-replaying="botReplaying"
      :bot-error="botError"
      :bot-replay-message="botReplayMessage"
      :last-bot-stats="lastBotStats"
      @retry="runBotTurn"
    />

    <PromotionDialog
      v-if="promotionRequest"
      :request="promotionRequest"
      :definitions="viewState.piece_definitions"
      @choose="choosePromotion"
      @cancel="cancelPromotion"
    />

    <GameOverDialog
      v-if="viewState.phase === 'ended'"
      :state="viewState"
      @restart="$emit('restart')"
    />

    <div class="main-layout" :class="{ locked: botThinking || botReplaying || isBotTurn }">
      <PocketPanel
        player="white"
        :groups="whitePocketGroups"
        :deck="whiteDeck"
        :selected-pocket-piece-id="selectedPocketPieceId"
        :max-count="maxWhitePocketCount"
        @piece-click="onPocketClick"
        @piece-drag-start="onPocketDragStart"
        @piece-drag-end="onPocketDragEnd"
      />

      <!-- Center: Board -->
      <div class="board-column">
        <Board
          :board="viewState.board"
          :pieces="viewState.pieces"
          :selected-piece-id="visibleSelectedPieceId"
          :movable-squares="visibleMovableSquares"
          :attack-squares="visibleAttackSquares"
          :drop-squares="visibleDropSquares"
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

      <PocketPanel
        player="black"
        :groups="blackPocketGroups"
        :deck="blackDeck"
        :selected-pocket-piece-id="selectedPocketPieceId"
        :max-count="maxBlackPocketCount"
        @piece-click="onPocketClick"
        @piece-drag-start="onPocketDragStart"
        @piece-drag-end="onPocketDragEnd"
      />
    </div>

    <!-- Footer: actions -->
    <div class="footer">
      <button
        class="btn btn-resign"
        :disabled="botThinking || botReplaying || viewState.phase === 'ended' || (Boolean(roomId) && !localPlayer)"
        @click="resign"
      >
        기권
      </button>
    </div>

    <div v-if="error || botError" class="error-banner">{{ error || botError }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type {
  BotDifficulty,
  GameState,
  PieceAbilityDefinition,
  PlayerId,
  Square,
} from '../types/game'
import type { PocketGroup, PromotionRequest } from '../domain/game'
import { useBotReplay } from '../composables/useBotReplay'
import { useGameActions } from '../composables/useGameActions'
import { useLegalOptions, type LegalPieceOptions } from '../composables/useLegalOptions'
import { useSelection } from '../composables/useSelection'
import { pieceLabel } from '../display/pieceDisplay'
import Board from './Board.vue'
import BotStatus from './game/BotStatus.vue'
import GameHeader from './game/GameHeader.vue'
import GameOverDialog from './game/GameOverDialog.vue'
import PocketPanel from './game/PocketPanel.vue'
import PromotionDialog from './game/PromotionDialog.vue'

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

const {
  selectedPieceId,
  selectedPocketPieceId,
  abilityMode,
  activeAbilityId,
  legalTargetSquares,
  movableSquares,
  attackSquares,
  dropSquares,
  clearSelection,
  clearTargets,
} = useSelection()
const error = ref<string | null>(null)
const draggedPocketPieceId = ref<string | null>(null)
const promotionRequest = ref<PromotionRequest | null>(null)
let promotionResolve: ((choice: string | null) => void) | null = null

const isMyTurn = computed(() => !props.localPlayer || props.state.current_player === props.localPlayer)
const isBotTurn = computed(() => Boolean(
  props.botPlayer
  && props.state.current_player === props.botPlayer
  && props.state.phase === 'playing',
))
const {
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
} = useBotReplay({
  getState: () => props.state,
  getBotPlayer: () => props.botPlayer,
  getBotDifficulty: () => props.botDifficulty,
  isBotTurn: () => isBotTurn.value,
  selectedPieceId,
  movableSquares,
  attackSquares,
  dropSquares,
  abilityMode,
  clearSelection: clearGameSelection,
  emitStateUpdate: state => emit('stateUpdate', state),
})
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
const canUsePlayerControls = computed(() => isMyTurn.value && !botThinking.value && !botReplaying.value && !promotionRequest.value)
const boardOrientation = computed(() => props.localPlayer ?? viewState.value.current_player)
const selectedPiece = computed(() => (
  selectedPieceId.value ? props.state.pieces[selectedPieceId.value] ?? null : null
))
const selectedPieceDefinition = computed(() => (
  selectedPiece.value ? props.state.piece_definitions[selectedPiece.value.type_id] ?? null : null
))
const selectedPieceAbilities = computed<PieceAbilityDefinition[]>(() => (
  selectedPieceDefinition.value?.abilities ?? []
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
const {
  loadPieceOptions,
  loadDropOptions,
  isLegalSquare,
} = useLegalOptions(() => props.state)
const {
  submitMove,
  submitDrop,
  resign,
} = useGameActions({
  getState: () => props.state,
  getRoomId: () => props.roomId,
  getLocalPlayer: () => props.localPlayer,
  selectedPieceId,
  selectedPocketPieceId,
  abilityMode,
  activeAbilityId,
  legalTargetSquares,
  dropSquares,
  loadPieceOptions,
  loadDropOptions,
  selectBoardPiece,
  selectPocketPiece,
  isLegalSquare,
  requestPromotionChoice,
  clearSelection: clearGameSelection,
  setError: message => {
    error.value = message
  },
  emitStateUpdate: state => emit('stateUpdate', state),
  confirmResign: () => window.confirm('정말 기권하시겠습니까?'),
})

function abilityUnavailableReason(ability: PieceAbilityDefinition): string {
  if (!selectedPiece.value) return '선택한 기물이 없습니다.'
  if (selectedPieceAbilities.value.length === 0) return '선택한 기물은 특수 능력이 없습니다.'
  if (selectedPiece.value.owner !== props.state.current_player) return '현재 턴의 기물이 아닙니다.'
  if (props.state.turn_state.mode === 'drop') return '착수 턴에는 사용할 수 없습니다.'
  if (props.state.turn_state.actions.length > 0) return '이번 턴에는 사용할 수 없습니다.'
  const usableTurn = selectedPiece.value.ability_cooldowns?.[ability.id]
  if (usableTurn && usableTurn > props.state.turn_number) {
    return `${usableTurn - props.state.turn_number}턴 후 다시 사용할 수 있습니다.`
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
      name: pieceLabel(piece.type_id, viewState.value.piece_definitions),
      representativeId: pieceId,
      pieceIds: [pieceId],
      count: 1,
    })
  }

  return Array.from(groups.values())
}

const PROMOTION_ORDER = [
  'queen',
  'rook',
  'bishop',
  'knight',
  'tempest-queen',
  'tempest-rook',
  'bouncing-bishop',
  'tempest-knight',
]

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

function clearGameSelection() {
  draggedPocketPieceId.value = null
  clearSelection()
}

async function selectBoardPiece(pieceId: string): Promise<LegalPieceOptions | null> {
  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player || props.state.turn_state.actions.length > 0) {
    clearGameSelection()
    return null
  }

  selectedPieceId.value = pieceId
  selectedPocketPieceId.value = null
  abilityMode.value = false
  activeAbilityId.value = null
  clearTargets()

  try {
    const options = await loadPieceOptions(pieceId)
    if (selectedPieceId.value !== pieceId || abilityMode.value) return options

    legalTargetSquares.value = options.legalTargets
    movableSquares.value = options.movable
    attackSquares.value = options.captures
    return options
  } catch {
    if (selectedPieceId.value === pieceId) {
      clearTargets()
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
  clearTargets()

  const pieceId = selectedPieceId.value
  const options = await loadPieceOptions(pieceId, abilityId)
  if (selectedPieceId.value === pieceId && abilityMode.value && activeAbilityId.value === abilityId) {
    legalTargetSquares.value = options.legalTargets
    movableSquares.value = options.movable
    attackSquares.value = options.captures
  }
}

async function selectPocketPiece(pieceId: string): Promise<Square[]> {
  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player || props.state.turn_state.mode === 'move' || props.state.turn_state.actions.length > 0) {
    clearGameSelection()
    return []
  }

  selectedPieceId.value = null
  selectedPocketPieceId.value = pieceId
  abilityMode.value = false
  activeAbilityId.value = null
  clearTargets()

  try {
    const drops = await loadDropOptions()
    const targets = drops.filter(drop => drop.piece_id === pieceId).map(drop => drop.to)
    if (selectedPocketPieceId.value === pieceId) {
      dropSquares.value = targets
    }
    return targets
  } catch {
    if (selectedPocketPieceId.value === pieceId) {
      clearTargets()
    }
    return []
  }
}

async function onSquareClick(sq: Square) {
  error.value = null
  if (promotionRequest.value) return
  if (!canUsePlayerControls.value) {
    error.value = '상대 턴입니다.'
    clearGameSelection()
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
  if (pieceId && piece && piece.owner === currentPlayer && props.state.turn_state.actions.length === 0) {
    await selectBoardPiece(pieceId)
  } else {
    clearGameSelection()
  }
}

async function onPocketClick(pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value) {
    error.value = '상대 턴입니다.'
    clearGameSelection()
    return
  }

  const piece = props.state.pieces[pieceId]
  if (!piece || piece.owner !== props.state.current_player) return
  if (props.state.turn_state.mode === 'move' || props.state.turn_state.actions.length > 0) return

  await selectPocketPiece(pieceId)
}

function onBoardPieceDragStart(pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value) {
    clearGameSelection()
    return
  }

  void selectBoardPiece(pieceId)
}

async function onSquareDrop(sq: Square | null, pieceId: string) {
  error.value = null
  if (!canUsePlayerControls.value || !sq) {
    clearGameSelection()
    return
  }

  const piece = props.state.pieces[pieceId]
  if (!piece) {
    clearGameSelection()
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
  if (!canUsePlayerControls.value || props.state.turn_state.mode === 'move' || props.state.turn_state.actions.length > 0) {
    event.preventDefault()
    clearGameSelection()
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

</script>

<style scoped>
.game-screen { display: flex; flex-direction: column; gap: 12px; padding: 16px; position: relative; }

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

@media (max-width: 900px) {
  .game-screen { padding: 12px; }
  .footer {
    flex-wrap: wrap;
  }
  .main-layout {
    flex-wrap: wrap;
    align-items: stretch;
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
