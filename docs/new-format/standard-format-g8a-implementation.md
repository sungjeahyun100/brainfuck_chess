# G8-A — Standard Bot / Hidden Information / ExtraSummon Search

작성일: 2026-09-10 (KST). 첨부된 G8-A 목표 및 AGENTS.md, G0~G7 문서를 읽고 기존 미커밋 작업 위에 구현했다. 시작 사본 `/tmp/g8a-baseline`과 비교하여 이번 변경을 검토했다. Standard gameplay semantic version은 **`deck-chess-standard-1`** 그대로다. Challenge, 기물 정의, Draw 정책, Base/Home, 제물 규칙, DB 및 Chessembly 문법은 변경하지 않았다.

## 1. Bot이 실제로 보는 정보

`engine/src/ai/observation.rs::BotObservation`이 authoritative state에서 결정용 상태를 만든다. 이 상태는 실전 commit이나 기록 snapshot으로 쓰지 않는다.

| 정보 | Standard Bot 입력 |
| --- | --- |
| Board, 지형, 현재 턴, 공개 action history | 유지 |
| 자기 Hand / Pocket / Extra | 정확한 인스턴스와 runtime state 유지 |
| 상대 Hand | 장수만 별도 필드에 보관; ID·Piece object 제거 |
| 상대 Pocket | 장수도 제공하지 않는 unknown reserve; ID·Piece object 제거 |
| 상대 Extra | 공개 인스턴스와 runtime state 유지 |
| 상대 원래 덱 점수 | 0으로 정규화하여 hidden material 역산 방지 |
| Starting 이력에 남은 hidden reserve ID | 제거 |
| 숨김 전용 custom package | manifest·정의에서 제거; 남은 정의로 실행 cache 구성 |

일반 내장 카탈로그와 보이는 인스턴스가 사용하는 custom package는 유지한다. 공개 history로 이미 알려진 정보를 재구성하는 별도 belief 시스템은 만들지 않았다. 관측 상태의 필드는 crate 내부에서만 접근한다.

## 2. 숨김 Hand/Pocket과 탐색 경계

모든 난이도는 동일한 `choose_bot_action_with_config`에서 한 번 관측 상태를 만든 뒤 root 생성, alpha-beta, quiescence, beam, ordering 및 TT를 실행한다. 재귀에서 authoritative state로 돌아가지 않는다. 공개 `evaluate`, `generate_ai_actions`, `order_ai_actions`도 입력을 투영한다. 내부 ordering/evaluation은 이미 투영된 상태를 사용한다.

상대 응수에서는 Board Move/Ability 및 관측 상태만으로 생성할 수 있는 행동을 탐색한다. 상대 원래 Hand의 Drop이나 Pocket의 교대/공수 후보를 생성하지 않는다. 상대 Extra는 공개 Board 제물로 가능한 소환을 탐색할 수 있다. 가상 공개 action으로 Board에서 Pocket에 돌아간 기물처럼 검색 중 공개적으로 알려진 결과는 해당 가상 상태에 유지한다.

실전 Bot action은 선택이 끝난 뒤 **원래 authoritative state**에 `submit_action`으로 다시 검증·적용한다. projection을 실전 상태로 저장하는 경로가 없다. 검색에서 사용하는 저수준 canonical apply는 기존 계약을 유지한다.

## 3. Public-information equivalence

상대 Hand 타입, 상대 Pocket 타입뿐 아니라 실제 Pocket 장수를 바꾼 두 상태를 비교했다. 다음이 일치한다.

- root evaluation 및 정렬된 root action 목록
- 모든 난이도의 같은 예산 fallback action/score
- 실제 깊이 1 완주 Hard search의 action/score/node 수: TT 켜짐/꺼짐 각각
- 관측 PositionKey; hidden-only custom package와 상대 total_score도 제거 확인

Easy의 완료 탐색 후 선택은 기존 시간 기반 top-3 random이다. 공정성은 동일 정보에서 후보 pool/score가 같다는 뜻이며 서로 다른 시각의 Easy 호출이 항상 같은 수를 고른다는 뜻이 아니다. Normal/Hard의 wall-clock 중단 깊이도 머신 부하에 따라 달라질 수 있다. 동등성 검증에는 충분한 동일 depth/node/time 예산을 사용했다.

## 4. Board / Hand / Pocket 평가

