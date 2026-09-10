# G3-A — Hand Zone / Standard 일반 Drop / Hand 공개 경계

작성일: 2026-09-10 (KST). 작업 시작 시 존재하던 G4 미커밋 변경을 보존하고 그 위에 G3-A를 추가했다. 이번 범위는 runtime Hand, 일반 Drop의 출처, 실시간 서버 공개 경계다. 초기/턴 시작 Draw, RNG/seed, Draw action/notation, ExtraSummon, Hand 평가, 전체 Hand UI는 구현하지 않았다.

## 1. 데이터 모델과 저장 경계

실제 소속은 `engine/src/types.rs::Deck.hand_pieces: Vec<PieceId>`이며 `GameState.players[player_id].deck.hand_pieces`로 접근한다. Pocket/Extra와 별도 목록이다. 좌표나 플래그만으로 Hand를 추론하지 않는다. 같은 타입의 서로 다른 PieceId를 여러 장 보유할 수 있고 장수 제한은 없다.

`#[serde(default, skip_serializing_if = "Vec::is_empty")]`로 구형 GameState의 필드 누락은 빈 Hand로 읽고, 빈 Hand는 직렬화하지 않는다. 명시적 null은 배열로 취급하지 않는다. 신규 게임 factory는 두 플레이어 모두 빈 Hand를 만든다. 생산 경로에서 Pocket을 자동으로 이동시키지 않는다.

`LobbyDeck`, `SavedDeck`, `DeckData`, `PlayerDeckSpec`, DC1/DC2/DC3, 덱 빌더의 저장 모델에는 Hand 필드를 추가하지 않았다. 프론트의 `types/game.ts::Deck.hand_pieces?`는 **실행 상태** 타입이다.

## 2. Hand zone 불변조건과 검증

`engine/src/hand.rs::validate_hand_zones(&GameState)`가 다음을 검사한다.

- Hand ID가 `pieces`에 존재하고 map key와 `Piece.id`가 일치한다.
- Player map key, `Player.id`, `Deck.player_id`, `Piece.owner`가 일치한다.
- 같은 목록 및 양측 Hand 전체에 중복 ID가 없다.
- 양측 Starting/Pocket/Extra/captured 목록과 겹치지 않는다.
- 지상 Board와 공중 Board 점유에 없다.
- `current_square=None`, `in_pocket=false`, `captured=false`다.
- Legacy에서 Hand는 비어 있어야 한다.

구조 역직렬화와 의미 검증은 별개다. public `actions::submit_action`은 Hand 불변조건을 검사하고, Standard 일반 Drop 생성도 authoritative 소속과 불변조건을 검사한다. 클라이언트가 기물 ID를 제출해도 플래그나 클라이언트 view를 권위 값으로 사용하지 않는다.

## 3. G3-B용 결정적 내부 전이

`move_pocket_piece_to_hand(&mut GameState, &PlayerId, &PieceId) -> Result<(), String>`를 제공한다. 선택된 **한 ID**를 검증하고 복사본에서 Pocket 제거, Hand 추가, `in_pocket=false`를 적용한 뒤 전체 Hand 검증에 성공해야 원본을 교체한다. 잘못된 소유/중복/Pocket 플래그/Board·Extra·captured 충돌은 원본을 변경하지 않는다.

이 함수는 무작위 선택, Draw 수량, 턴 시점, history/event를 결정하지 않는다. HTTP로 노출하지 않았으며 현재 호출자는 테스트뿐이다. 클라이언트가 Draw 대상을 선택하는 경로도 없다.

`starting_pieces`는 원래 초기 배치 이력 목록이다. 기존 능력으로 돌아온 초기 기물은 Pocket에 있으면서 이 목록에도 남을 수 있다. 위 전이에서는 **Hand로 옮기는 해당 ID만** runtime starting 목록에서 제거하여 Hand/Starting 배타성을 지킨다. 이미 동결한 GameRecord의 초기 배치·덱 snapshot은 수정하지 않는다. Pocket 능력 자체는 이 정리를 수행하거나 Hand로 이동시키지 않는다.

## 4. 일반 Drop 및 원자성

| 룰 | 일반 Drop 출처 | 위치 |
| --- | --- | --- |
| Legacy | `pocket_pieces` + 기존 Pocket 상태 검사 | 기존 Legacy Base ∪ Attack Map |
| Standard | `hand_pieces` + Hand 불변조건 | G2 Standard Base ∪ Attack Map |

