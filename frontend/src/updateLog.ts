export interface UpdateLogEntry {
  id: string
  date: string
  title: string
  changes: readonly string[]
}

// Add releases at the beginning and give each release a new, stable ID.
export const updateLog: readonly UpdateLogEntry[] = [
  {
    id: '2026-09-11.4',
    date: '2026-09-11',
    title: '상대 손패 위치와 카드 뒷면 표시',
    changes: [
      '상대 손패와 시계를 보드 위로 옮기고, 내 손패는 보드 아래에 유지했습니다.',
      '상대 손패는 기물 정보 대신 카드 뒷면과 총 장수로 표시해 수량을 확인할 수 있습니다.',
    ],
  },
  {
    id: '2026-09-11.3',
    date: '2026-09-11',
    title: '보드 주변 덱 배치와 손패 가독성 개선',
    changes: [
      'Standard 대국에서 내 덱을 보드 우하단, 상대 덱을 좌상단으로 옮겼습니다.',
      '양측 손패를 보드 아래에 가로로 표시하고, 시간과 플레이어 정보를 각 손패에 함께 배치했습니다.',
      '보드·덱·손패·시간·기보를 한 화면에서 확인하기 쉽도록 화면 크기에 맞춰 배치를 조정했습니다.',
    ],
  },
  {
    id: '2026-09-11.2',
    date: '2026-09-11',
    title: '덱 클릭 드로우와 카드형 손패',
    changes: [
      'Standard 대국의 매 턴 자동 드로우 1장을 덱을 직접 클릭해 1장 뽑는 방식으로 변경했습니다. 시작 손패는 양측 3장으로 유지되며, 드로우 전에는 이동·착수·능력·특수 소환을 할 수 없습니다.',
      '내 턴 안내와 덱 강조, 남은 카드 수를 추가했습니다. 덱이 비면 드로우 단계를 자동으로 건너뜁니다.',
      '손패와 포켓을 기물 이미지·이름·점수가 보이는 카드로 표시하고 가로 스크롤을 지원합니다.',
      '봇은 턴 시작에 드로우 행동을 실행하며, 기보에는 드로우와 확정 결과가 기록됩니다. 기존 자동 드로우 기보와 Legacy 규칙은 계속 지원합니다.',
    ],
  },
  {
    id: '2026-09-11.1',
    date: '2026-09-11',
    title: '포탄 기물 디자인 변경',
    changes: [
      '백·흑 포탄 기물을 현대 전차용 고폭탄 느낌의 길쭉한 탄두로 변경했습니다. 금속 신관과 노란 식별 띠를 더해 포탄의 형태를 알아보기 쉽게 했습니다.',
    ],
  },
  {
    id: '2026-09-10.2',
    date: '2026-09-10',
    title: 'Standard 덱 점수 상한 증가',
    changes: [
      'Standard Main Deck의 점수 상한을 모든 보드에서 20점씩 올렸습니다: 8×8은 39→59점, 9×9는 56→76점, 10×10은 75→95점, 11×11은 96→116점, 12×12는 119→139점입니다.',
      '덱 편집 화면의 상한 표시와 게임 시작 시 덱 검증에 새 상한을 적용했습니다. Legacy 덱 점수 상한과 Extra Deck 규칙은 그대로 유지됩니다.',
    ],
  },
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
