import assert from 'node:assert/strict'
import test from 'node:test'

import { accountSettingsDraft, persistAccountSettings } from './accountSettings.ts'
import type { AuthUser } from './api/authApi.ts'

const current: AuthUser = {
  id: 'internal-user',
  publicId: 'player',
  displayName: 'Player',
  avatarUrl: null,
  profileVisibility: 'public',
}

test('settings draft displays the current public or private state', () => {
  assert.equal(accountSettingsDraft(current).profileVisibility, 'public')
  assert.equal(
    accountSettingsDraft({ ...current, profileVisibility: 'private' }).profileVisibility,
    'private',
  )
})

test('successful save returns the authoritative server profile', async () => {
  const saved = await persistAccountSettings(
    { displayName: ' Private Player ', profileVisibility: 'private' },
    async (input) => ({
      user: { ...current, displayName: input.displayName ?? null, profileVisibility: 'private' },
    }),
  )
  assert.equal(saved.displayName, 'Private Player')
  assert.equal(saved.profileVisibility, 'private')
})

test('failed save does not mutate the previously rendered profile', async () => {
  const before = structuredClone(current)
  await assert.rejects(
    () => persistAccountSettings(
      { displayName: 'Changed', profileVisibility: 'private' },
      async () => { throw new Error('save failed') },
    ),
    /save failed/,
  )
  assert.deepEqual(current, before)
})
