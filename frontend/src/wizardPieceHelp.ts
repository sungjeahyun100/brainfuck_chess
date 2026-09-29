// Browsing help for built-in Wizard pieces. Placed pieces use the engine's live move options.
export const wizardMovementDescriptions: Readonly<Record<string, string>> = {
  'wizard-king': '일반 킹처럼 주변 한 칸으로 이동합니다.',
  'wizard-queen': '일반 퀸처럼 가로·세로·대각선으로 이동합니다.',
  'wizard-rook': '일반 룩처럼 가로·세로로 이동합니다.',
  'wizard-knight': '일반 나이트처럼 ㄱ자로 뛰어 이동합니다.',
  'wizard-bishop': '일반 비숍처럼 대각선으로 이동합니다.',
  'wizard-cadet': '앞으로 한 칸 이동하거나 그 칸의 기물을 포획합니다. 상대 끝줄에서 마법사 퀸·나이트·비숍·룩으로 승급합니다.',
  'wizard-cadet-black': '앞으로 한 칸 이동하거나 그 칸의 기물을 포획합니다. 상대 끝줄에서 마법사 퀸·나이트·비숍·룩으로 승급합니다.',
}

export const wizardAbilityHelp: Readonly<Record<string, { id: string; name: string; description: string }>> = {
  'wizard-king': { id: 'encourage', name: '격려', description: '상하좌우에 아군 마법사 생도가 있으면 아군 마법사에게 이번 턴의 추가 이동 1회를 부여합니다. 턴을 사용하지 않습니다.' },
  'wizard-cadet': { id: 'linked-teleport', name: '연동되는 축지법', description: '자신을 제외한 아군 마법사 하나와 위치를 교환하고 턴을 마칩니다.' },
  'wizard-cadet-black': { id: 'linked-teleport', name: '연동되는 축지법', description: '자신을 제외한 아군 마법사 하나와 위치를 교환하고 턴을 마칩니다.' },
  'wizard-queen': { id: 'alekhines-gun', name: '알레킨의 총', description: '같은 직선의 아군 마법사 룩 두 개로 마법진을 구성합니다. 끝 룩 앞의 모든 아군·적군 기물을 제거하고 턴을 마칩니다.' },
  'wizard-rook': { id: 'transfer-circle', name: '전이 마법진', description: '바로 좌우에 아군 마법사 생도가 있으면 바로 뒤의 아군 기물을 보드 전체의 빈칸으로 순간이동시키고 턴을 마칩니다.' },
  'wizard-knight': { id: 'wizard-knight-catch', name: '퀸형 포획', description: '보드 내부의 모든 나이트 목적지가 마법사로 채워지면 8방향에서 처음 만나는 적을 포획하고 턴을 마칩니다.' },
  'wizard-bishop': { id: 'wizard-bishop-jump-catch', name: '도약 포획', description: '상하좌우가 마법사 생도로 채워지면 대각선의 첫 기물을 넘어 빈칸을 지나 다음에 만나는 적을 포획하고 턴을 마칩니다. 중간에 아군 기물이 있으면 막힙니다.' },
}
