# Goal 4 — Deck-Chess-aware Quiescence Search 결과

## 완료 범위와 변경 파일

Goal 3의 Iterative Deepening controller, shared SearchContext/TT, last-completed fallback과 soft/hard/node semantics를 유지하면서 normal depth-0 leaf를 Deck-Chess-aware QSearch로 교체했다.

* `engine/src/ai/search.rs`
  * stand-pat alpha-beta QSearch와 abort propagation
  * noisy Move/Drop/Ability classifier 및 category별 canonical generation
  * QSearch node counting, 8-ply safety bound, normal TT 비저장 정책
  * horizon, exclusion, promotion, abort/safety 및 ID fallback regression
* `engine/src/ai/move_ordering.rs`
  * QSearch 전용 deterministic tactical priority
* `engine/tests/ai_benchmark.rs`
  * 기존 9개 출력에 `qnodes` 추가
  * recapture/capture-on-drop/enemy-recall 전용 benchmark 추가
* `server/src/main.rs`
  * bot turn 통계에 `qnodes` 노출

Aspiration Window, PVS, LMR, SEE, delta/futility pruning, QSearch TT, evaluation tuning, Airborne pruning과 arbitrary action truncation은 구현하지 않았다.

## QSearch architecture

```text
alpha_beta(depth == 0)
  -> quiescence_search(qply = 0)
       -> stand_pat = evaluate(state)
       -> terminal 또는 qply == 8이면 반환
       -> noisy category만 canonical generation
       -> deterministic noisy ordering
       -> canonical apply
       -> quiescence_search(qply + 1)
```

QSearch는 normal search와 같은 `SearchOutcome::Complete/Aborted`를 반환한다. hard time 또는 누적 node budget abort는 alpha-beta, root, Iterative Deepening controller까지 전파된다. 현재 iteration의 partial Q score/root scores는 폐기되고 마지막 completed iteration만 반환된다. depth 1조차 완료되지 않으면 Goal 3의 deterministic legal fallback과 static evaluation을 그대로 사용하며 fallback에서 새 QSearch를 시작하지 않는다.

## Stand-pat semantics

현재 evaluator는 bot 관점 score를 반환한다.

* bot 차례(maximizing): `stand_pat >= beta`이면 fail-soft stand-pat cutoff, 아니면 alpha를 올린다.
* 상대 차례(minimizing): `stand_pat <= alpha`이면 fail-soft stand-pat cutoff, 아니면 beta를 내린다.
* noisy action이 없으면 stand-pat을 반환한다.
* terminal state는 noisy generation 없이 기존 terminal `evaluate()`와 `WIN_SCORE` semantics로 즉시 반환한다.

soft limit은 QSearch를 중단하지 않는다. completed ID iteration 뒤 다음 normal depth 시작 여부만 결정하는 Goal 3 정책을 유지한다.

## Noisy action allow-list

다음만 포함한다.

* `MoveAction.captured_piece_id.is_some()`: 일반/Chessembly capture와 King capture
* `MoveAction.promotion.is_some()`: non-capture 및 capture promotion
* `DropAction.captured_piece_id.is_some()`: canonical capture-on-drop
* `ability_id == "recall"`이며 target owner가 action player와 다른 enemy recall

분류 책임은 `is_noisy_move`, `is_noisy_drop`, `is_noisy_ability`, `generate_quiescence_actions`에 모았다. 최종 action은 모두 기존 legal generator가 만든 canonical action이며 AI 전용 legality나 임의 action을 만들지 않는다.

제외한 action은 ordinary quiet move/drop, Airborne airdrop, Alternating Soldier `relieve`, friendly recall 및 capture/promotion이 없는 state/cooldown/type-transition move다. 이들을 포함하면 quiet state manipulation 또는 multi-deployment 조합이 tactical extension을 비정상적으로 확장할 수 있다.

## Category별 generation과 Airborne 회피

전체 `generate_ai_actions()`를 만든 뒤 filter하지 않는다.

* Move: canonical legal moves만 생성하고 capture/promotion을 유지한다.
* Drop: 현재 pocket에서 definition의 `can_capture_on_drop`이 true인 concrete piece ID만 고른 뒤 `generate_piece_legal_drop_actions`를 호출하고 captured action만 유지한다. `paratrooper` type ID를 policy에 하드코딩하지 않았다.
* Ability: 현재 player의 on-board piece 중 definition에 `recall` option이 있는 actor만 찾고 `generate_piece_legal_ability_actions(state, actor, "recall")`를 직접 호출한다. target owner 검사 후 enemy recall만 유지한다.

