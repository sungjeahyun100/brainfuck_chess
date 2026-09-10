# G4 — Extra Deck 데이터 모델과 덱 빌더

작성일: 2026-09-10 (KST). 현재 G1/G2 코드와 G2 결과 문서를 기준으로 G4만 구현했다. G3보다 먼저 수행했다. 작업 시작 시 Git 작업 트리는 깨끗했다.

## 1. 데이터 모델

| 경계 | 실제 필드 | 의미 |
| --- | --- | --- |
| frontend `LobbyDeck` / `SavedDeck` | `extra?: DeckPieceType[]` | 타입 참조 1개가 기물 1기. 구형 입력에는 필드가 없을 수 있음 |
| frontend 저장·복사 경계 | `normalizeExtra(value: unknown): DeckPieceType[]` | 누락만 `[]`로 정규화하고 새 배열로 복사 |
| server `DeckData` | `extra: Vec<String>` | 계정 JSONB의 중립 타입 참조 배열 |
| frontend `PlayerDeckRequest` | `extra?: DeckPieceRequest[]` | 각 인스턴스의 내장 또는 고정 커스텀 참조 |
| server `PlayerDeckSpec` | `extra: Vec<DeckPieceRef>` | 게임 생성 및 Room의 덱 계약 |
| engine `Deck` | `extra_deck_pieces: Vec<PieceId>` | 현재 Extra zone에 소속된 구체적 기물 ID |
| frontend 실행 `Deck` | `extra_deck_pieces?: PieceId[]` | 구형/빈 wire 필드 누락을 허용하는 실행 타입 |

예: `extra: ["guhang", "bomber", "bomber"]`는 세 인스턴스다. 타입별 uniqueness 규칙을 추가하지 않았다. 서로 다른 폭격기에 서로 다른 PieceId가 발급된다. 저장 배열에 영구 인스턴스 ID를 만들지는 않으며, ID 발급은 기존 게임 factory의 역할이다.

Rust 세 경계에는 `serde(default, skip_serializing_if = "Vec::is_empty")`를 적용했다. 누락은 빈 Extra이며 빈 필드 직렬화 생략은 기존 JSON/hash 호환을 보존한다. 프론트의 읽기/쓰기/clone은 누락을 빈 배열로 정규화한다. 명시적인 null, 배열이 아닌 값, 잘못된 타입/빈 ID/제어 문자는 조용히 버리지 않고 거부한다.

## 2. Runtime zone 불변조건

`Player.deck.extra_deck_pieces`가 Extra 소속의 권위 있는 목록이다. `current_square=None && in_pocket=false`만으로 소속을 추론하지 않는다.

Extra ID는 다음 조건을 만족해야 한다.

- `GameState.pieces`에 정확한 소유자의 기물로 존재한다.
- 같은 Extra 목록에서 ID가 중복되지 않는다. 같은 **타입**의 다른 ID는 허용한다.
- Starting/Pocket 목록과 겹치지 않는다.
- `current_square=None`, `in_pocket=false`, `captured=false`다.
- Board의 지상/공중 점유 목록과 captured 목록에 들어가지 않는다. factory는 처음부터 해당 목록에 넣지 않는다.
- 초기 ammo/state/layer/cooldown은 기존 `initialize_from_definition`을 사용한다.

덱 validator는 ID 존재·중복·소유·Main 목록 중첩·square/Pocket/captured 플래그를 검사한다. Board 전체 점유를 다시 전달받는 범용 runtime 감사기를 새로 만들지는 않았다. Board 점유와 captured 목록 배제는 factory가 보장하며 생성 후 테스트로 검증한다.

기존 `starting_pieces`는 초기 배치 목록이며 현재 Board 전체 목록이 아니다. Extra는 **현재 소속** 목록이라는 차이를 유지한다. G3/G5에서 zone을 이동할 때 출발 목록 제거, 목적 목록/Board 추가, 플래그·좌표·포획 상태 갱신을 하나의 원자적 전이로 처리해야 한다. 미래 Hand도 별도 명시적 소속을 가져야 한다.

