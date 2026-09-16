# 마법사 킹·마법사 생도 구현 기록

작업일: 2026-09-14

## 기존 구조 조사와 재사용

| 조사 대상 | 기존 구조와 이번 적용 |
| --- | --- |
| Royal 판정 | `PieceDefinition.is_king`, `endgame::is_royal_piece`. 마법사 킹도 같은 플래그 사용 |
| 덱 필수·중복 제한 | `rules::validate_deck_with_ruleset`가 시작 기물의 `is_king`을 정확히 1개로 제한하고 포켓의 킹을 금지. 기존 서버 검증 재사용 |
| 분류 | `interaction::InteractionProfile`의 기존 기물별 등록 구조를 확장. `categories`에 `Wizard`, `WizardCadet` 태그 추가 |
| 프로모션 | `promotion` 조건과 `promotion_pool`을 분리하는 기존 정의 사용. 풀을 비우면 일반 이동을 막거나 일반 폰 풀로 대체하지 않음 |
| 능력 생성 | 기존 `MoveOptionDefinition`의 `StandaloneAction`과 `generate_piece_legal_ability_actions` 사용 |
| 대상 interaction | 서버의 대상별 `AbilityAction.target_piece_id`·`to`, 기존 ability 모드·보드 강조·클릭 제출 사용. ExtraSummon의 다중 희생 선택은 이번 단일 대상 선택에 필요하지 않음 |
| 턴 미소모 | 기존 Draw 조기 반환과 강제 착륙의 턴 유지가 있음. 새 Magic API 없이 `apply_and_advance_turn`의 분기만 확장 |
| 턴 종료 위치 | `endgame::apply_and_advance_turn`에서 history 기록, cooldown·비행 시간 처리 후 current_player/turn_number 변경 |
| 일시 상태 | 직렬화되는 기물별 `Piece.state: HashMap<String, PieceStateValue>` 사용 |
| 위치 교환 | 재사용 가능한 독립 swap action은 없어 기존 AbilityAction 적용 분기에 두 기물의 보드 점유·좌표를 함께 갱신. MoveAction을 생성하지 않음 |
| 저장·해시 | 기물 state가 기존 GameState serde, 서버 기보 delta, authoritative 분석 state_after, canonical JSON 해시 및 AI PositionKey에 이미 포함됨 |

## 규칙과 상호작용

- 마법사 킹은 기존 `king_definition()`에서 이동 레이어·옵션·Royal 플래그를 재사용한다. 캐슬링도 생성과 적용 양쪽에서 Royal 판정을 사용한다.
- 생도는 1점 Front 기물이다. 백/중립 ID `wizard-cadet`는 `take-move(0, 1);`, 흑 ID `wizard-cadet-black`는 기존 폰 계열 관례대로 반전된 `take-move(0, -1);`을 사용한다. 사용자에게는 같은 생도로 표시한다.
- Wizard 여부는 이름이나 ID 부분 문자열로 검사하지 않는다. 향후 새 마법사 기물은 interaction 등록부에 `Wizard` 분류를 붙이면 공통 대상 선택에 참여한다. Royal은 중복된 새 태그 대신 기존 `is_king`을 유지한다.
- 격려는 현재 위치의 상하좌우 네 칸 모두 아군 WizardCadet일 때만 canonical 후보를 생성한다. 자신을 포함한 아군 Wizard가 대상이다. UI는 서버 후보가 있을 때만 격려 버튼을 표시하며 배치 조건을 계산하지 않는다.
- 확정 시 대상의 `state["extra_move_remaining"] = Integer(1)`을 저장한다. 일반 `TurnAction::Move`를 실행할 때 해당 권리를 제거하고 현재 턴을 유지한다. `Ability`에 의한 좌표 변경은 소비 경로를 통과하지 않는다.
- 격려와 추가 이동도 각각 history에 남지만 턴 번호, 드로우, cooldown 및 비행 시간 경과는 진행하지 않는다. 정상 턴 종료 시 남은 가속을 제거한다. 기존 커스텀 기물의 같은 이름 상태는 가속으로 해석하거나 지우지 않는다.
- 축지법은 자기 자신을 제외한 아군 Wizard를 대상으로 한다. 두 기물의 ID·소유자·상태를 보존하며 좌표만 원자적으로 교환한다. 서로 다른 물리 레이어의 경우 도착 레이어에 제3의 기물이 있으면 거부한다.
- **swap 적용 자체에서는 가속이 유지되고, 그 뒤 실제 턴 종료에서 만료된다.** 따라서 턴을 소비하는 축지법을 완전히 확정한 다음 상대 턴에 가속이 남지는 않는다.
- 대상 ID·좌표·소유권·기물 종류·현재 조건은 서버의 기존 canonical 후보 일치 검증에서 다시 확인한다. 오래된 UI 후보나 변조된 payload는 실행되지 않는다.
- 봇은 기존 턴 묶음/timeline 처리로 무료 행동 뒤의 행동도 이어서 실행한다. 후속 행동에는 기존 전술 정렬을 쓰며, 무료 격려만 무한히 재사용하지 않도록 후속 후보에서 격려를 제외한다. 사람의 합법 행동에는 이 봇 정책을 적용하지 않는다.

