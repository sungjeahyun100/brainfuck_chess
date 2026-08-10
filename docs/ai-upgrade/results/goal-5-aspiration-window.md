# Goal 5 — Aspiration Window 결과

## 완료 범위와 변경 파일

Goal 3의 Iterative Deepening controller에 previous completed score 기반 Aspiration Window를 추가했다. Goal 4의 QSearch, shared SearchContext/TT, abort propagation, iteration-boundary soft limit과 last-completed fallback은 그대로 유지했다.

* `engine/src/ai/search.rs`
  * root alpha/beta window 인자화
  * `Exact`, `FailLow`, `FailHigh`, `Aborted` root outcome
  * geometric widening, full-window fallback, failed-pass ordering과 abort-safe ID 승격
  * stable/fail-low/fail-high/multiple widening/re-search abort/overflow regression
* `engine/src/ai/types.rs`
  * `SearchOptions.use_aspiration_window`
  * aspiration search/re-search/fail-low/fail-high 통계
  * 이전 JSON에 새 option이 없을 때 enabled로 복원하는 serde default regression
* `engine/tests/ai_benchmark.rs`
  * Aspiration OFF release comparison
  * TT/Aspiration 4-way correctness matrix
  * 기존 9개/difficulty 출력에 aspiration stats 추가
* `server/src/main.rs`
  * bot turn response에 새 aspiration stats 노출
* `docs/ai-upgrade/results/goal-5-aspiration-window.md`
  * 설계, regression, release 재측정과 위험 기록

PVS, LMR, Null Move, SEE, futility/delta pruning, QSearch TT, evaluation/difficulty tuning과 action truncation은 추가하지 않았다.

## Controller architecture

```text
depth 1
  -> full root window
  -> Exact일 때만 last_completed 승격

depth 2+
  -> center = previous completed root score
  -> [center - delta, center + delta]
  -> Exact: current depth를 last_completed로 승격
  -> FailLow/FailHigh: window 확장 후 같은 depth 재검색
  -> Aborted: current depth를 폐기하고 직전 last_completed 반환
```

`search_root` status는 다음을 구분한다.

* `Exact(RootSearchResult)`: score가 current window 안에 있거나 full-window pass가 완료됨
* `FailLow(RootSearchResult)`: `score <= original alpha`
* `FailHigh(RootSearchResult)`: `score >= original beta`
* `Aborted`: hard time/node budget이 alpha-beta/QSearch/root 중에 소진됨

fail result의 score, best action과 candidate scores는 `last_completed`나 다음 ID depth의 aspiration center로 승격하지 않는다. 다만 같은 depth의 다음 pass에서 failed-pass best action을 canonical action list 맨 앞으로 옮기는 ordering hint로만 사용한다. 그 뒤에 depth N-1 completed best와 기존 deterministic ordering이 남는다. action을 새로 삽입하지 않는다.

## Initial delta, widening과 overflow safety

최종 `ASPIRATION_INITIAL_DELTA` 값은 250이다. evaluator의 board material 기본 단위가 definition score의 100배이고 mobility/QSearch 변동도 있으므로, 작은 mobility noise는 포함하되 실질적 material swing은 fail로 감지할 수 있는 폭으로 선택했다.

첫 release 측정은 delta 500으로 실행했다. 9개 Normal/Hard에서 모두 첫 pass에 성공했지만 Normal node 절감이 작아, position별 하드코딩 없이 250으로 한 번 조정했다. 이후 `tactical-captures`의 절감은 3 nodes에서 13 nodes로 커졌고 실제 difficulty benchmark에서는 여전히 불필요한 re-search가 없었다.

fail이 발생하면 delta를 `250, 500, 1,000, 2,000, 4,000`처럼 2배씩 늘린다. 초기 pass 후 최대 4회의 narrow widening에서도 exact가 아니면 다음 pass는 바로 `[SEARCH_NEG_INF, SEARCH_POS_INF]` full window를 사용한다. 따라서 loop는 유한하고 1씩 늘리는 경로가 없다.

window 산술은 `i64` subtraction/addition 후 search infinity로 clamp하고 delta 배증은 `saturating_mul` 을 사용한다. `i32::MIN`, `i32::MAX`, `WIN_SCORE`를 center로 사용하는 regression이 overflow 없이 통과했다. full-window pass는 boundary score를 다시 fail로 분류하지 않고 exact로 종료한다.

