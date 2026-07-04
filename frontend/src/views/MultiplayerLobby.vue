<template>
  <main class="lobby">
    <div class="page-bar">
      <button class="btn-secondary" @click="$emit('back')">로비로</button>
      <div>
        <p class="eyebrow">Multiplayer</p>
        <h1>멀티플레이</h1>
      </div>
      <button class="btn-secondary" :disabled="!currentRoom" @click="refreshRoom">새로고침</button>
    </div>

    <section v-if="decks.length === 0" class="card empty-state">
      <h2>먼저 덱을 만들어 주세요.</h2>
      <p>멀티플레이는 방에 들어가기 전에 저장된 덱 하나를 선택해야 합니다.</p>
      <button class="btn-start" @click="$emit('deck-building')">덱 빌딩으로 이동</button>
    </section>

    <template v-else>
      <section class="card multiplayer-panel">
        <div class="room-grid">
          <div class="room-actions">
            <label>
              <span class="limit-label">사용할 덱</span>
              <select v-model="selectedDeckId" class="text-input" :disabled="Boolean(currentRoom?.game_id)">
                <option value="">선택 안 함</option>
                <option v-for="deck in validDecks" :key="deck.id" :value="deck.id">
                  {{ deck.name }} · {{ deck.boardSize }}x{{ deck.boardSize }}
                </option>
              </select>
            </label>
            <div class="color-match">
              <span class="limit-label">색상 매칭</span>
              <label><input v-model="hostSideMode" type="radio" value="white" :disabled="Boolean(currentRoom)" /> White</label>
              <label><input v-model="hostSideMode" type="radio" value="black" :disabled="Boolean(currentRoom)" /> Black</label>
              <label><input v-model="hostSideMode" type="radio" value="random" :disabled="Boolean(currentRoom)" /> 랜덤</label>
            </div>
            <div class="room-code-row">
              <input v-model.trim="roomCodeInput" class="room-code-input" maxlength="6" placeholder="입장할 방 번호" />
            </div>
            <div class="room-buttons">
              <button class="btn-secondary" :disabled="!selectedDeck" @click="createRoom">방 만들기</button>
              <button class="btn-secondary" :disabled="!selectedDeck || !roomCodeInput.trim()" @click="joinRoom">입장하고 시작</button>
              <button class="btn-secondary" :disabled="!currentRoom || !selectedDeck || Boolean(currentRoom.game_id)" @click="applySelectedDeckToRoom">
                선택 덱 적용
              </button>
              <button class="btn-secondary" :disabled="!currentRoom || Boolean(currentRoom.game_id)" @click="readyRoom">
                준비
              </button>
            </div>
          </div>

          <div class="room-state">
            <span class="limit-label">현재 방</span>
            <strong>{{ currentRoom ? currentRoom.id : '없음' }}</strong>
            <p v-if="selectedDeck">선택 덱: {{ selectedDeck.name }} · {{ selectedDeck.boardSize }} x {{ selectedDeck.boardSize }}</p>
            <p v-if="currentRoom">
              방 보드: {{ currentRoom.board_size }} x {{ currentRoom.board_size }} · 방장 {{ playerLabel(currentRoom.host_side) }}
            </p>
            <p v-if="currentRoom">
              준비 상태: Host {{ currentRoom.host_ready ? 'Ready' : 'Not Ready' }} · Guest {{ currentRoom.guest_ready ? 'Ready' : 'Not Ready' }}
            </p>
            <p v-else>방 생성 시 선택한 덱의 보드 크기가 방 보드 크기로 사용됩니다.</p>
          </div>
        </div>
      </section>

      <p v-if="status" class="room-status">{{ status }}</p>
      <p v-if="error" class="error">{{ error }}</p>
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { GameState } from '../types/game'
import type { LobbyPlayer, SavedDeck } from '../types/deck'
import { api, type MultiplayerRoom } from '../api/gameApi'
import { useSavedDecks } from '../composables/useSavedDecks'
import { savedDeckToPlayerDeckRequest } from '../composables/useDeckSerialization'
import { validateSavedDeck } from '../composables/useDeckValidation'

const emit = defineEmits<{
  back: []
  'deck-building': []
  'game-started': [payload: { state: GameState; room: MultiplayerRoom; localPlayer: LobbyPlayer }]
}>()