기존 `ai_board_value()` / `ai_pocket_value()` 및 score fallback을 재사용한다. 새 PieceDefinition 필드는 없다. Centipoint 기준 구성은 다음과 같다.

```text
Board = 100 × ai_board_value
Hand = 100 × ai_pocket_value
Pocket = 40 × ai_pocket_value  (Hand 가치의 40%)
Standard score = 자기 known material − 상대 공개/known material
               + signed Extra option value
               − bounded hidden reserve threat
               + 기존 Move/Drop mobility와 King capture threat
```

King material 제외, 승패 ±1,000,000, mobility 가중치 각각 2, King capture threat 100,000을 유지한다. 지형과 기물별 이동·능력·포획 효과는 기존 합법수 생성 및 canonical transition을 사용한다. 기존 평가에 없던 독립 terrain/Ability 점수 체계를 새로 만들지 않았다.

Pocket discount는 `standard.rs::POCKET_DISCOUNT_PERCENT` 한 곳이다. 미래 Draw와 현재 Pocket Ability의 잠재 가치를 남기며 즉시 일반 Drop 가능한 Hand와 구별한다. 배치된 공수부대 대원(paratrooper)의 Board 0 / reserve 3이라는 기존 의도도 보존한다.

## 5. Hidden reserve uncertainty

상대 Hand는 장수당 200cp, Pocket unknown은 고정 100cp, 합산 최대 1,600cp다. 실제 Pocket이 비어 있어도 Bot만 빈 것을 알아내어 고정 불확실성을 지우지 않는다. 상수는 `standard.rs`에 모았다.

실제 hidden type의 정의나 점수를 순회하지 않는다. Hand count는 root 관측에서 고정하고 미래 Draw 장수/identity를 가정해 늘리지 않는다. 이는 작은 baseline 휴리스틱이며 확률적으로 보정된 위협 모델은 아니다.

## 6. Extra option value

Extra는 Board material로 합산하지 않는다. 플레이어별 현재 known sacrifice 후보로 bounded selector를 실행하여 최선 조합의 기회비용을 얻는다.

```text
소환 가능한 비용 조합이 없으면 option = 0
그 외 option = min(300, max(0, summoned_board_cp − sacrifice_loss) / 4 + 25)
```

상대 Hand는 없어진 관측 상태이므로 그 정체를 이용해 affordability를 계산하지 않는다. 옵션 평가는 비용 지불 가능성에 대한 근사이며 모든 목적지의 실제 전술 가치를 정적으로 계산하지 않는다. 실제 action 후보는 기존 target validator를 반드시 통과한다.

## 7. Future Draw와 RNG

Bot의 실제 턴 시작 Hand에는 기존 서버 initial/turn Draw가 이미 반영되어 있다. 탐색은 이 상태를 사용한다. engine AI는 `server::draw` / getrandom에 의존하지 않고 random PieceId를 Pocket→Hand로 이동시키지 않는다. 미래 Draw 가치는 Pocket discount와 hidden uncertainty로만 반영한다.

실제 action commit 이후의 상대 Draw는 G3-B `turn_start`에서 한 번 확정한다. 강제 착륙 중간에는 없고 마지막 frame에만 붙인다. 종료 action에는 Draw가 없다. RNG source/선택 알고리즘/재시도 및 resolution 형식은 변경하지 않았다.

## 8. AiAction / canonical 왕복

`AiAction::ExtraSummon(ExtraSummonAction)`을 추가했다. `From<AiAction> for TurnAction` 및 역변환은 Extra ID, 제물 목록의 **순서와 exact IDs**, target을 모두 보존한다. 제물 순서를 뒤집은 action도 roundtrip 및 실제 apply로 확인했다.

search apply, beam tactical simulation, 서버 record 변환, 프론트 AiAction union과 Bot label/preview에 variant를 연결했다. 별도 기록 형식은 없다.

## 9. Bounded sacrifice selector

`standard.rs::select_sacrifice_subsets`는 기존 `sacrifice_candidates`에서 후보를 얻는 deterministic beam knapsack이다.

