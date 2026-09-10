# G3-B — Initial Draw / Turn Draw / Server-authoritative RNG

작성일: 2026-09-10 (KST). 기존 미커밋 G3-A/G4 작업을 보존하고 G3-B만 추가했다. 현재 Hand 소속·Drop·Pocket Ability·공개 경계는 `standard-format-g3a-implementation.md`의 계약을 따른다. 엔진과 프론트의 실행 코드는 이번 작업에서 변경하지 않았다.

## 1. Draw policy

`server/src/draw.rs`가 서버 자동 전이 정책을 담당한다.

- Standard만 적용한다. 양측 initial 3기, 각 실제 턴 시작 1기이며 White 첫 턴도 포함한다.
- 후보는 해당 플레이어의 현재 `deck.pocket_pieces`에 있는 PieceId다. 같은 타입의 다른 ID는 각각 별개의 후보다.
- 선택한 후보는 임시 후보 배열에서 `swap_remove`하므로 복원 없이 선택한다. 원래 Pocket 목록 순서를 RNG state로 사용하거나 재생 시 재추출하지 않는다.
- 수량은 `min(requested_count, pocket_count)`다. 빈 Pocket은 정상적인 빈 resolution이며 RNG도 호출하지 않는다.
- Hand 상한은 없다. Extra·Board·기존 Hand는 후보에 넣지 않는다.
- Draw를 `TurnAction`에 추가하지 않았다. 플레이어 행동은 계속 Move/Drop/Ability다.

## 2. Production RNG

기존 `Cargo.lock`과 `/home/sjh100/.cargo/registry/src`의 설치된 crate를 먼저 확인했다. 이미 lock/cache에 있던 `getrandom 0.4.2`를 서버 직접 의존성으로 추가했다. lock 변경은 서버 dependency 목록에 해당 crate를 연결한 한 줄이며 새 crate 다운로드나 기존 버전 갱신은 없었다.

`getrandom::u64()`는 OS의 난수원을 사용한다. Linux 기본 backend를 사용하며 별도 backend override, seed API, 저장된 PRNG state는 추가하지 않았다. frontend 난수, Easy Bot 시간 선택, UUID 일부, 현재 시각을 Draw 선택에 사용하지 않는다.

`random_index(upper)`는 `limit = u64::MAX - u64::MAX % upper`보다 작은 난수만 받아 `value % upper`로 인덱스를 만든다. 허용 구간의 크기는 upper의 배수이므로 modulo bias가 없다. 거부 표본은 최대 128회까지 처리하며 OS 오류나 한도 초과는 실패다. 시간/UUID 기반 fallback은 없다. 이 상한은 게임의 Draw 횟수나 Pocket 부족 규칙과 별개다.

기존 UUID API에는 임의 구간 선택·명시적 실패 처리가 없어서 사용하지 않았다. `rand`를 추가하는 대신 이미 설치된 OS 난수 API와 작은 rejection sampling 함수를 사용했다. 의존성 유지보수 범위는 기존 getrandom 버전을 직접 사용하는 서버 항목 하나다.

## 3. 테스트 RNG 주입

`initialize`, `turn_start`는 내부 Rust 인자로 `&mut impl FnMut(usize) -> Result<usize, String>`을 받는다. production 호출자는 `random_index`만 전달한다. HTTP 요청에는 선택 인덱스/seed/resolved PieceId를 받는 새 필드가 없다.

테스트는 첫/마지막 인덱스, 호출별 후보 수 기록, 범위 밖 인덱스, 명시적 오류, 호출 시 panic하는 RNG를 주입한다. 동일 source로 초기 결과를 재현하고 동일 타입 인스턴스의 개별 선택, 중복 없는 선택, 빈 Pocket·Legacy·동일 플레이어 시 RNG 미호출을 확인한다. 대량 샘플 분포 테스트는 없다.

## 4. Initial Draw와 White 첫 턴

실제 생성 순서는 다음과 같다.

