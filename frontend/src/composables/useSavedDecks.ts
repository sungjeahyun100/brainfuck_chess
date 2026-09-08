import { computed, ref, watch } from 'vue'
import type { SavedDeck } from '../types/deck.ts'
import { AccountDeckRepository, LocalDeckRepository, type DeckRepository } from './deckRepository.ts'
import { nextId } from './localDeckRepository.ts'
import { DeckApiError } from '../api/deckApi.ts'
export { createNewSavedDeck } from './localDeckRepository.ts'

// undefined means authentication is still unresolved, never an implicit guest.
export const deckStorageIdentity = ref<string | null | undefined>(undefined)
const identityError = ref<string | null>(null)
let identityRevision = 0
export function setDeckStorageIdentity(identity: string | null | undefined, error: string | null = null) {
  if (deckStorageIdentity.value !== identity || identityError.value !== error) identityRevision += 1
  identityError.value = error
  deckStorageIdentity.value = identity
}
export function useSavedDecks() {
  const decks = ref<SavedDeck[]>([])
  const loading = ref(true)
  const busy = ref(false)
  const error = ref<string | null>(null)
  const localCount = ref(0)
  const importResult = ref<string | null>(null)
  const isAccount = computed(() => typeof deckStorageIdentity.value === 'string')
  let requestRevision = 0
  let operationRevision = 0
  function repository(): DeckRepository {
    const identity = deckStorageIdentity.value
    if (identity === undefined) throw new Error(identityError.value ?? '로그인 상태를 확인 중입니다.')
    return identity === null ? new LocalDeckRepository() : new AccountDeckRepository(identity)
  }
  function ensureIdentity(revision: number) {
    if (revision !== identityRevision) throw new Error('로그인 상태가 변경되었습니다. 덱 목록에서 다시 열어 주세요.')
  }
  function report(cause: unknown) {
    error.value = cause instanceof Error ? cause.message : String(cause)
    if (cause instanceof DeckApiError && (cause.status === 401 || cause.code === 'deck_identity_changed')) {
      setDeckStorageIdentity(undefined, error.value)
    }
  }
  async function loadDecks() {
    const request = ++requestRevision
    const revision = identityRevision
    loading.value = true
    error.value = null
    try {
      const items = await repository().listDecks()
      ensureIdentity(revision)
      if (request === requestRevision) decks.value = items
    } catch (cause) {
      if (revision === identityRevision && request === requestRevision) report(cause)
    } finally {
      if (revision === identityRevision && request === requestRevision) loading.value = false
    }
  }
  async function getDeck(id: string) {
    const revision = identityRevision
    try {
      const deck = await repository().getDeck(id)
      ensureIdentity(revision)
      return deck
    } catch (cause) {
      if (revision === identityRevision) report(cause)
      throw cause
    }
  }
  async function mutate<T>(action: (repo: DeckRepository) => Promise<T>): Promise<T> {
    if (busy.value) throw new Error('덱 저장 작업이 진행 중입니다.')
    const revision = identityRevision
    const operation = ++operationRevision
    busy.value = true
    error.value = null
    try {
      const result = await action(repository())
      ensureIdentity(revision)
      return result
    } catch (cause) {
      if (revision === identityRevision) report(cause)
      throw cause
    } finally {
      if (operation === operationRevision) busy.value = false
    }
  }
  async function saveDeck(deck: SavedDeck) {
    return mutate(repo => repo.saveDeck(deck))
  }
  async function deleteDeck(id: string) {
    const deck = decks.value.find(deck => deck.id === id)
    if (!deck) throw new Error('삭제할 덱을 찾을 수 없습니다.')
    await mutate(repo => repo.deleteDeck(deck))
  }
  async function duplicateDeck(id: string) {
    const source = decks.value.find(deck => deck.id === id)
    if (!source) throw new Error('복제할 덱을 찾을 수 없습니다.')
    const { version: _version, ...copy } = source
    const now = Date.now()
    return saveDeck({ ...copy, id: nextId(), name: `${[...source.name].slice(0, 95).join('')} 복사본`, createdAt: now, updatedAt: now })
  }
  async function renameDeck(id: string, name: string) {
    const source = decks.value.find(deck => deck.id === id)
    if (!source) throw new Error('이름을 바꿀 덱을 찾을 수 없습니다.')
    return saveDeck({ ...source, name: name.trim() })
  }
  async function importLocalDecks() {
    await mutate(async repo => {
      if (!(repo instanceof AccountDeckRepository)) throw new Error('계정 로그인이 필요합니다.')
      const revision = identityRevision
      const local = await new LocalDeckRepository().listDecks()
      let success = 0
      const failed: string[] = []
      for (const deck of local) {
        ensureIdentity(revision)
        try { await repo.importDeck(deck); success += 1 }
        catch (cause) {
          ensureIdentity(revision)
          if (cause instanceof DeckApiError && (cause.status === 401 || cause.code === 'deck_identity_changed')) throw cause
          failed.push(`${deck.name}: ${cause instanceof Error ? cause.message : String(cause)}`)
        }
      }
      ensureIdentity(revision)
      importResult.value = `${success}/${local.length}개 서버 저장 확인. 브라우저 원본은 그대로 보관됩니다.${failed.length ? ` 실패 ${failed.length}개 — ${failed.join(' / ')}` : ''}`
    })
    await loadDecks()
  }
  watch([deckStorageIdentity, identityError], async () => {
    ++requestRevision
    ++operationRevision
    decks.value = []
    busy.value = false
    importResult.value = null
    localCount.value = 0
    if (deckStorageIdentity.value === undefined) {
      loading.value = !identityError.value
      error.value = identityError.value
      return
    }
    const revision = identityRevision
    await loadDecks()
    if (revision === identityRevision && isAccount.value) {
      const local = await new LocalDeckRepository().listDecks()
      if (revision === identityRevision) localCount.value = local.length
    }
  }, { immediate: true, flush: 'sync' })
  return { decks, loading, busy, error, isAccount, localCount, importResult, loadDecks, getDeck, saveDeck, deleteDeck, duplicateDeck, renameDeck, importLocalDecks }
}