1. 기존 중앙 summon policy와 zone validation을 통과한 후보 ID만 사용한다.
2. 각 ID를 한 번 처리하며 현재 미완성 조합에 그 ID를 포함/제외하는 후보를 만든다.
3. cost 이상이면 완료 목록, 미만이면 frontier에 넣는다.
4. 같은 지급 score마다 최대 4개의 exact-ID 대안을 보존한다.
5. 전체 미완성 frontier는 **128개 이하**, Extra 인스턴스당 완료 조합은 **4개 이하**다.
6. frontier가 많으면 utility/score와 진행 score를 이용해 제한하고 빈 조합을 유지한다.
7. 최종 조합별 `generate_extra_summon_actions`를 호출하여 실제 합법 목적지를 얻는다.

`2^N` 전체 subset, cost 크기의 배열, 무제한 조합 목록은 없다. 30개 및 **2,003개** 실제 후보에서 4개 조합으로 제한되는 테스트를 통과했다. 정상 Extra 3기에서 최대 12개 subset이며 각 subset의 target 수는 보드 칸 수로 제한된다. 기물 수에 비례해 검증/목록 복사 비용은 증가한다. G5 zone 검사의 기존 비용을 선형이라고 주장하지 않는다.

## 10. Sacrifice opportunity cost / overpayment

Board 제물은 `100 × ai_board_value + 작은 중앙 위치 보너스`, Hand 제물은 `100 × ai_pocket_value`다. 단순 gameplay score 합과 다르다. 조합 정렬은 utility loss + 초과 score당 5cp, 이어서 제물 수 및 exact ID로 결정한다. 같은 score의 여러 다른 위치/상태 조합을 즉시 하나로 합치지 않는다.

게임 validator는 계속 **sum(score) >= summon_cost**다. 구행 25에 Queen 3기/27점 조합을 허용하고 선택 전부를 제거한다. King·Pocket·상대·중복 ID는 기존 후보/validation에서 제외된다. 구행 Hand+Board, 폭격기 Board는 AI에 복제한 ID 분기 없이 `summon_policy`를 사용한다.

## 11. Unified move ordering와 전술 처리

ExtraSummon의 ordering net은 소환 Board gain + 실제 포획 gain − sacrifice utility loss − 초과 지불 penalty다. 실제 canonical transition으로 King capture/적 제거 효과를 검사한다.

- King capture는 기존 최고 순위.
- 이익이 있는 capture summon은 기존 capture 범주.
- 이익이 있는 quiet summon은 Drop/Ability 범주.
- 손실이 큰 summon은 일반 이동보다 낮은 범주.

소환의 tactical/forced defense 처리는 기존 beam mandatory 경로, quiet summon은 reserve quota를 사용한다. noisy summon은 quiescence 후보에도 들어간다. Move/Drop/Ability의 Legacy ordering 비교 순서와 canonical tie-break는 유지한다. 현재 기본 구행/폭격기에 capture-on-drop 규칙을 새로 부여하지 않았다.

## 12. Beam / identity / cache

Legacy의 Pocket representative-ID 및 기존 canonical equivalence를 보존한다. Standard에서는 custom effect의 ID 참조까지 동등하다고 증명하기 어려워 **Hand/Ability/Extra의 representative 축약을 하지 않는다**. 타입과 runtime state가 같은 인스턴스도 정확한 ID로 유지한다. 기존 beam width/quota는 적용한다.

기존 `PositionKey` 구현을 변경하지 않았다. G1~G4의 ruleset/Board/Pocket/Hand/Extra/runtime-state 구분을 그대로 사용하며 **투영된 검색 상태만 key로 만든다**. 상대 hidden ID는 key에 없다. 자기 Hand state 변경은 key가 달라지고 두 Hand ID 모두 후보에 남는 테스트를 추가했다.

TT는 매 root 호출에서 새로 생성된다. root의 공개 Hand 장수와 viewer는 해당 SearchContext에서 고정되어 있으므로 별도 belief cache key가 필요하지 않다. 이 테이블을 서로 다른 관측/root 사이에서 공유하면 안 된다.

## 13. 난이도

현재 명칭은 Easy / Normal / Hard다. 사용자 목표의 Medium에 해당하는 기존 단계는 Normal이며 이름을 바꾸지 않았다.

| 난이도 | 최대 depth | nodes | soft / hard ms |
| --- | ---: | ---: | ---: |
| Easy | 3 | 500 | 50 / 100 |
| Normal | 4 | 3,000 | 150 / 300 |
| Hard | 5 | 10,000 | 400 / 800 |

기존 iterative deepening, aspiration, node/time abort, 완료 iteration fallback, Easy top-3 변동을 유지했다. Extra subset 상한은 세 난이도 공통 4다. 별도 Standard Bot framework를 만들지 않았다.

