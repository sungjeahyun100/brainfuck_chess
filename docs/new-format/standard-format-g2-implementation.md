# G2 — Standard Base/Home Zone 및 배치 규칙 구현 결과

작성일: 2026-09-10 (KST). G1 완료 작업 트리 위에 G2만 추가했다. 기존 미커밋 G1 변경사항은 보존했다. 현재 구현의 기준은 G1 코드이며, G0의 초기 배치에만 변경을 제한하자는 과거 권장은 이번 사용자의 명시적 Base/Home 전체 적용 요구로 대체한다.

## 1. 실제 좌표 공식

지원 보드는 기존 8~12다. 모든 좌표는 0-based file/rank다.

```text
n = board size
w = n이 짝수이면 2, 홀수이면 3
s = (n - w) / 2
e = s + w - 1

White Back  = {(f, 0) | s <= f <= e}
White Front = {(s-1, 0), (e+1, 0)} ∪ {(f, 1) | s-1 <= f <= e+1}
White Base/Home = Back ∪ Front
Black = 각 White 좌표 (f, r)를 (f, n-1-r)로 변환
```

| n | White Back | White Front | Base 칸 수 |
| --- | --- | --- | --- |
| 8 | (3,0), (4,0) | (2,0), (5,0), (2,1), (3,1), (4,1), (5,1) | 8 |
| 9 | (3,0), (4,0), (5,0) | (2,0), (6,0), (2,1), (3,1), (4,1), (5,1), (6,1) | 10 |
| 10 | (4,0), (5,0) | (3,0), (6,0), (3,1), (4,1), (5,1), (6,1) | 8 |
| 11 | (4,0), (5,0), (6,0) | (3,0), (7,0), (3,1), (4,1), (5,1), (6,1), (7,1) | 10 |
| 12 | (5,0), (6,0) | (4,0), (7,0), (4,1), (5,1), (6,1), (7,1) | 8 |

구현은 크기별 테이블을 사용하지 않는다. 표와 같은 명시적 expected set은 엔진/프론트 테스트에만 존재한다. `get_frontmost_base_rank_with_ruleset`은 Standard에서 White 1, Black n-2라는 rank 값만 반환한다. 전체 Front 영역 판정에는 사용하지 않는다.

## 2. G2에서 변경한 파일

파일 수가 여러 개인 이유는 엔진 공용 규칙, 편집기 표시/검증, 엔진 및 HTTP·Vue 테스트, 결과 문서를 각각 변경해야 하기 때문이다. 실행 규칙 변경은 세 파일에 국한되며, 서버의 변경은 테스트뿐이다.

| 파일 | 변경 이유 |
| --- | --- |
| [engine/src/rules.rs](../../engine/src/rules.rs) | Standard 기하 단일 구현, 배치 영역 API, Front 칸 집합 기반 덱 검사 |
| [frontend/src/composables/useDeckValidation.ts](../../frontend/src/composables/useDeckValidation.ts) | 좌표 predicate/집합, Standard 초기 배치·Front 채움 검증, 최소 유효 프리셋 |
| [frontend/src/views/DeckEditor.vue](../../frontend/src/views/DeckEditor.vue) | 실제 좌표 배치판, F/B/진영 밖 표시, 클릭/드래그 제한, ruleset별 프리셋·크기 변경 |
| [engine/tests/rule_engine.rs](../../engine/tests/rule_engine.rs) | 양 진영 8~12 좌표·모든 내장 기물 배치·validation·Drop·맵 독립성 및 Legacy 회귀 |
| [engine/tests/ammo_air_layer.rs](../../engine/tests/ammo_air_layer.rs) | 박격포, 탄약, 폭격기 귀환/강제 착륙/추락 회귀 |
| [frontend/src/composables/customDeckIntegration.test.ts](../../frontend/src/composables/customDeckIntegration.test.ts) | 명시적 좌표 집합, 배치, Front 채움, 프리셋, Black 요청 좌표 반전 |
| [frontend/src/views/DeckEditor.test.ts](../../frontend/src/views/DeckEditor.test.ts) | 실제 Vue setup의 표시/클릭/드래그/크기 변경/미완성 저장 가능 여부 |
| [server/src/main.rs](../../server/src/main.rs) | G1 Standard fixture를 새 유효 배치로 갱신, 모든 맵 생성·룸·sync 유지, 잘못된 양측 배치 HTTP 거부 |
| [STANDARD_FORMAT_IMPLEMENTATION_PLAN.md](STANDARD_FORMAT_IMPLEMENTATION_PLAN.md) | 확정된 G2 공식·runtime 적용 범위·G4 Extra-only 시점 기록 |
| 이 문서 | 결과와 완료 조건 대조 |