## Budget, TT와 QSearch semantics

모든 pass는 같은 `SearchContext`를 사용한다. `started`, `searched_nodes`, `qnodes`, hard/soft budget, SearchStats와 TT를 reset하지 않으므로 re-search 비용은 모두 실제 bot decision budget에 포함된다. failed narrow pass의 TT entry도 유지되어 re-search ordering/cutoff에 사용될 수 있다.

normal alpha-beta는 계속 호출 시점의 `original_alpha`, `original_beta`로 TT bound를 분류한다. 따라서 fail-low 경로는 UpperBound, fail-high 경로는 LowerBound 의미를 유지하고 Exact로 오염되지 않는다. tiny-window fail fixture에서 TT ON/OFF 최종 exact action/score가 모두 full-window result와 일치했다.

hard/node abort가 initial pass 또는 re-search에서 발생하면 해당 depth를 폐기하고 직전 completed iteration을 반환한다. soft limit은 pass 사이에서 확인하지 않고 exact ID iteration이 승격된 뒤에만 다음 depth 시작 여부를 결정한다. depth 1이 abort되면 기존 legal fallback + static evaluation + completed depth 0이다.

QSearch는 변경하지 않았다. aspiration alpha/beta가 normal alpha-beta를 통해 QSearch에 전달되지만, King-threat를 뺀 stand-pat, noisy allow-list, 8-ply bound, node/hard abort와 normal TT 비저장 policy는 그대로다. root score가 window boundary 밖이면 내부 QSearch의 fail-soft score이라도 exact로 승격하지 않는다.

## SearchStats와 option 의미

* `aspiration_searches`: depth 2+ initial aspiration pass를 시작한 횟수
* `aspiration_researches`: fail-low/high 후 추가 pass를 시작한 횟수
* `aspiration_fail_lows`: `score <= alpha`가 발생한 pass 수
* `aspiration_fail_highs`: `score >= beta`가 발생한 pass 수

`iterations_started/completed`는 여전히 ID depth 수만 세며 re-search pass로 증가하지 않는다. `SearchOptions.use_aspiration_window` 기본값은 true다. false일 때는 Goal 4와 같은 depth별 full-window ID를 수행하고 네 aspiration counter가 모두 0이다. serde에서 새 field가 누락된 이전 payload도 true로 복원된다. 새 stats는 SearchStats 직렬화와 server bot-turn stats 응답에 노출된다.

## Regression

* stable `WIN_SCORE` fixture: depth 2 aspiration 1회, fail/re-search 0, full-window와 exact result 일치
* tiny fail-low fixture: depth score `530 -> 524`, delta 1에서 3회 geometric re-search 후 exact 일치
* tiny fail-high fixture: depth score `2 -> 4`, delta 1에서 2회 geometric re-search 후 exact 일치
* fail-low/high fixture의 TT ON/OFF 모두 Aspiration OFF full-window action/score/completed depth와 일치
* 인위적으로 center 0, exact `WIN_SCORE`, delta 1을 준 pass가 fail-high 5회 후 full-window fallback으로 유한 종료
* deterministic node budget으로 fail-low 첫 pass 후 re-search를 abort하면 depth 1 action/score, started 2, completed 1을 반환
* 기존 TT depth/bound, ID abort, QSearch horizon/King threat/King capture, immediate King capture와 variant regression 유지

기존 9개에서 다음 matrix를 depth 2, 충분한 deterministic budget으로 검증했다.

```text
TT ON  + Aspiration OFF
TT ON  + Aspiration ON
TT OFF + Aspiration OFF
TT OFF + Aspiration ON
```

모든 조합의 action, score, completed depth가 같았다. TT OFF에서 position key generations와 probes/hits/cutoffs/stores는 모두 0이었다.

