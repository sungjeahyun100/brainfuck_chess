<template>
  <aside class="auth-account" aria-label="계정">
    <span v-if="loading" class="auth-muted">계정 확인 중…</span>
    <template v-else-if="user">
      <img v-if="user.avatarUrl" :src="user.avatarUrl" alt="" referrerpolicy="no-referrer" />
      <span>{{ user.displayName || '덱 체스 사용자' }}</span>
      <button type="button" :disabled="busy" @click="logout">로그아웃</button>
    </template>
    <button v-else type="button" :disabled="busy || !loginAvailable" @click="login">
      {{ busy ? '로그인 중…' : 'Google로 로그인' }}
    </button>
    <p v-if="!loginAvailable && !loading" class="auth-error">Google 로그인 설정이 필요합니다.</p>
    <p v-else-if="error" class="auth-error">{{ error }}</p>
  </aside>

  <div v-if="pendingToken" class="auth-modal-backdrop" role="presentation">
    <section class="auth-modal card" role="dialog" aria-modal="true" aria-labelledby="guest-import-title">
      <h2 id="guest-import-title">게스트 기물 가져오기</h2>
      <p>이 브라우저에서 로그인 전에 만든 커스텀 기물이 있습니다. 내 계정으로 가져올까요?</p>
      <p class="auth-muted">가져오지 않아도 게스트 데이터는 삭제되지 않습니다.</p>
      <div class="auth-modal-actions">
        <button type="button" :disabled="busy" @click="finishLogin(false)">가져오지 않고 로그인</button>
        <button type="button" class="auth-primary" :disabled="busy" @click="finishLogin(true)">내 계정으로 가져오기</button>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getApps, initializeApp } from 'firebase/app'
import { getAuth, GoogleAuthProvider, signInWithPopup, signOut, type Auth } from 'firebase/auth'
import { AuthApiError, authApi, type AuthUser } from '../api/authApi'
import { CUSTOM_PIECES_CHANGED_EVENT } from '../api/customPieceApi'
import { firebaseConfig } from '../config'

const user = ref<AuthUser | null>(null)
const loading = ref(true)
const busy = ref(false)
const error = ref<string | null>(null)
const pendingToken = ref<string | null>(null)
const loginAvailable = computed(() => firebaseConfig !== null)

function firebaseAuth(): Auth {
  if (!firebaseConfig) throw new Error('Google 로그인 설정이 없습니다.')
  const app = getApps()[0] ?? initializeApp(firebaseConfig)
  return getAuth(app)
}

async function refresh() {
  await authApi.ensureGuestSession()
  const state = await authApi.me()
  user.value = state.user
}

async function login() {
  busy.value = true
  error.value = null
  try {
    await refresh()
    const auth = firebaseAuth()
    const credential = await signInWithPopup(auth, new GoogleAuthProvider())
    const idToken = await credential.user.getIdToken(true)
    try {
      const result = await authApi.googleLogin(idToken)
      user.value = result.user
      await signOut(auth).catch(() => undefined)
      window.dispatchEvent(new Event(CUSTOM_PIECES_CHANGED_EVENT))
    } catch (cause) {
      if (cause instanceof AuthApiError && cause.code === 'guest_import_required') {
        pendingToken.value = idToken
        return
      }
      throw cause
    }
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : 'Google 로그인을 완료하지 못했습니다.'
  } finally {
    busy.value = false
  }
}

async function finishLogin(importGuestData: boolean) {
  if (!pendingToken.value) return
  busy.value = true
  error.value = null
  try {
    const result = await authApi.googleLogin(pendingToken.value, importGuestData)
    user.value = result.user
    pendingToken.value = null
    await signOut(firebaseAuth()).catch(() => undefined)
    window.dispatchEvent(new Event(CUSTOM_PIECES_CHANGED_EVENT))
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : '로그인을 완료하지 못했습니다.'
  } finally {
    busy.value = false
  }
}

async function logout() {
  busy.value = true
  error.value = null
  try {
    await authApi.logout()
    if (firebaseConfig) await signOut(firebaseAuth()).catch(() => undefined)
    user.value = null
    await authApi.ensureGuestSession()
    window.dispatchEvent(new Event(CUSTOM_PIECES_CHANGED_EVENT))
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : '로그아웃하지 못했습니다.'
  } finally {
    busy.value = false
  }
}

onMounted(async () => {
  try {
    await refresh()
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : '계정 상태를 확인하지 못했습니다.'
  } finally {
    loading.value = false
  }
})
</script>

<style scoped>
.auth-account { position: fixed; z-index: 80; top: 40px; right: 16px; display: flex; align-items: center; gap: 9px; max-width: min(460px, calc(100vw - 32px)); padding: 9px 11px; border: 1px solid var(--line); border-radius: 10px; background: rgba(10, 16, 25, .94); box-shadow: 0 10px 28px rgba(0,0,0,.3); }
.auth-account img { width: 30px; height: 30px; border-radius: 50%; }
.auth-account button, .auth-modal button { border: 1px solid rgba(217,164,65,.35); border-radius: 7px; padding: 8px 11px; background: #243142; color: var(--text); cursor: pointer; }
.auth-account button:disabled, .auth-modal button:disabled { opacity: .55; cursor: not-allowed; }
.auth-muted { color: var(--muted); font-size: 13px; }
.auth-error { color: var(--danger); font-size: 12px; }
.auth-modal-backdrop { position: fixed; z-index: 1200; inset: 0; display: grid; place-items: center; padding: 20px; background: rgba(5,8,13,.8); }
.auth-modal { width: min(560px, 100%); display: flex; flex-direction: column; gap: 15px; padding: 24px; }
.auth-modal-actions { display: flex; justify-content: flex-end; flex-wrap: wrap; gap: 10px; }
.auth-modal .auth-primary { background: linear-gradient(135deg,#f0c15f,#c68a1b); color: #221a0d; font-weight: 800; }
@media (max-width: 620px) { .auth-account { position: relative; top: auto; right: auto; align-self: flex-end; margin: 12px 12px 0; flex-wrap: wrap; } }
</style>
