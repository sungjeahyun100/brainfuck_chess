# G3-C — Draw Record / Replay / Analysis / Hand 공개 정책

작성일: 2026-09-10 (KST). G3-A/G3-B의 Hand, privacy, `DrawResolution` 및 authoritative state 계약을 기준으로 구현했다. 시작 시 존재한 G1/G2/G3-A/G3-B/G4 미커밋 변경은 보존했다. 아래 변경 목록은 작업 시작 시 사본과 비교한 **G3-C 추가분**이다.

## 1. 완료 Replay의 Hand 공개

기존 접근 정책으로 열람이 허용된 완료 Standard 기록은 양측 Hand와 실제 Draw PieceId를 제공한다. `ReplayPage.vue`의 별도 손패 패널은 현재 ply 또는 분석 노드의 White/Black Hand 이름과 ID를 보여준다. 시작 위치는 이미 White 4 / Black 3을 가진 `initial_state`다. 다시 초기 Draw를 하지 않는다.

시작 위치의 `initial_draws`, 각 action 이후 `draws`, 분석 노드의 `draws`를 현재 시점 패널에서 확인할 수 있다. 초기/턴 시작을 구분하고 count, 이름, ID를 표시한다. 기보 행의 count를 보고 해당 ply를 선택하면 실제 결과를 확인한다. 애니메이션이나 전체 Standard 게임 HUD는 추가하지 않았다.

## 2. 실시간/완료 기록 privacy 경계

`get_game_record`의 기존 접근 검사 뒤 `completed_record_view`가 완료 여부와 Standard Draw 무결성을 검사한다. 저장소에 잘못 들어간 진행 중 Standard 기록도 공개하지 않는다. 분석/retention의 기존 owner 검사 경로에서도 완료 Standard 기록을 검증한다. 기록을 열 수 있는 사람의 인증·공개 설정은 변경하지 않았다.

`game_view::project_state`, `view_for`, `sync_view_for`, Bot timeline은 변경하지 않았다. 진행 중에는 자기 Hand identity, 상대 Hand 장수만 제공하고 상대 Pocket/Hand ID와 숨긴 Piece object를 제거한다. 종료된 live GameView도 이 경계를 유지하며 **기록 조회**에서만 양측 Hand를 공개한다.

Extra 관련 코드와 공개 처리는 바꾸지 않았다. 현재 authoritative record에 있던 Extra 데이터를 새로 제거하거나 추가 공개 정책으로 확정하지 않았다. 최종 Extra 공개 규칙은 G5/G6에서 결정한다.

## 3. Draw event / notation

`TurnAction`과 `NotationActionKind`에 Draw를 추가하지 않았다. 기존 `GameRecord.initial_draws`, `RecordedAction.draws` 및 새 `AnalysisNode.draws`가 automatic transition metadata다.

`formatDraw`는 `백 드로우 1기` / `흑 드로우 1기`처럼 count만 출력한다. 기물 이름/ID는 완료 Replay의 별도 상세 표현에서만 조합한다. 0기 resolution은 저장하되 notation과 상세 행에 강제로 표시하지 않는다. live history/notation에 authoritative Draw metadata를 넣지 않았으므로 상대 identity가 추가로 노출되지 않는다.

## 4. Analysis Preview 계약

Standard Analysis의 HTTP 501 차단을 제거했다. 후보는 기존 canonical action을 엔진에 적용한 **Draw 전 hypothetical 상태**로 계산한다. `draw::starts_turn`으로 자동 전이 필요 여부만 판단하고 RNG나 `turn_start`를 호출하지 않는다.

```json
{
  "action": { "type": "move", "...": "기존 canonical action" },
  "state_delta": [],
  "draw_pending": true
}
```

예시의 빈 delta는 형식 설명용이며 실제 응답에는 action의 Draw 전 delta가 들어간다. `draw_pending: true`일 때 final `state_hash`는 **생략**한다. Draw가 필요 없는 forced-landing 중간 action이나 게임 종료 후보는 정확한 state delta/hash를 반환한다. false인 `draw_pending`은 생략해 Legacy wire를 유지한다.

빈 Pocket의 실제 턴 전환도 정상 자동 resolution을 처리할 예정이므로 pending으로 표시한다. commit 결과는 빈 resolution이며 RNG 호출은 없다. Preview는 특정 기물이 나온 것처럼 채우지 않는다.

