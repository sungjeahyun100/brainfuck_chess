# Goal 1 Search Foundation 결과

## 완료 범위

Goal 1에서는 이후 Transposition Table 및 Iterative Deepening을 안전하게 추가할 수 있도록 검색 경계와 통계 구조를 정리했다.

구현한 항목은 다음과 같다.

* recursive search 결과를 `Complete(score)`와 `Aborted`로 구분했다.
* hard time 또는 node budget으로 중단된 subtree의 평가값을 정상 minimax score로 사용하지 않는다.
* root 후보 탐색을 `search_root`, 재귀 탐색을 `alpha_beta`로 분리했다.
* 엔진이 생성한 canonical action을 검색 내부에서 적용하는 crate-private trusted 경로를 추가했다.
* public `submit_action`은 기존 canonical validation을 그대로 수행한다.
* 실제 진입 깊이인 `depth_reached`와 전체 완료 깊이인 `completed_depth`를 분리했다.
* `SearchStats`에 `beta_cutoffs`, `qnodes`, `tt_hits`, `tt_cutoffs`, `aspiration_researches`를 추가했다. 아직 구현하지 않은 검색 기능의 카운터는 0이다.
* hard limit은 진행 중인 탐색을 즉시 abort하고, soft limit은 새로운 root search 시작 여부를 판단하도록 역할을 분리했다.
* Airborne의 canonical multi-deployment action을 순열 중복 없이 생성하도록 확장했다.

이번 Goal에서는 TT, 전체 Iterative Deepening, Quiescence Search, Aspiration Window 및 평가 함수 튜닝을 구현하지 않았다.

## 구조 변경

검색 호출 구조는 다음과 같다.

```text
choose_bot_action
  -> search_root
       -> alpha_beta
```

`alpha_beta`에서 hard limit을 만나면 `Aborted`가 root까지 전파된다. 이 경우 부분 검색에서 얻은 action/score는 폐기하고, 정렬된 첫 legal action과 현재 state의 정적 평가를 fallback으로 사용하며 `completed_depth`는 0으로 남는다.

action 적용 경계는 다음과 같이 분리했다.

```text
public/untrusted action
  -> submit_action
       -> canonical validation
       -> shared apply

engine-generated canonical action
  -> crate-private trusted apply
       -> shared apply
```

따라서 public validation을 약화하지 않으면서 검색 중 동일 legal action을 다시 생성해 검증하던 비용을 제거했다.

Airborne action은 pocket piece ID 순서를 canonical 순서로 고정하고, 이미 사용한 square를 추적하여 다음을 보장한다.

* 같은 pocket piece를 한 action에서 두 번 사용하지 않는다.
* 같은 square를 한 action에서 두 번 사용하지 않는다.
* deployment 순서만 다른 action을 중복 생성하지 않는다.
* 기존 single-deployment eligibility와 public `is_legal_ability_action` 검증을 그대로 만족한다.

## 변경 파일

* `engine/src/actions.rs`
  * validated public apply와 crate-private canonical apply 경계 분리
  * Move, Drop, Ability apply parity 단위 테스트 추가
* `engine/src/ai/search.rs`
  * abort 전파, root/recursive search 분리, trusted apply 사용, hard/soft limit 분리
  * node budget abort 회귀 테스트 추가
* `engine/src/ai/types.rs`
  * completed depth와 확장 가능한 `SearchStats` 추가
* `engine/src/legal_moves.rs`
  * Airborne canonical multi-deployment 생성
* `engine/tests/ai.rs`
  * multi-deployment validity, uniqueness 및 permutation 중복 회귀 테스트 추가
* `engine/tests/ai_benchmark.rs`
  * reached/completed depth와 beta cutoff 출력 분리
  * Airborne benchmark가 multi-deployment 후보를 요구하도록 강화
* `server/src/main.rs`
  * bot turn 통계에 `completed_depth`, `beta_cutoffs` 노출

작업 전에 존재하던 `docs/ai-upgrade/goal-1-search-foundation.md`의 사용자 변경은 보존했다.

## 추가 테스트

다음 회귀 테스트를 추가했다.