일반 Move는 Board 기물만, Drop과 기존 Pocket 능력은 Pocket 목록/플래그를 사용한다. Extra에는 후보가 생기지 않는다. 위조 Extra Drop 요청도 기존 canonical submit 검사에서 거부된다. Move/Drop/Ability 구현과 TurnAction enum은 변경하지 않았다.

## 3. Eligibility 정책

`engine/src/rules.rs`의 공용 정책:

```rust
pub enum DeckZone { Main, Extra }
pub fn piece_deck_zone(type_id: &str, ruleset: DeckRuleset) -> DeckZone
pub const MAX_EXTRA_DECK_PIECES: usize = 3;
```

구행/폭격기 ID 비교는 이 정책 한 곳에 있다. 엔진 덱 검사, 초기 배치 predicate, 서버 요청 검사, 카탈로그 metadata 투영이 이를 사용한다. `DeckZone`은 덱 편성 허용 구분이며 Piece의 전체 runtime zone enum이 아니다.

`PieceDefinition`은 변경하지 않았다. 따라서 기존 정의·커스텀 package hash에 새 속성이 삽입되지 않는다. 커스텀 runtime ID는 현재 Main으로 분류한다. 제물 출처/비용/대상/재사용 metadata는 추가하지 않았다.

서버 `/api/piece-catalog`는 `standard_deck_zone`을 추가한다. 프론트 `PieceCatalogItem.standardDeckZone` 및 `canUseInMain`, `canUseInExtra`, ruleset 인자를 받는 `canUseInPocket`으로 편집기와 validation을 통일했다. 구형 metadata 응답에는 기존 카탈로그의 구행/폭격기 Extra 표시를 유지하며, 명시적인 알 수 없는 정책 값은 오류다. 프론트 판단과 별도로 서버가 최종 검증한다.

## 4. 구행·폭격기 및 수량 검증

| 기물 | 기존 score | deployment_zone | Legacy | Standard |
| --- | --- | --- | --- | --- |
| 구행 | 25 | Back | 기존 Starting/Pocket 허용 | Extra만 허용 |
| 폭격기 | 13 | Back | 기존 Starting/Pocket 허용 | Extra만 허용 |

Standard Extra는 0~3기 허용, 4기부터 게임용 validation 실패다. `array.length` / `Vec::len()`으로 실제 인스턴스를 센다. `bomber ×4`도 거부한다. King/일반 Main 기물/eligibility가 없는 커스텀 기물은 Extra로 플레이할 수 없다.

프론트 게임 validator, 서버 `validate_spec_deck_zones`, 엔진 `validate_deck_with_ruleset`에 적용했다. 초기 배치 predicate도 geometry와 Main eligibility를 함께 검사한다. Front/Back의 좌표 공식과 `deployment_zone=Back` 자체는 바꾸지 않았다.

Legacy에 Extra가 남아 있으면 게임 생성을 거부한다. 저장은 기존 미완성 초안 정책을 유지한다. 안전한 구조 한도는 Extra 최대 4096개/ID 최대 256자이며, 계정 요청의 기존 128 KiB 한도도 유지한다. 이 한도는 저장 안전 경계이고 게임의 3기 규칙과 다르다.

## 5. Main 점수

`calculate_deck_score`와 프론트 `calculateDeckScore`의 계산 대상은 그대로 Starting + Pocket의 non-King이다. Extra 목록을 합산하지 않는다. `Deck.total_score`는 게임 생성 시 같은 Main 계산으로 채운다.

정의의 점수를 0으로 바꾸지 않았다. 테스트에서 Main 39/39에 구행 25 + 폭격기 13 ×2를 추가해도 Main은 39다. 추가·제거 뒤에도 Main 점수는 동일하다. UI에는 Main Deck 점수와 Extra의 개별 점수를 분리해 표시한다.

