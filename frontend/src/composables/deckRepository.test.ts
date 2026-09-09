import { parseDeckRuleset } from '../deckRulesets.ts'
import { savedDeckToPlayerDeckRequest, serializeNeutralDeck } from './useDeckSerialization.ts'
import assert from 'node:assert/strict'
import test from 'node:test'
import { effectScope } from 'vue'
import { LocalDeckRepository, AccountDeckRepository } from './deckRepository.ts'
import { createNewSavedDeck, setDeckStorageIdentity, useSavedDecks } from './useSavedDecks.ts'
import { decodeDeckCode, encodeDeckCode } from './useDeckCodeCodec.ts'
import { importDeckCode } from './useDeckCode.ts'
import { applyPieceMetadata, pieceCatalog, validateDeckForStorage, validateSavedDeck } from './useDeckValidation.ts'
import type { SavedDeck } from '../types/deck.ts'

const key = 'brainfuck_chess_saved_decks_v1'
function storage() {
  const data = new Map<string, string>()
  const store: Storage = {
    get length() { return data.size }, clear() { data.clear() },
    key(index) { return [...data.keys()][index] ?? null },
    getItem(name) { return data.get(name) ?? null },
    setItem(name, value) { data.set(name, value) }, removeItem(name) { data.delete(name) },
  }
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: store })
  return store
}
function fixture() {
  const decks = new Map<string, Map<string, SavedDeck>>()
  const imported = new Map<string, string>()
  const requests: { account: string; method: string; body: any }[] = []
  let unavailable = false
  const fetcher: typeof fetch = async (url, init) => {
    const account = new Headers(init?.headers).get('X-Deck-Account')!
    const method = init?.method ?? 'GET'
    const body = init?.body ? JSON.parse(String(init.body)) : undefined
    requests.push({ account, method, body })
    if (unavailable) return Response.json({ error: '서버 연결 실패', code: 'deck_store_unavailable' }, { status: 503 })
    let items = decks.get(account)
    if (!items) { items = new Map(); decks.set(account, items) }
    const path = String(url).replace('/api/decks', '')
    if (method === 'GET' && !path) return Response.json({ items: [...items.values()] })
    const id = decodeURIComponent(path.slice(1))
    if (method === 'GET') return items.has(id) ? Response.json(items.get(id)) : Response.json({ error: '덱 없음' }, { status: 404 })
    if (method === 'POST') {
      const identity = `${account}:${path === '/import' ? JSON.stringify(body) : body.requestId}`
      if (imported.has(identity)) return Response.json(items.get(imported.get(identity)!))
      const saved: SavedDeck = { id: crypto.randomUUID(), name: body.name, ...body.deckData, createdAt: 1, updatedAt: 1, version: 1 }
      items.set(saved.id, saved); imported.set(identity, saved.id)
      return Response.json(saved)
    }
    const existing = items.get(id)
    if (!existing) return Response.json({ error: '덱 없음' }, { status: 404 })
    if (body.expectedVersion !== existing.version) return Response.json({ error: '수정 충돌', code: 'deck_conflict' }, { status: 409 })
    if (method === 'DELETE') { items.delete(id); return new Response(null, { status: 204 }) }
    const saved = { ...existing, name: body.name, ...body.deckData, version: existing.version! + 1 }
    items.set(id, saved)
    return Response.json(saved)
  }
  return { fetcher, requests, decks, fail(value: boolean) { unavailable = value } }
}
const settle = async () => { for (let i = 0; i < 12; i++) await new Promise(resolve => setImmediate(resolve)) }

test('guest CRUD survives refresh and reads the original key/legacy missing fields', async () => {
  const browser = storage()
  const legacy = createNewSavedDeck()
  const { mapId: _map, customPieces: _custom, ...old } = legacy
  browser.setItem(key, JSON.stringify([old]))
  const repo = new LocalDeckRepository()
  assert.equal((await repo.listDecks())[0].mapId, 'standard-8x8')
  assert.deepEqual((await repo.getDeck(old.id))!.customPieces, [])
  const created = await repo.saveDeck(createNewSavedDeck())
  assert.equal((await new LocalDeckRepository().listDecks()).length, 2)
  created.name = '새 이름'
  await repo.saveDeck(created)
  assert.equal((await repo.getDeck(created.id))!.name, '새 이름')
  await repo.deleteDeck(created)
  assert.equal((await repo.listDecks())[0].id, old.id)
})

