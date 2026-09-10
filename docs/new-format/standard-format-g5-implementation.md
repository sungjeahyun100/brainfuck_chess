# G5 — Extra Summon / Sacrifice

작성일: 2026-09-10 (KST). 목표 첨부 파일의 G5 확정 규칙과 현재 G1/G2/G3-A/B/C/G4 작업 트리를 기준으로 구현했다. 시작 시 미커밋 변경을 보존했고 `/tmp/g5-baseline` 사본과 비교하여 G5 추가분을 검토했다.

## 1. Canonical action

```json
{
  "type": "extra_summon",
  "player_id": "white",
  "extra_piece_id": "white_guhang_instance",
  "sacrifice_piece_ids": ["queen3", "queen1", "queen2"],
  "target_square": { "file": 4, "rank": 0 }
}
```

`engine/src/types.rs::ExtraSummonAction`, `TurnAction::ExtraSummon`이다. 클라이언트 submit은 위 값에서 `player_id`만 제외한 intent를 `{ action: ... }`에 담는다. 서버는 저장된 현재 player를 넣고 기존 capability 권한을 확인한다. 제물 순서/내용을 정렬·축약·대체하지 않는다. 별도 Sacrifice player action이나 중간 commit은 없다. capture 결과는 목적지의 authoritative 상태에서 결정하며 클라이언트가 지정하지 않는다.

기존 Move/Drop/Ability 직렬화는 그대로다. 새로운 tagged variant만 추가했고 GameRecord format_version=2, snapshot_version=1, DC-G2 envelope를 유지했다. 기존 JSONB action 열은 variant를 저장할 수 있어 별도 schema 변경이 필요하지 않다. 구형 기록에는 variant가 없으므로 이전 동작으로 계속 읽는다.

## 2. 중앙 소환 정책

`engine/src/summon.rs::summon_policy(ruleset, type_id) -> Option<SummonPolicy>`:

```rust
pub enum SacrificeZone { Hand, Board }
pub struct SummonPolicy { pub sacrifice_zones: &'static [SacrificeZone] }
```

Standard 구행은 `[Hand, Board]`, 폭격기는 `[Board]`, Legacy 및 다른 타입은 None이다. `rules::piece_deck_zone`도 이 API를 사용한다. ID 비교는 정책 한 곳에만 존재한다. 비용은 PieceDefinition.score이며 별도 cost override/exact/reusable/script/editor 필드는 없다. 미래 zone은 실제 요구가 생기면 enum과 source 판정을 확장한다. 현재 Pocket 제물은 제공하지 않는다.

## 3. 비용과 제물 검증

구행 25, 폭격기 13은 기존 정의 값이다. 선택한 모든 정의 score를 u64로 합산하여 `sum >= cost`를 검사한다. 27점으로 25점을 소환하면 27점 기물을 전부 제거한다. 남은 점수/환급/최소 조합 대체는 없다.

public `actions::submit_action`에서 종료·Hand 불변조건을 확인한 뒤 `validate_extra_summon`이 Standard/현재 actor/실제 자기 Extra 소속/소환 정책/강제 착륙 대기를 검사한다. 제물의 고유 ID, 실재, 자기 소유, non-King, 정책이 허용한 현재 zone, score를 확인한다. 잘못된 ID 형식도 거부한다. 서버 후보 endpoint 역시 제출한 선택에 중복/허용 후보 밖 ID가 있으면 오류로 반환한다.

## 4. 제거 semantics

제물은 `captured=true`, `current_square=None`, `in_pocket=false`로 표현한다. Board는 해당 ground/air 점유에서 제거하고 Hand는 `hand_pieces`에서 제거한다. `Player.captured_pieces`에는 넣지 않는다. 이 목록은 실제 포획 기록용이다.

기존 `remove_captured_piece`는 상대 포획 기록과 Fanatic death/capturer cascade를 함께 처리하므로 제물 제거에는 호출하지 않는다. 따라서 제물로 바친 Fanatic의 적 포획 후속 효과나 포획 보상이 발생하지 않는다. 소환 목적지에서 발생하는 **실제 capture**에는 기존 제거·후속 규칙을 그대로 적용한다. captured 플래그의 의미는 저수준 게임 제외 상태이며, 공개 canonical action이 제거 원인을 구분한다. 대규모 capture subsystem 변경은 없다.