새 의존성, migration, 프레임워크, 기물 정의 변경은 없다. G0/G1 결과 문서는 당시 결과로 보존했다.

## 3. Authoritative engine API

기존 public 경계와 인자를 유지한다.

- `validate_deck_with_ruleset(deck, board_size, pieces, definitions, ruleset)`
- `get_base_zone_squares_with_ruleset(player_id, board_size, ruleset)`
- `get_frontmost_base_rank_with_ruleset(player_id, board_size, ruleset)`
- `can_piece_be_placed_at_start_with_ruleset(definition, player_id, square, board_size, ruleset)`

추가 API:

- `get_deployment_zone_squares_with_ruleset(player_id, board_size, zone, ruleset)`
- `get_front_zone_squares_with_ruleset(player_id, board_size, ruleset)`
- `get_back_zone_squares_with_ruleset(player_id, board_size, ruleset)`

Standard의 private `standard_zone_squares`가 공식 한 곳을 담당한다. Base는 zone 필터 없이 전체를 반환한다. 초기 배치는 `PieceDefinition.deployment_zone`의 영역을 사용한다. public 함수는 기존처럼 앞단에서 지원 보드 크기와 플레이어를 검증한 엔진 문맥을 받는다.

Standard 덱 검증은 Front 집합에 놓인 **Front 정의 기물**의 고유 좌표 수를 센다. 모든 칸이 있어야 성공하고, 하나라도 빠지거나 Back 기물로 채우면 실패한다. 별도의 배치 검사도 유지한다. Back은 King 1기 요구 외에 전체 채움을 강제하지 않는다. 점수 계산과 Pocket 검사는 그대로다.

## 4. Frontend 대응 API 및 UI

`deploymentZoneAtSquare(square, boardSize, side, ruleset)`가 `front | back | null`을 반환한다. `baseZoneSquares`, `frontZoneSquares`, `backZoneSquares`가 이를 사용한다. 엔진과 같은 공식을 쓰며 정수/범위 밖 좌표는 거부한다.

`placementRestriction`과 `canPieceBePlacedAtStart`의 두 번째 인자는 `SetupSquare | number`다. 숫자 rank만 전달하던 기존 Legacy 호출은 유지한다. Standard에서 숫자만 전달하면 좌표 정보 부족으로 거부한다. 에디터와 덱 validator는 항상 `{file, rank}`를 전달한다. rank helper는 화면 줄 나열 또는 Legacy 프리셋에만 사용한다.

Standard 배치판은 두 rank의 모든 file을 실제 위치에 표시한다. F는 Front, B는 Back이며 진영 밖은 회색/비활성이다. 선택/드래그 기물에 맞지 않는 칸은 빨간 테두리와 오류 안내로 표시하고 실제 추가를 거부한다. Back 구분 테두리가 배치 불가 강조를 덮지 않도록 CSS도 확인했다. 지우개로 표시된 진영 밖 기존 기물도 제거할 수 있다. Legacy의 분리된 앞줄/뒷줄 화면과 프리셋은 보존한다.

`createPresetDeck(boardSize, presetId, ruleset)`의 Standard 분기는 중앙 `floor(n/2),0`의 King 1기, 모든 Front 칸의 Pawn, 빈 Pocket을 만든다. UI에는 이 최소 기본 배치 하나를 표시한다. board size 변경/reset-to-classic 경로도 이를 사용한다. 같은 크기 맵 전환은 내용을 보존한다. 룰만 전환할 때는 내용 자동 삭제 없이 새 검증 결과와 프리셋 사용 안내를 표시한다. 전체 초기화의 기존 빈 초안 동작은 유지한다.