test('account CRUD persists across repositories/devices with optimistic concurrency and no local writes', async t => {
  const browser = storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  const a = new AccountDeckRepository('alice'); const b = new AccountDeckRepository('alice')
  const created = await a.saveDeck(createNewSavedDeck())
  assert.deepEqual(await b.getDeck(created.id), created)
  assert.equal((await new AccountDeckRepository('alice').listDecks()).length, 1)
  assert.equal((await new AccountDeckRepository('bob').listDecks()).length, 0)
  const edited = await b.saveDeck({ ...created, name: '브라우저 B' })
  await assert.rejects(a.saveDeck({ ...created, name: '오래된 A' }), /충돌/)
  await assert.rejects(a.deleteDeck(created), /충돌/)
  assert.equal((await a.getDeck(created.id))!.name, '브라우저 B')
  await a.deleteDeck(edited)
  assert.equal((await b.listDecks()).length, 0)
  assert.equal(browser.getItem(key), null)
  assert.ok(backend.requests.every(r => !r.body || (!('owner_id' in r.body) && !('ownerId' in r.body))))
})

test('login/logout switches stores without copying account data and requires explicit import', async t => {
  storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  const local = new LocalDeckRepository(); const guest = await local.saveDeck(createNewSavedDeck())
  setDeckStorageIdentity(null)
  const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  assert.equal(state.decks.value[0].id, guest.id)
  setDeckStorageIdentity('alice'); await settle()
  assert.equal(state.decks.value.length, 0); assert.equal(state.localCount.value, 1)
  assert.equal(backend.requests.filter(r => r.method === 'POST').length, 0)
  await state.importLocalDecks()
  const importedId = state.decks.value[0].id
  assert.notEqual(importedId, guest.id)
  assert.equal((await local.getDeck(guest.id))!.id, guest.id)
  await state.importLocalDecks()
  assert.equal(state.decks.value.length, 1)
  setDeckStorageIdentity(null); await settle()
  assert.equal(state.decks.value[0].id, guest.id)
  assert.equal(await local.getDeck(importedId), null)
  setDeckStorageIdentity('alice'); await settle()
  assert.equal(state.decks.value[0].id, importedId)
})

test('network load/save/import failures stay errors and preserve guest originals', async t => {
  const browser = storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  await new LocalDeckRepository().saveDeck(createNewSavedDeck()); const original = browser.getItem(key)
  setDeckStorageIdentity('alice'); const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  const account = await state.saveDeck(createNewSavedDeck()); await state.loadDecks()
  backend.fail(true)
  await state.loadDecks()
  assert.equal(state.error.value, '서버 연결 실패')
  assert.equal(state.decks.value[0].id, account.id)
  await assert.rejects(state.saveDeck({ ...account, name: '실패' }), /서버 연결 실패/)
  await state.importLocalDecks()
  assert.match(state.importResult.value!, /0\/1개/)
  assert.match(state.importResult.value!, /실패 1개/)
  assert.equal(browser.getItem(key), original)
  assert.equal(backend.decks.get('alice')!.get(account.id)!.name, account.name)
})

test('unresolved/failed authentication never silently chooses local storage', async t => {
  storage(); await new LocalDeckRepository().saveDeck(createNewSavedDeck())
  setDeckStorageIdentity(undefined, '인증 확인 실패')
  const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  assert.equal(state.decks.value.length, 0)
  assert.equal(state.error.value, '인증 확인 실패')
  await assert.rejects(state.saveDeck(createNewSavedDeck()), /인증 확인 실패/)
})

test('late account responses cannot repopulate the guest list after logout', async t => {
  storage(); const local = await new LocalDeckRepository().saveDeck(createNewSavedDeck())
  let resolve!: (value: Response) => void
  t.mock.method(globalThis, 'fetch', () => new Promise<Response>(done => { resolve = done }))
  setDeckStorageIdentity('alice'); const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  setDeckStorageIdentity(null); await settle()
  resolve(Response.json({ items: [{ ...local, id: 'account-only', version: 1 }] })); await settle()
  assert.equal(state.decks.value[0].id, local.id)
  assert.equal(state.loading.value, false)
  assert.equal(state.error.value, null)
})