## 14. 서버 endpoint / timeline / record

Standard Bot capability는 G3-A와 같다. 지정된 인간 client가 지정된 상대 Bot만 실행할 수 있다. 서버 변환 함수에 ExtraSummon을 추가한 것 외에 `run_bot_turn` commit 순서를 바꾸지 않았다.

실제 양쪽 진영 테스트에서:

- 초기/자기 턴 Draw 완료 상태의 Hand 4 확인.
- Bot Hand Drop 실제 제출, 다음 인간 Hand +1.
- 공수부대 대원 Board 9기(기존 Board AI 가치 0, rule score 합 27)를 제물로 구행을 실제 선택·commit.
- Extra 제거, 제물 exact IDs 및 순서, target, 후속 Draw, record action 보존.
- 마지막 timeline의 pieces/players/board/current_player/hand_counts와 최종 view 일치.
- `state_at_ply` canonical action+saved Draw replay와 최종 authoritative hash 일치.
- forced landing은 같은 Bot의 2개 frame, 마지막에만 Draw 1회.

소환 fixture는 정상 내장 기물 정의와 runtime zone을 사용한다. 특정 중반 상태를 만드는 integration fixture이며 일반 시작 덱을 바꾸거나 소환 규칙을 완화하지 않았다.

## 15. Debugger / privacy

Standard 응답의 **stats 생략 정책 유지**. 공정한 Bot 평가라도 Bot 자신의 Hand/Pocket 값은 인간에게 비공개이므로 이를 공개하지 않는다. timeline과 최종 GameView는 기존 수신자별 projection을 사용한다. 전체 JSON에서 남은 Bot hidden reserve IDs와 stats 부재를 검사했다.

프론트는 `extra_summon`을 typed AiAction으로 받고 공개 Extra 이름, 제물 수, target을 표시한다. 누락 Piece object에 hidden ID를 이름 fallback으로 사용하지 않는다. preview는 소환 목적지를 강조하며 상태 적용은 서버 timeline을 사용한다. 별도 debugger 데이터 포맷은 추가하지 않았다.

## 16. 성능 benchmark

명시 실행:

```sh
cargo test --offline -p brainfuck-chess-engine --test ai g8a::standard_search_benchmark -- --ignored --nocapture
cargo test --offline -p brainfuck-chess-engine --test ai_benchmark ai_search_difficulty_budgets -- --ignored --nocapture
```

Standard fixture: Board 중반 배치/능력 기물, Hand 27기, Extra 구행 2+폭격기 1, Hand+Board 다수 제물 후보. 디버그 빌드 로컬 단일 실행이며 절대 성능 보장은 아니다.

| 측정 | 결과 |
| --- | ---: |
| root generated legal actions | 415 |
| Extra subset 총수 | 11 (최대 12) |
| generation 및 subset 집계 시간 | 45ms |
| depth 1 / completed / nodes / 시간 | 1 / 1 / 24 / 151ms |
| depth 2 / completed / nodes / 시간 | 2 / 2 / 74 / 771ms |
| depth 3 / completed / nodes / 시간 | 3 / 3 / 444 / 3,105ms |
| depth 3 cumulative legal actions | 13,175 |

위 depth 측정은 node 500 / soft 2,000ms / hard 4,000ms의 명시 benchmark 예산이다. 제품 기본 Hard 800ms와 혼동하지 않는다. K=4는 세 Extra의 다양한 기회비용 조합을 남기면서 이 fixture에서 11개로 제한되는 작은 baseline 값으로 선택했다.

기존 Legacy 난이도 benchmark는 모든 fixture × Easy/Normal/Hard의 합법 선택을 확인하고 통과(전체 8.67초)했다. 기존 시간 예산 특성상 개별 평가/합법수 생성 중 hard deadline을 중단하지 않으므로 약간의 초과는 가능하다. Standard 역시 2,003개 fixture의 selector bound가 일반 gameplay의 800ms 응답 보장을 뜻하지 않는다.

## 17. G8-A 변경 파일