## 5. Base/Home 호출 지점 재조사

`get_base_zone*`, `get_frontmost_base*`, `can_piece_be_placed*`, base/home/진영 및 rank 관련 참조를 엔진·서버·프론트에서 재검색했다. 아래 runtime 호출은 G1에서 이미 `state.ruleset`을 전달하므로 G2에서는 호출 코드 재작성 없이 새 기하를 사용한다.

| 사용처 | 적용되는 규칙 |
| --- | --- |
| `placement.rs::get_placement_candidates` | 일반 Drop의 자기 Base |
| `legal_moves.rs::generate_piece_legal_ability_actions` 박격포 분기 | 상대 Base 조준 금지 |
| `legal_moves.rs::bomber_landing_targets` | 현재 폭격기 위치가 상대 Base이면 착륙 후보 없음 |
| `endgame.rs::apply_ability_action` 착륙 분기 | 자기 Base 착륙 시 탄약 전량 회복 |
| `endgame.rs::replenish_depleted_ammo_at_home` | 자기 Base 지상 기물의 탄약 소진 직후 회복 |
| `endgame.rs::replenish_ammo_on_home_entry` | Base 밖에서 안으로 이동하면 회복 |
| `server/src/main.rs::build_player_deck` | 서버의 초기 Base 검사 후 ruleset-aware 덱 validation |

Legacy wrapper 사용은 `rules.rs`의 Legacy 분기와 기존 Legacy 전용 Challenge/테스트에 한정된다. Standard 실행 경로에서 Legacy wrapper를 사용하지 않는다. 기타 검색 결과의 승격 rank, 수리병 주변 칸, 방향 등은 Base/Home 규칙이 아니며 변경하지 않았다.

## 6. Legacy 보존 방식

Legacy Base의 기존 반복문과 순서를 보존한다. 8/9는 전체 file의 뒤 두 rank, 10/11/12는 뒤 세 rank다. Front는 그중 최전방 한 rank 전체, Back은 나머지다. 기존 이름의 함수는 계속 Legacy wrapper다.

Front 채움에서 Legacy는 기존처럼 점유 좌표를 세고, 기물 분류는 별도 배치 검사에서 확인한다. Standard만 Front 정의를 채움 계산에 추가한다. King/점수/Pocket·기물 능력·턴 전환·air/terrain·Chessembly·Challenge·DC1/DC2/DC3 의미를 변경하지 않았다.

G1의 “두 룰셋이 동일한 배치/Drop 결과” 단언은 G2 요구와 양립하지 않으므로, Legacy의 명시적 기존 좌표·모든 기물 배치·각 Front 칸 누락 검사와 Standard의 명시적 새 좌표·runtime 테스트로 바꿨다. G1 저장/전송/룸/sync 단언은 유지했고 Standard 테스트 덱만 새 배치에 맞췄다.

## 7. Standard Drop 변화

Standard 후보는 **새 Base ∪ 기존 Attack Map**이다. deployment_zone 구분은 일반 Drop에 적용하지 않는다. Back인 Knight도 Front를 포함한 Base 전 칸에 일반 Drop할 수 있다. 예전 Legacy Base이지만 새 Base 밖인 `(0,0)` 등은 다른 근거가 없으면 불가하다. `(1,1)`처럼 새 Base 밖이더라도 기존 공격맵에 포함되면 여전히 허용된다.

크기 8~12·양 진영·두 ruleset으로 빈 보드의 정확한 후보와 각 후보 실제 submit을 검사한다. 공격맵 추가 이후에도 합집합과 비교한다. 12×12 Plain/CentralHighGround에서 Base 후보가 같은지 검사한다. 지형·점유·포획·공중 레이어의 기존 후속 검증은 유지한다.

## 8. 폭격기/탄약/기타 능력

폭격기의 기존 귀환은 `forced-landing` action이다. 새로운 귀환 action이나 비행 규칙은 추가하지 않았다. 상대 Base에서 착륙 불가/비행 만료 시 추락하는 기존 판정은 새 상대 Base를 따른다. 기존 Legacy 진영 외곽은 Standard에서 이 사유만으로 착륙을 차단하지 않는다.