## 5. Preview UI와 분기 생성

기존 Legacy의 낙관적 임시 노드·연속 pending action은 유지한다. Standard에서는 서버 응답을 기다리는 동안 후속 착수/후보 조회를 차단하고, 저장된 node를 받은 뒤 최종 상태를 표시한다. Hand에서 일반 Drop 기물을 선택하도록 분석 도구의 출처도 바꿨다.

Draw 미확정 후보가 있으면 무작위 드로우와 분기 저장 시 확정된다는 안내를 표시한다. Standard의 미저장 `pending_actions`를 서버에 제출하면 400으로 거부한다. Preview를 여러 수의 확정된 상태처럼 이어 실행하지 않는다. 실제 대국과 같은 다음 수를 선택하면 기존 UI처럼 메인 라인으로 이동하며, 이 동작은 새 분석 분기 생성이 아니다.

## 6. Analysis branch의 Draw 확정 시점

```text
완료 기록 접근 검사 → 부모 exact state 검증 → canonical action 적용
→ 저장소의 request_id/버전/부모 검사 및 쓰기 잠금
→ draw::turn_start(parent, candidate, server RNG)
→ final state/hash + DrawResolution을 함께 저장 → 응답
```

`AnalysisRepository::create/append`는 저장하지 않는 일시적 `draw_parent` 인자를 받는다. Standard 신규 노드에만 부모를 전달한다. Legacy 생성/append는 None으로 기존 상태를 저장하며 RNG를 호출하지 않는다. 이미 검증된 직렬화 노드를 복원하는 내부 테스트도 None을 사용한다. HTTP 요청은 이 인자를 받지 않는다.

`analysis::resolve_node`는 G3-B의 `turn_start`와 OS RNG `random_index`를 재사용한다. 새로운 난수 알고리즘, seed, 클라이언트 선택 인덱스는 없다. create/append 요청의 알 수 없는 최상위 필드도 거부한다.

## 7. 노드 저장 구조와 DB migration

기존 `parent_node_id`, `action`, `state_after`, `state_hash`를 그대로 사용하고 다음 한 필드만 추가했다.

```json
"draws": [{
  "player_id": "black",
  "timing": "turn_start",
  "piece_ids": ["실제로 선택된 PieceId"]
}]
```

Rust는 `serde(default, skip_serializing_if = "Vec::is_empty")`를 사용한다. Draw가 없는 구형 Legacy node는 기존처럼 읽고 빈 필드는 wire에서 생략한다. 빈 Pocket 턴은 목록 한 개 안의 `piece_ids: []`로 처리 완료를 보존한다. 부모 snapshot/delta/seed 같은 중복 필드는 추가하지 않았다.

실제 환경별 `game_analysis_nodes`에는 `draws JSONB NOT NULL DEFAULT '[]'` 및 array CHECK를 추가한다. 신규 forward release:

- `server/db/prod/20260910000000_analysis_draws.sql`
- `server/db/test/20260910000000_analysis_draws.sql`

`migrate-db.sh`의 순서에 등록했다. 기존 관리자/임시 schema-owner 권한 계약, 잠금, role 복구·멤버십 제거 검증을 유지했다. SQL adapter의 create/append/select/reload에 모두 연결했다. 서버 시작 및 두 환경의 postflight도 새 열을 확인한다. 실제 배포에서 **새 서버 시작 전에 해당 release SQL을 적용해야 한다**.

`server/migrations`는 실행하지 않는 과거 SQLx 이력이라는 README를 따라 변경하지 않았다. 기존 Legacy JSON/hash와 record format_version=2, DC1/DC2/DC3는 변경하지 않았다.

## 8. Idempotency와 원자성

메모리 저장소는 기존 write lock 안에서 request_id/버전/부모를 확인한 뒤 RNG를 호출한다. PostgreSQL create는 `(owner_user_id, request_id)` unique insert에 성공한 트랜잭션만 Draw를 확정한다. 충돌한 요청은 저장된 트리를 반환한다.