따라서 QSearch는 global `generate_legal_ability_actions()`를 호출하지 않으며 Airborne single/deployment combination을 qnode마다 생성하지 않는다. `airdrop`, `relieve`, friendly recall 제외 regression도 통과했다.

## Ordering

Q action은 다음 category priority를 사용한다.

1. King capture
2. capture + promotion
3. capture(동일 category에서는 captured piece score가 큰 순서)
4. promotion
5. capture-on-drop
6. enemy recall

동점은 Goal 2의 allocation-free canonical comparator를 그대로 사용하므로 HashMap/HashSet iteration order에 의존하지 않는다.

## Node counting과 safety bound

`searched_nodes`는 normal + QSearch 전체 방문 node다. normal depth-0 leaf에서 QSearch로 handoff할 때 동일 state를 한 번만 `searched_nodes`로 세고 동시에 `qnodes` subset으로 표시한다. 재귀 qnode는 두 counter를 모두 증가시킨다. 따라서 `normal-only nodes = searched_nodes - qnodes`, `qnodes <= searched_nodes`다. `depth_reached`와 `completed_depth`는 normal ply/ID depth 의미를 유지하며 qply로 증가하지 않는다.

각 재귀 qnode와 noisy loop는 같은 `SearchContext::hard_limit_reached()`를 사용한다. safety bound는 `MAX_QUIESCENCE_PLIES = 8`이다. qply 8에서는 stand-pat만 반환하며 action count를 자르지 않는다. node budget 1에서 QSearch가 `Aborted`되고, qply 8 regression이 정확히 한 node/한 evaluation으로 종료됨을 확인했다.

## QSearch와 TT

QSearch는 기존 normal TT를 probe하거나 store하지 않는다. depth-0 분기는 PositionKey 생성보다 앞에 있어 qleaf 및 재귀 qnode에서 key canonicalization도 하지 않는다. q-depth/bound 구분이 없는 normal depth-0 Exact entry로 Q result를 오염시키지 않기 위한 보수적 정책이다.

normal depth 1 이상 node는 기존 variant-safe key, Exact/Lower/Upper bound, best-action ordering과 incomplete node 비저장 정책을 그대로 쓴다. 같은 TT는 ID iteration 전체에서 유지된다. depth-0 normal entries가 사라졌으므로 cross-iteration shallow hit는 depth 2→3부터 관찰되며 regression도 이 경계를 검증한다. TT OFF에서는 key generation 및 probes/hits/cutoffs/stores가 계속 모두 0이다.

## Regression tests

추가하거나 강화한 검증:

* capture Move와 non-capture promotion이 noisy이며 promotion canonical apply 결과가 일치
* `can_capture_on_drop` piece의 canonical captured Drop이 noisy, ordinary drop은 제외
* enemy recall은 noisy이고 public canonical apply가 성공, friendly recall은 제외
* quiet Move, Airborne, `relieve`, friendly recall 제외
* 일반 recapture가 stand-pat보다 불리한 tactical score를 산출
* capture-on-drop reply가 stand-pat보다 불리한 tactical score를 산출
* normal move cooldown으로 capture를 배제한 Green Camp position에서 enemy recall 자체가 score에 반영
* QSearch node abort와 8-ply safety termination
* QSearch의 TT store 0
* Q abort가 depth-2 iteration을 중단하면 depth-1 action/score 유지 및 `depth_reached > completed_depth`
* 모든 difficulty의 immediate King capture 유지
* 5개 variant position과 기존 9개 benchmark의 TT ON/OFF action/score/completed-depth 일치
* TT OFF key/TT counters 0

## 전체 검증

```sh
cargo test --workspace --all-features
```

* 실행 146개 통과
* 실패 0개
* 명시 실행 benchmark 4개 ignored

```sh
cargo clippy --workspace --all-features -- -D warnings
```

* warning/error 없이 통과

debug/release에서 TT ON/OFF benchmark를 모두 실행했고 release에서 horizon 및 difficulty benchmark도 실행했다. elapsed는 단일 실행 관측값이며 strict assertion이 아니다.

## 기존 9개 release benchmark