1. 기존 factory가 양측 덱을 검증하고 Board/Pocket/Extra 인스턴스를 생성한다.
2. `StoredGame::new_with_players_and_deck_names`가 원래 편성의 DeckSnapshot을 동결한다.
3. `StoredGame::initialize_draws`가 White initial 3 → Black initial 3 → White turn_start 1을 처리한다.
4. 성공한 상태와 resolution을 `record.initial_state`, `record.initial_draws`에 보관한다.
5. 접근 capability를 고정하고 GameStore에 넣어 playable view를 만든다.

Pocket이 충분하면 최초 상태는 White Hand 4/Pocket −4, Black Hand 3/Pocket −3이다. 이후 White가 실제 턴을 끝내면 Black Hand 4가 된다. Pocket 0~3이면 White의 first-turn Draw도 남은 수량에 맞춰 정상 완료된다.

`initialize_draws`는 이미 저장한 initial resolution이 있으면 중복 초기화를 거부한다. 빈 Pocket도 세 개의 빈 resolution이 남아 재초기화를 구분한다. 초기화 전체가 실패하면 원래 상태/기록이 유지되며 게임을 store에 공개하지 않는다. 룸의 `game_id`도 초기화 성공 이후에만 설정한다.

## 5. 원자적 Pocket → Hand 이동

선택과 소속 이동을 분리했다. G3-A의 `hand::move_pocket_piece_to_hand`는 그대로이며 RNG를 넣지 않았다.

복수 Draw는 필요한 ID를 먼저 선택하고 상태 복사본에서 각 primitive를 실행한다. primitive의 소유·중복·Board/Extra/captured 충돌 검사를 통과한 뒤 전체 Hand zone 불변조건을 검사하고 commit한다. 초기화는 세 resolution 전체를 한 겹 더 바깥의 복사본에서 실행하므로 Black 또는 White first-turn 실패도 앞선 initial Draw를 남기지 않는다.

턴 Draw는 canonical action이 적용된 아직 공개하지 않은 복사본에 들어간다. 실패 시 `StoredGame.state`, action record, clock finish가 실행되지 않는다. 실패 응답은 숨긴 PieceId나 원문 내부 상태를 포함하지 않는 오류다. 시계 자체가 이미 만료된 경우에는 기존 adjudication의 게임 종료만 유지된다.

## 6. 실제 턴 전환과 clock 경계

연결 지점은 `server/main.rs::submit_action`이다.

```text
권한 → 시간/종료 검사 → canonical action을 복사본에 적용
→ 엔진 endgame / forced landing / current_player 결정
→ 확정 직전 기존 adjudicate(confirmed_at)
→ draw::turn_start(before, candidate_after)
→ game.state 교체 → clock.finish_turn → record → 응답
```

`turn_start`는 Standard이며 before/after 모두 종료 상태가 아니고 result도 없으며, 실제 `current_player`가 달라졌을 때만 새 플레이어에게 한 번 적용한다. action 수, history 길이 또는 HTTP 호출 횟수를 Draw 조건으로 쓰지 않는다.

잘못된 행동·canonical 불일치·권한 실패·기종료 요청·확정 전 timeout은 Draw 호출 전에 거절된다. 유효한 행동이 King capture로 종료되면 다음 턴 Draw가 없다. 같은 action 재전송은 기존 소유/합법성 검사에서 거절되고 추가 Draw를 만들지 않는다.

## 7. Forced landing

엔진 `apply_and_advance_turn`은 그대로다. 일반 White action 뒤 폭격기 착륙 대기이면 current_player가 White로 남으므로 양측 모두 Draw하지 않는다. 마지막 forced-landing action으로 Black에게 실제 턴을 넘겼을 때만 Black이 한 기 Draw한다. 중간 action은 기존 history/notation/시계 계약을 유지한다.

실제 submit handler 테스트에서 White Hand 4/Black 3이 중간에도 유지되고, 착륙 후 4/4로 바뀌며 두 action record 중 마지막에만 resolution이 있음을 검사한다. 기존 ammo/air·착륙·추락·탄약 회복 회귀도 통과했다.

## 8. Pocket 부족과 기존 Ability

