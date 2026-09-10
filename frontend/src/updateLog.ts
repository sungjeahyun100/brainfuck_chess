export interface UpdateLogEntry {
  id: string
  date: string
  title: string
  changes: readonly string[]
}

// Add releases at the beginning and give each release a new, stable ID.
export const updateLog: readonly UpdateLogEntry[] = [
  {
    id: '2026-09-10.1',
    date: '2026-09-10',
    title: 'Standard 룰 추가와 대국 화면 개선',
    changes: [
      '덱 편집과 대국에 Standard 룰을 추가했습니다. 기존 Legacy 덱과 룰도 계속 사용할 수 있습니다.',
      'Standard 덱은 Front·Back 구역에 맞춰 시작 기물을 배치하며, 전용 프리셋과 배치 검사를 제공합니다.',
      'Standard에 손패(Hand)와 자동 드로우를 추가했습니다. 일반 착수는 손패에서 기물을 선택하고, 포켓은 드로우와 관련 능력에 사용합니다.',
      'Standard 덱에 최대 3기의 Extra Deck을 구성할 수 있습니다. 소환할 기물과 제물, 배치할 칸을 선택해 특수 소환하며, Extra 기물은 Main Deck 점수 상한에 포함되지 않습니다.',
      '봇 플레이에서도 Standard 손패 착수와 Extra 소환을 지원합니다.',
      'Standard 대국의 드로우와 특수 소환을 기보에 기록합니다. 완료된 대국을 복기할 때 양측 손패와 드로우 내역을 확인하고 분석할 수 있습니다.',
      '덱 저장과 덱 코드 공유에 룰 및 Extra Deck 정보를 반영했습니다. 기존 덱과 덱 코드도 계속 불러올 수 있습니다.',
      '덱 편집에서 기물을 선택한 뒤 포켓 영역을 클릭해 추가할 수 있습니다.',
      '대국 중 상대 Extra Deck의 기물·점수·잔여 수량과 포켓 내용, 덱 점수를 숨겼습니다. 로컬 2인은 양측 손패가 보이지만 덱 내용은 현재 차례만 확인할 수 있으며, 봇전에서는 상대 손패도 장수만 표시합니다.',
      'Standard와 Legacy 모두 양측 패를 보드 오른쪽에 모았습니다. 화면 크기에 맞춰 보드를 조절해 초기 화면에 보드·패·기보·조작 버튼이 들어오도록 개선했으며, 긴 목록은 패널 안에서 스크롤됩니다.',
    ],
  },
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