실제 Normal limits(depth 2, nodes 3,000, soft 150 ms, hard 300 ms), TT ON 결과다. 9개 모두 depth 2를 완료했다.

| position | selected action | score | nodes/qnodes | completed | elapsed ms | legal/drop gen | applications | beta cutoffs | keys/probes | hits/cutoffs/stores |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | `wq d3-f5 x bb` | 416 | 878/833 | 2 | 226.095 | 1,880/1,498 | 878 | 44 | 45/45 | 0/0/45 |
| `tactical-captures` | `wr e3-h3` | 100,198 | 254/221 | 2 | 25.172 | 471/374 | 254 | 32 | 33/33 | 0/0/33 |
| `drop-branching` | `drop wq b2` | 1,903 | 222/157 | 2 | 22.870 | 465/380 | 222 | 63 | 65/65 | 0/0/65 |
| `standalone-ability` | `camp d4-e4 x enemy` | 506 | 42/30 | 2 | 3.202 | 86/73 | 42 | 11 | 12/12 | 0/0/12 |
| `piece-state-cooldown` | `windmill d4-g1` | 1,150 | 112/83 | 2 | 9.355 | 222/178 | 112 | 26 | 29/29 | 0/0/29 |
| `immediate-king-capture` | `wr e1-e8 x bk` | 1,000,000 | 46/31 | 2 | 3.259 | 91/76 | 48 | 15 | 15/15 | 0/0/15 |
| `drop-capture` | `drop para d1 x enemy` | 290 | 75/57 | 2 | 5.583 | 167/131 | 75 | 17 | 18/18 | 0/0/18 |
| `airborne-deployment` | `airdrop bishop/knight` | 1,488 | 1,036/728 | 2 | 87.415 | 1,996/1,632 | 1,036 | 297 | 308/308 | 9/9/299 |
| `alternating-soldier-pocket-swap` | `soldier d4-c3 x enemy` | 1,046 | 154/105 | 2 | 9.086 | 175/170 | 154 | 48 | 49/49 | 0/0/49 |

TT OFF도 9개 action/score/completed depth가 같고 position keys 및 네 TT 통계가 0이었다. Airborne OFF/ON은 nodes 1,047/1,036, qnodes 739/728이며 기존 transposition cutoff 9회가 유지됐다.

## Goal 3 대비

release TT ON 단일 실행 비교다.

| position | action changed | score G3 -> G4 | nodes G3 -> G4 | qnodes G4 | elapsed ms G3 -> G4 | completed | TT hits/cutoffs G3 -> G4 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | no | 228 -> 416 | 173 -> 878 | 833 | 58.243 -> 226.095 | 2 -> 2 | 45/0 -> 0/0 |
| `tactical-captures` | no | -318 -> 100,198 | 147 -> 254 | 221 | 36.434 -> 25.172 | 2 -> 2 | 33/0 -> 0/0 |
| `drop-branching` | no | 1,903 -> 1,903 | 211 -> 222 | 157 | 24.078 -> 22.870 | 2 -> 2 | 65/0 -> 0/0 |
| `standalone-ability` | no | 506 -> 506 | 38 -> 42 | 30 | 2.861 -> 3.202 | 2 -> 2 | 12/0 -> 0/0 |
| `piece-state-cooldown` | no | 1,150 -> 1,150 | 106 -> 112 | 83 | 8.704 -> 9.355 | 2 -> 2 | 29/0 -> 0/0 |
| `immediate-king-capture` | no | 1,000,000 -> 1,000,000 | 45 -> 46 | 31 | 3.111 -> 3.259 | 2 -> 2 | 15/0 -> 0/0 |
| `drop-capture` | no | 290 -> 290 | 56 -> 75 | 57 | 3.825 -> 5.583 | 2 -> 2 | 18/0 -> 0/0 |
| `airborne-deployment` | no | 1,488 -> 1,488 | 974 -> 1,036 | 728 | 79.631 -> 87.415 | 2 -> 2 | 317/18 -> 9/9 |
| `alternating-soldier-pocket-swap` | no | 1,046 -> 1,046 | 151 -> 154 | 105 | 9.191 -> 9.086 | 2 -> 2 | 49/0 -> 0/0 |