Pocket 부족/empty는 패배·오류·턴 스킵이 아니다. Turn Draw의 빈 결과도 `turn_start` resolution으로 기록하여 정상 턴 시작과 이벤트 누락을 구분한다.

교대병 `relieve`, 공수부대 `airdrop`, 그린캠프 `recall`은 G3-A처럼 Pocket을 직접 조작한다. 해당 엔진 구현을 변경하지 않았다. Pocket 복귀 그 자체는 Hand 이동을 일으키지 않는다. 그 뒤 정상 Draw 시점이면 복귀한 ID도 후보다. 예를 들어 White recall로 Black 기물이 돌아온 뒤 Black의 실제 턴이 시작된다면 같은 확정 action의 후속 Turn Draw에서 뽑힐 수 있다.

복귀한 초기 기물이 Draw되면 primitive가 해당 ID의 runtime starting 목록만 정리한다. 동결된 원래 덱 snapshot은 계속 보존된다.

## 9. Bot과 Multiplayer

Multiplayer는 `start_room_game`, 로컬/Bot은 `create_game`에서 동일 initial 정책을 호출한다. host/guest 또는 Bot 여부가 RNG source를 바꾸지 않는다. G3-A의 참가자별 capability를 유지한다.

Bot은 이미 자신의 Turn Draw가 확정된 authoritative 상태에서 `play_bot_turn_detailed`를 시작한다. White Bot은 생성 시 4기, Black Bot은 인간 White 턴 종료 이후 4기를 받는다. 엔진/search는 서버 draw 모듈에 의존하지 않으며 hypothetical action에 Draw를 적용하지 않는다. Standard 전략·Draw 기대값 휴리스틱은 추가하지 않았다.

`run_bot_turn`은 search 결과에 대한 기존 시간 재검사를 통과한 다음 다음 플레이어의 Draw를 확정한다. 현재 Bot 구현은 같은 플레이어의 forced landing을 마친 뒤 반환하므로 마지막 timeline frame만 최종 Draw 상태로 갱신한다. 중간 frame은 유지하고 마지막 action record에만 resolution을 붙인다. 인간에게 돌아온 응답/마지막 frame/record delta는 동일한 Draw 완료 상태다. Bot 재요청은 현재 턴 불일치로 거절된다.

## 10. Authoritative resolution과 기록

추가한 metadata는 다음과 같다.

```json
{
  "player_id": "black",
  "timing": "turn_start",
  "piece_ids": ["black_knight_5"]
}
```

- `GameRecord.initial_draws`: White initial, Black initial, White turn_start 순서의 세 resolution.
- `RecordedAction.draws`: 실제 새 턴이면 한 resolution, 같은 플레이어 후속 action/종료/Legacy이면 빈 목록.
- `GameRecord.initial_state`: 초기 Draw 전체가 완료된 실제 playable state.
- 기존 `RecordedAction.state_delta`: action과 자동 Draw를 모두 포함하는 최종 authoritative 차분.
- `DeckSnapshot.extra`: 원래 Extra 인스턴스 편성을 count 1의 snapshot 목록으로 보존. 기존 Starting/Pocket snapshot은 Draw 전에 생성한다.

새 필드는 `serde(default, skip_serializing_if = "Vec::is_empty")`다. Legacy 기록에서는 추가 Draw/Extra 필드가 출력되지 않으며 구형 기록은 누락 필드를 빈 값으로 읽는다. format_version=2, snapshot_version=1, 기존 TurnAction·notation·canonical GameState hash 알고리즘을 변경하지 않았다. DB record JSONB가 metadata를 함께 저장하므로 migration은 없다.

초기 결과는 initial_state와 ordered resolution에, 턴 결과는 delta와 resolution에 보관한다. RNG state/seed를 기록하거나 복구 시 OS 난수원에 같은 값이 나올 것을 기대하지 않는다. 실행 중 게임의 기존 메모리 store 및 완료 기록 저장 시점은 유지했다. 프로세스 장애 복구용 새 영속 저장 시스템을 구현한 것은 아니다.