자기 Back, 맨 뒤 rank의 측면 Front, 앞 rank의 Front 모두 착륙 시 재장전된다. Legacy에만 속하는 외곽에 착륙하면 Standard 탄약은 회복되지 않는다. 강제 착륙 전에는 같은 플레이어 턴을 유지하며, 착륙 후 상대 턴으로 바뀌는 flow도 양쪽 ruleset으로 확인했다.

박격포 조준 금지는 새 상대 Base 전체에 적용된다. 지상 탄약 소진 직후 회복과 Base 진입 회복도 새 전체 Base를 사용한다. `(1,0) → (2,0)` 같은 **동일 rank에서의 Standard 진영 진입**을 별도로 테스트했다. Legacy에서는 이미 진영 안이므로 이 이동만으로 재장전하지 않는다.

구행/폭격기는 기존 Back 정의를 유지하며 Standard Back에 초기 배치할 수 있다. Extra-only는 구현하지 않았다.

## 9. 실행한 검증과 결과

| 검증 명령/범위 | 결과 |
| --- | --- |
| `cargo test --offline -p brainfuck-chess-engine --test rule_engine --test ammo_air_layer` | 79 + 20 passed, 실패 0 |
| `cargo test --offline -p brainfuck-chess-engine` | 207 passed, 7 ignored, 실패 0 |
| Legacy 전 크기 배치/누락 검사 보강 후 `cargo test --offline -p brainfuck-chess-engine --test rule_engine` | 79 passed, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 108 passed, 8 ignored, 실패 0 |
| `npm test --prefix frontend` | 기존 runner 기준 21개 파일 passed, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 저장소 script는 typecheck와 같은 `vue-tsc --noEmit` |
| `npm run build --prefix frontend` | 통과; CSS 수정 후 재빌드 통과 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| 변경 Rust 파일 `rustfmt --edition 2021 --config skip_children=true` | 적용 및 diff 검토 |
| `git diff --check` 및 G2 시작 시 보관한 파일과 diff 비교 | 통과; G1 변경과 구분해 G2 변경 범위 검토 |
| 실제 브라우저 + 로컬 빌드 + 엔진 카탈로그 | Legacy 초기 표시, Standard 전환/프리셋, 8·9·12 고지전 크기/좌표/유효 표시 확인. Front 선택 시 Back 착수 거부 및 강조색을 스크린샷으로 확인 |

초기 신규 테스트 작성 중 Board 생성의 Result unwrap 누락과 중립/Black 직렬화 API 혼동을 수정했다. 기존 동작이나 검증 요구를 약화시켜 통과시키지 않았다.

브라우저 테스트는 별도 포트의 외부 DB 없는 메모리 서버를 사용했으며 임시 서버와 탭을 종료했다. 브라우저 전체 대국 E2E나 모바일 시각 검증은 수행하지 않았다. 실제 생성·룸·sync는 자동 HTTP router 테스트로 검증했다.

기존 경고: 프론트 번들 500 kB 초과 경고, 서버 미사용 생성자 경고 2개. ignored는 엔진 성능 벤치마크 7개, 서버 외부 DB 테스트 7개 및 payload 진단 1개다. 운영/외부 DB, 배포, ignored 벤치마크는 실행하지 않았다. Chessembly 호환 20개 및 expression 2개, 기존 ammo/air/terrain/Challenge/덱 코드/저장·sync 회귀는 해당 전체 suite에 포함되어 통과했다.

## 10. 새로 확인한 위험과 제한

