import assert from 'node:assert/strict'
import test from 'node:test'

import { AuthApiError, authApi } from './authApi.ts'

test('Google completion sends only the ID token and explicit import choice', async () => {
  let request: RequestInit | undefined
  globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
    request = init
    return new Response(JSON.stringify({
      authenticated: true,
      user: { id: 'internal-user', displayName: 'Player', avatarUrl: null },
      importedGuestData: true,
    }), { status: 200, headers: { 'Content-Type': 'application/json' } })
  }) as typeof fetch

  await authApi.googleLogin('verified-id-token', true)
  assert.equal(request?.credentials, 'same-origin')
  assert.deepEqual(JSON.parse(String(request?.body)), {
    idToken: 'verified-id-token',
    importGuestData: true,
  })
})

test('guest import conflict remains a typed decision state', async () => {
  globalThis.fetch = (async () => new Response(JSON.stringify({
    code: 'guest_import_required',
    error: '선택이 필요합니다.',
  }), { status: 409, headers: { 'Content-Type': 'application/json' } })) as typeof fetch

  await assert.rejects(
    () => authApi.googleLogin('verified-id-token'),
    (error: unknown) => error instanceof AuthApiError
      && error.status === 409
      && error.code === 'guest_import_required',
  )
})

test('logout is a server request rather than a local-only state change', async () => {
  let request: RequestInit | undefined
  globalThis.fetch = (async (_url: string | URL | Request, init?: RequestInit) => {
    request = init
    return new Response(null, { status: 204 })
  }) as typeof fetch
  await authApi.logout()
  assert.equal(request?.method, 'POST')
  assert.equal(request?.credentials, 'same-origin')
})
