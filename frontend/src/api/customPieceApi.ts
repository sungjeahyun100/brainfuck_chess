import type { TurnAction } from '../types/game'
import type {
  CustomPieceImageAsset,
  CustomPieceInput,
  CustomPieceRecord,
  CustomPieceTestBoard,
  CustomPieceTestDefinition,
  CustomPieceTestResult,
  CustomPieceValidation,
} from '../types/customPiece'

const BASE = '/api/custom-pieces'
const PROTOTYPE_USER = 'browser-prototype-user'

export type CustomPieceErrorKind =
  | 'authentication'
  | 'permission'
  | 'input'
  | 'chessembly'
  | 'image'
  | 'not_found'
  | 'conflict'
  | 'execution_limit'
  | 'server'

export class CustomPieceApiError extends Error {
  readonly kind: CustomPieceErrorKind
  readonly code: string
  readonly status: number

  constructor(
    kind: CustomPieceErrorKind,
    code: string,
    message: string,
    status: number,
  ) {
    super(message)
    this.kind = kind
    this.code = code
    this.status = status
  }
}

export function classifyCustomPieceError(status: number, code: string): CustomPieceErrorKind {
  if (status === 401) return 'authentication'
  if (status === 403) return 'permission'
  if (status === 404) return 'not_found'
  if (status === 409 || code.includes('conflict')) return 'conflict'
  if (code.includes('image') || code.includes('svg')) return 'image'
  if (code.includes('execution_limit')) return 'execution_limit'
  if (code.includes('chessembly') || code.includes('piece_missing') || code.includes('reference')) return 'chessembly'
  if (status >= 500) return 'server'
  return 'input'
}

async function request<T>(url: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      'X-User-Id': PROTOTYPE_USER,
      ...options.headers,
    },
  })
  if (!response.ok) {
    const body = await response.json().catch(() => ({})) as { error?: string; code?: string }
    const code = body.code ?? 'request_failed'
    throw new CustomPieceApiError(
      classifyCustomPieceError(response.status, code),
      code,
      userFacingError(classifyCustomPieceError(response.status, code), body.error),
      response.status,
    )
  }
  if (response.status === 204) return undefined as T
  return response.json() as Promise<T>
}

function userFacingError(kind: CustomPieceErrorKind, serverMessage?: string): string {
  const fallback: Record<CustomPieceErrorKind, string> = {
    authentication: '로그인이 필요합니다.',
    permission: '이 기물에 접근할 권한이 없습니다.',
    input: '입력 내용을 확인해 주세요.',
    chessembly: '체섬블리 코드를 해석할 수 없습니다.',
    image: '이미지를 사용할 수 없습니다.',
    not_found: '기물을 찾을 수 없습니다.',
    conflict: '다른 곳에서 기물이 수정되었습니다. 다시 불러온 뒤 변경을 적용해 주세요.',
    execution_limit: '코드 실행 제한을 초과했습니다.',
    server: '서버가 일시적으로 응답하지 않습니다. 잠시 뒤 다시 시도해 주세요.',
  }
  return serverMessage && kind !== 'server' ? serverMessage : fallback[kind]
}

export const customPieceApi = {
  list: () => request<{ items: CustomPieceRecord[] }>(BASE),
  get: (id: string) => request<CustomPieceRecord>(`${BASE}/${encodeURIComponent(id)}`),
  getVersion: (id: string, version: number) =>
    request<CustomPieceRecord>(`${BASE}/${encodeURIComponent(id)}/versions/${version}`),
  create: (input: CustomPieceInput) => request<CustomPieceRecord>(BASE, {
    method: 'POST',
    body: JSON.stringify(input),
  }),
  update: (id: string, input: CustomPieceInput, expectedVersion: number) =>
    request<CustomPieceRecord>(`${BASE}/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify({ ...input, expected_version: expectedVersion }),
    }),
  delete: (id: string, expectedVersion: number) => request<void>(`${BASE}/${encodeURIComponent(id)}`, {
    method: 'DELETE',
    body: JSON.stringify({ expected_version: expectedVersion }),
  }),
  validate: (input: CustomPieceInput) => request<CustomPieceValidation>(`${BASE}/validate`, {
    method: 'POST',
    body: JSON.stringify(input),
  }),
  uploadImage: async (file: File) => request<CustomPieceImageAsset>('/api/custom-piece-images', {
    method: 'POST',
    body: JSON.stringify({
      filename: file.name,
      media_type: file.type,
      bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
    }),
  }),
  testOptions: (
    definition: CustomPieceTestDefinition,
    board: CustomPieceTestBoard,
    selectedPieceId: string,
  ) => request<CustomPieceTestResult>(`${BASE}/test/options`, {
    method: 'POST',
    body: JSON.stringify({ definition, board, selected_piece_id: selectedPieceId }),
  }),
  testAction: (
    definition: CustomPieceTestDefinition,
    board: CustomPieceTestBoard,
    action: TurnAction,
  ) => request<CustomPieceTestResult>(`${BASE}/test/actions`, {
    method: 'POST',
    body: JSON.stringify({
      definition,
      board,
      action: {
        ...action,
        type: 'from' in action ? 'move' : 'drop',
      },
    }),
  }),
}