test('deck codes remain independent of guest/account persistence and preserve pinned refs in transport', async t => {
  storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  applyPieceMetadata(Object.fromEntries(pieceCatalog.filter(p => !p.custom).map(p => [p.id, { score: 0, deployment_zone: ['pawn','tempest-pawn','bouncing-pawn','dozer'].includes(p.id) ? 'front' : 'back' }])))
  const source = createNewSavedDeck()
  for (const repo of [new LocalDeckRepository(), new AccountDeckRepository('alice')]) {
    const decoded = importDeckCode(encodeDeckCode(source), createNewSavedDeck())
    assert.equal(decoded.ok, true)
    if (!decoded.ok) throw new Error(decoded.message)
    const saved = await repo.saveDeck(decoded.deck)
    assert.equal(encodeDeckCode(saved), encodeDeckCode(source))
  }
  const custom = { id: 'existing-package-id', version: 3, contentHash: 'fnv1a64:0123456789abcdef', exposedPieceKey: 'hero' }
  const key = `custom:${custom.id}:v3:hero`
  const saved = await new AccountDeckRepository('alice').saveDeck({ ...source, id: crypto.randomUUID(), customPieces: [custom], pocket: { [key]: 2 } })
  assert.equal(decodeDeckCode(encodeDeckCode(saved)).ok, true)
  assert.equal(decodeDeckCode(encodeDeckCode({ ...saved, name: '♟️'.repeat(50) })).ok, true)
  assert.equal(decodeDeckCode(encodeDeckCode({ ...saved, name: '😀'.repeat(100) })).ok, true)
  assert.equal(decodeDeckCode(encodeDeckCode({ ...saved, name: '😀'.repeat(101) })).ok, false)
  assert.deepEqual(saved.customPieces, [custom]); assert.equal(saved.pocket[key], 2)
})

test('partial import reports individual failures and keeps every original', async t => {
  const browser = storage(); const backend = fixture()
  t.mock.method(globalThis, 'fetch', async (url: string, init: RequestInit) => {
    const payload = init.body ? JSON.parse(String(init.body)) : null
    if (String(url).endsWith('/import') && payload?.name === '실패할 덱') {
      return Response.json({ error: '기물 권한 없음', code: 'invalid_deck' }, { status: 400 })
    }
    return backend.fetcher(url, init)
  })
  const local = new LocalDeckRepository()
  await local.saveDeck(createNewSavedDeck())
  await local.saveDeck({ ...createNewSavedDeck(), name: '실패할 덱' })
  const original = browser.getItem(key)
  setDeckStorageIdentity('alice'); const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  await state.importLocalDecks()
  assert.equal(state.decks.value.length, 1)
  assert.match(state.importResult.value!, /1\/2개/)
  assert.match(state.importResult.value!, /실패할 덱: 기물 권한 없음/)
  assert.equal(browser.getItem(key), original)
})

test('a save response from a prior account never reports success after identity changes', async t => {
  storage(); const backend = fixture(); let resolve!: (value: Response) => void
  const b = { ...createNewSavedDeck(), id: 'bob-only', version: 1 }
  backend.decks.set('bob', new Map([[b.id, b]]))
  t.mock.method(globalThis, 'fetch', (url: string, init: RequestInit) => {
    if (init.method === 'POST') return new Promise<Response>(done => { resolve = done })
    return backend.fetcher(url, init)
  })
  setDeckStorageIdentity('alice'); const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  const deck = createNewSavedDeck()
  const pending = state.saveDeck(deck)
  const rejected = assert.rejects(pending, /로그인 상태가 변경/)
  setDeckStorageIdentity(null); await settle()
  setDeckStorageIdentity('bob'); await settle()
  resolve(Response.json({ ...deck, id: 'alice-only', version: 1 }))
  await rejected
  assert.deepEqual(state.decks.value, [b])
  assert.deepEqual([...backend.decks.get('bob')!.values()], [b])
  assert.equal(state.error.value, null)
  assert.equal(localStorage.getItem(key), null)
})

