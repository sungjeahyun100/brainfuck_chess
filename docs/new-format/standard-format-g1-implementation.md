# G1 — Board Size / Map / Ruleset 분리 구현 결과

작성일: 2026-09-10 (KST). 기준 HEAD: `50db27d09c6ec3dfa580af5e18057a567889e65f`.

G1만 구현했다. Standard를 선택·저장·게임 생성할 수 있으며 현재 실행 규칙은 Legacy와 같다. 중앙 Base Zone, Hand/Draw/RNG, Extra/Summon, 새 Bot 정책과 Standard Challenge는 구현하지 않았다. [계획](STANDARD_FORMAT_IMPLEMENTATION_PLAN.md)과 [G0 분석](standard-format-codebase-analysis.md)은 원래 내용 그대로 보존했다.

## 1. 변경 파일 목록과 변경 이유

파일이 여러 개인 이유는 저장/요청/동기화에서 직접 열거하는 필드, 서로 다른 서버 진입점, base/home 함수 사용처를 연결해야 하고 Rust struct literal은 serde default와 별개로 필드 초기화가 필요하기 때문이다. fixture만 변경한 파일은 아래에 구분했다. 새 의존성·프레임워크·migration·Chessembly 변경은 없다.

| 파일 | 필요한 변경 |
| --- | --- |
| [`engine/src/actions.rs`](../../engine/src/actions.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/src/ai/beam.rs`](../../engine/src/ai/beam.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/src/ai/evaluate.rs`](../../engine/src/ai/evaluate.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/src/ai/search.rs`](../../engine/src/ai/search.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/src/ai/transposition_table.rs`](../../engine/src/ai/transposition_table.rs) | 캐시 위치 키에 룰셋 포함, 분리 회귀 테스트 및 기존 fixture 필드 |
| [`engine/src/attack_map.rs`](../../engine/src/attack_map.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/src/endgame.rs`](../../engine/src/endgame.rs) | 귀환·착륙·탄약 회복의 base 참조에 실행 상태 룰셋 전달 |
| [`engine/src/legal_moves.rs`](../../engine/src/legal_moves.rs) | 박격포·폭격기 등 base 참조에 실행 상태 룰셋 전달 |
| [`engine/src/placement.rs`](../../engine/src/placement.rs) | 일반 Drop의 공용 base 호출에 실행 상태 룰셋 전달 |
| [`engine/src/rules.rs`](../../engine/src/rules.rs) | 기존 함수 보존 + 룰셋을 받는 validation/base/front/배치 API |
| [`engine/src/types.rs`](../../engine/src/types.rs) | DeckRuleset 및 GameState 권위 필드, Legacy 희소 직렬화 |
| [`engine/tests/ai.rs`](../../engine/tests/ai.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/tests/ai_benchmark.rs`](../../engine/tests/ai_benchmark.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/tests/ammo_air_layer.rs`](../../engine/tests/ammo_air_layer.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/tests/custom_piece_runtime.rs`](../../engine/tests/custom_piece_runtime.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/tests/custom_piece_visuals.rs`](../../engine/tests/custom_piece_visuals.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/tests/piece_options_benchmark.rs`](../../engine/tests/piece_options_benchmark.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`engine/tests/rule_engine.rs`](../../engine/tests/rule_engine.rs) | 기존 fixture 필드 + 두 룰셋의 geometry/validation/Drop 동등성 회귀 |
| [`engine/tests/v2_supplement.rs`](../../engine/tests/v2_supplement.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`frontend/src/App.vue`](../../frontend/src/App.vue) | 게임/Bot 덱 룰셋 일치 확인, 편집기 이름·맵·룰 3열 배치 |
| [`frontend/src/api/deckApi.ts`](../../frontend/src/api/deckApi.ts) | account 입력 allowlist 및 응답 룰셋 정규화 |
| [`frontend/src/api/gameApi.test.ts`](../../frontend/src/api/gameApi.test.ts) | 룰셋 저장·요청·UI/코드 호환 회귀 테스트 추가 |
| [`frontend/src/api/gameApi.ts`](../../frontend/src/api/gameApi.ts) | 덱/룸/sync 타입, 생성 요청 룰셋 및 merge 보존 |
| [`frontend/src/composables/deckRepository.test.ts`](../../frontend/src/composables/deckRepository.test.ts) | 룰셋 저장·요청·UI/코드 호환 회귀 테스트 추가 |
| [`frontend/src/composables/localDeckRepository.ts`](../../frontend/src/composables/localDeckRepository.ts) | 신규 Legacy 및 저장/읽기 정규화, unknown 오류 원본 보존 |
| [`frontend/src/composables/useDeckCode.test.ts`](../../frontend/src/composables/useDeckCode.test.ts) | 룰셋 저장·요청·UI/코드 호환 회귀 테스트 추가 |
| [`frontend/src/composables/useDeckCode.ts`](../../frontend/src/composables/useDeckCode.ts) | DC1/DC2/DC3 가져오기는 명시적으로 Legacy |
| [`frontend/src/composables/useDeckCodeCodec.ts`](../../frontend/src/composables/useDeckCodeCodec.ts) | Standard/unknown 내보내기 거부, 기존 payload 유지 |
| [`frontend/src/composables/useDeckSerialization.ts`](../../frontend/src/composables/useDeckSerialization.ts) | 중립 덱 및 흑 방향 변환 시 룰셋 보존 |
| [`frontend/src/composables/useDeckValidation.ts`](../../frontend/src/composables/useDeckValidation.ts) | 저장/플레이 검증의 unknown 거부 및 룰셋 문맥 |
| [`frontend/src/composables/useSavedDecks.ts`](../../frontend/src/composables/useSavedDecks.ts) | 계정 로그인 시 로컬 가져오기 후보 조회의 룰셋 오류 표시 |
| [`frontend/src/replayCodec.test.ts`](../../frontend/src/replayCodec.test.ts) | 룰셋 저장·요청·UI/코드 호환 회귀 테스트 추가 |
| [`frontend/src/replayCodec.ts`](../../frontend/src/replayCodec.ts) | 복기 코드 initial_state의 명시적 unknown 룰셋 거부 |
| [`frontend/src/replayDeckCode.test.ts`](../../frontend/src/replayDeckCode.test.ts) | 룰셋 저장·요청·UI/코드 호환 회귀 테스트 추가 |
| [`frontend/src/replayDeckCode.ts`](../../frontend/src/replayDeckCode.ts) | 복기에서 Standard 덱을 Legacy 코드로 내보내지 못하게 차단 |
| [`frontend/src/types/deck.ts`](../../frontend/src/types/deck.ts) | LobbyDeck 구형 입력 optional, 정규화 SavedDeck 필수 ruleset |
| [`frontend/src/types/game.ts`](../../frontend/src/types/game.ts) | 구형 저장 상태를 허용하는 optional 실행 상태 ruleset |
| [`frontend/src/views/Challenges.vue`](../../frontend/src/views/Challenges.vue) | 기존 Challenge는 Legacy 덱만 선택 가능 |
| [`frontend/src/views/DeckEditor.test.ts`](../../frontend/src/views/DeckEditor.test.ts) | 룰셋 저장·요청·UI/코드 호환 회귀 테스트 추가 |
| [`frontend/src/views/DeckEditor.vue`](../../frontend/src/views/DeckEditor.vue) | 독립 선택 UI, clone/미리보기 전달, Standard 코드 미지원 표시 |
| [`frontend/src/views/DeckLibrary.vue`](../../frontend/src/views/DeckLibrary.vue) | 저장 덱 룰셋 표시 |
| [`frontend/src/views/DeckSelect.vue`](../../frontend/src/views/DeckSelect.vue) | 같은 룰셋만 시작 허용 및 덱 룰셋 표시 |
| [`frontend/src/views/MultiplayerLobby.vue`](../../frontend/src/views/MultiplayerLobby.vue) | 룸 룰셋 표시 및 참가/재선택 비교 |
| [`server/src/analysis.rs`](../../server/src/analysis.rs) | 기존 fixture 필드 + 고정 구형 JSON/canonical hash 회귀 |
| [`server/src/challenge.rs`](../../server/src/challenge.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`server/src/custom_piece.rs`](../../server/src/custom_piece.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`server/src/deck.rs`](../../server/src/deck.rs) | strict DeckData 룰셋, 희소 Legacy, PlayerDeckSpec 전달 |
| [`server/src/deck/tests.rs`](../../server/src/deck/tests.rs) | 기존 literal 필드 + 구형 fingerprint/계정 API/unknown 회귀 |
| [`server/src/game_record.rs`](../../server/src/game_record.rs) | 기존 Rust struct literal에 Legacy 기본 필드만 추가; 기존 동작·검증 의미 유지 |
| [`server/src/main.rs`](../../server/src/main.rs) | 요청·룸·factory 권위 계약, 경로별 통합 테스트, Lab/fixture Legacy |
| [`server/src/time_control.rs`](../../server/src/time_control.rs) | GameView 명시 필드 투영 및 매 sync에 ruleset, 기존 fixture 필드 |
| [`frontend/src/deckRulesets.ts`](../../frontend/src/deckRulesets.ts) | 공용 타입·선택지·누락 기본값·unknown 검증 함수 신규 |