PostgreSQL append는 owner가 일치하는 트리 행을 `FOR UPDATE`로 잠근 뒤 request_id를 다시 확인한다. 버전 갱신에 성공한 신규 요청만 Draw를 확정한다. 노드와 version은 같은 트랜잭션에 저장한다. 같은 요청의 재전송·동시 실행은 저장된 resolution을 재사용한다. 네트워크 응답을 받지 못했더라도 DB commit이 성공했다면 같은 request_id로 결과를 복구한다.

RNG 실패는 노드/트리 상태를 저장하지 않는다. 메모리 실패 테스트는 신규 트리 없음과 기존 트리 version/node 수 불변을 검사한다. DB 트랜잭션 실패 시 SQLx rollback 계약을 사용한다. 아직 commit되지 못한 실패 시도의 entropy 소비까지 되돌리는 계약은 아니며, 확정·저장된 node는 재추첨하지 않는다.

## 9. Exact Replay / hash validation

`draw::validate_initial`이 initial Hand를 임시 Pocket 상태로 되돌린 뒤 White initial → Black initial → White turn_start resolution을 정확히 적용하고 playable state의 canonical hash와 비교한다. 이 역구성은 검증용이며 Pocket 원래 순서나 RNG seed를 복원하지 않는다. 결과 상태를 Replay 시작 상태에 덧적용하지 않는다.

Standard `state_at_ply`는 initial metadata를 먼저 검증한다. 각 action에서:

1. 부모 상태에 canonical action을 재적용한다.
2. `draw::replay_turn`으로 exact resolution을 적용한다.
3. 별도로 authoritative delta를 적용한 상태와 canonical hash를 비교한다.

누락·존재하지 않는 ID·Pocket 밖 ID·잘못된 player/timing/count·불필요한 metadata·delta 불일치는 거부한다. Legacy에는 기존 delta 경로를 유지하며 Draw metadata가 끼어든 기록은 거부한다.

Standard 분석은 root부터 부모를 검증해 canonical action + saved resolution으로 재구성하고 저장된 state/hash와 비교한다. 중복 node ID, 없는 부모, 순환도 거부한다. 구형 `sha256:` 허용은 Legacy에만 적용하며 Standard의 hash 검사를 우회하지 못한다. 새로 생성하거나 멱등 반환한 Standard tree도 검증한다.

브라우저 Replay는 기존 initial_state + state_delta 경로로 복원하며 RNG가 없다. imported replay code에는 Draw 구조/type/수량 상한/중복 ID 검사도 추가했다. **브라우저 자체가 Rust 엔진의 canonical 합법성/hash 검증을 수행하는 것은 아니다.** 서버 조회 기록의 exact 검증과 사용자가 가져온 로컬 코드의 구조 검증을 구분한다.

## 10. Forced landing / game end / empty Pocket

G3-B의 `starts_turn` 조건을 그대로 사용한다.

| 상태 | Preview | Commit |
| --- | --- | --- |
| 같은 플레이어에게 forced landing 남음 | 정확한 delta/hash | Draw 없음, RNG 없음 |
| 착륙 후 실제 상대 턴 시작 | draw_pending, final hash 없음 | 상대 turn_start resolution 한 번 |
| King capture 등 게임 종료 | 정확한 delta/hash | Draw 없음, RNG 없음 |
| 실제 턴 전환, 빈 Pocket | 자동 전이 pending | `piece_ids: []` 저장, RNG 없음 |
| Legacy | 기존 정확한 preview | 기존 저장, Draw/RNG 없음 |

## 11. 변경 파일