`qnodes`는 기존 leaf node도 subset으로 포함하므로 `nodes 증가량 == qnodes`는 아니다. depth-0 Q result를 normal TT에 저장하지 않아 Goal 3의 depth-1 leaf 기반 shallow hits가 사라졌고, Normal depth 2에서는 Airborne의 실제 normal transposition 9회만 남았다. 이는 cutoff가 아닌 shallow hit 수치를 줄이는 대신 Q-depth가 다른 Exact result 오염을 막는 correctness tradeoff다.

`middlegame`은 긴 capture continuation으로 work가 크게 증가했지만 depth 2를 완료했다. `tactical-captures`의 score 변화는 QSearch가 leaf tactical continuation과 king-capture threat를 반영한 결과다. 선택 action은 9개 모두 Goal 3과 같았다.

## QSearch horizon benchmark

depth 1, 충분한 deterministic budget의 release 결과다. 각 selected action은 기존 public canonical validation으로도 legal이다.

| position | selected tactical action | score | nodes/qnodes | elapsed ms | legal/drop gen | eval/apply |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `qsearch-recapture` | black rook captures white queen | 528 | 13/13 | 6.097 | 28/27 | 13/13 |
| `qsearch-capture-drop` | black paratrooper drops onto/captures white queen | 300 | 25/25 | 7.329 | 45/37 | 25/25 |
| `qsearch-enemy-recall` | black Green Camp recalls white queen | -115 | 4/4 | 2.090 | 10/9 | 4/4 |

unit horizon regression은 각 reply 직전 position의 static stand-pat과 Q score를 직접 비교해 recapture, capture-on-drop, enemy recall이 white 관점 score를 실제로 낮추는지 검증한다. 단순 `qnodes > 0` 확인에 그치지 않는다.

## 실제 difficulty budget

limits는 변경하지 않았다. release TT ON의 `nodes, reached/completed, elapsed ms`다.

| position | Easy | Normal | Hard |
| --- | --- | --- | --- |
| `middlegame` | 51, 1/1, 20.271 | 878, 2/2, 161.617 | 3,144, 3/3, 462.088 |
| `tactical-captures` | 41, 1/1, 5.226 | 254, 2/2, 25.637 | 1,137, 3/3, 119.683 |
| `drop-branching` | 65, 1/1, 7.948 | 222, 2/2, 23.662 | 5,716, 3/2, 801.144 |
| `standalone-ability` | 12, 1/1, 4.834 | 42, 2/2, 12.292 | 181, 3/3, 32.209 |
| `piece-state-cooldown` | 30, 1/1, 4.441 | 112, 2/2, 10.461 | 992, 3/3, 90.800 |
| `immediate-king-capture` | 15, 1/1, 1.367 | 46, 2/2, 3.342 | 316, 3/3, 24.129 |
| `drop-capture` | 18, 1/1, 1.659 | 75, 2/2, 5.758 | 200, 3/3, 14.538 |
| `airborne-deployment` | 308, 1/1, 32.017 | 1,036, 2/2, 89.411 | 5,558, 3/2, 800.592 |
| `alternating-soldier-pocket-swap` | 49, 1/1, 5.246 | 154, 2/2, 9.314 | 463, 3/3, 33.673 |

Easy는 모두 depth 1, Normal은 모두 depth 2를 완료했다. Hard는 Goal 3에서 Airborne만 depth 2 fallback이었으나 Goal 4에서는 `drop-branching`도 QSearch 비용 때문에 depth 3 중 abort되어 depth 2 결과를 반환했다. 나머지 7개는 depth 3을 완료했다.

Airborne Hard는 Goal 3의 9,456 nodes, 809.430 ms에서 Goal 4의 5,558 nodes, 800.592 ms로 바뀌었다. hard deadline 초과가 악화되지 않았고 오히려 작아졌지만, 이는 더 이른 time abort와 실행 변동을 포함하므로 QSearch 성능 개선으로 해석하지 않는다. global Airborne ability generation은 qnode에서 호출하지 않았다. known legal/action generation blind spot 때문에 deadline을 약 0.6 ms 넘긴 사실은 남아 있다.

## 남은 위험과 다음 단계