## 5. 원자성과 zone invariant

`validate_summon_zones`는 기존 Hand 검사와 Extra/Pocket의 중복·소유·플래그·목록 충돌, Board의 양 레이어 점유/좌표/실재/소유를 검사한다. Starting은 초기 이력 목록이며 현재 Board 목록으로 재해석하지 않는다. 현재 두 Extra 기물은 초기 Ground 상태로 존재하며 잘못된 Air reserve 상태는 거부한다.

검증과 target 생성은 복사본에서 수행한다. 성공한 canonical action에 한해 제물 전부 제거 → Extra ID 제거 → 공용 Drop 목적지 적용을 수행한다. 서버는 기존 복사본 action → 시간 재확인 → Draw → state/clock/record 확정 경계를 유지한다. 실패한 요청은 기물/소속/기록/clock finish를 변경하지 않는다. 기존 시계 만료 adjudication은 별도 계약으로 유지한다.

소환 기물의 초기 ammo/state/cooldown을 다시 초기화하지 않는다. 기존 정상 턴의 cooldown/비행 시간 처리는 동일하게 적용된다. 같은 타입의 다른 Extra ID는 남는다.

## 6. Target 공용 경로와 판정 시점

`legal_moves::generate_piece_drop_targets`를 일반 Drop의 source 검사에서 분리했다. 일반 Drop은 계속 `is_ordinary_drop_source`를 먼저 검사한다. Extra를 Hand에 임시로 넣는 방식은 사용하지 않는다.

ExtraSummon은 제물을 제거한 hypothetical Board에서 공용 target 함수를 호출한다. 따라서 제물로 비운 자기 칸은 소환 후보가 될 수 있고, 제물이 제공하던 Attack Map은 제거 후 상태에서 다시 계산된다. 이는 비용 지불 후 소환이라는 적용 순서와 일치하며, UI도 같은 서버 결과를 표시한다.

공용 경로는 G2 `get_piece_placement_squares` → Base/Home ∪ Attack Map → bounds/점유/capture-on-drop → `can_capture_piece`의 지형·wall 검사를 사용한다. 기존 Drop과 같이 비포획 빈 고지 칸 배치는 허용하고 지상 높이에서 고지의 적을 capture하는 경우는 거부한다. 기존 Ground 소환과 Air 점유 공존 계약을 유지한다.

`endgame::apply_drop_placement`는 실제 포획, fanatic capturer 제거, shell 효과, 앙파상 만료 및 종료 처리를 공유한다. 일반 Drop 출처 제거 코드는 기존 위치에 남아 있다.

## 7. 이후 삶과 턴/Draw

소환된 ID는 Board 기물처럼 이동/포획/능력/Pocket 복귀의 대상이다. Extra 목록에 자동 추가하는 경로는 없다. 그린캠프 recall로 Pocket에 돌아온 소환 기물은 기존 Draw로 Hand에 들어가고 일반 Hand Drop을 할 수 있다.

ExtraSummon은 `apply_and_advance_turn`을 한 번 통과하고 history에 하나의 action을 남긴다. 기존 endgame → 비행 시간/cooldown → forced landing → current_player 변경 계약이다. 서버 `draw::starts_turn`/`turn_start`를 재사용하여 실제 상대 턴 시작만 Draw한다. 중간 착륙 대기, 종료 또는 실패 요청은 Draw하지 않는다.

## 8. 공개 projection

기존 `game_view::project_state`는 숨길 상대 Pocket/Hand ID 집합만 제거하므로 양쪽 Extra는 시작부터 Piece object/종류/score/남은 수량이 공개된다. full view, heartbeat, 재조회/guest 재입장 모두 같은 경계다. 소환 후 해당 Extra ID는 목록에서 사라진다.

희생한 Hand는 확정 후 Hand 목록을 떠난 removed Piece이므로 공개 상세에서 이름과 score를 읽을 수 있다. 선택 중인 자기 Hand 후보는 현재 진영 권한이 있는 endpoint에만 제공된다. 상대 Hand/Pocket ID는 계속 숨긴다.

기존 custom manifest/정의 projection을 유지한다. 현재 소환 정책은 내장 구행/폭격기만 Extra에 허용하므로 Extra 공개가 숨긴 custom reserve package를 새로 공개할 경로를 만들지 않는다. 실제 보이는 인스턴스가 쓰는 package만 보이는 기존 계약 및 해당 privacy 회귀 테스트를 유지했다.