| 파일 | 변경 이유 |
| --- | --- |
| `server/src/analysis.rs` | sparse node metadata, SQL 영속화, 쓰기 잠금 이후 resolution, 테스트 RNG 주입 |
| `server/src/main.rs` | 완료 record 경계, 501 제거, unresolved preview, exact 분석 검증, 신규 저장 연결 |
| `server/src/draw.rs` | 기존 turn predicate 재사용 및 RNG 없는 initial metadata 검증 |
| `server/src/game_record.rs` | initial 검증 및 Standard exact 재생 필수화 |
| `server/src/draw/tests.rs` | G3-B의 임시 501 기대값을 G3-C 지원 계약으로 갱신 |
| `server/src/draw/g3c_tests.rs` | exact record/node, 멱등성, RNG 경계, privacy, 실패 원자성, SQL opt-in 테스트 |
| `server/src/database.rs`, 환경별 postflight 2개 | 새 SQL 저장 계약 사전 검사 |
| 환경별 `20260910000000_analysis_draws.sql` 2개, `migrate-db.sh`, `server/db/testing/README.md` | forward migration 등록 및 DB 테스트 실행 방법 |
| `frontend/src/types/gameRecord.ts` | Draw metadata와 unresolved preview wire 타입 |
| `frontend/src/views/ReplayPage.vue` | 양측 Hand/Draw 표시, Standard Hand 선택, 확정 응답 대기 |
| `frontend/src/replayNotation.ts`, `replayCodec.ts` | count-only 표기, metadata 입력 구조 검사 |
| `frontend/src/replayAnalysis.test.ts`, `replayCodec.test.ts`, `replayState.test.ts` | 실제 Vue setup 및 codec/delta/notation 회귀 |
| 계획 문서, 이 문서 | 확정 규칙과 구현·검증 결과 |

파일 수가 여러 개인 이유는 SQL schema/배포/시작 검사, 서버 읽기·쓰기·공개 경계, UI/codec 타입과 테스트가 별도 책임이기 때문이다. 엔진, live projection, Extra 정책, Chessembly, Deck Code 실행 코드는 G3-C에서 수정하지 않았다. 새 의존성은 없다.

## 12. 실행한 검증 결과

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | 21개 파일 통과, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 script는 vue-tsc |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 217 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 136 passed, 9 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| `bash -n migrate-db.sh` | 통과 |
| 변경 Rust rustfmt, `git diff --check`, 작업 시작 사본과 diff 검토 | 통과 |

기존 frontend 500 kB 번들 경고, 서버 미사용 생성자 2개 경고가 있다. 엔진 ignored 7개는 성능 측정이며 서버 기존 ignored 8개에 외부 DB용 G3-C 테스트 1개를 추가했다.

외부 PostgreSQL migration 적용/실제 SQL 왕복은 **미실행**이다. `psql` 클라이언트는 있으나 PostgreSQL 서버 실행 파일은 없었고 외부 DB를 시작하거나 변경하지 않았다. `postgres_analysis_draws_roundtrip_and_concurrent_idempotency`는 현재 release를 적용한 disposable DB와 `TEST_ANALYSIS_DATABASE_URL`이 있어야 실행한다. 실제 SQL 구문·잠금 동작 검증은 그 환경에서 남아 있다. 브라우저 시각/E2E, 배포, 통계적 RNG 분포 검증도 실행하지 않았다. UI는 기존 방식의 실제 Vue SFC setup/handler 자동 테스트와 빌드로 검증했다.

## 13. 요청 테스트 62개 대조 및 Legacy 회귀

| 요구 번호 | 자동 검증 근거 |
| --- | --- |
| 1~7 | `completed_initial_and_ordered_turns_restore_exact_hand_without_rng`, `completed_record_publication_is_separate_from_live_privacy`, frontend delta/실제 ReplayPage 테스트 |
| 8~16 | initial player/ID/order/Hand 변조, 5턴 exact hash, action ID/player/timing/count/누락/delta 변조 검사 및 G3-B exact replay tests |
| 17~20 | `analysis_forced_landing_endgame_empty_and_legacy_keep_transition_contracts`, 실제 HTTP forced landing/empty commit 테스트 |
| 21~26 | `preview_commit_idempotency_children_and_reload_use_one_resolution`의 반복 preview/0 RNG/final hash 생략, `forced_landing_preview_and_commit_distinguish_exact_and_unresolved_states` |
| 27~35 | 동시 create/append 각 RNG 1회, actual Hand/hash, 저장 parent에서 child 생성, 재조회/restart fixture, empty HTTP 저장 |
| 36~40 | serialized tree + 새 repository에서 resolution 재검증, Legacy 누락 필드, malformed node 거부. **실제 SQL 왕복은 opt-in ignored 테스트로 추가했으나 미실행** |
| 41~45 | G3-A/G3-B full/heartbeat/action/Bot serialized privacy suite 유지, 완료/미완료 record 및 analysis 접근 구분 새 테스트 |
| 46~50 | 실제 ReplayPage setup의 initial/current resolved IDs, count-only `formatDraw`, zero 숨김, codec metadata 왕복/거부 |
| 51~53 | 기존 Legacy 4-ply branch/preview/tree, replay codec/state/notation, pre-G1 고정 canonical SHA |
| 54~60 | 전체 engine/server suite의 G2 geometry, G3-A Hand/privacy, G3-B initial/turn RNG, forced landing/ammo/air, G4 Extra, Challenge; frontend DC1/DC2/DC3 |
| 61~62 | 위 전체 frontend/server/engine test, typecheck/lint/check/build |

