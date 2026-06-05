<template>
  <div class="app">
    <!-- Lobby: choose board size -->
    <div v-if="!gameState" class="lobby">
      <h1>Brainfuck Chess</h1>
      <p>가장 어려운 변형 체스</p>
      <div class="board-select">
        <label>보드 크기</label>
        <select v-model="selectedSize">
          <option v-for="n in [8, 9, 10, 11, 12]" :key="n" :value="n">
            {{ n }} × {{ n }} (최대 {{ scoreLimit(n) }}점)
          </option>
        </select>
      </div>
      <button class="btn-start" @click="startGame">게임 시작</button>
      <p v-if="lobbyError" class="error">{{ lobbyError }}</p>
    </div>

    <!-- Game -->
    <GameScreen
      v-else
      :state="gameState"
      @state-update="gameState = $event"
      @restart="gameState = null"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { GameState } from './types/game'
import { api } from './api/gameApi'
import GameScreen from './components/GameScreen.vue'

const gameState = ref<GameState | null>(null)
const selectedSize = ref(8)
const lobbyError = ref<string | null>(null)

function scoreLimit(n: number): number {
  return n * n - 25
}

async function startGame() {
  lobbyError.value = null
  try {
    const { state } = await api.createGame(selectedSize.value)
    gameState.value = state
  } catch (e: unknown) {
    lobbyError.value = e instanceof Error ? e.message : String(e)
  }
}
</script>

<style>
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: 'Segoe UI', sans-serif; background: #1a1a2e; color: #eee; min-height: 100vh; }

.app { min-height: 100vh; display: flex; flex-direction: column; }

.lobby {
  display: flex; flex-direction: column; align-items: center;
  justify-content: center; gap: 20px; flex: 1; padding: 40px;
}
.lobby h1 { font-size: 2.5rem; color: #f0c040; }
.board-select { display: flex; gap: 12px; align-items: center; }
.board-select select { padding: 8px 12px; font-size: 16px; border-radius: 6px; }
.btn-start {
  padding: 12px 32px; font-size: 18px; background: #f0c040; color: #333;
  border: none; border-radius: 8px; cursor: pointer; font-weight: bold;
}
.btn-start:hover { background: #e0b030; }
.error { color: #ff6b6b; }
</style>