이 문서 `docs/new-format/standard-format-g1-implementation.md`도 신규 산출물이다. 기존 untracked 계획/G0 문서는 이번 G1 변경 목록에 포함하지 않았다. 빌드 산출물은 직접 수정하지 않았다.

## 2. 실제 Ruleset 타입과 위치

- Rust: `engine/src/types.rs`의 `DeckRuleset::{Legacy, Standard}`. 서버에서도 이 타입을 사용한다.
- TypeScript: `frontend/src/deckRulesets.ts`의 `DeckRuleset`, `deckRulesets`, `isDeckRuleset`, `parseDeckRuleset`.
- 실행 중 권위 값은 `GameState.ruleset`이다. 엔진의 각 플레이어 `Deck`에는 동일 값을 중복 저장하지 않는다.

## 3. serialization ID와 독립성

ID는 `legacy`, `standard`이다. `standard-8x8`~`standard-12x12`는 계속 기존 일반 맵 ID이며, `central-high-ground-12x12`와 `BoardVariant`의 지형 의미도 그대로다. `GameMode::Standard`와 GameRecord의 `ruleset_version = "deck-chess-1"`은 새 룰셋과 별개이며 변경하지 않았다.

기존 mapId→boardSize 관계는 유지한다. mapId/boardSize와 ruleset 사이의 자동 변환은 없다. 8×8/10×10 일반 맵, 12×12 중앙 고지 맵 각각 두 룰셋으로 생성되는 테스트에서 ID·룰셋 이외 실행 상태가 같음을 확인했다.