Legacy는 Draw를 생성하거나 RNG에 의존하지 않는다. 기존 hash 알고리즘, action enum, wire의 빈 필드 생략을 유지한다. Legacy 분석의 새 부모 전체 트리 검증·Draw용 추가 state clone을 피했고 기존 pending action 흐름도 보존했다. 현재 Standard에는 G3-B initial metadata가 필요하며, Draw 도입 이전 개발용 Standard 기록을 정상적인 완료 기록으로 조용히 해석하지 않는다.

## 14. G5/G6/G7 재사용 계약

- **G5:** `hand_pieces`, `extra_deck_pieces` 및 canonical action 적용 후의 실제 turn boundary를 사용한다. 제물/소환이 끝난 최종 turn transition에만 Draw를 연결해야 한다. 구행 Hand/Board, 폭격기 Board 제물 규칙은 별도로 구현한다.
- **G6:** live Hand count/identity 투영을 유지한다. 분석의 `draw_pending`과 완료 Replay 상세 패널을 전체 HUD의 확정 상태처럼 혼용하지 않는다. Extra 공개는 별도 결정한다.
- **G7:** exact `DrawResolution`, 초기 metadata 검증, delta/exact hash 일치, SQL node의 `draws`, 기존 request_id 멱등성을 재사용한다. Standard Deck Code 및 소환 기록 통합은 여기서 구현하지 않았다.
- **Bot/search:** 기존 hypothetical search는 production Draw RNG를 호출하지 않는다. 사용자의 영속 분석 분기와 AI search tree는 별개다.

## 15. 남은 위험과 G5 전 결정

- 새 서버 배포에는 환경별 Draw SQL migration이 선행되어야 한다. 실제 PostgreSQL 통합 검증은 위 disposable 테스트로 확인해야 한다.
- Standard 기록/트리 검증은 엔진을 재실행한다. 아주 큰 기록/트리의 성능 측정, 캐시 설계는 하지 않았다.
- 서버의 현재 엔진에 대한 검증이며 개발 단계 Standard 룰 변경의 장기 versioning은 G7 과제다. Legacy 고정 hash는 회귀 검증했다.
- G5 전에 ExtraSummon의 소환 가능 칸, 제물 가치 정확 일치의 예외, 소환 후 포획·귀환·재사용, 동일 타입 추가 제한, Extra 공개 범위, 향후 Pocket 제물 규칙을 결정해야 한다. 총 3기, 구행 Hand/Board·폭격기 Board 출처라는 기존 확정 규칙은 유지한다.

## 16. G3-C 완료 조건 대조

- [x] 접근 허용된 완료 Replay의 양측 Hand 및 exact Draw 확인
- [x] live Hand/Pocket privacy와 Extra 처리 보존
- [x] Draw는 automatic transition, count-only notation, zero metadata 보존
- [x] initial playable state 및 exact resolution으로 RNG 없는 Replay 검증
- [x] Standard Analysis 501 제거, preview RNG 미사용/unresolved 표시
- [x] 저장소 멱등성/버전 검사 뒤에만 Draw 확정, node와 state/hash 영속화
- [x] reload/child/traversal/검증에서 저장된 resolution 재사용
- [x] forced landing/endgame/empty Pocket 계약 유지
- [x] Legacy Analysis/Replay 및 G1/G2/G3-A/G3-B/G4 전체 회귀 통과
- [x] 요청한 로컬 test/typecheck/lint/check/build 통과
- [x] 실제 PostgreSQL·브라우저 E2E 미실행 범위 명시

G3-A(Hand/Drop/privacy), G3-B(initial/turn Draw), G3-C(record/replay/analysis)의 구현 범위는 완료했다. 배포 전 실제 DB 검증은 별도이며 G5~G8 기능의 완료를 뜻하지 않는다.