`hand::ordinary_drop_pieces`를 전체/기물별/타입별 Drop 생성 및 search 후보 열거에 연결했다. `placement::validate_drop_action`도 같은 출처 검사를 사용한다. `endgame::apply_drop_action`은 canonical action에 대해 룰셋에 맞는 출발 목록을 제거하고 기존 Board/좌표/플래그 변경을 수행한다.

`TurnAction::Drop`, canonical 필드, history 형식은 동일하다. 공개 submit은 합법 목적지와 capture 결과까지 기존 generator의 canonical action과 비교한다. 목적지 오류, Pocket-only, Extra, 상대 Hand, 목록 없이 플래그만 꾸민 ID는 거부한다. 실패 시 입력 상태는 그대로다. 서버는 기존 복사본 검증 → 시간 재확인 → 상태 확정 계약을 유지한다. 별도의 시계 만료 판정은 기존처럼 게임을 종료할 수 있다.

위치 기하·지형·capture-on-drop·air layer 알고리즘은 바꾸지 않았다. 포획 기물 처리, 포탄의 착수 후 폭발, 앙파상 만료, 턴 증가와 강제 착륙 계약도 기존 경로를 사용한다. `apply_drop_action`은 이전과 마찬가지로 이미 canonical인 action을 적용하는 저수준 함수이며 외부 입력의 진입점은 `submit_action`이다.

## 5. 기존 Pocket Ability 및 Extra 보존

교대병 `relieve`, 공수부대 `airdrop`, 그린캠프 `recall`의 생성·적용 코드는 수정하지 않았다. Standard에서도 Pocket ID와 Pocket 플래그를 직접 사용한다. Hand/Extra에 같은 타입이 있어도 능력의 Pocket 후보에 섞이지 않는다. Board에서 Pocket으로 돌아온 기물은 Pocket에 남고 일반 Drop 후보가 되지 않는다.

Extra의 소속, eligibility, 최대 3기, 구행/폭격기 Extra-only, Main 점수 제외, factory, runtime 상태는 유지했다. G4의 룸 테스트는 비공개 덱 전체 응답 대신 서버 원본의 Extra 보존을 확인하도록 갱신했으며, 생성된 게임/sync에서 Extra가 유지되는 검사는 그대로다. 새로운 소환/제물 동작은 없다.

## 6. 서버 client projection

`server/src/game_view.rs`가 실시간 공개 경계다. `StoredGame.state`와 `record`는 모든 Hand를 보존한다. `project_state`는 복사본을 만든 뒤 허용되지 않은 정보를 제거한다. 이 부분 상태는 응답용이며 엔진에 다시 제출하지 않는다.

실제 Standard wire 예시는 다음과 같다. 기존 필드 중 관련 부분만 표시했다.

```json
{
  "hand_counts": { "white": 2, "black": 3 },
  "players": {
    "white": { "deck": { "hand_pieces": ["own-id-a", "own-id-b"], "pocket_pieces": ["own-reserve"] } },
    "black": { "deck": { "pocket_pieces": [] } }
  },
  "pieces": { "own-id-a": { "owner": "white", "type_id": "knight" } }
}
```

- 자기 Hand의 ID와 실제 Piece object는 제공한다. 빈 배열은 sparse serializer로 생략될 수 있다.
- 상대 `hand_pieces`는 wire에서 생략하고 `hand_counts[side]`만 제공한다. 장수는 full view, heartbeat dynamic, Bot timeline의 각 state에 있다.
- 상대 Hand Piece object를 `pieces`에서 제거한다.
- **상대 Pocket의 ID 목록과 Piece object도 함께 가린다.** 이전 응답의 Pocket에서 사라진 ID를 빼서 미래 Hand를 알아내는 우회 경로를 막기 위해서다. 이는 공개 projection 변경이며 엔진 Pocket semantics는 그대로다.
- 돌아온 초기 기물의 Starting 목록에서도 숨긴 reserve ID를 제거한다.
- 숨긴 reserve에만 존재하는 커스텀 package는 manifest와 그 package의 보조 runtime 정의까지 제외한다. 보이는 인스턴스가 사용하는 package 및 일반 내장 카탈로그는 유지한다.