* `middlegame`처럼 capture chain이 많은 state에서는 qnodes와 legal/evaluation 비용이 크게 늘어난다. safety bound는 runaway 방어일 뿐 primary pruning이 아니며 SEE/delta pruning은 이번 범위 밖이다.
* evaluator 자체가 양측 move/drop mobility를 생성하므로 각 qnode stand-pat도 비싸다.
* QSearch는 모든 legal Move를 canonical 생성한 뒤 capture/promotion만 유지한다. 현재 별도 capture-only canonical move API가 없어 correctness를 우선한 선택이며 향후 성능 경계가 될 수 있다.
* capture-on-drop은 capability가 있는 pocket piece만 선별하지만 해당 piece의 quiet placement도 per-piece generator 내부에서는 만든 뒤 captured action만 남긴다.
* enemy recall allow-list는 현재 definition의 `recall` option과 canonical built-in per-piece generator 계약에 의존한다. 새로운 recall mechanic은 동일 canonical target/owner semantics를 명시해야 한다.
* normal TT의 Q-aware result는 depth 1 이상 entry로 저장되지만 Q leaf 자체는 저장하지 않는다. QSearch 전용 TT를 추가하려면 qnode 구분, remaining qdepth와 bound semantics가 필요하다.
* debug Normal `middlegame`은 300 ms hard limit으로 depth 2가 중단돼 depth 1 fallback을 반환했다. release에서는 depth 2를 완료했다. 성능 판단은 release를 기준으로 하되 debug abort semantics도 정상 동작했다.

noisy policy, horizon correctness, shared budget abort, TT isolation과 release budget 동작이 검증됐으므로 Goal 5 — Aspiration Window로 넘어갈 수 있는 상태다.

---

## Goal 4 보완

### 문제 분석과 변경 파일

Goal 4 초안의 QSearch stand-pat이 일반 `evaluate()`를 그대로 호출했다. 이 evaluator는 실제 `current_player`와 무관하게 양쪽 player view를 만들어 “해당 player가 지금 두면 King capture가 가능한지”를 계산하고 `KING_CAPTURE_THREAT = 100,000`을 더했다. 하지만 QSearch noisy allow-list는 quiet King escape/block을 탐색하지 않으므로, 상대 차례에 회피 가능한 위협도 확정된 전술 점수처럼 stand-pat에 남는 의미 충돌이 있었다.

변경 파일은 다음과 같다.

* `engine/src/ai/evaluate.rs`: 공통 `evaluate_internal` 경로와 QSearch용 threat-제외 entry point 추가
* `engine/src/ai/search.rs`: QSearch stand-pat 호출 교체, avoidable threat/actual King capture/King capture Drop regression 추가
* `engine/src/ai/move_ordering.rs`: captured definition이 King인 Drop을 Move King capture와 같은 최상위 priority로 배치
* `docs/ai-upgrade/results/goal-4-deck-chess-quiescence-search.md`: 보완 설계, 검증과 release 재측정 기록

### Evaluator 구조와 stand-pat 의미

material, pocket material, move/drop mobility, terminal result, profiling count와 score clamp은 하나의 `evaluate_internal` 구현을 공유한다. 차이는 King-capture-threat 적용 여부뿐이다.

```text
normal evaluate
  -> include_king_capture_threat = true

QSearch stand-pat
  -> include_king_capture_threat = false
```

terminal 분기는 옵션보다 앞서 공통으로 처리되므로 실제 `GameResult::KingCapture`의 `±WIN_SCORE` 의미는 변하지 않았다. 현재 player에게 합법적인 King capture가 있으면 QSearch가 그 canonical noisy action을 직접 적용하고 terminal score를 받는다.

quiet King move, block, ordinary drop을 QSearch에 추가하지 않았다. 이들을 추가하면 기존의 보수적 noisy allow-list가 무너지고 Deck Chess action space에서 QSearch가 일반 search처럼 확장될 수 있다. 이번 보완은 탐색 policy가 아닌 stand-pat의 의미만 바꾸는 선을 지켰다.

### King capture regression과 Drop ordering

`qsearch_stand_pat_excludes_avoidable_king_capture_threat` fixture는 White rook이 Black King을 공격하지만 현재 player는 Black이고 Black King에게 non-capture quiet move가 있다. 결과는 다음과 같다.

* normal evaluation과 Q stand-pat의 차이는 정확히 `100,000`
* Q stand-pat의 절댓값은 `100,000` 미만
* safety qply에서 QSearch 반환값은 threat를 뺀 stand-pat과 일치

`qsearch_finds_actual_move_and_drop_king_captures`는 반대 의미를 함께 검증한다.

