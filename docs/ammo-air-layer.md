# 탄약 및 공중 레이어 기물 계약

이 문서는 탄약을 사용하는 기물과 `Air Layer` 기물을 추가할 때 사용하는
현재 엔진 계약을 정리한다. 기준 구현은 `engine/src/types.rs`,
`engine/src/legal_moves.rs`, `engine/src/endgame.rs`이며 일반 플레이와 기물
테스트장은 동일한 canonical action 검증·적용 경로를 사용한다.

## 정의 필드

### `PieceDefinition.max_ammo`

| JSON/Rust 필드 | 타입 | 기본값 | 의미 |
|---|---:|---:|---|
| `max_ammo` | `u32` | `0` | 기물 인스턴스의 최대 탄약. `0`이면 탄약 자원을 사용하지 않는다. |

새 기물 인스턴스는 `current_ammo = max_ammo`로 초기화된다. `max_ammo`가
0인 기물도 탄약을 소비하지 않는 능력은 정상적으로 사용할 수 있다.

### `MoveOptionDefinition` 추가 필드

| JSON/Rust 필드 | 타입 | 기본값 | 의미 |
|---|---:|---:|---|
| `ammo_cost` | `u32` | `0` | 해당 옵션을 commit할 때 소비할 탄약 |
| `enabled_when` | `PieceStatePredicate[]` | `[]` | 구체 기물의 `state`가 모두 만족해야 옵션 활성화 |

옵션은 쿨타임과 상태 조건을 만족하고 `current_ammo >= ammo_cost`일 때만
합법 행동을 생성한다. 탄약은 미리보기나 합법수 조회가 아니라 canonical
action commit 시점에만 소비한다.

## 런타임 필드

### `Piece`

| JSON/Rust 필드 | 타입 | 기본값 | 의미 |
|---|---:|---:|---|
| `current_ammo` | `u32` | `0` | 현재 남은 탄약 |
| `layer` | `"ground" \| "air"` | `"ground"` | 현재 실제로 점유하는 보드 레이어 |
| `remaining_flight_turns` | `u32` | `0` | 소유자 기준 남은 공중 행동 턴 |

한 기물은 한 시점에 하나의 레이어에만 존재한다. `layer = "air"`인 기물을
Ground Board에도 중복 등록하면 안 된다. 폭격기의 `airborne` 값은 모든
기물에 공통인 필드가 아니라 폭격기 `state_schema`에 선언된 인스턴스 상태다.

### `Board`

| JSON/Rust 필드 | 타입 | 기본값 | 의미 |
|---|---|---|---|
| `squares` | `Record<SquareId, PieceId \| null>` | 필수 | Ground Layer 점유 |
| `air_squares` | `Record<SquareId, PieceId \| null>` | `{}` | Air Layer 점유 |

두 레이어는 같은 좌표계를 사용하며 같은 좌표에 지상 기물 하나와 공중 기물
하나가 동시에 존재할 수 있다. 이동, 충돌, 포획은 기본적으로 행동하는 기물의
레이어에서만 계산한다. 다른 레이어에 영향을 주는 폭격 같은 능력만 예외다.

이전 저장 상태에 `air_squares`, `current_ammo`, `layer`,
`remaining_flight_turns`가 없으면 각각 빈 Air Layer, `0`, `ground`, `0`으로
역직렬화된다.

## 탄약 소비와 재보급

- 게임 시작, 포켓 기물 생성 및 타입 초기화 시 현재 탄약을 최대 탄약으로 설정한다.
- `ammo_cost > 0`인 능력은 탄약이 부족하면 선택하거나 실행할 수 없다.
- 탄약 기물이 Ground Layer에서 자기 진영 밖에서 안으로 진입하면 최대치로 재보급한다.
- 자기 진영 안에서 마지막 탄약을 소비해 `0`이 된 경우에도 즉시 최대치로 재보급한다.
- Air Layer에서는 진영 좌표 위에 있더라도 재보급하지 않는다. 실제로 착륙해야 한다.
- UI는 `max_ammo > 0`인 기물에 현재 탄약을 표시하며 `0`도 숨기지 않는다.