## 6. 저장부터 GameState까지 전달

```text
SavedDeck.extra
  → cloneSavedDeck / normalizeExtra
  → LocalDeckRepository → localStorage → normalizeExtra
  → AccountDeckRepository → deckInput allowlist
    → DeckData.extra → sqlx Json / JSONB → SavedDeck 응답 정규화
  → savedDeckToPlayerDeckRequest / serializeNeutralDeck
    → PlayerDeckRequest.extra → PlayerDeckSpec.extra
    → CreateGame 또는 Room
    → materialize_neutral_deck (Extra는 그대로 복사)
    → build_player_deck (고유 PieceId 발급)
    → Player.deck.extra_deck_pieces + GameState.pieces
    → GameView / GameDynamicView.players → sync
```

Extra에는 좌표가 없으므로 Black 변환에서 rank mirror를 하지 않는다. 전체 view와 heartbeat는 이미 `players`를 전달하므로 별도 top-level 필드나 sync 구조 재작성 없이 Extra 목록이 보존된다.

에디터 저장 시 사용 타입 수집, 고정 커스텀 버전 갱신, `DeckInput::spec`의 사용 참조 검사, `resolve_custom_packages` 모두 Extra를 포함한다. 현재 커스텀 Extra 플레이는 거부하지만 저장/참조 수집 자체를 구행·폭격기 전용 포맷으로 만들지는 않았다. Extra에만 있는 커스텀 참조도 저장 시 소유·version/hash 검사 대상이다.

## 7. Local / Account 호환 및 Deck Code

Local 읽기, 저장, clone과 account 입력 allowlist/응답 정규화를 보완했다. 복제·이름 변경·계정 update는 기존 전달 경로로 Extra를 보존한다. 제거한 Extra가 다시 나타나지 않도록 빈 값 저장 왕복도 검사했다.

계정 `fingerprint`는 Extra 배열의 **복사본**을 정렬한 뒤 해시한다. 수량/타입 변화는 구분하고 순서 변화는 같은 덱으로 취급한다. 중복 기물을 제거하지 않으며 저장 원본 순서도 보존한다. 빈 Extra 생략으로 G1 이전 고정 fingerprint 회귀를 유지했다.

DB의 `deck_data JSONB` 제약은 객체 및 131072바이트 한도다. 새 열이나 migration은 필요하지 않다. account `format_version=1`을 유지한다. 실제 외부 PostgreSQL 접속 테스트는 실행하지 않았다.

DC1/DC2/DC3 스키마와 의미는 동일하다. Standard export는 계속 차단한다. Legacy로 전환한 Extra 초안도 직접 DC3 export로 유실되지 않도록 차단했다. 사용자가 명시적으로 Legacy 코드를 불러와 적용할 때는 그 코드의 빈 Extra로 교체하며, 미리보기 단계에서는 현재 초안을 수정하지 않는다. 새 Deck Code/Replay action/record version은 없다.

## 8. DeckEditor UI

Standard일 때 Main 점수 패널 다음에 독립 Extra 영역을 표시한다.

- 현재 수량/3기, 빈 슬롯, 각 인스턴스 이름·아이콘·원래 점수.
- 허용 카탈로그 기물별 추가 버튼과 각 인스턴스 제거 버튼.
- 3기에서 추가 비활성화 및 handler의 수량 재검사.
- Extra 점수가 Main 상한에 포함되지 않는다는 안내.
- Standard Main 팔레트에서 Extra-only 기물 제외. 클릭/드래그/포켓 증가 handler도 차단.

기존 Standard Main의 구행·폭격기는 삭제/자동 이동하지 않는다. validation 오류를 표시하고 지우개·Pocket 감소로 제거할 수 있다. Pocket 증가 버튼도 비활성화된다.