- `engine/src/ai/observation.rs` 신규: 결정용 정보 경계.
- `engine/src/ai/standard.rs` 신규: 중앙 heuristic 상수, 기회비용, bounded subset/소환 후보.
- `engine/src/ai/{mod,types,evaluate,search,beam,move_ordering}.rs`: 공개 AI API, Extra variant/왕복, zone 평가, 관측 탐색, 정확 ID, 통합 ordering 및 unit regression.
- `engine/tests/ai.rs`: Standard legality/fairness/reserve/subset/왕복 및 ignored benchmark.
- `server/src/main.rs`: AI Extra action의 canonical record 변환 한 분기.
- `server/src/draw.rs`, `server/src/draw/g8a_tests.rs` 신규: 테스트 모듈 등록 및 실제 endpoint/Draw/timeline/replay/forced-landing 회귀.
- `frontend/src/types/game.ts`, `components/GameScreen.vue`: AiAction union 및 Extra label/preview.
- `frontend/src/standardGameUi.test.ts`: 실제 GameScreen setup에서 Extra preview/hidden ID 미표시.
- 이 문서: 계약/완료 조건/검증 결과.

분산된 기존 action 전달 경계와 각 경계의 테스트를 연결해야 하므로 여러 파일이 필요했다. 기존 G1~G7의 다른 변경, 특히 Challenge/SQL/덱 코드/기물 정의는 보존했다. 브라우저용 임시 harness 4파일은 제거했다.

## 18. 자동 검증

| 명령 | 결과 |
| --- | --- |
| `npm test --prefix frontend` | runner 22개 파일 통과, 실패 0 |
| `npm run typecheck --prefix frontend` | 통과 |
| `npm run lint --prefix frontend` | 통과; 기존 vue-tsc script |
| `npm run build --prefix frontend` | 통과 |
| `cargo test --offline -p brainfuck-chess-engine` | 228 passed, 8 ignored, 실패 0 |
| `cargo test --offline -p brainfuck-chess-server` | 147 passed, 9 ignored, 실패 0 |
| `cargo check --offline --workspace --all-targets` | 통과 |
| `cargo build --offline --workspace` | 통과 |
| 위 Standard 및 Legacy 명시 benchmark | 통과 |
| 변경 Rust rustfmt / `git diff --check` / 시작 사본 diff 검토 | 통과 |

추가 exact 제물 순서 보강 후 `cargo test --offline -p brainfuck-chess-engine --test ai g8a`도 통과했다. 초기 새 fixture의 Rust 타입/기물 ID 및 capture-on-drop에 대한 잘못된 가정을 테스트 작성 중 수정했으며, 실제 규칙을 테스트에 맞춰 변경하지 않았다.

기존 frontend 500kB 번들 경고, 서버 미사용 생성자 2개 경고가 남는다. 엔진 ignored 8개는 기존 7개와 새 Standard benchmark 1개이며 위 명시 실행을 전체 suite와 별도로 수행했다. 서버 ignored 9개는 기존 외부 DB/진단 테스트다. 외부 PostgreSQL, 배포, migration 실행은 이번 범위에서 하지 않았다.

## 19. 실제 브라우저 플레이

외부 DB 없는 임시 서버 18088 / Vite 15188에서 실제 GameScreen + gameApi + 서버 생성/submit/Bot endpoint를 사용했다. harness는 **정상 유효 Standard 덱**(King+Front Pawn6, Pocket Knight8, Extra3)을 제출할 뿐 engine state나 응답을 주입하지 않는다. 로비/덱 편집기 전체 E2E는 아니다.

- 인간 White / Bot Black: initial 자기 Hand4·상대3·Pocket 비공개·Extra 양측3 확인. 인간 Knight e1 Drop → Bot Knight f6 Drop → 인간 Knight e1→f3 Board Move → Bot Knight e4 Drop. 턴/Hand Draw 및 기보 확인.
- Bot White / 인간 Black: Bot Knight f3 Drop, Black의 실제 첫 턴 Hand4 확인. Black Knight e8 Drop 제출 확인.
- 별도 짧은 Bot White 게임을 Black 기권으로 종료: White 승리/resignation 표시, 완료 record 조회 성공, `deck-chess-standard-1` 및 frontend Replay 2 frame 복원 확인. 기권은 별도 result metadata이므로 마지막 **action frame**은 기존 계약대로 playing이다.
- live 종료 화면에서도 상대 Hand count/Pocket privacy 유지.

짧은 실기에서는 ExtraSummon 미발생. 양쪽 Bot의 실제 소환 commit 및 exact Replay는 §14 integration tests로 별도 확인했다. 모든 난이도 장기 대국/모바일/ReplayPage 전체 탐색/브라우저 forced landing 실기는 하지 않았다. 검증 탭과 임시 서버/파일을 정리했다.