`GameView`, `GameDynamicView`, `GameStaticData`, Bot frame 모두 같은 투영 함수를 사용한다. catalog revision은 공개 대상과 현재 투영된 정의 목록을 반영하며 JS safe integer 범위다. 다른 대상의 revision을 보내도 자기 카탈로그와 일치할 때만 생략한다. 카탈로그 내용의 기존 버전은 base revision으로 반영한다. Legacy revision은 기존 값을 유지한다.

프론트는 `hand_pieces?`, `GameState.hand_counts?`로 내용을 표현한다. `mergeGameSync`는 이전 `players`/`pieces`를 병합해 남겨두지 않고 새 dynamic 값으로 교체하므로 catalog 없는 sync에서도 사라진 ID가 재등장하지 않는다.

## 7. 모드별 view와 권한 계약

기존 탭 `client_id`를 private view의 bearer capability로 사용한다. 프론트 공용 game API는 `x-game-client-id` 헤더를 보낸다. 서버는 게임 생성 당시의 capability와 진영 관계를 `StoredGame.access: GameAccess`에 저장하며 이 값은 serialize하지 않는다. 요청의 `player_id`, `local_side`를 조회 시점의 임의 공개 권한으로 사용하지 않는다.

| 모드 | 서버에 고정되는 계약 | 실시간 응답 |
| --- | --- | --- |
| Standard Multiplayer | 방의 host/guest client ID → 각 진영 | 해당 참가자의 Hand/Pocket만 공개 |
| Standard 로컬 2인 | 생성한 탭의 client ID, human 지정 없음 | 그 탭에만 양측 Hand/Pocket 공개 |
| Standard Bot | 생성한 탭 client ID + 인간 진영 | 인간 Hand/Pocket만 공개 |
| 알 수 없는/누락된 capability | `Public` | 양쪽 Hand/Pocket identity 제거, Hand 장수 공개 |
| 서버 내부 Bot/search | 원본 `GameState` | 전체 authoritative 상태 사용 |
| Legacy/Challenge | 기존 계약 | 기존 응답·Pocket 규칙 보존, Hand 기본 empty |
| 완료 기록/Replay | 기존 기록 접근 검사 | 기존 authoritative 기록 정책 유지; 최종 Standard 공개 정책은 G3-C/G7 |

로컬 2인은 실제 프론트가 양측을 조작하므로 명시적인 shared 계약을 사용한다. Bot과 로컬 2인은 기존 `local_side`만으로 구분되지 않았기 때문에 생성 요청에 optional `bot_player_id`를 추가했다. 서버가 인간의 반대 진영인지 검사하고 모드를 고정한다. nickname의 “Bot” 문자열로 추론하지 않는다. Standard Bot 실행은 지정된 인간 클라이언트만 요청할 수 있으며 Multiplayer나 다른 진영을 Bot으로 실행해 손패를 조사할 수 없다. Bot 생성 시 모드를 지정하지 않으면 로컬 2인 계약이고, 그 게임에 Standard Bot endpoint는 허용되지 않는다.

룸 heartbeat/기권은 기존 body의 client ID와 player ID 일치를 검사한 뒤 해당 진영으로 투영한다. GET, submit, 합법수, Bot, 일반 기권은 헤더 capability를 사용한다. 재입장에서도 저장된 참가자 ID를 확인해 자기 view를 만들며, 낯선 client ID에는 public view만 반환한다. 일반 합법수/기물 옵션 요청은 Standard 현재 턴 진영의 권한을 요구한다. 잘못된 진영의 submit/기권도 거부한다.

Standard 룸의 `host_deck`/`guest_deck` 전체 내용은 응답에서 null로 바꾸고 `host_has_deck`/`guest_has_deck`로 선택 상태를 제공한다. 프론트는 이미 서버의 준비 상태와 자신의 선택 덱을 사용하므로 전체 상대 덱을 받지 않아도 된다. Legacy 룸 내용은 유지한다. 룸 원본 덱과 게임 내 Extra는 제거하지 않는다.

## 8. 검사한 누출 경로