룰 전환은 Main/Extra를 수정하지 않는다. Legacy 전환 시 Extra 영역 대신 보존 상태와 처리 방법을 안내하며, 초안 저장은 가능하다. Standard로 돌아오면 기존 Extra가 다시 보인다. 크기 변경/명시적 Main 프리셋 적용은 기존 Main reset 동작을 유지하고 Extra는 보존한다. 전체 초기화는 확인 문구에 Extra까지 포함해 모두 비운다.

신규 Standard 최소 프리셋은 G2의 중앙 King + 모든 Front Pawn + 빈 Pocket/Extra다. 구행·폭격기를 자동 추가하지 않는다. 게임 화면의 Extra UI는 구현하지 않았다.

## 9. Multiplayer / Challenge

G1의 Room 룰셋 계약을 유지한다. 생성·join·host/guest 재선택에서 서버가 Main/Extra 편성 검사를 수행하고, 실패한 선택은 Room에 넣지 않는다. 최종 게임 factory에서도 다시 검사한다. 전체 geometry·score·커스텀 활성 권한 검증은 기존 최종 factory 경계에 남아 있다.

Standard Room의 양측 Extra는 중립 덱, Black materialization, 게임 view와 heartbeat를 통과한다. Legacy Room은 non-empty Extra를 거부한다.

기존 Challenge는 계속 Legacy이며 registry 내용은 변경하지 않았다. `challenge.rs` 변경은 `PlayerDeckSpec` literal에 빈 Extra 추가뿐이다. `raining_men` 구행 초기 Board 배치와 전체 Challenge factory/HTTP 회귀가 통과했다.

## 10. 상태 identity / hash / Replay

AI `PositionKey`의 `PlayerKey`에 정렬된 `extra_deck_pieces`를 포함했다. Board/Pocket/pieces가 같아도 Extra 소속이 다르면 key가 다르고, Extra 목록 순서만 바뀌면 같다. Extra의 평가값·소환 후보·전략은 추가하지 않았다.

분석 canonical hash 알고리즘은 그대로다. non-empty Extra는 GameState 직렬화에 포함되므로 hash가 달라진다. 빈 Extra와 기존 정의 JSON은 그대로여서 고정된 G1 이전 Legacy SHA 회귀가 통과한다. 기존 GameRecord의 initial_state/재생 차분은 players/pieces를 보존한다. Standard 전용 덱 snapshot/Deck Code/재실행 버전 정책은 G7 범위로 남겼다.

## 11. 공개 정보와 미확정 규칙

authoritative server state에 Extra를 보존했다. 현재 wire는 기존처럼 양쪽 players/pieces를 전송하므로 Extra도 포함된다. **이 기술적 전달을 상대 Extra 공개 규칙의 확정으로 해석하면 안 된다.** G6에서 공개 정책을 정할 때 view, heartbeat, record, Bot timeline 등 전체 전달 경계를 검토해야 한다. UI에서만 숨기는 것으로 해결되지 않는다.

미정인 Hand 상한·상대 Hand 공개·첫 턴 추가 Draw·빈 Pocket UI·Extra 공개·소환 위치·재사용·동일 기물별 추가 제한·비용 예외를 결정하지 않았다. 지금은 총 3기 제한만 있으며 동일 타입 여러 인스턴스를 허용한다.

Hand/Draw/RNG/ExtraSummon/제물/새 action/소환 클릭/재사용/Standard Bot 전략/Standard Challenge는 구현하지 않았다.

## 12. 주요 변경 파일과 필요한 이유