## 9. 최소 게임/분석 UI

공용 `ExtraSummonPanel.vue`를 GameScreen과 ReplayPage에 연결했다.

1. 양쪽 Extra 목록과 개별 score 표시.
2. 현재 자기 차례의 Extra 선택.
3. 서버가 제공한 자기 Hand/Board 후보 선택.
4. 선택된 목록, 합계/필요 점수, 초과분 환급 없음 표시.
5. 서버 합법 target 버튼 및 보드 강조; 보드를 클릭해 target 선택 가능.
6. 명시적인 소환 확정 또는 취소.

조회 중/확정 중 중복 조작을 차단하고 게임 상태 변경과 취소에 대한 늦은 응답을 무시한다. clock-only heartbeat는 제물 선택을 유지하고 실제 Board/zone/턴 변경만 선택을 취소한다. 서버 오류를 표시한다. 제물을 자동으로 줄이지 않는다. 분석에는 기존 draw_pending 안내와 저장 대기를 연결했다. 게임 마지막 action 및 Replay/Analysis 현재 action 상세에 소환 기물·제물들·총점·위치를 표시한다. 전체 HUD/애니메이션은 G6 범위다.

## 10. Legal API와 Bot/search

`POST /api/games/:id/summon-options`는 `{ extra_piece_id, sacrifice_piece_ids? }`를 받는다. 응답은 후보 ID 목록, policy, cost, 선택한 목록에 대한 canonical action payload 목록이다. 부족 점수의 유효한 선택은 빈 actions이며, 잘못된 선택은 오류다. 전체 subset을 생성하지 않는다.

엔진 `sacrifice_candidates`, `generate_extra_summon_actions`를 G8에서 사용할 수 있다. action 수는 선택당 최대 보드 칸 수다. 2,003개 Hand 후보 fixture로 후보 목록과 한 subset의 target 생성을 검증했다. 기존 zone 감사가 목록 교차검사를 하므로 전 경로가 엄밀히 선형이라는 성능 보장은 하지 않는다. 2^N subset 탐색은 없다.

G8 전 `AiAction`은 Move/Drop/Ability만 유지하여 Bot에서 ExtraSummon 탐색을 명시적으로 제외한다. canonical TurnAction과 JSON action identity는 새 variant와 제물 순서를 보존한다. 기존 PositionKey의 Extra/Hand/Board/Piece 상태가 소환 결과를 구분한다. 엔진 및 hypothetical search에서 서버 production Draw RNG를 호출하지 않는다.

## 11. Record / Replay / codec

기록은 `RecordedAction.action=ExtraSummon`, `draws=기존 TurnStart resolution`, 전체 최종 `state_delta`를 저장한다. notation kind에 `extra_summon`을 추가했다. 기존 sparse JSON/hash 의미는 바꾸지 않았다.

Standard Replay는 저장된 exact 제물 목록과 target/Extra ID를 적용한 뒤 저장 DrawResolution을 적용하고 delta 상태와 canonical hash를 비교한다. 변조 action은 기존 exact 검증 경계에서 거부한다. RNG나 새 제물 선택은 없다. 브라우저는 기존 delta 재생을 사용한다.

frontend replay codec은 새 action/notation을 허용하며 중복·빈/제어문자 ID·잘못된 square를 거부한다. owner/cost/실제 zone 합법성은 서버/엔진 책임이다. 로컬 imported 코드의 구조 검증을 Rust exact 실행 검증이라고 주장하지 않는다.

## 12. Standard Analysis

기존 analysis options 요청에 optional `sacrifice_piece_ids`를 추가했다. Extra를 선택하면 `summon` 후보 metadata와 선택 subset의 preview를 제공한다. preview는 canonical action만 적용하고 실제 턴 전환이면 draw_pending=true, final hash 생략이다.

create/append의 action 저장 경로는 G3-C 공통 구현을 그대로 쓴다. 저장소 lock/idempotency/version 검사 이후에만 Draw를 한 번 확정하고 action+draws+state/hash를 저장한다. reload/부모 검증은 exact action+recorded draws를 재사용한다. 중복 commit 요청은 기존 저장 노드로 반환한다. 변조 제물·Extra ID·target은 거부한다.