test('drafts save and reload while game validation still blocks incomplete decks', async t => {
  storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  applyPieceMetadata(Object.fromEntries(pieceCatalog.filter(p => !p.custom).map(p => [p.id, {
    score: p.id === 'queen' ? 9 : 0,
    deployment_zone: ['pawn', 'tempest-pawn', 'bouncing-pawn', 'dozer'].includes(p.id) ? 'front' : 'back',
  }])))
  const repo = new AccountDeckRepository('opaque-alice')
  for (const incomplete of [
    { ...createNewSavedDeck(), starting: [] },
    { ...createNewSavedDeck(), starting: [{ pieceType: 'king', square: { file: 4, rank: 0 } }] },
    { ...createNewSavedDeck(), pocket: { queen: 100 } },
    { ...createNewSavedDeck(), pocket: { king: 1 } },
  ]) {
    assert.equal(validateDeckForStorage(incomplete).valid, true)
    assert.equal(validateSavedDeck(incomplete).valid, false)
    const saved = await repo.saveDeck(incomplete)
    const loaded = (await new AccountDeckRepository('opaque-alice').getDeck(saved.id))!
    assert.deepEqual(loaded.starting, incomplete.starting)
    assert.deepEqual(loaded.pocket, incomplete.pocket)
    const complete = { ...loaded, ...createNewSavedDeck(), id: loaded.id, version: loaded.version }
    assert.equal(validateSavedDeck(complete).valid, true)
    const updated = await repo.saveDeck(complete)
    assert.equal(updated.version, 2)
  }
  assert.equal(localStorage.getItem(key), null)
})

test('storage and game validators share structure errors without enforcing game rules on drafts', () => {
  const duplicate = createNewSavedDeck()
  duplicate.starting.push({ ...duplicate.starting[0] })
  const outside = createNewSavedDeck()
  outside.starting[0].square.file = 8
  for (const invalid of [
    duplicate, outside,
    { ...createNewSavedDeck(), pocket: { knight: -1 } },
    { ...createNewSavedDeck(), pocket: { knight: 1.5 } },
    { ...createNewSavedDeck(), pocket: { knight: 1025 } },
    { ...createNewSavedDeck(), name: 'x'.repeat(101) },
    { ...createNewSavedDeck(), boardSize: 7 },
  ]) {
    const storage = validateDeckForStorage(invalid)
    assert.equal(storage.valid, false)
    const game = validateSavedDeck(invalid)
    assert.ok(storage.errors.every(error => game.errors.includes(error)))
  }
})

test('intentional creates and duplicate commands preserve identical content as distinct decks', async t => {
  storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  setDeckStorageIdentity('opaque-alice')
  const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  const original = await state.saveDeck(createNewSavedDeck())
  const other = await state.saveDeck({ ...original, id: crypto.randomUUID(), version: undefined })
  assert.notEqual(other.id, original.id)
  assert.equal(other.name, original.name)
  await state.loadDecks()
  const duplicate = await state.duplicateDeck(original.id)
  assert.ok(duplicate)
  assert.notEqual(duplicate.id, original.id)
  assert.deepEqual(duplicate.starting, original.starting)
  assert.deepEqual(duplicate.pocket, original.pocket)
  await state.loadDecks()
  assert.equal(state.decks.value.length, 3)
  assert.equal(localStorage.getItem(key), null)
})

test('late GET from A cannot replace B after logout and login', async t => {
  storage(); const backend = fixture()
  const b = { ...createNewSavedDeck(), id: 'bob-deck', version: 1 }
  backend.decks.set('bob', new Map([[b.id, b]]))
  let resolve!: (value: Response) => void
  t.mock.method(globalThis, 'fetch', (url, init) => {
    if (new Headers(init?.headers).get('X-Deck-Account') === 'alice') {
      return new Promise<Response>(done => { resolve = done })
    }
    return backend.fetcher(url, init)
  })
  setDeckStorageIdentity('alice'); const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop()); await settle()
  setDeckStorageIdentity(null); await settle()
  setDeckStorageIdentity('bob'); await settle()
  assert.deepEqual(state.decks.value, [b])
  resolve(Response.json({ items: [{ ...createNewSavedDeck(), id: 'alice-deck', version: 1 }] }))
  await settle()
  assert.deepEqual(state.decks.value, [b])
  assert.equal(state.error.value, null)
  assert.equal(state.loading.value, false)
})


