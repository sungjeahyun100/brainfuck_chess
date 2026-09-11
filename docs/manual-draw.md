# Standard 직접 드로우

## 기존 구조와 적용 범위

Standard의 `pocket_pieces`는 뽑기 전 덱이며, `hand_pieces`가 일반 Drop에 사용하는 손패다. Legacy의 포켓 Drop과 Chessembly 문법은 변경하지 않는다. 서버의 기존 초기 손패 지급(양측 3장), Hand 이동 검증, 게임 접근 권한, 시계, 기록의 DrawResolution을 재사용한다.

## 상태와 책임

새 Standard 대국은 `global_state.manual_draw_v1 = 1`로 식별한다. 실제 대기 여부는 서버/엔진이 관리하는 `global_state.draw_required`의 1/0이다. `engine/src/actions.rs::prepare_draw`가 초기 지급 이후와 턴 전환 시 이 값을 설정한다. 덱이 비었으면 즉시 0으로 설정한다. 새 규칙에서는 이 두 키를 게임 수명주기 전용으로 보존하므로 커스텀 기물의 전역 상태 효과로 덮어쓸 수 없다. 기존 규칙 기록의 전역 상태 동작은 유지한다.

클라이언트 요청은 `{ "type": "draw", "turn_number": 1 }`뿐이다. 서버는 기존 접근 권한으로 플레이어를 결정하고 현재 턴 번호와 대기 여부를 검증한다. 알 수 없는 필드와 임의의 결과는 받지 않는다. 턴 번호는 다음 턴까지 늦게 도착한 재전송도 차단한다. 게임 잠금 안에서 복제 상태에 드로우를 계산하고 시계 만료를 확인한 뒤 확정하므로 오류나 중복 요청이 추가 카드를 지급하지 않는다.

Canonical `TurnAction::Draw`에는 플레이어와 턴 번호를 기록한다. 엔진은 드로우 전 다른 행동을 `DRAW_REQUIRED`로 거부한다. Draw는 history를 추가하고 대기를 해제하며 턴·쿨다운·비행 시간을 진행하지 않는다. 실제 카드 선택과 Pocket→Hand 이동은 서버가 수행한다. 별도 End Turn API는 없으며 일반 행동이 기존 규칙에 따라 턴을 끝낸다.

## 기록과 봇

신규 기록의 규칙 버전은 `deck-chess-standard-2`다. JSON 포맷 2는 유지한다. 드로우 결과의 실제 ID는 기존 recorded action의 `draws`에 붙이고, live Draw intent에는 노출하지 않는다. 완료 기보와 분석 분기는 저장된 resolution을 적용해 RNG 없이 재현한다. 분석 저장소의 기존 잠금과 요청 ID 검증 이후에만 새 분기 드로우를 확정한다. 미확정 Draw를 분석의 pending action으로 넘기는 것은 거부한다.

`deck-chess-standard-1`과 Legacy 기록도 계속 읽는다. 수동 드로우 표식이 없는 기존 Standard 스냅샷에는 기존 자동 드로우 재생 경로를 적용한다. 과거 자동 드로우 회귀 테스트는 명시적인 v1 fixture로 보존했다.

봇은 서버에서 Draw를 실행한 뒤 확정된 손패로 기존 탐색을 수행한다. Draw와 뒤따르는 수를 순서대로 기록하고 타임라인에 포함한다. 탐색 중 가상의 미래 드로우 RNG를 실행하지 않는 기존 방침은 유지한다.

## UI와 변경 파일

- `engine/src/types.rs`, `actions.rs`, `endgame.rs`: 행동, 검증, 턴 대기. `ai/*`: 새 canonical 행동을 타입 변환·정렬에서 처리.
- `server/src/draw.rs`, `main.rs`: 초기화, 권한 있는 요청, 봇 드로우.
- `server/src/game_record.rs`, `analysis.rs`: 규칙 버전과 결정된 드로우의 기록·재생.
- `server/src/game_view.rs`, `time_control.rs`: 비공개 ID 없이 덱 장수 전달.
- `frontend/src/components/GameScreen.vue`, `StandardReservePanel.vue`: 내 덱을 먼저 표시, 턴 배너, 직접 가이드, 보드·손패·능력 잠금, 가로 스크롤 카드와 간단한 등장 효과.
- 프론트엔드 게임 타입/API/기보 codec/notation/ReplayPage: Draw 전송·검증·표시와 분석 UI. 테스트장 두 화면과 서버 custom_piece 경계에서는 Draw를 지원하지 않는 상태임을 명시.
- 관련 서버/프론트엔드 테스트 및 `frontend/src/updateLog.ts`: 새 흐름과 기존 규칙 회귀 검증, 사용자 안내.

외부 데이터베이스 스키마, 계정, 덱 저장 형식과 멀티플레이 방 구조는 변경하지 않는다.