## 4. 기존 데이터의 Legacy 기본값

| 경계 | 누락 필드 처리 |
| --- | --- |
| Local SavedDeck | `readStorage`가 `parseDeckRuleset(undefined)`로 Legacy 정규화. 신규 덱도 Legacy |
| Account API 입력·응답 | allowlist 및 각 응답의 정규화에서 누락만 Legacy |
| 서버 `DeckData` | `#[serde(default, skip_serializing_if = "DeckRuleset::is_legacy")]` |
| `PlayerDeckSpec`, 생성 요청, Room | enum 필드의 `#[serde(default)]` |
| 엔진 `GameState` / GameRecord의 initial_state | 누락을 Legacy로 읽고 Legacy 직렬화는 필드 생략 |
| full GameView / heartbeat | 권위 GameState로부터 `legacy`도 명시하여 응답 |
| 구형 sync | frontend merge에서 누락만 Legacy 정규화 |
| DC1/DC2/DC3 | 기존 exact schema 유지, 가져온 편집 덱은 명시적으로 Legacy |
| 구형 Replay Code | 기존 JSON 형태 유지. 실행 해석 시 누락은 Legacy 계약 |

Account DB는 `deck_data JSONB`이며 객체·크기 제약만 있다. 기존 SQL의 `sqlx::types::Json(DeckData)`가 새 Standard 필드를 그대로 저장/읽는다. 추가 열이나 migration은 필요하지 않다. account `format_version = 1`, Deck Code DC3, GameRecord `format_version = 2`를 올리지 않았다.

## 5. unknown 거부

TypeScript parser는 `undefined` 외에는 두 문자열만 허용한다. `null`, 숫자, 다른 문자열은 오류다. 저장·게임 validation은 지원하지 않는 룰셋을 유효하지 않은 덱으로 반환한다. Local 읽기의 룰셋 오류는 기존 JSON 파싱 fallback 밖에서 발생하여 빈 목록으로 대체하거나 원본을 덮어쓰지 않는다. 저장 화면/로컬 가져오기 조회에는 오류를 전달한다.