## 11. Privacy와 public 응답

resolution은 GameState/history/notation에 추가하지 않고 내부 record 옆에만 두었다. G3-A의 `project_state`, `view_for`, `sync_view_for`가 새 Hand를 그대로 투영한다.

| 경계 | 유지하는 계약 / 검증 |
| --- | --- |
| 최초 GameView / action response / GET | 자기 Hand ID·Piece object, 상대 Hand count만; 상대 Pocket ID/object도 숨김 |
| 룸 heartbeat / sync | initial 및 turn Draw 후 양측 요청 검사; 자기 새 Hand identity 확인, 상대 hidden ID 부재 |
| legal move/drop / piece options | G3-A 진영 권한 검사 유지, public/상대 진영 요청 거절 |
| Bot timeline | 모든 frame 투영, 마지막 frame의 인간 Draw 반영, Standard stats 생략 유지 |
| history / notation | 기존 player action만 포함; Draw metadata가 섞이지 않음 |
| live state delta | live 응답에 내부 RecordedAction delta를 추가하지 않음; 분석 preview는 Standard 거절 |
| debug/stat/catalog | 기존 Standard Bot stats 생략, 숨긴 커스텀 reserve catalog 투영 유지 |
| record export | 진행 중 export 차단·기존 완료 기록 접근 정책 유지 |

privacy 테스트는 serialized 응답 전체에서 상대 Hand/Pocket ID와 `initial_draws`/`draws` 필드가 없는지 확인한다. 상대 Pocket→Hand 이동 전후 projected state는 동일하고 별도 hand_count만 바뀌는 G3-A 회귀도 유지했다. 알려진 Board action의 공개 정보나 기본 카탈로그까지 숨기는 계약은 아니다.

완료된 기록의 전체 공개 정책은 G3-C/G7 범위이며 이번에 바꾸지 않았다. 기존 접근 정책으로 허용된 완료 record에는 authoritative state/delta와 새 resolution이 포함된다. 이를 실시간 상대 Hand 공개로 사용하지 않는다.

## 12. Replay 결정성과 분석 제한

본게임 프론트 Replay는 기존 initial_state + state_delta를 적용하므로 Draw 결과가 그대로 복원되며 난수를 실행하지 않는다. codec/action 형식과 프론트 코드를 변경하지 않았다.

서버 `GameRecord::state_at_ply`는 G3-B initial metadata가 있는 기록에 대해 canonical action을 재적용하고 `draw::replay_turn`으로 저장된 exact ID를 이동시킨다. 별도로 복원한 delta 결과와 canonical state hash가 같아야 성공한다. 누락/잘못된 소유/수량/ID/timing, 전이 실패 또는 다른 상태는 `invalid_record`로 거절한다. Legacy 및 이전 metadata 없는 기록은 기존 delta 복원 경로를 유지한다.

**Standard 분석 분기는 G3-C까지 HTTP 501로 명시적으로 거절한다.** 사용자 목표의 허용 대안 2를 적용했다. 조사 결과 현재 analysis는 옵션마다 hypothetical preview delta/hash를 만들고 클라이언트의 미저장 `pending_actions`를 재실행하며, AnalysisNode는 별도 SQL 열로 저장한다. 저장 노드에 metadata 하나만 추가해도 preview·pending·멱등 생성의 확정 결과 연결이 해결되지 않는다. 이번에는 본게임의 exact resolution과 재적용 API를 완성하고 이 분기 경로를 차단했다.

create, options, list/기존 tree 검증, analysis_state에 명시적 지원 검사를 두었다. 잘못된 Standard state/hash를 만들어 성공으로 반환하거나 RNG를 재실행하는 fallback은 없다. Legacy 분석 생성·분기·재검증 테스트는 계속 통과한다. 일반 완료 record 조회와 retention API는 analysis 차단 대상이 아니다.

## 13. 변경 파일

이번 작업 시작 시 보관한 사본과 비교했다. 기존 G3-A/G4 변경을 이번 변경으로 계산하지 않았다.