- G1에서 Legacy 모양으로 저장한 Standard 덱은 G2 게임 validation을 통과하지 않을 수 있다. 저장 데이터는 자동 변환하거나 삭제하지 않는다. 에디터에서 수정하거나 Standard 기본 프리셋을 명시적으로 적용해야 한다. 10~12의 예전 rank 2 배치는 새 두 rank 표시판 밖에 남을 수 있으므로 전체 초기화/프리셋으로 재구성할 수 있다.
- G1 Standard 초기 상태를 현재 엔진에서 다시 실행하면 Base 관련 의미가 달라진다. G2에서 기록 버전/마이그레이션을 임의로 추가하지 않았다. G7에서 개발 단계 Standard 기록의 재실행 호환 정책을 결정해야 한다. Legacy 기록/hash 호환은 기존 테스트를 보존했다.
- 프론트와 Rust는 언어가 달라 같은 공식을 각각 구현한다. 크기별 명시 expected set과 양 진영 전 칸 검사로 불일치를 감시한다.
- Standard의 영역 축소에 맞춘 Bot 평가 정책은 구현하지 않았다. 기존 합법수 생성은 새 영역을 사용하지만 전략 품질 보장은 G8 범위다.

## 11. G3/G4와의 접점 및 결정 사항

- **G3:** 첫 턴의 추가 Draw 여부, Hand 상한/상대 공개 범위, 빈 Pocket UI, 기존 Pocket 교체·공수 배치·복귀 능력이 Hand와 어떻게 상호작용할지 결정해야 한다. 일반 Drop의 위치 계약은 이 문서의 Base ∪ Attack Map을 유지하고 출처 규칙만 G3에서 다룬다. 폭격기 강제 착륙의 두 action 사이에는 새 턴이 없다는 계약을 유지해야 한다.
- **G4:** 구행/폭격기 Extra-only를 ruleset별 validation 정책으로 추가해야 한다. 기물의 전역 Back 정의를 바꾸어 Legacy/Challenge를 깨뜨리지 않는다. Front/Back 기하와 Extra-only 허용 여부는 별도 검사로 결합한다. 기존 Standard에 저장된 해당 Back 기물의 편집 전환 안내도 필요하다.
- G4/G5 전에는 Extra 공개 범위, 중복/재사용, 기본 소환 위치 같은 미확정 항목을 확정해야 한다. G2에서는 값을 정하거나 확장용 필드를 추가하지 않았다.
- 충돌 가능 파일은 `rules.rs`, `useDeckValidation.ts`, `DeckEditor.vue`, `server/src/main.rs`와 대응 fixture다. runtime 위치 계약은 `get_base_zone_squares_with_ruleset`, 초기 분류는 `get_deployment_zone_squares_with_ruleset`을 재사용한다.

## 12. G2 완료 조건 대조

| 조건 | 결과 및 증거 |
| --- | --- |
| Legacy Base/Home 동일 | 기존 계산/순서 보존, 양 진영 8~12 명시 좌표·모든 내장 기물 배치·Drop·능력 회귀 통과 |
| Standard 정확한 Base/Front/Back | 8~12 expected set, 양 진영 rank mirror, 영역 밖/경계 좌표 테스트 통과 |
| Front/Back 초기 배치 | 모든 내장 정의 × 모든 보드 칸 × 양 진영 검사 통과 |
| Front 전 칸 채움 | Front 각 칸 제거/Back 기물 대체 실패, Back 공백 허용 및 최소 프리셋 성공 |
| 일반 Drop 새 Base | 정확한 Base 및 Attack Map 합집합, 각 후보 submit과 밖의 잘못된 submit 거부 |
| 기존 base/home 능력 | 박격포와 폭격기의 상대 Base, 탄약 두 경로 회귀 통과 |
| 폭격기 귀환/탄약/강제 착륙 flow | 실제 합법 action/submit, Front·Back·Legacy-only 좌표, 같은 플레이어 연속 action 검사 통과 |
| Map/Terrain 독립 | 기하에 map 인자 없음, 12 Plain/고지 Base 후보 동일, 기존 terrain suite 및 모든 맵 생성 통과 |
| DeckEditor 실제 영역 표시/검증 | 좌표 predicate 연결, 실제 Vue handler 및 브라우저 확인 |
| G1 저장/요청/sync 유지 | 프론트·서버 전체 회귀, Standard 유효 fixture로 생성/룸/heartbeat 확인 |
| Hand/Extra/Summon 미구현 | 관련 필드/action/RNG/새 Deck Code/정책 추가 없음; 구행/폭격기 Back 허용 |
| 관련 테스트/회귀 통과 | §9의 실패 0 및 미실행 범위 명시 |