Rust enum 역직렬화는 알 수 없는 값과 null/다른 타입을 거부한다. `DeckData`의 `deny_unknown_fields`도 유지했다. HTTP 입력은 성공 응답 대신 400/422의 JSON 역직렬화 오류가 된다. 정상 enum 간 불일치는 명시적 400이다. Replay Code initial_state도 명시적 unknown을 `invalid_schema`로 거부한다.

## 6. SavedDeck → GameState 전달 경로

```text
DeckEditor ruleset select
  → SavedDeck.ruleset → cloneSavedDeck
  → LocalDeckRepository (localStorage)
    / AccountDeckRepository → deckInput.deckData.ruleset
      → server DeckData → JSONB → SavedDeck 응답 정규화
  → serializeNeutralDeck / savedDeckToPlayerDeckRequest
  → PlayerDeckRequest.ruleset
  → CreateGameRequest.ruleset + white_deck.ruleset + black_deck.ruleset
    / CreateRoomRequest.ruleset + deck.ruleset → MultiplayerRoom
  → PlayerDeckSpec → materialize_neutral_deck (흑 rank 반전에도 보존)
  → build_player_deck / validate_deck_with_ruleset
  → build_game_state_with_variant → GameState.ruleset
  → GameView / GameDynamicView.ruleset
  → heartbeat → mergeGameSync.ruleset
```

일반 생성 endpoint는 요청=White=Black을 확인하고, 공용 factory에서도 White=Black을 다시 확인한다. state는 검증된 White 값에서 만든다. GameStaticData는 카탈로그에 한정하며, 매번 전달되는 GameDynamicView에 룰셋을 둬 catalog 생략 sync에도 유실되지 않는다.

## 7. Multiplayer Room 계약

Room 생성 시 `room.ruleset == host deck.ruleset`, join/reselect 시 `room.ruleset == submitted deck.ruleset`을 서버에서 검증한다. 불일치는 참가자 등록·덱 교체 전에 거부한다. 양쪽 ready 이후 실제 게임을 만들 때도 `room == host == guest`를 재검증한다. Room 응답은 룰셋을 명시하고 게임 생성 후에는 GameState/sync에서 유지한다. 기존 룸 생성 JSON의 누락은 Legacy다.

UI 비교는 빠른 안내 역할이다. 클라이언트를 우회한 요청, 룸 내부 덱 불일치 fixture에 대한 최종 factory 검사도 통합 테스트로 확인했다. 기존 참가자 권한/인증 검사를 제거하지 않았다.

## 8. Deck Editor UI

기존 이름·맵 선택 옆에 `룰` select를 추가했다. Legacy/Standard를 독립적으로 선택하며, map 변경 핸들러는 ruleset을 수정하지 않는다. ruleset select도 boardSize/mapId를 수정하지 않는다. clone과 저장/재로드에서 값을 보존한다. 좁은 화면에서는 기존 단일 열 레이아웃을 사용한다.

Standard 선택 시 현재 Legacy와 같은 게임 규칙임을 표시하고, 덱 코드 복사 버튼을 비활성화하며 미지원 이유를 표시한다. 덱 목록/대전 선택/룸에서도 룰셋을 볼 수 있다. UI 검증은 실제 Vue setup·handler 테스트와 SFC 빌드이며 브라우저 시각 E2E는 실행하지 않았다.

## 9. validation과 base/home 경계

Rust 공용 API:

- `validate_deck_with_ruleset(deck, board_size, pieces, definitions, ruleset)`
- `get_base_zone_squares_with_ruleset(player_id, board_size, ruleset)`
- `get_frontmost_base_rank_with_ruleset(player_id, board_size, ruleset)`
- `can_piece_be_placed_at_start_with_ruleset(definition, player_id, square, board_size, ruleset)`

기존 이름/인자를 가진 함수는 Legacy wrapper로 보존했다. 서버 초기 배치 검사뿐 아니라 일반 Drop(`placement.rs`), base를 참조하는 능력/폭격기 합법수(`legal_moves.rs`), 귀환/착륙/탄약 회복(`endgame.rs`)도 공용 base API에 `state.ruleset`을 전달한다. G1의 두 enum 분기는 동일한 기존 계산을 사용한다.

