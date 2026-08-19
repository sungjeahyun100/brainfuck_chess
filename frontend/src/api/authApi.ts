export interface AuthUser {
  id: string
  publicId: string | null
  displayName: string | null
  avatarUrl: string | null
}

export interface AuthStateResponse {
  authenticated: boolean
  user: AuthUser | null
}

export class AuthApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(
    message: string,
    status: number,
    code: string,
  ) {
    super(message)
    this.status = status
    this.code = code
  }
}

async function parse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { code?: string; error?: string }
    throw new AuthApiError(body.error ?? '요청을 처리하지 못했습니다.', response.status, body.code ?? 'request_failed')
  }
  if (response.status === 204) return undefined as T
  return response.json() as Promise<T>
}

export const authApi = {
  ensureGuestSession: () => fetch('/api/auth/session', {
    method: 'POST',
    credentials: 'same-origin',
  }).then(parse<{ userId: string }>),

  me: () => fetch('/api/auth/me', {
    credentials: 'same-origin',
  }).then(parse<AuthStateResponse>),

  googleLogin: (idToken: string, importGuestData?: boolean) => fetch('/api/auth/google', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ idToken, importGuestData }),
  }).then(parse<AuthStateResponse & { importedGuestData: boolean }>),

  updateProfile: (publicId: string) => fetch('/api/auth/profile', {
    method: 'PATCH',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ publicId }),
  }).then(parse<{ user: AuthUser }>),

  logout: () => fetch('/api/auth/logout', {
    method: 'POST',
    credentials: 'same-origin',
  }).then(parse<void>),
}