| 경로 | 처리 / 검증 |
| --- | --- |
| GET `/games/:id`, 생성 응답, submit/기권 응답 | 수신자별 `view_for`, 낯선 탭은 Public |
| `/rooms/:id/heartbeat` | 참가 진영 검증 → `sync_view_for`; catalog 포함/생략 모두 JSON 검사 |
| 룸 생성/조회/재선택/준비/재입장 | 상대 reserve의 원본 덱 목록 미전송, 게임 재입장 view 투영 |
| `/legal-drops`, `/legal-moves`, `/pieces/:id/options` | 상대 턴/무권한 요청 403; 자기 합법 Drop만 반환 |
| piece/player attack map API | 기존 보드 위 기물 공격맵만 계산, Hand/Pocket 기물 객체·목록 없음 |
| `/bot-turn`와 debugger | 지정된 Bot 게임만 실행, 각 frame의 Piece/Player/카탈로그 투영; Standard stats 생략 |
| `/record`, analysis APIs | 진행 중 record export 차단 유지, analysis는 소유한 저장 완료 기록만 사용 |
| Lab/custom-piece test APIs | 제출된 임시 상태를 별도 생성; live GameStore Hand 조회 경로 없음 |
| 응답 캐시 | games/rooms API에 `Cache-Control: no-store`, `Vary: x-game-client-id` |

Bot의 원본 탐색 score/candidate 통계도 숨긴 reserve의 정보에 영향을 받으므로 Standard 응답에서 생략한다. 프론트 stats 타입과 기존 debugger 표시만 optional에 대응했다. Legacy Bot 통계는 그대로다.

과거에 보드에 드러난 기물과 공개된 action/notation의 지식까지 지우는 계약은 아니다. 기존 공개 history/notation은 유지하며, Hand/Pocket을 함께 감춰 그 기록의 ID가 **현재 어느 reserve zone에 있는지** 새로 공개하지 않는다. 게임 규칙상 공개된 이동·반환·장수에서 가능한 추론 자체를 제거한다고 주장하지 않는다. future Draw event를 history/delta에 추가할 때는 G3-B/C에서 별도 공개 투영이 반드시 필요하다.

## 9. PositionKey / hash / Replay

AI `PlayerKey`에 정렬된 `hand_pieces`를 포함했다. Board/Pocket/Extra 및 Piece flags가 같아도 Hand 소속이 다르면 다른 key이며 목록 순서만 바뀌면 같다. UI/원본 목록 순서는 변경하지 않는다.

canonical analysis hash 알고리즘은 변경하지 않았다. non-empty Hand는 원본 GameState JSON에 들어가므로 hash를 구분한다. 이 hash는 직렬화 목록 순서도 보존하므로 search key의 집합 동일성과 계약이 다르다. 빈 Hand는 생략되어 pre-G1 Legacy 고정 SHA 회귀가 유지된다. 별도 Hand material/Draw expectation/전술 평가를 추가하지 않았다. Search의 일반/포획 Drop 후보만 엔진과 같은 출처를 사용한다.

GameRecord format/version, 기존 TurnAction, DC1/DC2/DC3, Replay delta/action 형식은 변경하지 않았다. 기록 생성/검증에는 client projection을 사용하지 않는다.

## 10. 변경 파일과 이유

G3-A 시작 시 파일 사본과 비교한 목록이다. 기존 G4 변경을 G3-A가 새로 만든 변경으로 계산하지 않았다.

| 파일 | G3-A 변경 이유 |
| --- | --- |
| `engine/src/hand.rs` (신규), `lib.rs`, `types.rs` | 명시적 소속, 불변조건, 결정적 원자 전이 |
| `engine/src/actions.rs`, `legal_moves.rs`, `placement.rs`, `endgame.rs` | canonical 검증 및 룰셋별 일반 Drop 출처 |
| `engine/src/ai/search.rs`, `ai/transposition_table.rs` | 합법 Drop 열거·위치 identity 및 회귀 |
| `engine/tests/rule_engine.rs` | Hand/위조 요청/원자성/Pocket Ability, G2 위치 fixture |
| `server/src/game_view.rs`, `game_view/tests.rs` (신규) | 권한·공개 투영, 캐시 정책, 실제 HTTP/JSON privacy 테스트 |
| `server/src/time_control.rs`, `main.rs`, `routes.rs` | factory/모드 권한, full/sync/Bot/룸/API 경계 연결 |
| `frontend/src/types/game.ts`, `api/gameApi.ts`, `api/gameApi.test.ts` | runtime Hand/장수, capability 전달, Bot 모드, sync 회귀 |
| `frontend/src/App.vue`, `components/GameScreen.vue` | Bot 생성 계약 전달, 생략된 stats 대응 |
| `STANDARD_FORMAT_IMPLEMENTATION_PLAN.md`, 이 문서 | 확정 규칙·Goal 분할·구현/검증 결과 |
| `engine/src/ai/beam.rs`, `ai/evaluate.rs`, `attack_map.rs`; `engine/tests/ai.rs`, `ai_benchmark.rs`, `ammo_air_layer.rs`, `custom_piece_runtime.rs`, `piece_options_benchmark.rs`, `v2_supplement.rs`; `server/src/custom_piece.rs` | 기존 Deck struct literal에 빈 Hand 필드 추가만 수행 |