프론트의 baseZoneDepth/baseZoneRanks/frontmostBaseRank/placementRestriction/canPieceBePlacedAtStart는 룰셋 인자를 받는다. 저장·플레이 validator 및 에디터가 룰셋을 전달한다. 점수·앞줄 채움·front/back·Pocket 규칙은 변경하지 않았다.

## 10. Deck Code 및 Replay 처리

DC1/DC2/DC3 reader의 exact key와 payload 의미는 변경하지 않았다. Legacy export는 기존 DC3 문자열과 같다. Standard를 기존 DC3로 내보내려 하면 codec이 `STANDARD_DECK_CODE_UNSUPPORTED` 오류를 반환한다. UI 방어 외에 직접 codec 호출도 거부한다.

구형 코드를 Standard 편집 덱에 불러오면 그 코드의 의미대로 Legacy가 된다. Replay의 frozen 덱 코드 경로에서도 Standard를 내보내지 못하게 했다. GameRecord initial_state에는 Standard가 유지되고, `state_at_ply(0)` 및 기존 Replay Code 왕복도 보존한다. 기존 기록 버전이나 delta action 종류는 변경하지 않았다. Draw/Summon 기록과 새 Standard Deck Code는 G7 범위다.

## 11. 분석 hash 및 import fingerprint 호환

기존 canonical hash 알고리즘 자체는 바꾸지 않았다. GameState Legacy 기본값을 직렬화에 무조건 추가하면 기존 해시가 달라지므로 Legacy만 생략한다. 고정된 G1 이전 JSON과 고정 SHA를 회귀 테스트에 넣어 누락/명시적 Legacy가 같은 hash임을 확인했다. Standard는 실제 필드가 직렬화되므로 별도 hash다.

DeckData에도 같은 희소 정책을 적용하여 기존 content import fingerprint를 유지했다. 고정된 구형 계정 덱 JSON/fingerprint를 검증하고, 동일 내용의 Legacy/Standard import가 서로 다른 저장 ID를 가지며 같은 Standard의 재import는 동일 ID임을 확인했다.

GameView의 wire 투영은 `legacy`도 명시한다. 이 투영은 저장 GameState나 hash 입력을 수정하지 않는다. 엔진의 일시적 Bot transposition key에는 룰셋을 포함했다. 이는 다른 규칙 상태가 같은 캐시 항목으로 취급되지 않게 하는 정체성 수정이며 평가/탐색 정책 추가가 아니다.

## 12. 실행한 검증 결과

| 명령 | 최종 결과 |
| --- | --- |
| `npm test --prefix frontend` | 테스트 파일 21개 모두 통과, 실패 0. 기존 파일 안에 새 회귀 테스트 추가 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과. 저장소 script상 `vue-tsc --noEmit`으로 typecheck와 동일 |
| `npm run build --prefix frontend` | 통과. 500 kB 초과 번들 경고 있음 |
| `cargo test --offline -p brainfuck-chess-engine` | 총 200 passed, 7 ignored, 실패 0. rule_engine 76개 포함 |
| `cargo test --offline -p brainfuck-chess-server` | 107 passed, 8 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과. 기존 미사용 생성자 경고 2개 |
| 변경 Rust 파일의 `rustfmt --edition 2021 --config skip_children=true` | 적용 및 diff 검토 |
| `git diff --check` | 통과 |
| 신규 문서·타입 파일의 whitespace/참조 경로 점검 | 통과 |

G0 baseline의 frontend 21개 파일, rule_engine 75개, server 99 passed/8 ignored와 비교하여 기존 테스트 실패가 없다. rule_engine 1개와 서버 8개 테스트를 추가했다. 전체 엔진은 G0에서 실행하지 않았으므로 새 전체 실행 결과를 과거 전체 baseline과 비교했다고 주장하지 않는다.

ignored 항목은 엔진 성능 벤치마크 7개, 서버 외부 DB 통합 7개와 payload 진단 1개다. 운영/외부 DB 접속, 배포, 브라우저 E2E, 성능 프로파일은 실행하지 않았다. 계정 API 회귀는 저장소의 in-memory repository와 실제 HTTP router를 사용하며 외부 PostgreSQL 왕복을 검증했다는 뜻은 아니다. JSONB/migration 불필요 판단은 기존 SQL 제약과 adapter 조사에 근거한다.