| 파일 | 변경 이유 |
| --- | --- |
| `server/src/draw.rs` (신규) | 서버 Draw 정책, OS RNG, 원자적 resolution 적용, 결정적 재적용 |
| `server/src/draw/tests.rs` (신규) | 수량/선택/원자성/턴/실패/착륙/Ability/record/analysis 회귀 |
| `server/Cargo.toml`, `Cargo.lock` | 이미 설치된 getrandom의 서버 직접 연결 |
| `server/src/main.rs` | 생성·룸·action·Bot 확정 경계, 분석 미지원 검사 |
| `server/src/time_control.rs` | frozen snapshot 이후 playable initial 상태 확정 |
| `server/src/game_record.rs` | sparse resolution, 원래 Extra snapshot, exact Draw 재생 검증 |
| `server/src/game_view/tests.rs` | 실제 initial/turn Draw에 대한 기존 HTTP privacy/Bot 테스트 갱신·확장 |
| 이 문서 | 구현 계약, 검증 결과, 후속 범위 |

모듈을 여러 엔진 계층에 분산하지 않고 서버 policy 하나로 묶었다. 생성/일반 action/Bot/기록이 서로 다른 기존 경계여서 위 파일들에 연결이 필요했다. 엔진 Hand primitive·Pocket Ability·Chessembly·frontend UI·Deck Code는 변경하지 않았다.

## 14. 실행한 검증

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | 21개 파일 통과, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 script는 vue-tsc |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 217 passed, 7 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 128 passed, 8 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| 변경 Rust 파일의 rustfmt, `git diff --check`, 시작 시 사본과 diff 검토 | 통과 |

테스트 작성 중 잘못 참조한 Ability 필드/PieceId helper와 누락 import를 실제 타입에 맞게 수정했다. 기존 검사를 삭제하거나 약화시키지 않았다. G3-A의 수동 Hand 주입 기반 HTTP 테스트는 실제 초기 Draw 4/3 및 정상 턴 전환을 사용하도록 갱신했고 기존 privacy 검사에 새 상태/record 검사를 추가했다.

기존 경고는 서버 미사용 생성자 2개와 frontend 500 kB 초과 번들이다. ignored는 엔진 성능 벤치마크 7개, 서버 외부 DB 관련 7개 및 payload 진단 1개다. 외부 PostgreSQL·운영 배포·브라우저 대국 E2E·OS entropy 장애의 실기 주입·통계 분포 검증은 실행하지 않았다. 오류/범위/RNG 호출 여부는 주입 가능한 source와 손상 상태 테스트로 검증했다.

## 15. 요청 테스트 58개 대조