* node budget abort가 completed result로 기록되지 않는지 검증
* generated Move의 public/internal apply 최종 `GameState` parity
* generated Drop의 public/internal apply 최종 `GameState` parity
* generated Ability의 public/internal apply 최종 `GameState` parity
* Airborne action에 `deployments.len() >= 2`인 후보가 존재하는지 검증
* 생성된 multi-deployment를 public `submit_action`이 승인하는지 검증
* 한 action 안의 pocket piece 및 square uniqueness 검증
* deployment 집합의 순열 중복이 없는지 검증
* Goal 0 Airborne benchmark correctness 조건 강화

## 전체 테스트 결과

실행 명령:

```sh
cargo test --workspace --all-features
```

결과:

* 실행된 테스트 127개 통과
* 실패 0개
* Goal 0 profiling benchmark 1개는 의도대로 ignored
* engine, server, Chessembly compatibility, rule engine 및 AI 테스트 모두 통과

benchmark 실행 명령:

```sh
cargo test -p brainfuck-chess-engine --features profiling --test ai_benchmark ai_search_baseline -- --ignored --nocapture --test-threads=1
```

9개 position 모두 검색을 완료했고 선택 action의 canonical validity assertion을 통과했다.

## Goal 0 대비 benchmark

아래 비교의 Goal 0 값은 코드 변경 직전 동일 머신, 동일 debug test profile 및 동일 명령으로 다시 측정한 값이다. elapsed는 환경 노이즈가 있으므로 correctness assertion이나 절대 성능 보장으로 사용하지 않는다.

| position | legal generation | searched nodes | elapsed (ms) |
| --- | ---: | ---: | ---: |
| `middlegame` | 373 → 212 | 125 → 128 | 169.074 → 92.406 |
| `tactical-captures` | 938 → 714 | 315 → 373 | 159.737 → 123.707 |
| `drop-branching` | 951 → 254 | 350 → 159 | 152.348 → 44.634 |
| `standalone-ability` | 534 → 41 | 187 → 26 | 51.955 → 4.023 |
| `piece-state-cooldown` | 494 → 182 | 174 → 105 | 46.174 → 20.096 |
| `immediate-king-capture` | 257 → 46 | 90 → 30 | 19.746 → 3.998 |
| `drop-capture` | 386 → 113 | 140 → 66 | 32.623 → 10.446 |
| `airborne-deployment` | 992 → 1,051 | 392 → 679 | 133.838 → 129.527 |
| `alternating-soldier-pocket-swap` | 1,298 → 66 | 478 → 102 | 150.089 → 9.086 |
| **합계** | **6,223 → 2,679 (-57.0%)** | **2,251 → 1,668 (-25.9%)** | **915.584 → 437.923 (-52.2%)** |

대부분의 position에서 검색 내부 재검증 제거와 root alpha-beta 경계 공유로 legal generation 및 node 수가 감소했다. `airborne-deployment`는 기존에 없던 multi-deployment action space가 추가되어 nodes가 392에서 679로, legal generation이 992에서 1,051로 증가했다.

완료된 benchmark에서는 모든 position이 `reached_depth=2`, `completed_depth=2`였다. 각 position의 beta cutoff는 다음과 같다.

| position | beta cutoffs |
| --- | ---: |
| `middlegame` | 44 |
| `tactical-captures` | 29 |
| `drop-branching` | 62 |
| `standalone-ability` | 11 |
| `piece-state-cooldown` | 27 |
| `immediate-king-capture` | 15 |
| `drop-capture` | 16 |
| `airborne-deployment` | 304 |
| `alternating-soldier-pocket-swap` | 48 |

## 하위호환성과 남은 위험

* public `submit_action`의 validation은 유지되며 malformed 또는 non-canonical action을 trusted 경로로 전달할 공개 API는 추가하지 않았다.
* 기존 `searched_nodes`, `depth_reached` 필드는 유지했다. 추가 통계 필드는 serde default를 사용하여 이전 serialized input과의 호환성을 보존한다.
* 기존 Chessembly 문법과 action mutation logic은 변경하지 않았다.
* Airborne은 합법 multi-deployment 조합을 순열 중복 없이 열거한다. 합법 pocket piece가 매우 많은 비정상적으로 큰 position에서는 canonical 조합 수 자체가 커질 수 있으며, 이는 이후 move generation/search 최적화에서 관찰해야 할 성능 위험이다.
* elapsed 개선 폭은 단일 debug 실행 결과이므로 반복 측정 전에는 확정적인 성능 보장으로 해석하지 않는다.