## 보류·의도적인 제한

- 마법사 퀸·나이트·비숍·룩은 만들지 않았다. `wizard.rs`의 `WIZARD_PROMOTION_POOL`에 예정 ID 네 개를 모아 두었고, 실제 생도의 pool은 비어 있다. 후속 구현에서 등록된 기물만 이 pool에 연결해야 한다.
- 격려의 턴당 재사용 횟수는 요구사항에 지정되지 않아 추가 제한을 만들지 않았다. 효과는 1회로 갱신되며 중첩 합산되지 않는다. 진형이 유지되면 이동권을 소비한 후 다시 격려할 수 있다.
- 기물 그림은 첨부된 왕관·초승달 디자인을 옮긴 마법사 킹 SVG와 고깔모자·초승달 디자인의 생도 SVG를 사용한다. 백·흑 전용 자산을 각각 제공하며, 과거 저장된 일반 킹·폰 자산 참조도 표시 단계에서 새 그림으로 연결한다. 가속 표시는 기존 작은 보드 배지 스타일의 ⚡이다.

## 변경 파일과 이유

- `engine/src/pieces/default_pieces/wizard.rs`, `default_pieces.rs`: 두 종류의 기물 정의, 흑 방향 정의, 능력·프로모션 예정 ID 및 등록.
- `engine/src/interaction.rs`: Wizard 분류, 아군 후보와 네 방향 조건.
- `engine/src/legal_moves.rs`: canonical 능력 후보 생성.
- `engine/src/endgame.rs`: 가속·턴 수명, atomic swap, Royal 캐슬링 적용.
- `engine/src/ai/search.rs`: 같은 턴의 후속 행동을 기존 봇 timeline에 포함.
- `engine/src/ai/transposition_table.rs`: 가속 상태의 위치 키 회귀 테스트.
- `engine/tests/wizard.rs`, `engine/tests/rule_engine.rs`: 새 규칙과 기존 전체 기물 배치 회귀 검사.
- `server/src/main.rs`: 서버 덱 입력의 새 기물 ID 허용 및 흑 방향 해석.
- `server/src/game_record.rs`: 기보 직렬화·ply별 복원·상태 해시 검증 테스트.
- `frontend/src/composables/useDeckValidation.ts`: 기존 카탈로그와 Royal 분류를 사용하는 덱 편집기 검증.
- `frontend/src/composables/useDeckCode.test.ts`: 새 기물의 덱 공유 코드 왕복과 Royal 배타 검사.
- `frontend/src/components/GameScreen.vue`, `Board.vue`: 서버 후보에 따른 격려 표시, 기존 대상 클릭, 가속 배지.
- `frontend/src/pieceAssets.ts`: 기존 자산 연결.
- `frontend/src/standardGameUi.test.ts`: 서버 후보가 버튼 표시를 결정하는 UI 테스트.
- `frontend/src/updateLog.ts`: 새 고유 ID `2026-09-14.3`으로 사용자 변경 안내.
- 이 문서: 조사 내용, 적용 계약, 보류 사항과 검증 결과 기록.

## 검증 결과