| 파일/그룹 | 이유 |
| --- | --- |
| `engine/src/types.rs`, `rules.rs` | runtime 소속, 중앙 eligibility, 덱 검증 |
| `engine/src/ai/transposition_table.rs` | 상태 identity와 회귀 |
| `server/src/main.rs` | 요청·생성·Room·카탈로그·통합 테스트 |
| `server/src/deck.rs`, `deck/tests.rs` | strict 저장 모델·fingerprint·참조 수집·계정 왕복 테스트 |
| `frontend/src/types/deck.ts`, `types/game.ts`, `api/gameApi.ts` | 저장/요청/runtime 타입 |
| `api/deckApi.ts`, `composables/localDeckRepository.ts`, `useDeckSerialization.ts` | 저장·API allowlist·좌표 변환 전달 |
| `composables/useDeckValidation.ts`, `views/DeckEditor.vue` | 구조/게임 검증과 Extra 편집 UI |
| `composables/useDeckCode.ts`, `useDeckCodeCodec.ts` | 기존 코드로 Extra 유실 차단, 명시 import 교체 |
| frontend 기존 덱/편집기/코드 테스트 4개 파일, `engine/tests/rule_engine.rs` | G4 회귀 및 G2와 달라진 허용 조건 명시 |
| 나머지 Rust fixture/생성자 파일 | 새 필수 Deck/PlayerDeckSpec 필드에 빈 목록 추가만 수행 |
| 이 문서 | 계약·검증·후속 결정 기록 |

변경 파일 수가 많은 주된 이유는 Rust struct literal들이 분산되어 있기 때문이다. 그 외 AI actions/evaluation, ammo/air/terrain, Challenge 내용, Chessembly 구현에는 기능 변경이 없다. 새 의존성·프레임워크·DB migration은 없다.

## 13. 검증 결과

| 명령/검증 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | 기존 runner 기준 21파일 passed, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과. 기존 script는 `vue-tsc --noEmit` |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 209 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 112 passed, 8 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| 변경 Rust 파일 `rustfmt --edition 2021 --config skip_children=true` | 적용 |
| `git diff --check`, 전체 변경 diff/파일 목록 검토 | 통과 |
| 실제 브라우저, 로컬 빌드 + 엔진 카탈로그 | Legacy/Standard 전환, 최소 프리셋, Extra 3기와 중복, 점수 25/13, Main 6/39 불변, 가득 찼을 때 추가 차단, 개별 제거, 로컬 저장 확인 |

G2의 Standard Back 구행/폭격기 허용 단언만 G4의 명시적인 금지로 갱신했다. geometry expected set 및 다른 기물 배치 검사는 유지했다. 신규 테스트 작성 과정의 컴파일 인자·API status·손상 localStorage fixture 문제를 수정했으며 기존 테스트를 삭제하거나 약화시키지 않았다.

기존 경고는 프론트 번들 500 kB 초과와 서버 미사용 생성자 2개다. ignored는 엔진 성능 벤치마크 7개, 서버 외부 DB 테스트 7개 및 payload 진단 1개다. 운영/외부 DB·배포·벤치마크·모바일 화면 및 전체 대국 브라우저 E2E는 미실행이다. 계정 왕복은 메모리 repository와 실제 HTTP router, JSONB는 스키마/adapter 확인이다. 브라우저용 외부 DB 없는 임시 서버는 검증 후 종료했다.

## 14. 요청 테스트 50개 대조