## 13. DB와 호환성

G5는 새 DB 열/migration을 추가하지 않았다. G3-C의 `game_analysis_nodes.draws` forward migration을 수정하지 않았고 새 서버보다 먼저 적용해야 한다는 배포 계약을 유지한다. `TEST_ANALYSIS_DATABASE_URL`이 없어 외부 PostgreSQL opt-in 통합 테스트는 실행하지 않았다.

Legacy는 ExtraSummon을 거부하며 기존 Drop/Ability/geometry/hash/record/analysis를 보존한다. Challenge는 계속 Legacy이며 raining_men의 Board 구행을 변경하지 않았다. DC1/DC2/DC3 및 Standard export 차단도 그대로다.

## 14. G5 변경 파일과 이유

- 엔진 `summon.rs`, `types.rs`, `lib.rs`, `actions.rs`: policy/action/검증/원자 전이 및 공개 API.
- 엔진 `legal_moves.rs`, `endgame.rs`, `rules.rs`: Drop target/후속 처리 공유, 중앙 eligibility, 정상 턴 연결.
- 엔진 `ai/types.rs`: G8 전 명시적 제외 설명. `tests/rule_engine.rs`: G5 규칙/원자성/대량 후보/terrain 테스트.
- 서버 `main.rs`, `routes.rs`: authoritative 제출, 후보 endpoint, analysis preview.
- 서버 `game_record.rs`, `custom_piece.rs`: notation 및 exhaustive action actor 처리. 기존 Lab은 Legacy라 소환 불가.
- 서버 `game_view.rs`: Extra 공개 계약 명시; `game_view/tests.rs`: 실제 Room/양측 소환/heartbeat/재접속 테스트.
- 서버 `draw.rs`, `draw/g5_tests.rs`: G5 통합 테스트 등록/구현. `draw/tests.rs`, `draw/g3c_tests.rs`: 새 action/optional request 필드 fixture 대응.
- frontend `types/game.ts`, `types/gameRecord.ts`, `api/gameApi.ts`: wire 계약.
- frontend `components/ExtraSummonPanel.vue`, `components/GameScreen.vue`, `views/ReplayPage.vue`: 공용 최소 UI와 상세.
- frontend `replayCodec.ts`, `replayNotation.ts`, 관련 codec/analysis tests: 읽기·표기·구조·실제 Vue setup 검증.
- frontend `views/PieceLab.vue`, `components/custom-piece/CustomPieceTestBoard.vue`: Legacy Lab의 새 variant 명시적 제외로 타입 경계 보존.
- 계획 문서와 이 문서: 확정 규칙/완료 증거.

서로 다른 엔진 규칙, 서버 권한/기록/분석, 프론트 UI/codec을 연결해야 하므로 파일이 여러 개다. 저장 덱·Chessembly·DB·의존성은 G5에서 변경하지 않았다.

## 15. 실행 검증