파일 수가 많은 주된 이유는 Rust struct literal의 필수 초기화, 서로 다른 실시간 endpoint와 full/sync/timeline 경계를 모두 연결해야 하기 때문이다. 새로운 의존성/DB migration/프레임워크/범용 PieceZone refactor는 없다. Chessembly 코드는 수정하지 않았다.

## 11. 검증 명령과 결과

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | 21개 파일 passed, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 script는 vue-tsc |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 217 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 116 passed, 8 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| 변경 Rust 파일 `rustfmt --edition 2021 --config skip_children=true` | 적용 |
| `git diff --check`, G3-A 시작 시 사본과 diff 검토 | 통과 |

테스트 작성 중 TurnAction의 PartialEq 부재는 serialized canonical action 비교로, 공수부대의 단일 선택값은 기존 deployments canonical 형식으로 수정했다. 기존 테스트를 삭제하거나 약화시키지 않았다. G2의 Standard Drop 테스트는 Pocket fixture를 명시적으로 Hand로 전이시킨 뒤 **같은 위치 expected set**을 검사한다.

기존 경고는 frontend 500 kB 초과 번들, server 미사용 생성자 2개다. ignored는 엔진 성능 벤치마크 7개, 서버 외부 DB 관련 7개와 payload 진단 1개다. 외부 PostgreSQL, 운영 배포, 브라우저 대국 E2E/모바일 시각 검증, ignored 성능 측정은 실행하지 않았다. 이번 UI 변경은 모드 전달과 optional 통계뿐이며 privacy 검증은 실제 Axum router/serialized JSON 및 frontend sync 자동 테스트로 수행했다.

## 12. 요청한 50개 테스트 대조 / G1·G2·G4 회귀

| 요구 번호 | 자동 검증 증거 |
| --- | --- |
| 1~10 | `hand_runtime_sparse_serialization_and_disjoint_zone_invariants`: 구형 읽기, 동일 타입 ID, 모든 목록·양측·지상/공중 Board·flags 충돌 |
| 11~18 | `standard_drop_uses_only_hand_and_preserves_canonical_history_and_atomicity`, HTTP privacy/submit 테스트 |
| 19 | 기존 `runtime_drop_uses_full_home_union_attack_map_for_both_rulesets_and_maps`: 8~12, 양측, 두 맵, Base ∪ Attack Map |
| 20 | `standard_hand_capture_drop_keeps_paratrooper_and_shell_followup_rules` 및 기존 ammo/air/terrain suite |
| 21~24 | `legacy_drop_candidates_and_transfer_rejection_keep_original_contract`, 기존 Legacy Drop/canonical/history 회귀 |
| 25~29 | `standard_pocket_abilities_keep_their_sources_and_returns_without_automatic_draw`: 교대/공수/그린캠프, Hand·Extra와 후보 분리, 복귀 후 일반 Drop 불가 |
| 30~37 | `game_view::tests` 실제 HTTP full/양측 heartbeat/catalog 생략/재입장/public/합법수/submit/Bot timeline JSON 전체에서 숨긴 ID 부재 검사. 커스텀 manifest/정의도 별도 검사 |
| 38~39 | `hand_membership_changes_position_key_but_order_does_not` |
| 40~41 | Hand canonical hash 변화 및 기존 analysis의 pre-G1 고정 Legacy SHA |
| 42 | 기존 ruleset 생성/룸/저장/sync suite |
| 43~44 | 기존 G2 geometry·Front/Back·모든 크기 배치 및 Drop 위치 suite |
| 45 | `ammo_air_layer` 및 기존 폭격기 강제 착륙/서버 history 회귀 |
| 46~47 | 기존 G4 Extra factory/validation/룸/sync, 구행·폭격기 Extra-only, Main 점수 회귀 |
| 48 | 기존 Challenge registry/factory/HTTP/clear suite |
| 49~50 | 기존 frontend DC1/DC2/DC3 및 Replay codec/state/notation, server record/analysis suite |