* Move King capture가 noisy list의 최우선 action이고 QSearch score가 `WIN_SCORE`
* built-in Paratrooper generator가 만든 `captured_piece_id == enemy King`인 canonical Drop이 noisy
* captured definition의 `is_king` 확인으로 King Drop priority가 `(7, u32::MAX)`이며 일반 queen capture Drop보다 앞서 정렬
* public `apply_ai_action` 결과가 `GamePhase::Ended`, winner White, `GameEndReason::KingCapture`
* 같은 position의 QSearch score가 `WIN_SCORE`

보완 후 ordering은 King capture(Move/Drop), capture+promotion, high-value Move capture, promotion, ordinary capture-on-drop, enemy recall 순이다. 동점은 기존 allocation-free canonical comparator로 해결하므로 deterministic ordering을 유지한다.

기존 recapture, capture-on-drop horizon, enemy recall, promotion, quiet exclusions, node abort, safety limit, TT store 0, Q abort→ID fallback, immediate King capture와 variant TT ON/OFF regression은 삭제하거나 약화하지 않았다. noisy allow-list, `MAX_QUIESCENCE_PLIES = 8`, normal TT isolation, `searched_nodes = normal + QSearch`, hard/node abort와 last-completed ID fallback 의미도 그대로다.

### 전체 검증

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
```

* workspace test: 148개 통과, 실패 0, 명시 benchmark 4개 ignored
* clippy: warning/error 없이 통과
* targeted QSearch unit test 4개 통과
* 9개 benchmark legality, TT ON/OFF equality, TT OFF key counter regression 통과
* release TT ON, TT OFF, difficulty budget, QSearch horizon benchmark 모두 명시 실행

### 보완 후 9개 release benchmark

Normal limits, TT ON 단일 실행이다. elapsed는 strict assertion이 아니다. 모두 normal depth 2를 완료했다.

| position | selected action | score | nodes/qnodes | elapsed ms | completed | TT hits/cutoffs |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | `wq d3-f5 x bb` | 414 | 426/381 | 126.506 | 2 | 0/0 |
| `tactical-captures` | `wq d4-g4 x br2` | 198 | 271/238 | 29.654 | 2 | 0/0 |
| `drop-branching` | `drop wq b2` | 1,903 | 215/150 | 21.942 | 2 | 0/0 |
| `standalone-ability` | `camp d4-e4 x enemy` | 506 | 42/30 | 3.233 | 2 | 0/0 |
| `piece-state-cooldown` | `wk a1-b2` | 1,150 | 110/81 | 9.030 | 2 | 0/0 |
| `immediate-king-capture` | `wr e1-e8 x bk` | 1,000,000 | 46/31 | 3.209 | 2 | 0/0 |
| `drop-capture` | `drop para d1 x enemy` | 290 | 75/57 | 5.621 | 2 | 0/0 |
| `airborne-deployment` | `airdrop bishop/knight` | 1,488 | 1,037/729 | 87.473 | 2 | 9/9 |
| `alternating-soldier-pocket-swap` | `soldier d4-c3 x enemy` | 1,046 | 155/106 | 9.161 | 2 | 0/0 |

TT OFF는 9개 모두 TT ON과 action, score, completed depth가 같았다. 모든 position의 position-key generations, probes, hits, cutoffs, stores는 0이었다. Airborne의 OFF/ON nodes는 1,048/1,037로 normal TT cutoff 9회가 유지됐다.

### Goal 4 초안 대비

| position | score 초안 → 보완 | nodes 초안 → 보완 | qnodes 초안 → 보완 | elapsed ms 초안 → 보완 | completed |
| --- | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | 416 → 414 | 878 → 426 | 833 → 381 | 226.095 → 126.506 | 2 → 2 |
| `tactical-captures` | 100,198 → 198 | 254 → 271 | 221 → 238 | 25.172 → 29.654 | 2 → 2 |
| `drop-branching` | 1,903 → 1,903 | 222 → 215 | 157 → 150 | 22.870 → 21.942 | 2 → 2 |
| `standalone-ability` | 506 → 506 | 42 → 42 | 30 → 30 | 3.202 → 3.233 | 2 → 2 |
| `piece-state-cooldown` | 1,150 → 1,150 | 112 → 110 | 83 → 81 | 9.355 → 9.030 | 2 → 2 |
| `immediate-king-capture` | 1,000,000 → 1,000,000 | 46 → 46 | 31 → 31 | 3.259 → 3.209 | 2 → 2 |
| `drop-capture` | 290 → 290 | 75 → 75 | 57 → 57 | 5.583 → 5.621 | 2 → 2 |
| `airborne-deployment` | 1,488 → 1,488 | 1,036 → 1,037 | 728 → 729 | 87.415 → 87.473 | 2 → 2 |
| `alternating-soldier-pocket-swap` | 1,046 → 1,046 | 154 → 155 | 105 → 106 | 9.086 → 9.161 | 2 → 2 |

`tactical-captures`의 정확히 `+100,000` 수준 score artifact가 사라졌다. 초안의 `100,198`은 QSearch가 quiet evasion을 보지 않는 상태에서 speculative threat를 stand-pat에 확정 이득처럼 포함한 것이 원인이었다. 보완 후 `198`은 threat heuristic을 뺀 뒤 noisy capture continuation으로 계산된 값이다. 선택도 quiet `wr e3-h3`에서 canonical `wq d4-g4 x br2`로 바뀌어 artifact가 실제 root decision에 영향을 주고 있었음을 확인했다. 단, Goal 3의 `-318`과 같아야 한다고 강제하지 않았다. 보완 후 score에는 정상적인 QSearch continuation이 남아 있다.

`middlegame`은 동일 action을 선택하면서 nodes/qnodes가 크게 줄었다. threat를 뺀 stand-pat이 alpha/beta window와 continuation 순서에 영향을 준 결과다. `drop-branching`은 거의 동일하고, Airborne는 nodes/qnodes 1개 정도의 실행 차이만 있었다. 즉 보완이 noisy category나 Airborne generation 범위를 넓히지 않았다.

### 실제 difficulty budget

limits는 변경하지 않았다. 보완 후 release TT ON의 `Normal completed / Hard reached-completed, Hard elapsed ms`다.

| position | Normal completed | Hard reached/completed | Hard elapsed ms |
| --- | ---: | ---: | ---: |
| `middlegame` | 2 | 3/3 | 292.993 |
| `tactical-captures` | 2 | 3/3 | 117.845 |
| `drop-branching` | 2 | 3/3 | 703.082 |
| `standalone-ability` | 2 | 3/3 | 14.120 |
| `piece-state-cooldown` | 2 | 3/3 | 93.565 |
| `immediate-king-capture` | 2 | 3/3 | 24.255 |
| `drop-capture` | 2 | 3/3 | 14.558 |
| `airborne-deployment` | 2 | 3/2 | 800.652 |
| `alternating-soldier-pocket-swap` | 2 | 3/3 | 32.730 |

Normal은 여전히 9개 모두 depth 2를 완료했다. Hard `drop-branching`은 초안의 depth 3 abort/depth 2 fallback에서 이번 실행의 depth 3 완료로 바뀌었다. 실행 시간에 의존하는 결과이므로 strict 개선 보장으로 해석하지는 않는다. Airborne은 여전히 depth 3 진입 후 800 ms hard limit에서 abort되고 마지막 completed depth 2를 반환했다.

QSearch horizon release도 재실행했다: recapture 13/13 nodes/qnodes, 4.526 ms; capture-on-drop 25/25, 7.074 ms; enemy recall 4/4, 2.054 ms였다. 모두 기존 canonical tactical action과 score regression을 유지했다.

### 남은 위험과 다음 단계

* QSearch stand-pat에서 speculative King threat를 제거하는 것은 quiet evasion을 더하지 않고 위양성을 없애는 보수적 정책이다. 위협 정밀도 향상이 필요하면 향후 check/check-evasion 전용 설계로 다뤄야 한다.
* evaluator가 양쪽 mobility를 위해 legal Move/Drop을 생성하는 비용과 capture chain의 qnode 확장은 남아 있다.
* Hard Airborne의 expensive legal/action generation hard-limit blind spot과 depth 2 fallback은 여전히 재현된다. 이번 변경은 Airborne ability generation을 QSearch에 추가하지 않았다.
* QSearch는 계속 normal TT를 probe/store하지 않는다. q-depth가 다른 result를 normal Exact entry로 재사용하는 correctness 위험은 없다.

King-threat stand-pat 의미 충돌과 Drop King-capture ordering 누락이 수정됐고, actual terminal win, 기존 noisy policy, abort/ID fallback, TT isolation과 release budget 행동이 검증됐다. 따라서 **Goal 4는 완료 상태이며 Goal 5 — Aspiration Window로 진행할 수 있다.**