최종 전체 검증 결과는 아래에 기록한다. 초기 신규 테스트의 typecheck fixture/endpoint 기대값 오류는 실제 기존 계약에 맞게 수정했고 기존 테스트를 삭제하거나 약화하지 않았다.

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | 21개 test 파일 통과, 실패 0 (실제 Panel/ReplayPage setup 포함) |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 vue-tsc script |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 222 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 141 passed, 9 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server g5` | 마지막 child/동시 append 보강 후 G5 5개 통과 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| 변경 Rust rustfmt 및 `git diff --check` | 통과 |

기존 서버 미사용 생성자 경고 2개와 frontend 500 kB 번들 경고가 있다. 엔진 ignored 7개는 벤치마크이며 서버 ignored 9개는 기존 외부 DB/진단 테스트다. G3-C DB 테스트는 `TEST_ANALYSIS_DATABASE_URL` 미설정으로 미실행이다. 기존 migration을 변경하거나 외부 DB/서비스를 조작하지 않았다.

## 16. 요구 테스트 대조

| 요구 번호 | 검증 근거 |
| --- | --- |
| 1~17, 21~31 | engine `g5::cost_thresholds_sources_and_full_overpayment_removal`: 실제 정의 점수, Board/Hand/혼합/Pocket × 경계값, 선택 전부 제거/zone/동일 Extra 유지/원본 불변 |
| 18~20, 23~25, 68 | `untrusted_selection_and_invalid_state_never_change_any_zone`, 서버 `multiplayer_atomic_submit_public_extra_and_exact_record`: 상대/King/중복/Legacy/손상 zone 및 상태·clock 불변 |
| 32~34 | 서버 `summon_forced_landing_game_end_and_later_pocket_draw_contracts`: 포획·recall·Pocket→Hand→일반 Drop 후보·Extra 복원 없음 |
| 35~40 | engine `targets_reuse_drop_contract_after_sacrifices_and_preserve_air_removal`, `summon_drop_terrain_and_capture_immunity_are_shared`: Base/Attack/bounds/점유/air 제거/고지/wall/capture 종료 |
| 41~45 | 서버 G5 실제 submit 및 forced landing/endgame 테스트: 실제 턴 Draw, 중간/종료 RNG 금지 |
| 46~50 | `public_extra_projection_and_hand_sacrifice_reveal_only_used_pieces`, 기존 custom reserve projection suite, 실제 Room public/full/sync 검사 |
| 51~54 | `g5_room_both_players_summon_and_rejoin_heartbeat_preserve_public_extra`, 직접 submit 상대 제물/권한 검사 |
| 55~60 | 서버 exact record/변조 검사, frontend canonical codec 순서/Draw 왕복·malformed 검사 |
| 61~67 | `analysis_preview_no_rng_commit_once_exact_reload_and_tamper_rejection`, 기존 G3-C create/append/child/SQL opt-in 테스트, 실제 ReplayPage/Panel setup 테스트 |
| 69~76 | 전체 기존 engine/server/frontend suite: Legacy, G2 geometry, G3 Hand/Draw/privacy, G4 최대3/Extra-only, ammo/forced landing, Challenge, Deck Code, pre-G1 canonical SHA |
| 성능/조합 | 2,003 Hand 후보 fixture, 선택 subset당 최대 64 canonical targets; Bot에서는 소환 제외 |
| 최소 UI | 실제 ExtraSummonPanel setup의 27/25 선택 보존·target/확정·취소·권한 비활성화, 타입 검사/빌드 및 ReplayPage 기존 저장 대기 테스트 |

## 17. G6/G7/G8 재사용과 남은 위험

G6는 `SummonOptions`/공용 Panel과 live projection, G7은 canonical action+draws/exact 재생 및 sparse record, G8은 선택 subset 기반 generator를 재사용한다. 새 비용/공개/재사용 규칙의 결정은 필요 없다. 전체 HUD/모바일 polish, Standard Deck Code와 개발 단계 Standard record versioning, Bot의 subset 선택/불완전 정보 전략은 해당 후속 Goal 범위다.

실제 브라우저 시각/E2E와 운영 배포는 실행하지 않았다. 실제 PostgreSQL migration/동시성 테스트도 미실행이다. UI 자동 검증은 Vue SFC의 실제 setup/handler와 build 기준이며 시각 QA를 대신했다고 주장하지 않는다. 매우 큰 analysis tree의 장기 성능/캐시는 기존 G3-C 과제다.

## 18. 완료 조건

- [x] 하나의 atomic canonical ExtraSummon, exact 제물 목록 보존.
- [x] 구행 Hand+Board / 폭격기 Board, score>=cost, 초과 전부 소모.
- [x] King/상대/잘못된 zone/중복/잘못된 target 거부.
- [x] Extra→Board, 사용 인스턴스 제거, 포획/Pocket/Draw로 Extra 재생 없음.
- [x] 공용 Standard Base∪Attack/Drop target 및 capture 후속 처리.
- [x] 정상 턴 소비, forced landing/game end/actual Draw 경계.
- [x] 양쪽 Extra 공개, 상대 Hand/Pocket privacy, 실제 Multiplayer 경로.
- [x] exact Record/Replay, RNG 없는 Analysis preview, 멱등 commit/저장 resolution 재사용.
- [x] 최소 게임/분석 소환 UI와 action 상세.
- [x] G8 전 Bot 소환 제외, subset 전수 생성 없음.
- [x] Legacy/G2/G3/G4/Challenge/Deck Code/hash 회귀 유지.
- [x] 최종 전체 검증 통과; 외부 DB/브라우저 미실행 범위 명시.