추가로 실패한 Pocket→Hand 전이의 원본 JSON 불변, search의 Hand Drop/포획 Drop 열거, 상대 Pocket→Hand 전이 전후 **projected state가 동일하고 Hand 장수만 변함**을 검사한다. Draw/RNG를 실행한 테스트는 아니다.

## 13. G3-B의 정확한 연결 지점과 남은 위험

- 엔진 소속 전이: `hand::move_pocket_piece_to_hand`. G3-B의 서버 정책이 선택한 ID만 전달한다. 복수 Draw를 한 번에 확정하려면 전체 복사본에 여러 번 적용하고 성공 후 commit한다.
- 초기화: `server/main.rs::build_game_state_with_variant`가 양측 기물을 만든 뒤 `create_game`/`start_room_game`이 `StoredGame::new_with_players_and_deck_names`를 호출한다. 이 사이에서 초기 Draw 상태와 원래 편성 덱 snapshot을 구분해야 한다. **Draw 뒤의 Pocket만으로 원래 덱 snapshot을 만들면 초기 3기가 빠진다.** G3-B에서 이 기록 시점을 함께 연결해야 한다.
- 실제 턴 전환: `engine/endgame.rs::apply_and_advance_turn`의 current_player 변경 지점. 서버 `submit_action`은 엔진 결과를 받고 확정 직전 시간을 재검사한 후 `game.state`와 record delta를 확정한다. Draw는 실패 요청/강제 착륙 중간 action이 아닌 실제 새 턴에만 연결해야 한다.
- Bot: `server/main.rs::run_bot_turn`은 `play_bot_turn_detailed`의 결과와 각 frame을 기록한다. G3-B는 마지막 실제 턴 전환의 Draw를 권위 결과/frame/record에 같은 시점으로 반영해야 하며 탐색 중 실전 RNG를 소비해서는 안 된다.
- 공개 경계: `view_for`, `sync_view_for`, `game_view::project_state`는 non-empty Hand를 이미 지원한다. 미래 Draw 이벤트나 state delta에 숨긴 ID를 추가할 경우 이 경계의 history/notation 정책도 확장해야 한다.
- capability는 기존 탭 client ID의 bearer 계약이다. 값은 응답에 공개하지 않는다. 다른 탭/소실된 탭에는 자동으로 양측 권한을 부여하지 않는다. 장기 세션 복구·새 인증 구조는 이번 범위가 아니다.
- live client state는 부분 상태다. 이를 authoritative GameState로 검색/기록 검증에 재사용하면 안 된다. 서버 내부 Bot은 여전히 양측 정보를 사용하며 Standard 전략/불완전 정보 탐색은 G8 범위다.
- 완료 기록은 기존 정책을 유지했다. 최종 Standard 기록 공개/Draw 재실행/notation은 G3-C/G7에서 정리해야 한다. 실제 게임에는 아직 자동 Draw가 없어 Standard 일반 Drop용 Hand가 비어 있다는 제한이 의도적이다.

## 14. G3-A 완료 조건

- [x] Hand의 명시적 runtime 소속 및 Pocket/Extra/Board/Captured 배타성
- [x] Standard Hand 일반 Drop, Legacy Pocket 일반 Drop
- [x] G2 위치·capture 후속 규칙 및 canonical/history 보존
- [x] 기존 Pocket Ability와 Pocket 복귀 의미 보존
- [x] 자기 identity / 상대 Hand 장수, wire에서 숨긴 Piece/목록 제거
- [x] full/heartbeat/catalog/합법수/룸/Bot 등 일반 실시간 우회 경로 검사
- [x] 로컬 2인·Bot·Multiplayer의 명시적 권한 계약
- [x] Hand PositionKey/hash 구분, Legacy 고정 hash 및 G1/G2/G4 회귀
- [x] 저장 덱에 Hand 없음, 자동 Draw/RNG/ExtraSummon/전체 Hand UI 없음
- [x] 요구된 테스트/typecheck/lint/check/build 통과; 미실행 외부 검증 명시