const savedDecks = useSavedDecks()
const decks = ref<SavedDeck[]>([])
const selectedDeckId = ref('')
const hostSideMode = ref<LobbyPlayer | 'random'>('random')
const roomCodeInput = ref('')
const currentRoom = ref<MultiplayerRoom | null>(null)
const status = ref<string | null>(null)
const error = ref<string | null>(null)
const pollTimer = ref<number | null>(null)
const localPlayer = ref<LobbyPlayer | null>(null)

const validDecks = computed(() => decks.value.filter(deck => validateSavedDeck(deck).valid))
const selectedDeck = computed(() => validDecks.value.find(deck => deck.id === selectedDeckId.value) ?? null)

function playerLabel(player: LobbyPlayer): string {
  return player === 'white' ? 'White' : 'Black'
}

function randomSide(): LobbyPlayer {
  return Math.random() < 0.5 ? 'white' : 'black'
}

function refreshDecks() {
  decks.value = savedDecks.loadDecks()
  selectedDeckId.value = validDecks.value[0]?.id ?? ''
}

function stopPolling() {
  if (pollTimer.value !== null) {
    window.clearInterval(pollTimer.value)
    pollTimer.value = null
  }
}

function startPolling(roomId: string) {
  stopPolling()
  pollTimer.value = window.setInterval(async () => {
    try {
      const room = await api.getRoom(roomId)
      currentRoom.value = room
      if (!room.game_id || !localPlayer.value) return
      const state = await api.getGame(room.game_id)
      stopPolling()
      emit('game-started', { state, room, localPlayer: localPlayer.value })
    } catch {
      // Keep waiting through transient failures.
    }
  }, 1200)
}

async function createRoom() {
  if (!selectedDeck.value) return
  error.value = null
  status.value = null
  try {
    const hostSide = hostSideMode.value === 'random' ? randomSide() : hostSideMode.value
    const room = await api.createRoom(
      selectedDeck.value.boardSize,
      hostSide,
      savedDeckToPlayerDeckRequest(selectedDeck.value),
    )
    currentRoom.value = room
    localPlayer.value = hostSide
    roomCodeInput.value = room.id
    status.value = `방 ${room.id} 생성 완료. 내 색상은 ${playerLabel(hostSide)}입니다.`
    startPolling(room.id)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function refreshRoom() {
  if (!currentRoom.value) return
  error.value = null
  try {
    const room = await api.getRoom(currentRoom.value.id)
    currentRoom.value = room
    if (room.game_id && localPlayer.value) {
      const state = await api.getGame(room.game_id)
      stopPolling()
      emit('game-started', { state, room, localPlayer: localPlayer.value })
    }
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function applySelectedDeckToRoom() {
  if (!selectedDeck.value || !currentRoom.value) return
  error.value = null
  try {
    if (selectedDeck.value.boardSize !== currentRoom.value.board_size) {
      error.value = '방의 보드 크기와 선택한 덱의 보드 크기가 다릅니다.'
      return
    }
    currentRoom.value = await api.selectRoomDeck(
      currentRoom.value.id,
      savedDeckToPlayerDeckRequest(selectedDeck.value),
    )
    status.value = '선택 덱을 방에 적용했습니다. 다시 준비를 눌러 주세요.'
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function readyRoom() {
  if (!currentRoom.value) return
  error.value = null
  try {
    const room = await api.readyRoom(currentRoom.value.id)
    currentRoom.value = room
    status.value = room.game_id ? '양쪽 준비가 완료되어 게임이 시작됩니다.' : '준비 상태로 변경했습니다.'
    if (room.game_id && localPlayer.value) {
      const state = await api.getGame(room.game_id)
      stopPolling()
      emit('game-started', { state, room, localPlayer: localPlayer.value })
    }
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function joinRoom() {
  if (!selectedDeck.value) return
  error.value = null
  status.value = null
  try {
    const room = await api.getRoom(roomCodeInput.value.toUpperCase())
    if (room.board_size !== selectedDeck.value.boardSize) {
      error.value = '방의 보드 크기와 선택한 덱의 보드 크기가 다릅니다.'
      return
    }
    const { state } = await api.joinRoom(room.id, savedDeckToPlayerDeckRequest(selectedDeck.value))
    const joinedRoom = { ...room, game_id: state.id }
    currentRoom.value = joinedRoom
    localPlayer.value = room.guest_side
    stopPolling()
    emit('game-started', { state, room: joinedRoom, localPlayer: room.guest_side })
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

onMounted(refreshDecks)
onUnmounted(stopPolling)
</script>