## 전체 검증

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
```

* workspace test: 154개 통과, 실패 0, 명시 benchmark 5개 ignored
* clippy: warning/error 없이 통과
* 변경 Rust 파일 `rustfmt --check`, `git diff --check` 통과
* release Aspiration ON, OFF, TT OFF, actual difficulty, QSearch horizon benchmark 실행

elapsed는 단일 실행 관측값이며 strict assertion이 아니다.

## 9개 release benchmark: Aspiration ON

Normal limits, TT ON, delta 250이다. 모두 depth 2를 exact하게 완료했다.

| position | selected action | score | nodes/qnodes | elapsed ms | depth r/c; iter s/c | asp search/re/low/high | TT p/h/c/s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | `wq d3-f5 x bb` | 414 | 419/374 | 139.508 | 2/2; 2/2 | 1/0/0/0 | 45/0/0/45 |
| `tactical-captures` | `wq d4-g4 x br2` | 198 | 258/225 | 30.207 | 2/2; 2/2 | 1/0/0/0 | 33/0/0/33 |
| `drop-branching` | `drop wq b2` | 1,903 | 215/150 | 21.923 | 2/2; 2/2 | 1/0/0/0 | 65/0/0/65 |
| `standalone-ability` | `camp d4-e4 x enemy` | 506 | 42/30 | 3.275 | 2/2; 2/2 | 1/0/0/0 | 12/0/0/12 |
| `piece-state-cooldown` | `wk a1-b2` | 1,150 | 110/81 | 9.037 | 2/2; 2/2 | 1/0/0/0 | 29/0/0/29 |
| `immediate-king-capture` | `wr e1-e8 x bk` | 1,000,000 | 46/31 | 3.188 | 2/2; 2/2 | 1/0/0/0 | 15/0/0/15 |
| `drop-capture` | `drop para d1 x enemy` | 290 | 75/57 | 5.797 | 2/2; 2/2 | 1/0/0/0 | 18/0/0/18 |
| `airborne-deployment` | `airdrop bishop/knight` | 1,488 | 1,032/724 | 88.464 | 2/2; 2/2 | 1/0/0/0 | 308/9/9/299 |
| `alternating-soldier-pocket-swap` | `soldier d4-c3 x enemy` | 1,046 | 155/106 | 9.316 | 2/2; 2/2 | 1/0/0/0 | 49/0/0/49 |

Normal의 production delta에서 fail-low/high와 re-search는 없었다. TT probes/stores는 full-window와 같았고 Airborne에서만 기존 transposition hit/cutoff 9회가 남았다. aspiration은 주로 QSearch에 전달된 window를 통해 qnode를 줄였다.

## Goal 4 보완 및 Aspiration OFF 대비

Goal 4 보완은 현재 Aspiration OFF와 같은 algorithm이다. 표의 elapsed는 `Goal 4 -> OFF -> ON`이며 nodes/qnodes는 `OFF -> ON`이다.

| position | score G4/ON | nodes OFF -> ON | qnodes OFF -> ON | elapsed ms G4 -> OFF -> ON | re-search | completed |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | 414/414 | 426 -> 419 | 381 -> 374 | 126.506 -> 144.513 -> 139.508 | 0 | 2 |
| `tactical-captures` | 198/198 | 271 -> 258 | 238 -> 225 | 29.654 -> 31.588 -> 30.207 | 0 | 2 |
| `drop-branching` | 1,903/1,903 | 215 -> 215 | 150 -> 150 | 21.942 -> 22.310 -> 21.923 | 0 | 2 |
| `standalone-ability` | 506/506 | 42 -> 42 | 30 -> 30 | 3.233 -> 3.341 -> 3.275 | 0 | 2 |
| `piece-state-cooldown` | 1,150/1,150 | 110 -> 110 | 81 -> 81 | 9.030 -> 8.993 -> 9.037 | 0 | 2 |
| `immediate-king-capture` | 1,000,000/1,000,000 | 46 -> 46 | 31 -> 31 | 3.209 -> 3.229 -> 3.188 | 0 | 2 |
| `drop-capture` | 290/290 | 75 -> 75 | 57 -> 57 | 5.621 -> 5.718 -> 5.797 | 0 | 2 |
| `airborne-deployment` | 1,488/1,488 | 1,037 -> 1,032 | 729 -> 724 | 87.473 -> 88.041 -> 88.464 | 0 | 2 |
| `alternating-soldier-pocket-swap` | 1,046/1,046 | 155 -> 155 | 106 -> 106 | 9.161 -> 9.190 -> 9.316 | 0 | 2 |

node 기준 실질적 이득은 `middlegame` 7, `tactical-captures` 13, Airborne 5 nodes/qnodes였다. 나머지 6개는 중립이었고 node가 늘어난 position은 없었다. elapsed는 대체로 변동 범위였으며 `piece-state-cooldown`, `drop-capture`, Airborne, alternating-soldier는 ON 단일 실행이 OFF보다 0.04~0.42 ms 느렸다. 재검색이 없고 node는 같거나 줄었으므로 고정 overhead와 측정 noise가 섞인 결과로 해석한다. Aspiration이 항상 wall-clock을 줄인다고 주장하지 않는다.

TT OFF + Aspiration ON도 동일 exact action/score/depth를 반환했다. Airborne의 TT OFF/ON nodes는 1,043/1,032였고, TT OFF의 position keys와 TT 통계는 모두 0이었다.

## 실제 difficulty budget

limits는 변경하지 않았다. `nodes, reached/completed, aspiration searches/re-searches, elapsed ms`다.

| position | Easy | Normal | Hard |
| --- | --- | --- | --- |
| `middlegame` | 51, 1/1, 0/0, 21.184 | 419, 2/2, 1/0, 136.933 | 2,221, 3/3, 2/0, 293.796 |
| `tactical-captures` | 33, 1/1, 0/0, 4.199 | 258, 2/2, 1/0, 34.432 | 1,061, 3/3, 2/0, 134.889 |
| `drop-branching` | 65, 1/1, 0/0, 8.003 | 215, 2/2, 1/0, 22.463 | 5,796, 3/3, 2/0, 700.601 |
| `standalone-ability` | 12, 1/1, 0/0, 1.159 | 42, 2/2, 1/0, 3.227 | 181, 3/3, 2/0, 13.959 |
| `piece-state-cooldown` | 29, 1/1, 0/0, 2.941 | 110, 2/2, 1/0, 9.009 | 1,018, 3/3, 2/0, 93.593 |
| `immediate-king-capture` | 15, 1/1, 0/0, 1.375 | 46, 2/2, 1/0, 3.288 | 316, 3/3, 2/0, 23.969 |
| `drop-capture` | 18, 1/1, 0/0, 1.641 | 75, 2/2, 1/0, 5.736 | 200, 3/3, 2/0, 14.252 |
| `airborne-deployment` | 308, 1/1, 0/0, 31.245 | 1,032, 2/2, 1/0, 87.828 | 8,134, 3/2, 2/0, 800.688 |
| `alternating-soldier-pocket-swap` | 49, 1/1, 0/0, 5.128 | 155, 2/2, 1/0, 9.240 | 464, 3/3, 2/0, 32.897 |

Easy는 max depth 1이므로 aspiration search가 0이고 기존 random weakening policy가 변하지 않았다. Normal은 9개 모두 depth 2를 완료했다. Hard는 Airborne을 제외한 8개가 depth 3을 완료했고, production delta에서 fail/re-search는 없었다.

Airborne Hard는 depth 3 aspiration initial pass 중 800 ms hard limit에서 abort되어 depth 2를 반환했다. Goal 4 보완의 7,988 nodes, 800.652 ms에서 8,134 nodes, 800.688 ms로 관측됐다. time abort 시점과 expensive generation 단위의 실행 변동이므로 aspiration regression으로 단정하지 않는다. 기존 legal/action generation hard-limit blind spot은 그대로며 Airborne action을 cap하지 않았다.

QSearch horizon release도 재실행했다: recapture 13/13 nodes/qnodes, capture-on-drop 25/25, enemy recall 4/4였고 선택 action과 score는 Goal 4 regression과 일치했다.

## 남은 위험과 다음 단계

* 현재 9개 depth 2/3에서 production delta re-search가 없었다. 이는 안정적이지만 더 낮은 delta가 다른 position에서 더 유리한지는 더 넓은 corpus로 측정해야 한다.
* narrow pass가 실패하는 tactical swing position은 re-search 비용을 지불한다. TT를 재사용하지만 항상 full-window보다 싸다고 보장할 수는 없다.
* candidate scores는 exact pass의 것만 승격하므로 Easy에 partial bound가 섞이지 않는다. 현재 Easy는 depth 1이라 aspiration을 사용하지 않는다.
* QSearch evaluator/action generation 비용, canonical PositionKey 비용과 Airborne hard-limit blind spot은 남아 있다.
* PVS나 다른 pruning을 추가할 때는 aspiration fail-soft/TT bound와의 상호작용을 다시 검증해야 한다.

Exact-only ID 승격, finite widening/full fallback, shared budget/TT, re-search abort fallback, QSearch/TT correctness와 ON/OFF 재현성이 검증됐다. **Goal 5는 완료 상태이며, 후속 search heuristic 단계 또는 다음 AI 개선 계획으로 넘어갈 수 있다.**
