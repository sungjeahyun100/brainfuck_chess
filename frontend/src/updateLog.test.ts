import assert from 'node:assert/strict'
import test from 'node:test'
import { acknowledgeUpdate, readUpdateStatus, updateLog, UPDATE_LOG_STORAGE_KEY } from './updateLog.ts'

function memoryStorage(initial?: string) {
  const values = new Map<string, string>()
  if (initial !== undefined) values.set(UPDATE_LOG_STORAGE_KEY, initial)
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value) },
  }
}

test('first visit is unread and reading alone does not acknowledge the update', () => {
  const storage = memoryStorage()
  assert.equal(readUpdateStatus('release-1', () => storage), 'unread')
  assert.equal(storage.getItem(UPDATE_LOG_STORAGE_KEY), null)
})

test('acknowledgment survives a new visit but a new release is unread', () => {
  const storage = memoryStorage()
  assert.equal(acknowledgeUpdate('release-1', () => storage), 'saved')
  assert.equal(readUpdateStatus('release-1', () => storage), 'read')
  assert.equal(readUpdateStatus('release-2', () => storage), 'unread')
  assert.equal(acknowledgeUpdate('release-2', () => storage), 'saved')
  assert.equal(readUpdateStatus('release-2', () => storage), 'read')
})

test('skipped releases and unrecognized or malformed stored values show the latest update', () => {
  for (const previous of ['release-1', '', 'null', '{}', 'unknown-release']) {
    assert.equal(readUpdateStatus('release-3', () => memoryStorage(previous)), 'unread')
  }
})

test('reading and writing fail explicitly when storage access is blocked', () => {
  const blocked = () => { throw new Error('SecurityError') }
  assert.equal(readUpdateStatus('release-1', blocked), 'unavailable')
  assert.equal(acknowledgeUpdate('release-1', blocked), 'unavailable')
})

test('a write failure does not acknowledge an update or erase the previous value', () => {
  const storage = memoryStorage('release-1')
  const full = { ...storage, setItem: () => { throw new Error('QuotaExceededError') } }
  assert.equal(acknowledgeUpdate('release-2', () => full), 'unavailable')
  assert.equal(storage.getItem(UPDATE_LOG_STORAGE_KEY), 'release-1')
  assert.equal(readUpdateStatus('release-2', () => storage), 'unread')
})

test('a storage read failure is reported without crashing', () => {
  const broken = { ...memoryStorage(), getItem: () => { throw new Error('read failed') } }
  assert.equal(readUpdateStatus('release-1', () => broken), 'unavailable')
})

test('release entries have unique IDs, valid dates and nonempty user-facing content, newest first', () => {
  assert.ok(updateLog.length > 0)
  assert.equal(new Set(updateLog.map(entry => entry.id)).size, updateLog.length)
  updateLog.forEach((entry, index) => {
    assert.ok(entry.id.trim())
    assert.match(entry.date, /^\d{4}-\d{2}-\d{2}$/)
    assert.equal(new Date(entry.date).toISOString().slice(0, 10), entry.date)
    assert.ok(entry.title.trim())
    assert.ok(entry.changes.length > 0)
    assert.ok(entry.changes.every(change => change.trim()))
    if (index > 0) assert.ok(updateLog[index - 1].date >= entry.date)
  })
})