- `cargo test -p brainfuck-chess-engine --test wizard --quiet`: 13개 통과. Royal 배타 조건(두 형식), 네 방향 조건, 변조·stale 대상, 자기 대상 규칙, 가속 소비·만료, atomic swap, canonical replay/serde, 생도 이동·승급 보류, 흑 방향, 캐슬링, 봇 연속 행동, 기존 커스텀 상태 보존을 검사했다.
- `cargo test -p brainfuck-chess-server wizard --quiet`: 2개 통과. 서버 카탈로그·색상 해석과 기보의 세 행동, ply별 저장/복원·state hash를 검사했다.
- `cargo test --workspace --no-fail-fast --quiet`: 새 기능과 배치 회귀 검사는 통과. 기존 `engine/tests/ai.rs`의 `g8a::legal_actions_include_hand_abilities_and_bounded_exact_summons` 1개가 실패했다. 수정 전 HEAD를 임시 디렉터리에 추출해 같은 테스트를 실행했고 동일하게 실패했다. 기존 테스트의 교대병 주변에는 교대 대상이 될 수 없는 킹만 있어 Ability 후보가 없는데, 테스트는 후보가 있다고 기대한다. 이번 작업에서는 해당 테스트나 규칙을 변경하지 않았다.
- `npm test`: 지정된 프론트엔드 테스트 파일 23개 모두 통과. 주요 파일을 직접 실행해서도 덱 코드 17개, 게임 UI 22개, 업데이트 로그 7개 테스트가 통과하는 것을 확인했다.
- `cargo build --workspace --quiet`: 통과. 기존 dead_code 경고가 있다.
- `npm run build`: vue-tsc와 Vite 빌드 통과. 번들 크기 500kB 경고가 있다. 별도 lint 스크립트도 동일한 vue-tsc 검사이므로 중복 실행하지 않았다.
- `git diff --check`: 통과.
- 외부 PostgreSQL 등 별도 환경을 요구하는 ignored 테스트 및 수동 브라우저 조작 검증은 수행하지 않았다. 배포하지 않았다.


## 2026-09-14 후속 수정: 실험실 상태 키 등록

기물 실험실은 응답의 기물 상태를 다음 옵션 조회·행동 요청에 다시 담으며, `build_lab_game_state`가 정의의 `state_schema`로 키와 타입을 검사한다. 최초 구현에서 가속 키를 실제 상태에만 저장하고 스키마에 등록하지 않아 이 경로가 실패했다.

마법사 킹과 생도 정의의 스키마에 `extra_move_remaining: Integer(0)`을 등록했다. 흑 생도는 생도 정의를 상속해 같은 스키마를 사용한다. 알 수 없는 키나 잘못된 타입을 거부하는 검증은 그대로 유지한다.

`lab_wizard_acceleration_survives_followup_requests`는 백·흑의 킹·생도 각각에 대해 실험실 API의 격려 응답 → 옵션 조회 → 추가 이동 및 축지법 확정 요청을 검사하며 통과했다. 업데이트 로그 `2026-09-14.4`에도 오류 수정을 별도로 기록했다.

## 2026-09-14 후속 추가: 마법사 퀸·룩과 알레킨의 총

- `wizard-queen`은 기존 Queen 정의의 이동·시각 자산·9점을, `wizard-rook`은 Rook 정의의 이동·시각 자산·5점을 재사용한다. 두 기물 모두 `Wizard`, 룩은 추가로 `WizardRook` interaction 분류를 갖는다. 가속 상태 스키마도 등록한다. 생도의 기존 승급 보류를 해제하여 실제 구현된 퀸·룩만 승급 풀에 연결했다. 나이트·비숍은 계속 보류한다.
- 새 Magic API 없이 기존 `alekhines-gun` standalone ability와 `AbilityAction.piece_id` / `target_piece_id` / `to`를 사용한다. UI는 서버가 생성한 후보가 있을 때만 버튼을 표시하고 기존 능력 대상 강조·클릭 제출을 재사용한다. 별도 전용 프리뷰는 추가하지 않았다.
- 엔진은 퀸에서 네 직교 방향으로 탐색한다. 빈칸은 건너뛰고, 첫 두 점유 칸이 각각 같은 소유자의 마법사 룩 한 개일 때 두 번째 룩만 끝 룩으로 제공한다. 지상·공중 모든 레이어의 기물이 장애물이며, 한 칸에 두 기물이 겹치면 통과하지 않는다. 세 번째 룩은 마법진 참여자가 아니라 공격 대상이다.
- 서버의 canonical 후보 일치 검증이 현재 소유권·기물 종류·위치·끝 룩·장애물·쿨다운·턴 제약을 재검증한다. 클라이언트가 중간 룩이나 제거 대상을 지정하지 않는다.
- 적용 시 끝 룩의 다음 칸부터 보드 끝까지 지상·공중 제거 대상을 계산하고 기존 `remove_captured_piece`와 `apply_removal_result`를 사용한다. 아군·적군 및 Royal을 같은 경로로 제거한다. 양측 Royal이 동시에 제거되는 경우에도 기존 제거 결과 정책을 그대로 따른다.
- 능력은 기존 기본 정책에 따라 한 턴을 사용한다. 가속 이동 소비 분기에 진입하지 않으며 실제 턴 종료 단계에서 가속이 만료된다. 전체 공격은 history에 단 하나의 ability action으로 남는다.
- Chessembly 문법과 기존 기물 이동·공개 action 형식은 변경하지 않았다. 퀸·룩의 행마법 일치 회귀 테스트를 추가했다. 기존 작업 트리의 마법사 킹·생도 관련 변경은 보존했다.