| 요청 번호 | 증거 |
| --- | --- |
| 1~5 구형 데이터·Legacy/Standard | `deckRepository.test.ts` 누락 Extra, 기존 G1/G2 저장 테스트, `deck/tests.rs` 구형 typed JSON/fingerprint |
| 6~8 local/account 0·3기 | frontend Extra repository 테스트 + server Extra account 왕복 테스트 |
| 9 clone | 실제 `DeckEditor` clone 배열 독립성/보존 |
| 10 update/import identity | 서버 Extra fingerprint 차이·정렬·재import·update/GET |
| 11 제거 저장 | frontend 양 repository 및 server update 후 재조회 |
| 12~16 0·1·3·4기/실제 수량 | frontend validation 및 `extra_factory_preserves_instances_scores_and_inert_zone` |
| 17~22 Standard 구행/폭격기 Main 금지·Extra 허용 | frontend 클릭/드래그/validation, 양 기물 Board/Pocket factory 테스트 |
| 23~25 Legacy Back/Challenge | 양 기물 Legacy Board/Pocket 성공 및 기존 `raining_men` factory/registry suite |
| 26~30 score 보존/분리 | 정의 25·13 검사, frontend/runtime Main 39/39, engine 추가/제거 점수 불변 |
| 31~37 runtime ID/zone/Move/Drop/능력 | factory 양 진영 ID·Board·Pocket·capture·후보·위조 Drop, 교대병/공수부대의 비어 있지 않은 능력 후보 비교 |
| 38~42 룰셋/Room/전환 | Legacy factory/Room 거부, Standard Room 재선택·join·sync, Vue 룰 전환 내용 보존 |
| 43~48 G2 geometry/Front/Drop/탄약/Legacy | 전체 rule_engine 및 ammo_air_layer suite, frontend G2 좌표/프리셋 회귀 |
| 49 identity | AI Extra 소속 차이 및 순서 동등성, server canonical hash Extra 차이 |
| 50 Legacy hash/Replay | 고정 pre-G1 SHA 및 기존 analysis/game_record/frontend replay suite |

추가로 engine validator의 누락 ID, 중복 ID, Main 목록 중첩, 잘못된 square/owner/Pocket/captured 플래그를 거부하는 테스트를 추가했다. 커스텀 참조가 Extra에만 있을 때도 고정 version/hash 저장과 외부 소유 거부를 검사한다.

## 15. G3 / G5 재사용 API와 결정 사항

G3는 `extra_deck_pieces`를 건드리지 않는 별도 Hand 소속을 추가해야 한다. 일반 Drop 위치는 G2의 Base ∪ Attack Map을 재사용한다. 첫 턴 추가 Draw, Hand 상한/공개, 빈 Pocket UI, 기존 교체/공수/복귀 능력의 Hand/Pocket 상호작용, RNG 및 Replay 재현 계약을 결정해야 한다. 강제 착륙 중간 action은 새 턴이 아니라는 기존 계약도 유지한다.

G5는 `Player.deck.extra_deck_pieces`, `GameState.pieces`, 정의의 원래 `score`, `piece_deck_zone`을 재사용할 수 있다. 고정된 구행 Hand/Board, 폭격기 Board 제물 조건은 G5에서 구현한다. 소환 가능 칸, 비용 정확 일치의 예외, 재사용/귀환, 추가 동일 타입 제한, 공개 범위를 확정해야 한다. 이번에는 이들을 위한 추측성 metadata/API를 추가하지 않았다.

## 16. G4 완료 조건

- [x] Standard Extra 모델 및 Main/Pocket/Board와 별도 소속
- [x] 총 3기 제한, 중복 타입의 여러 인스턴스 표현
- [x] 구행·폭격기 Standard Extra-only 및 Legacy Back 보존
- [x] 원래 piece score 유지, Main 상한 제외
- [x] Local/Account 저장·복사·API 왕복과 구형 누락 호환
- [x] 게임 생성·양측 Room·GameState·sync의 Extra PieceId 보존
- [x] 일반 Move/Drop 및 Pocket 능력에서 Extra 제외
- [x] Legacy non-empty Extra 게임 거부, 미완성 저장 초안 유지
- [x] Standard 덱 편집기 구성·제거·수량/점수 표시
- [x] G2 Base/Home·Front/Back·탄약 회귀 통과
- [x] Legacy/Challenge/DC1/DC2/DC3/Replay/hash/커스텀/Chessembly 회귀 통과
- [x] Hand/Draw/ExtraSummon 및 공개 정책 미구현
- [x] 관련 suite, typecheck/lint/build 통과. 미실행 외부 검증은 §13에 명시