test('ruleset normalizes only missing local values and preserves both formats across refresh', async () => {
  const browser = storage()
  const { ruleset: _ruleset, ...old } = createNewSavedDeck()
  browser.setItem(key, JSON.stringify([old]))
  const repo = new LocalDeckRepository()
  assert.equal((await repo.getDeck(old.id))!.ruleset, 'legacy')
  for (const ruleset of ['legacy', 'standard'] as const) {
    const source = { ...createNewSavedDeck(), ruleset }
    const saved = await repo.saveDeck(source)
    assert.equal(saved.ruleset, ruleset)
    assert.equal((await new LocalDeckRepository().getDeck(saved.id))!.ruleset, ruleset)
    assert.equal(JSON.parse(browser.getItem(key)!).find((entry: any) => entry.id === saved.id).ruleset, ruleset)
  }
  for (const unknown of ['future', null, 1, false]) {
    const invalid = { ...old, ruleset: unknown }
    const raw = JSON.stringify([invalid])
    browser.setItem(key, raw)
    await assert.rejects(repo.listDecks(), /지원하지 않는 덱 룰/)
    assert.equal(browser.getItem(key), raw)
    assert.throws(() => parseDeckRuleset(unknown), /지원하지 않는 덱 룰/)
    assert.equal(validateDeckForStorage(invalid as SavedDeck).valid, false)
    assert.equal(validateSavedDeck(invalid as SavedDeck).valid, false)
  }
})

test('account ruleset roundtrips through allowlist and rejects unknown outbound and inbound values', async t => {
  storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  const repo = new AccountDeckRepository('alice')
  for (const ruleset of ['legacy', 'standard'] as const) {
    const created = await repo.saveDeck({ ...createNewSavedDeck(), ruleset })
    assert.equal(created.ruleset, ruleset)
    assert.equal(backend.requests.at(-1)!.body.deckData.ruleset, ruleset)
    assert.equal((await new AccountDeckRepository('alice').getDeck(created.id)).ruleset, ruleset)
  }
  const old = { ...createNewSavedDeck(), id: 'old', version: 1 }
  delete (old as Partial<SavedDeck>).ruleset
  backend.decks.get('alice')!.set(old.id, old)
  assert.equal((await repo.getDeck(old.id)).ruleset, 'legacy')
  const invalid = { ...old, ruleset: 'future' } as unknown as SavedDeck
  const count = backend.requests.length
  assert.throws(() => repo.saveDeck(invalid), /지원하지 않는 덱 룰/)
  assert.equal(backend.requests.length, count)
  backend.decks.get('alice')!.set(old.id, invalid)
  await assert.rejects(repo.getDeck(old.id), /지원하지 않는 덱 룰/)
})

test('ruleset survives neutral/black serialization without changing coordinates or map', () => {
  storage()
  for (const ruleset of ['legacy', 'standard'] as const) {
    const deck = { ...createNewSavedDeck('standard-10x10'), ruleset }
    const before = JSON.parse(JSON.stringify(deck))
    assert.equal(savedDeckToPlayerDeckRequest(deck).ruleset, ruleset)
    const white = serializeNeutralDeck(deck, 'white')
    const black = serializeNeutralDeck(deck, 'black')
    assert.equal(white.ruleset, ruleset)
    assert.equal(black.ruleset, ruleset)
    assert.deepEqual(black.starting.map(piece => piece.square), white.starting.map(piece => ({ file: piece.square.file, rank: 9 - piece.square.rank })))
    assert.deepEqual(deck, before)
  }
})

test('unknown local ruleset is reported during guest load and account import discovery without overwriting storage', async t => {
  const browser = storage(); const backend = fixture(); t.mock.method(globalThis, 'fetch', backend.fetcher)
  const saved = createNewSavedDeck()
  const original = JSON.stringify([{ ...saved, ruleset: 'future' }])
  browser.setItem(key, original)
  setDeckStorageIdentity(null)
  const scope = effectScope(); const state = scope.run(useSavedDecks)!
  t.after(() => scope.stop())
  await settle()
  assert.match(state.error.value!, /지원하지 않는/)
  assert.equal(state.loading.value, false)
  setDeckStorageIdentity('alice')
  await settle()
  assert.match(state.error.value!, /지원하지 않는/)
  assert.equal(state.localCount.value, 0)
  await assert.rejects(state.importLocalDecks(), /지원하지 않는/)
  assert.equal(backend.requests.filter(request => request.method === 'POST').length, 0)
  assert.equal(browser.getItem(key), original)
})
