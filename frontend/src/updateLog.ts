export interface UpdateLogEntry {
  id: string
  date: string
  title: string
  changes: readonly string[]
}

// Add releases at the beginning and give each release a new, stable ID.
export const updateLog: readonly UpdateLogEntry[] = [
  {
    id: '2026-09-09.1',
    date: '2026-09-09',
    title: '덱 저장 개선과 업데이트 안내',
    changes: [
      '로그인하면 계정에 덱을 저장하고 불러올 수 있습니다. 게스트 덱은 브라우저에 별도로 보관됩니다.',
      '덱을 저장한 뒤에도 편집 화면과 작업 상태가 유지됩니다.',
      '신규 변형 기물의 이동 설명을 보강했습니다.',
      '로비에서 업데이트 로그를 확인할 수 있습니다. 새 업데이트 이후 처음 방문하면 변경 내용을 안내합니다.',
    ],
  },
]

export const UPDATE_LOG_STORAGE_KEY = 'deck_chess_last_seen_update'
type UpdateStorage = Pick<Storage, 'getItem' | 'setItem'>
type StorageProvider = () => UpdateStorage

export function readUpdateStatus(
  releaseId: string,
  storage: StorageProvider = () => window.localStorage,
): 'read' | 'unread' | 'unavailable' {
  try {
    return storage().getItem(UPDATE_LOG_STORAGE_KEY) === releaseId ? 'read' : 'unread'
  } catch {
    return 'unavailable'
  }
}

export function acknowledgeUpdate(
  releaseId: string,
  storage: StorageProvider = () => window.localStorage,
): 'saved' | 'unavailable' {
  try {
    storage().setItem(UPDATE_LOG_STORAGE_KEY, releaseId)
    return 'saved'
  } catch {
    return 'unavailable'
  }
}