## 공중 턴과 강제 착륙

- 이륙 행동 자체는 비행 지속시간에 포함하지 않는다.
- 이후 소유자의 공중 행동 턴이 끝날 때만 `remaining_flight_turns`를 1 감소시킨다.
- 상대 턴에는 감소하지 않는다.
- 값이 `0`이 되면 다른 일반 행동을 허용하지 않고 강제 착륙 선택을 즉시 활성화한다.
- 적 진영 위이거나 유효한 착륙로가 없으면 즉시 추락하여 제거한다.
- 유효한 착륙로가 여러 개면 플레이어가 하나를 선택한다.

## 기본 기물 정의

### 탱크 (`tank`)

- 점수 `12`, `max_ammo = 3`, Ground Layer 기물
- 기본 이동: 직교 방향으로 `take-move`를 연속 두 번 실행
- `tank-fire`: 8방향 조준, 최초 기물 칸까지만 선택 가능
- 발사당 탄약 `1`, 소유자 턴 기준 쿨타임 `1`
- 착탄점과 직교 인접 4칸의 Ground Layer 기물을 진영과 관계없이 제거

### 폭격기 (`bomber`)

- 점수 `13`, `max_ammo = 3`
- 지상 이동: 직교 1칸
- `takeoff`: Ground Layer의 연속 5칸 활주로를 사용해 정확히 5칸 이동하고
  Air Layer로 전환한다. 탄약과 쿨타임은 없다.
- 공중 이동: Air Layer에서 퀸 행마. Ground Layer 기물은 경로를 막지 않는다.
- `bomb`: 탄약 `1`을 사용하며 폭격기 자신의 칸을 눌러 실행한다. 현재 좌표와
  직교 인접 4칸의 Ground Layer 기물을 진영과 관계없이 제거한다.
- 비행 지속시간: 이륙 이후 소유자 공중 행동 5회
- `forced-landing`: 카운트가 `0`이 되면 즉시 활성화된다. 8방향 중 Ground와
  Air Layer 모두 연속 4칸이 빈 방향을 고르고 정확히 4칸 떨어진 지상 칸에 착륙한다.
- 자기 진영 착륙 시 최대 탄약으로 재보급한다.

### 지대공 미사일 (`surface-to-air-missile`)

- 점수 `2`, `max_ammo = 2`, 앞줄 배치 기물
- White는 전방 대각선 1칸과 전방 연속 2칸, Black은 이를 상하 반전한 행마
- `intercept`: 자신의 랭크와 앞뒤 1개 랭크, 좌우 2개 파일이 만드는 5×3 범위의
  적 Air Layer 기물 하나를 선택해 제거한다.
- 발사당 탄약 `1`, 쿨타임 없음

## UI와 기물 테스트장

- 같은 진영의 지상·공중 기물이 같은 좌표에 있으면 선택창에서 레이어별 기물을 고른다.
- 상대 공중 기물은 지상 칸 클릭을 가로채지 않는다. 아래 지상 기물 선택과 지상 이동을 우선한다.
- 폭격은 능력 선택 후 폭격기 자신의 칸을 눌러 확정한다.
- 기물 테스트장은 `/api/lab/piece-options`로 canonical 후보를 조회하고
  `/api/lab/apply-action`으로 실제 엔진 전이를 적용한다. 로컬 전용 능력 모사를 추가하지 않는다.

## 검증 위치

- 엔진 회귀 테스트: `engine/tests/ammo_air_layer.rs`
- 테스트장 엔진 경계 테스트: `server/src/main.rs`의 `lab_*` 테스트
- 프런트 능력 목표 테스트: `frontend/src/moveOptionUi.test.ts`