변경 위치: `engine/src/pieces/default_pieces{.rs,/wizard.rs}`(정의·등록), `interaction.rs`(분류·마법진), `legal_moves.rs`(후보), `endgame.rs`(원자적 제거), `server/src/main.rs`(덱 ID), `frontend/src/components/GameScreen.vue`(서버 후보 기반 표시). 관련 엔진·서버 기보·UI 테스트와 업데이트 로그 `2026-09-14.6`을 함께 추가했다.

검증:

- `cargo test -p brainfuck-chess-engine --test wizard --quiet`: 19개 통과. 네 방향·간격·장애물·진영·변조·오래된 후보·대각선·분리된 룩·쿨다운·빈 공격선·세 번째 룩 제거·양 레이어·가속·Royal·serde 및 canonical replay 검사.
- `cargo test -p brainfuck-chess-server wizard --quiet`: 4개 통과. 새 기물 카탈로그와 ability 기보 저장/복원, state_after 및 state hash, Royal 결과 검증 포함.
- `cargo test --workspace --no-fail-fast --quiet`: 기존 문서에 기록된 `g8a::legal_actions_include_hand_abilities_and_bounded_exact_summons` 1개 실패. 나머지 실행된 테스트 통과. 외부 환경 등을 요구하는 ignored 테스트는 실행하지 않았다.
- `npm test`: 업데이트 로그 및 능력 버튼의 서버 후보 표시 검사를 포함한 23개 테스트 파일 통과.
- `npm run build`: 타입 검사·프론트엔드 빌드 통과. 기존 번들 크기 경고가 남아 있다.
- `cargo build --workspace --quiet`: 통과.
- `git diff --check`: 통과. 수동 브라우저 검증 및 배포는 수행하지 않았다.

## 2026-09-16 수정: 프론트엔드 카탈로그 등록 누락

앞선 퀸·룩 구현에서 엔진 등록과 서버 ID 검증은 완료했지만, `useDeckValidation.ts`의 `builtInPieceCatalog` 등록이 누락되어 덱 편집기와 Piece Lab의 공유 선택 목록에 두 기물이 표시되지 않았다. 서버 metadata 적용은 이미 프론트엔드 목록에 있는 항목만 갱신하므로 엔진 등록만으로 화면에 추가되지 않는다. 앞선 검증은 이 화면 진입 경로를 다루지 못했다.

- `frontend/src/composables/useDeckValidation.ts`: 퀸·룩을 기존 카탈로그에 등록. 점수·배치 구역은 기존 서버 metadata 적용 경로 사용.
- `frontend/src/pieceAssets.ts`: 카탈로그가 기물 ID로 요청하는 그림에 기존 퀸·룩의 백·흑 자산 연결.
- `frontend/src/views/DeckEditor.test.ts`: 실제 컴포넌트의 두 규칙 모드에서 이름 검색 → Back 배치 → 포켓 추가 검증. 수정 전 검색 결과가 빈 배열인 실패를 재현했고 수정 후 통과.
- `frontend/src/composables/useDeckCode.test.ts`: 서버 점수·배치 정보 적용 및 퀸·룩이 들어간 덱 코드의 실제 import 검증.
- `frontend/src/updateLog.ts`: 새 항목 `2026-09-16.1` 기록.

`npm test` 23개 파일과 `npm run build`(타입 검사 포함), `git diff --check` 통과. 기존 번들 크기 경고는 남아 있다. 게임 규칙·Chessembly·서버 동작은 변경하지 않았다. 사용자 브라우저에서의 수동 확인이나 사용자 프로세스 재시작은 수행하지 않았다.

## 2026-09-16 추가: 마법사 룩의 전이 마법진

사용자 그림과 후속 확인에 따라 룩의 바로 좌우 아군 생도 두 개가 뒤쪽 아군 기물 하나를 전체 보드의 빈칸으로 순간이동시키는 `transfer-circle` 능력을 추가했다. 사용 후 턴을 종료한다. 뒤는 소유자 전진 방향의 반대로 계산하여 흑에서는 rank가 증가하는 쪽이다. 좌우 생도는 룩과 같은 물리 레이어에 있어야 한다.