| 요청 번호 | 자동 검증 / 근거 |
| --- | --- |
| 1~9 | `initial_order_counts_instances_and_short_pockets`: Pocket 0/1/2/3/4/8, 양측 3, White +1, 정확한 Pocket 감소, Board/Extra 보존 |
| 10~12 | 같은 테스트: 전부 동일 타입의 서로 다른 ID, 고유 ID 수, 선택 범위 8/7/6→8/7/6→5, 동일 source 재현 |
| 13 | production 호출 위치·getrandom 소스/lock/cache 검토; server-only 모듈, 클라이언트/엔진 Draw RNG 경로 없음 |
| 14 | `rng_and_transition_failures_roll_back_entire_initial_and_multi_draw`, `turn_rng_failure_and_empty_turn_preserve_atomicity`, `failed_draw_cannot_commit_action_clock_or_record` |
| 15~19 | `actual_turn_draws_and_exact_replay_require_valid_resolution`: 5회 실제 action/턴 교대 및 Pocket 0/1 |
| 20~22 | `forced_landing_defers_draw_until_actual_player_change`: 실제 submit 2개, 중간 4/3, 종료 4/4, resolution 정확히 1개 |
| 23~27 | `committed_actions_include_draw_in_record_and_reject_retry_invalid_and_unauthorized`, `timeout_and_king_capture_do_not_start_draw`, 기존 canonical/clock 회귀 및 Bot retry 검사 |
| 28~30 | `returned_pocket_piece_is_eligible_only_at_normal_draw_time`, 기존 `standard_pocket_abilities_keep_their_sources_and_returns_without_automatic_draw` (교대/공수/그린캠프) |
| 31~35 | `multiplayer_hand_privacy_covers_full_sync_legal_submit_rejoin_and_public_routes`, `opponent_pocket_to_hand_transfer_exposes_only_count_not_membership_by_subtraction` |
| 36~40, 42 | `local_and_bot_creation_bind_distinct_view_capabilities_and_bot_frames_are_projected`: 양 인간 진영, Bot 시작 4기, 인간 턴 +1, timeline·record 재생, 룸 테스트 양측 교대 |
| 41 | `legacy_and_hypothetical_search_never_consume_draw_rng`: search 전후 원본/양측 Pocket 보존, engine→server RNG 의존성 없음 |
| 43~44 | `frozen_decks_and_playable_initial_state_are_distinct_and_sparse_for_legacy`: original Pocket 8·Extra·초기 4/3 및 중복 초기화 거절 |
| 45~48 | committed action/record JSON 왕복·exact replay·변조 거절, Bot 마지막 frame/record 일치, `standard_analysis_endpoints_reject_unresolved_branches`, privacy 전체 JSON 검사 |
| 49~50 | Legacy initial/turn RNG 미호출 및 Pocket Drop 테스트, 기존 rule_engine/AI suite |
| 51~55 | G2 geometry·Hand Drop 위치·forced landing/ammo/air·Extra·capability 관련 전체 엔진/서버 회귀 |
| 56~58 | 기존 Challenge Legacy, frontend DC1/DC2/DC3·Replay codec/state/notation, server analysis pre-G1 고정 SHA 및 Legacy 분기 suite |

## 16. G3-C에 넘기는 계약과 남은 작업

G3-C는 `DrawResolution { player_id, timing, piece_ids }`, `initial_draws`, action별 `draws`, `draw::replay_turn` 및 최종 state_delta를 재사용할 수 있다. 기록된 ID를 확정 결과로 삼아야 한다. 빈 Pocket의 정상 빈 resolution도 보존한다.

남은 작업은 Draw notation/animation, 최종 Hand UI, 완료 기록의 최종 공개 정책, Standard analysis preview/pending/저장 노드를 하나의 확정 resolution으로 연결하는 계약이다. 분석 경로를 지원할 때 멱등 저장과 SQL/node serialization을 함께 다루고 현재 501 차단을 해제해야 한다. Standard Deck Code·ExtraSummon·sacrifice·Standard Bot 휴리스틱·Standard Challenge는 이번 범위가 아니다.

## 17. G3-B 완료 조건

- [x] 양측 initial 3 + White first-turn 1, 모든 실제 새 턴 +1
- [x] Pocket 부족/empty 정상, Hand 상한 없음, without replacement·instance 선택
- [x] 서버 OS RNG, 클라이언트 seed/Draw 선택 API 없음, 테스트 source 주입
- [x] 복수 Draw/initial 전체 원자성, 실패 action/Draw/권한/timeout 시 추가 Draw 없음
- [x] forced landing 중간 Draw 없음, 실제 current_player 변경 시만 Draw, 종료 후 없음
- [x] 기존 Pocket Ability 유지, Extra/Board는 Draw 대상에서 제외
- [x] 실제 Bot 턴 전 Draw, search에서 production RNG 미사용, 종료 후 인간 Draw 및 frame 일치
- [x] Multiplayer 및 Hand/Pocket privacy·capability 유지
- [x] 원래 Starting/Pocket/Extra snapshot 보존, playable initial state 분리
- [x] resolved outcome 저장·RNG 없는 exact 재생, Standard 분석은 명시적 미지원 처리
- [x] Legacy/Challenge/Deck Code/Replay/hash 및 G1/G2/G4/G3-A 회귀 유지
- [x] 요구 test/typecheck/lint/check/build/diff 검증 통과; 미실행 외부 검증 명시