추가 테스트 작성 중 Drop 목록 비교가 HashSet 순서 차이로 한 번 실패했다. 기존 엔진 동작을 바꾸지 않고 새 테스트에서 좌표 정렬 후 후보와 실행 결과를 비교했다. HTTP 역직렬화 오류의 status도 실제 400/422 오류 계약에 맞춰 새 테스트를 조정했다. 기존 테스트의 의미/단언을 약화시키지 않았다.

## 13. 최소 회귀 요구사항 23개 대조

아래 표의 파일은 각 테스트가 들어 있는 위치이며 해당 전체 테스트 명령이 통과했다.

| # | 요구사항 | 증거 |
| --- | --- | --- |
| 1 | 기존 SavedDeck 누락 → Legacy | frontend `deckRepository.test.ts` 로컬 구형 JSON |
| 2 | 기존 account deck 누락 → Legacy | 서버 `deck/tests.rs` 고정 구형 JSON + 프론트 account 응답 |
| 3 | 명시적 unknown 거부 | 저장·일반/룸 HTTP·GameState·sync·Replay Code 오류 테스트 |
| 4 | Legacy 저장/로드 | 로컬/계정 repository 및 server API 양쪽 룰셋 loop |
| 5 | Standard 저장/로드 | 동일 loop에서 Standard 조회·update |
| 6 | clone 보존 | `DeckEditor.test.ts` 실제 cloneSavedDeck |
| 7 | Local 보존 | `deckRepository.test.ts` 새 repository/JSON 확인 |
| 8 | Account 보존 | frontend whitelist/mock HTTP + server actual router |
| 9 | size 변경 시 룰셋 유지 | `DeckEditor.test.ts` 맵 8/10/12 크기 변경 |
| 10 | map 변경 시 룰셋 유지 | 동일 size의 12 일반/중앙 고지 전환 |
| 11 | 룰 변경 시 size/map 유지 | 에디터 실제 상태/핸들러 테스트 |
| 12 | Legacy+Legacy 생성 | 서버 `ruleset_game_creation_is_independent_of_map_and_preserved_in_views` |
| 13 | Standard+Standard 생성, 동일 규칙 | 같은 생성 테스트의 상태 비교 + engine geometry/validation/Drop 비교 |
| 14 | 서로 다른 룰셋 생성 거부 | 서버 request/white/black 조합, 게임 수 불변 |
| 15 | Room 보존 | 생성·재선택·ready·즉시 join·heartbeat 양쪽 룰셋 |
| 16 | Room 불일치 거부 | 생성/join/재선택/최종 factory 및 미변경 상태 |
| 17 | sync 보존 | 서버 두 heartbeat + frontend catalog 생략 merge |
| 18 | 기존 Drop | engine 전체 rule_engine/placement/action/AI + 새 동등성 테스트 |
| 19 | 폭격기 강제 착륙 연속 행동 | 서버 `forced_landing_records_two_white_actions_before_the_black_action`, time_control 연속 행동 및 engine ammo_air_layer |
| 20 | 기존 Challenge | 기존 registry/factory/Bot/clear 테스트 + Standard 거부/Legacy 구행 초기 배치 확인 |
| 21 | DC1/DC2/DC3 호환 | `useDeckCode.test.ts` 기존 suite + exact schema/Legacy export 동일성 |
| 22 | Standard 코드 유실 차단 | 직접 codec/편집기 + `replayDeckCode.test.ts` |
| 23 | 기존 분석 hash | `analysis.rs` 고정 이전 JSON/SHA, Legacy 명시/누락 및 Standard 구분 |

Legacy의 점수·진영·배치·Drop·기물 능력·턴 전환 코드는 기존 알고리즘을 유지하고 룰셋 인자만 연결했다. Chessembly parser/interpreter/문법은 변경하지 않았으며 호환 테스트 20개와 piece expression 2개도 전체 엔진 실행에서 통과했다.

## 14. G2가 사용할 공용 계약

§2의 공용 타입 및 §9의 Rust `_with_ruleset` API가 G2 진입점이다. 실행 중에는 항상 `GameState.ruleset`을 전달한다. 기존 Legacy wrapper를 Standard 실행 경로에서 호출하면 안 된다. G1에서는 공용 base/home 사용처를 이미 새 API로 연결했다.