- 기존 Piece Definition, interaction 분류, canonical AbilityAction, 서버 재검증, 턴 처리 및 UI를 재사용했다. payload는 룩 ID, 뒤쪽 아군 기물 ID, 도착 좌표로 구성된다. 오래된 배치·적군·생도가 아닌 좌우 기물·점유 칸·범위 밖 좌표·다른 대상 ID·쿨다운을 제출하면 거부한다.
- 대상은 왕·일반 기물·마법사·공중 기물 모두 허용한다. 뒤쪽 칸의 두 레이어에 아군이 각각 있을 때는 기존 복수 후보 선택창으로 이동할 기물을 선택한다. 도착 칸은 지상·공중 모두 비어 있어야 한다.
- 기물 ID·종류·소유자·탄약·레이어를 유지한다. 순간이동 자체로 승급·포획하지 않는다. `has_moved`를 기록하여 폰 첫 두 칸 이동과 킹·룩 캐슬링 권리가 남지 않게 한다. 가속 추가 이동권을 소비하지 않으며 기존 턴 종료 단계에서 가속 만료·비행 시간·쿨다운이 처리된다.
- 게임 화면에서 유효한 서버 후보가 있을 때 전이 마법진 버튼을 표시한다. 클릭하면 빈 목적지를 강조하며 선택 시 하나의 능력으로 제출한다. 기존 기물 실험실의 능력 조회·실행 API에서도 같은 검증을 사용한다.

수정 파일: `engine/src/pieces/default_pieces/wizard.rs`와 등록 모듈(능력 정의), `interaction.rs`(배치·대상 탐색), `legal_moves.rs`(빈칸 후보), `endgame.rs`(순간이동 적용), `frontend/src/components/GameScreen.vue`(버튼·후보 선택), 관련 엔진·실험실 API·화면 테스트. 업데이트 로그 `2026-09-16.2`에 사용자 규칙을 기록했다. 기존 Chessembly 문법과 알레킨의 총·격려·축지법은 유지했다.

검증: `cargo test -p brainfuck-chess-engine --test wizard --quiet` 23개 통과, `cargo test -p brainfuck-chess-server wizard --quiet` 5개 통과. 8~12칸 보드·두 규칙 모드·백흑 배치·변조 거부·Royal 이동·레이어·가속·직렬화 재생·상태 해시 및 실제 UI의 목적지 선택 제출을 검사했다. `npm test` 23개 파일 및 `npm run build`(타입 검사 포함) 통과. 수동 브라우저 조작은 수행하지 않았다.

최종 검증: `cargo build --workspace --quiet`, `git diff --check` 통과. `cargo test --workspace --no-fail-fast --quiet`는 기존 `g8a::legal_actions_include_hand_abilities_and_bounded_exact_summons` 1개가 동일하게 실패했고 나머지 실행된 테스트는 통과했다. ignored 테스트는 실행하지 않았다. 기존 Rust 경고와 프론트엔드 번들 크기 경고가 남아 있다.

## 2026-09-16 디자인: 마법사 퀸·룩 전용 SVG

기존 마법사 SVG의 100×120 좌표계·검은 외곽선·초승달 형태를 재사용했다. 퀸은 별 문양의 뾰족한 왕관, 룩은 별 문양 성탑으로 구분하며 백·흑 SVG 4개를 추가했다. 래스터 생성이나 새 의존성은 사용하지 않았다.

- `frontend/src/assets/pieces/wizard-{queen,rook}-{white,black}.svg`: 전용 원본 자산.
- `frontend/src/pieceAssets.ts`: 덱 편집기·실험실·보드에서 쓰는 기존 자산 등록.
- `engine/src/pieces/default_pieces/wizard.rs`: 전용 visual asset key 지정.
- `frontend/src/pieceVisual.ts` 및 기존 테스트: 이전 기보의 일반 queen/rook 자산 키를 전용 그림으로 표시. 별도 사용자 자산은 보존.
- `frontend/design-previews/wizard-family.png`: 킹·퀸·룩·생도 순서의 백/흑 비교 렌더링. 형태와 여백을 시각 검수했다.
- `frontend/src/updateLog.ts`: 업데이트 로그 `2026-09-16.3` 추가.

`npm test`(업데이트 로그 포함 23개 파일), `npm run build`(타입 검사 포함), `cargo test -p brainfuck-chess-engine --test wizard --quiet`(23개), `cargo build --workspace --quiet`, `git diff --check` 통과. 기존 dead_code와 번들 크기 경고는 남아 있다. 게임 규칙·점수·능력·Chessembly·저장 형식 변경 없음. 수동 브라우저 확인은 수행하지 않았다.