## 20. Legacy / G1~G7 회귀

Legacy는 Pocket Drop, Board/Pocket 원래 값, 기존 beam equivalence, Move/Drop/Ability ordering, 난이도/시간 예산을 유지한다. 명시 King-capture golden은 세 난이도에서 `wr→(7,7), captured=bk, score=1,000,000`이다. 기존 TT/aspiration/quiescence/Ability 및 Legacy benchmark도 통과했다.

전체 suite는 G2 Base/Home, Hand/Draw/privacy, Extra/ExtraSummon, forced landing/ammo/air, Replay/Analysis, DC4, stable rules version, HUD, pre-G1 hash 및 Challenge registry/실행을 포함한다. `raining_men`의 Legacy Board 구행을 수정하지 않았다. 전략 선택 결과가 모든 Legacy 중반에서 과거 실행과 bit-for-bit 동일하다는 무제한 보장은 하지 않으며 고정 golden/기존 회귀 및 실제 benchmark가 검증 범위다.

## 21. 남은 위험과 G8-B 계약

- opponent hidden reserve의 구체적인 Drop/Ability 응수와 future Draw는 모델링하지 않는다. bounded uncertainty 기반의 공정한 baseline이며 최적 불완전정보 전략이 아니다.
- beam knapsack은 상한 안에서 좋은 조합을 찾는 근사다. 전략적으로 좋은 조합/목적지가 pruning될 수 있고 전수 최적성을 보장하지 않는다.
- Standard exact IDs 유지 및 canonical validation은 비용이 든다. 매우 큰 Hand/custom action tree의 hard latency 보장은 별도 과제다.
- 공개 custom action의 복잡한 전술 품질/장기 자기 대국 통계는 별도 검증 범위다. 실전 적용은 항상 authoritative validator를 통과한다.
- 외부 DB/운영 배포는 미실행이며 G3-C migration 선행 계약을 유지한다.

G8-B는 ChallengeDefinition의 format 표현, Standard 공식 덱/시작 상태, Challenge 선택·clear·UI·회귀를 별도로 다룬다. 이번 Bot의 관측 경계, bounded Extra action 및 canonical Draw/Record를 재사용한다. 기존 세 Challenge와 raining_men을 자동으로 Standard로 전환하지 않는다. Bot 전략의 변화는 gameplay semantic version 변경이 아니다.

## 22. 완료 조건 대조

| 목표 요구 | 증거 / 결과 |
| --- | --- |
| 1~8 합법 Move/Hand Drop/Ability/Extra 및 제물 zone | 모든 생성 action authoritative apply, 각 Extra ID/정책/King/Pocket 제외 검사 |
| 9~13 hidden fairness/난이도/debug | root action/order/evaluation, 실제 fixed-depth TT on/off, 공통 모든 난이도 경계, custom pruning, 서버 전체 JSON privacy |
| 14~17 자기 zone 가치 | Hand/Pocket 타입 변화 반영, Pocket discount, Extra option 감소, Board/Hand utility 차이 |
| 18~21 Draw | 서버 양측 Hand4, engine search 원본 불변 및 server RNG 미의존, 실제 commit Draw1 |
| 22~30 bounded subsets | cost>=, Queen27 overpayment, policy/King/unique ID, 30/2,003 후보 상한, opportunity order |
| 31~36 canonical/commit | 뒤집은 exact 제물 순서 양방향 변환, target/Extra/제물 제거, endpoint record+Draw+timeline |
| 37~41 unified search/cache | Extra tactical/quiet ordering 및 QSearch 연결, 실제 profitable 소환 선택, exact IDs/관측 key/TT 동등성 |
| 42~48 actual games | 양쪽 Bot Drop/소환 endpoint, forced landing 2-frame, 기존 종료 회귀, exact replay 및 브라우저 양방향/기권 |
| 49~55 Legacy/Challenge | 3난이도 golden, 기존 AI/Ability/Pocket/Challenge tests 및 명시 Legacy benchmark |
| 56~63 G1~G7 | 전체 frontend/engine/server suite 및 pre-G1 hash |
| 문서/최종 검증 | 본 문서 및 §18 명령 통과 |

**G8-A 구현 완료.** G8-B Challenge 변경과 운영 배포 완료를 뜻하지 않는다.