**사용자의 G1 지시로 확정된 후속 범위:** G2의 Standard 진영 정의는 초기 배치만이 아니라 Drop의 자기 진영, 진영 참조 능력, 폭격기 귀환, 탄약 회복 등 모든 base/home 규칙에 적용한다. G0 §9/§11의 “초기 배치에만 제한하고 home을 보존” 권장은 당시 계획 해석이며 이 최신 명시적 지시로 대체된다. 단, Legacy의 진영은 그대로 보존한다.

프론트의 기존 rank 중심 helper는 룰셋을 받지만 Standard의 중앙 file/rank 영역을 아직 계산하지 않는다. G2에서 좌표 전체 predicate와 앞줄 채움 규칙·미리보기를 같이 바꿔야 한다. 기존 validator의 `board_size` 전체 앞줄 채움도 G1에서는 의도적으로 유지했다. 이번 변경이 G2의 구체적인 영역 기하를 결정한 것은 아니다.

G2/G3/G4 병렬 작업은 여전히 `rules.rs`, `types.rs`, `main.rs`, `useDeckValidation.ts`, `DeckEditor.vue`를 공유한다. 특히 G2가 runtime base 사용처까지 관여하므로 G3의 placement/endgame 편집과 의미상 충돌도 조정해야 한다. 실제 병렬 작업이나 Worktree 생성은 수행하지 않았다.

## 15. 남은 위험과 후속 Goal

- Standard는 현재 Legacy 규칙으로 플레이한다. G2 이후 이미 저장한 Standard 덱은 새 validation에 따라 다시 편집해야 할 수 있다. 새 진영·Draw·Extra의 정책은 해당 Goal에서 구현/검증한다.
- Standard Deck Code는 G7까지 미지원이다. 기존 DC3에 필드를 추가하거나 일부 데이터를 제거해 내보내지 않는다.
- Standard의 새 규칙 도입 시 Bot 평가/숨김 정보/탐색·Challenge 지원 정책을 G8과 조정해야 한다. 이번 G1의 기존 Bot은 현재 동일한 실행 규칙을 사용한다.
- 새 프론트/서버를 함께 사용하는 경로를 검증했다. G1 이전 클라이언트/서버가 Standard를 이해한다고 보장하지 않는다. 구형 Legacy 데이터 읽기와 기존 버전 의미 보존이 이번 호환 계약이다.
- 외부 DB·브라우저 E2E·성능 검증은 §12처럼 미실행이다. GameView의 명시 룰셋 투영은 JSON 값 생성을 추가하며 별도 성능 측정은 하지 않았다.

## 16. 계획 문서 G1 완료 조건과 실제 결과

| 계획의 G1 완료 조건 | 실제 결과 |
| --- | --- |
| **Legacy 게임 동작이 작업 전과 동일** | 기존 알고리즘 유지. 전체 엔진/서버 및 기존 프론트 회귀 통과. 구형 JSON/hash 보존 |
| 기존 덱을 읽을 수 있다 | 구형 Local/Account JSON, DC1/DC2/DC3 테스트 통과 |
| 기존 덱은 Legacy로 동작한다 | 누락 기본값·서버 생성·Challenge 확인 |
| UI에서 Board Size와 Ruleset 별도 선택 | 맵/보드 select와 룰 select 분리 |
| 크기를 바꿔도 ruleset이 암묵적으로 바뀌지 않는다 | 실제 에디터 handler에서 8/10/12 및 맵 전환 확인 |
| ruleset을 바꿔도 board size가 암묵적으로 바뀌지 않는다 | 룰 select 상태 변경 후 mapId/boardSize 보존 확인 |
| 관련 테스트가 통과한다 | §12 최종 명령 모두 통과, 의도된 ignored 항목 명시 |

추가 사용자 완료 조건도 충족했다: 독립 표현/두 룰셋/누락 호환/unknown 거부/에디터/양쪽 저장/요청·서버/Room/서버 불일치 검사/GameState·sync/Legacy 규칙/기존 Deck Code 의미/Standard 코드 유실 차단/새 Standard 규칙 미구현/테스트 통과. 각 증거는 §2~§13에 대응한다.
