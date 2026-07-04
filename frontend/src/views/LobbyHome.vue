<template>
  <main class="lobby home-view">
    <section class="lobby-hero">
      <p class="eyebrow">Deck Chess Lobby</p>
      <h1>덱체스 <span class="hero-en">Deck Chess</span></h1>
    </section>

    <section class="home-actions">
      <button class="home-action" @click="$emit('navigate', 'single-select')">싱글플레이</button>
      <button class="home-action" @click="$emit('navigate', 'multiplayer')">멀티플레이</button>
      <button class="home-action" @click="$emit('navigate', 'bot-select')">봇 플레이</button>
      <button class="home-action" @click="$emit('navigate', 'deck-library')">덱 빌딩</button>
      <button class="home-action" @click="$emit('navigate', 'piece-lab')">기물 테스트장</button>
    </section>
  </main>
</template>

<script setup lang="ts">
import type { AppView } from '../types/deck'

defineEmits<{
  navigate: [view: AppView]
}>()
</script>
