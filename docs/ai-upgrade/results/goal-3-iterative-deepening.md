# Goal 3 — Iterative Deepening 결과

## 완료 범위와 변경 파일

Goal 2 보완의 variant-safe TT, bound semantics, abort 비저장, deterministic ordering 및 TT OFF key-generation 차단을 유지하면서 fixed-depth root search를 Iterative Deepening controller로 변경했다.

* `engine/src/ai/search.rs`
  * depth 1부터 `max_depth_actions`까지 순차 실행하는 controller
  * 마지막 completed `RootSearchResult` 보존과 partial iteration 폐기
  * previous completed root best-action ordering
  * full completion, node abort fallback, soft limit, TT reuse 및 ordering regression
* `engine/src/ai/types.rs`
  * `SearchStats.iterations_started`, `iterations_completed` 추가
* `engine/tests/ai_benchmark.rs`
  * iteration stats 출력과 실제 Easy/Normal/Hard budget benchmark 추가
* `server/src/main.rs`
  * bot turn 통계 응답에 iteration started/completed 노출

Quiescence Search, Aspiration Window, Zobrist/incremental hashing, PVS, LMR, Killer/History heuristic, evaluation tuning과 action-space pruning은 추가하지 않았다.

## Architecture와 lifetime

```text
choose_bot_action_with_config
  -> deterministic legal root actions 생성
  -> search-local SearchContext + optional TT 한 번 생성
  -> for depth in 1..=max_depth_actions
       -> 이전 completed best를 canonical root list의 앞으로 이동
       -> search_root(depth)
            -> alpha_beta
       -> completed일 때만 last_completed 교체
       -> completed iteration 뒤 soft limit 확인
  -> last_completed 반환
     또는 completed iteration이 없으면 legal fallback + static evaluation
```

`started`, 누적 node counter, `SearchStats`, soft/hard budget과 TT는 bot decision 전체에서 하나의 `SearchContext`가 공유한다. iteration마다 timer, node count 또는 TT를 reset하지 않는다. TT는 search가 끝난 뒤에만 폐기된다.

## TT cross-iteration reuse

depth 1에서 저장한 entry는 같은 table에 남아 depth 2 이상에서 probe된다. 기존 `apply_table_bound`는 `entry.depth < requested depth`이면 score/bound를 사용하지 않으므로 shallow Exact를 deeper cutoff로 오용하지 않는다. 다만 entry의 `best_action`은 현재 node의 canonical legal action 목록에서 동일 action을 찾았을 때 ordering에 사용한다.

regression은 같은 state의 depth-2 ID search가 depth-1 단독 search보다 더 많은 TT hit를 만들며, depth-2의 hits가 cutoffs보다 많음을 확인한다. 이는 shallow ordering hit와 score cutoff가 구분된다는 증거다. 기존 Exact/LowerBound/UpperBound depth regression도 그대로 유지된다.

## Previous iteration root ordering

각 iteration은 원래 deterministic root action 목록을 clone한다. 직전 completed iteration의 best action이 목록에 있으면 `swap(0, index)`로 앞으로 옮긴다. action을 새로 삽입하지 않으므로 중복이 없고 stale/임의 action을 trusted apply로 보내지 않는다. aborted iteration의 partial best는 `previous_iteration_best`나 `last_completed`를 갱신하지 않는다.

## Budget와 partial-result semantics

* hard time 또는 누적 `max_nodes`: 현재 iteration 중에도 `SearchOutcome::Aborted`를 root까지 전파한다. 해당 iteration의 best와 scores를 전부 버리고 마지막 completed iteration을 반환한다.
* soft time: 현재 iteration을 abort하지 않는다. completed iteration을 승격한 다음, 다음 depth를 시작할지 결정할 때만 확인한다.
* 첫 iteration: 사전 legal generation 뒤 soft limit이 지났더라도 hard/node limit이 남아 있으면 depth 1을 시도한다. `soft_time_ms = 0` regression에서 depth 1 완료 후 정확히 멈춤을 확인했다.
* zero completed: hard/node limit으로 depth 1도 완료하지 못하면 deterministic ordered legal action과 current-state static evaluation을 반환하며 `completed_depth = 0`이다.
* `depth_reached`: 실제 alpha-beta가 방문한 최대 ply다. `completed_depth`: 완전히 끝난 root iteration의 최대 depth다. depth 2 중 abort regression에서 `depth_reached = 2`, `completed_depth = 1`을 확인했다.

Easy의 상위 completed root scores 대상 기존 randomness는 유지한다. aborted iteration의 partial scores는 `last_completed`에 들어가지 않으므로 Easy candidate pool에도 사용되지 않는다. `WIN_SCORE`이면 기존대로 random weakening을 건너뛰며 모든 difficulty의 immediate King capture regression이 통과한다.

## 추가 regression tests

* 충분한 budget에서 depth 1, 2, 3을 모두 실행하고 `completed_depth == 3`
* depth 1 완료 node 수를 기준으로 depth 2 중 deterministic node abort를 만들고 depth-1 action/score 반환
* depth 1 미완료 시 `completed_depth == 0` 및 legal fallback
* interrupted deeper iteration에서 `depth_reached > completed_depth`
* `soft_time_ms = 0`이어도 depth 1은 완료하고 다음 iteration만 차단
* depth-1 대비 depth-2 TT hit 증가와 shallow hit/cutoff 구분
* previous root best가 중복 없이 첫 action으로 이동
* 동일 limits의 Normal/Hard 반복 검색이 action/score/completed-depth에서 deterministic
* variant positions `piece-state-cooldown`, `drop-capture`, `airborne-deployment`, `alternating-soldier-pocket-swap`, `immediate-king-capture`의 TT ON/OFF action/score 및 depth 일치
* 기존 9개 benchmark의 TT ON/OFF action/score/completed-depth 일치
* TT OFF key generation과 probes/hits/cutoffs/stores 모두 0
* 모든 difficulty의 immediate King capture 유지

## 전체 검증

```sh
cargo test --workspace --all-features
```

* 실행 142개 통과
* 실패 0개
* 명시 실행용 profiling benchmark 3개 ignored

```sh
cargo clippy --workspace --all-features -- -D warnings
```

* warning/error 없이 통과

debug와 release에서 TT ON/OFF benchmark를 모두 실행했다. elapsed는 strict assertion이 아닌 단일 실행 관측값이다.

```sh
cargo test -p brainfuck-chess-engine --features profiling --test ai_benchmark ai_search_baseline -- --ignored --nocapture --test-threads=1
cargo test -p brainfuck-chess-engine --features profiling --test ai_benchmark ai_search_tt_off_comparison -- --ignored --nocapture --test-threads=1
cargo test -p brainfuck-chess-engine --release --features profiling --test ai_benchmark ai_search_baseline -- --ignored --nocapture --test-threads=1
cargo test -p brainfuck-chess-engine --release --features profiling --test ai_benchmark ai_search_tt_off_comparison -- --ignored --nocapture --test-threads=1
```

## Goal 3 release benchmark

Normal의 실제 budget(depth 2, nodes 3,000, soft 150 ms, hard 300 ms)과 TT ON 결과다. 9개 모두 두 iteration을 완료했다.

| position | selected action | score | nodes | reached/completed | elapsed ms | legal gen | applications | beta cutoffs | keys/probes | hits/cutoffs/stores |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | `wq d3-f5 x bb` | 228 | 173 | 2/2 | 58.243 | 302 | 173 | 44 | 173/173 | 45/0/173 |
| `tactical-captures` | `wr e3-h3` | -318 | 147 | 2/2 | 36.434 | 262 | 147 | 32 | 147/147 | 33/0/147 |
| `drop-branching` | `drop wq b2` | 1,903 | 211 | 2/2 | 24.078 | 358 | 211 | 63 | 211/211 | 65/0/211 |
| `standalone-ability` | `camp d4-e4 x enemy` | 506 | 38 | 2/2 | 2.861 | 65 | 38 | 11 | 38/38 | 12/0/38 |
| `piece-state-cooldown` | `windmill d4-g1` | 1,150 | 106 | 2/2 | 8.704 | 184 | 106 | 25 | 106/106 | 29/0/106 |
| `immediate-king-capture` | `wr e1-e8 x bk` | 1,000,000 | 45 | 2/2 | 3.111 | 76 | 47 | 15 | 45/45 | 15/0/45 |
| `drop-capture` | `drop para d1 x enemy` | 290 | 56 | 2/2 | 3.825 | 93 | 56 | 17 | 56/56 | 18/0/56 |
| `airborne-deployment` | `airdrop bishop/knight` | 1,488 | 974 | 2/2 | 79.631 | 1,614 | 974 | 297 | 974/974 | 317/18/956 |
| `alternating-soldier-pocket-swap` | `soldier d4-c3 x enemy` | 1,046 | 151 | 2/2 | 9.191 | 164 | 151 | 48 | 151/151 | 49/0/151 |

TT OFF에서도 선택 action, score와 completed depth는 모두 같았다. 모든 position에서 key generation과 TT 통계는 0이었다. Airborne만 TT cutoff가 발생해 OFF/ON에서 nodes 984/974, legal generation 1,661/1,614, applications 984/974였다. 나머지 position의 ON hits는 주로 이전 shallow iteration entry이며 score cutoff가 아니다.

## Goal 2 보완 fixed-depth 대비 Goal 3 ID

두 표 모두 release TT ON 단일 실행이다. Goal 2는 depth 2 한 번, Goal 3은 depth 1+2 누적이다.

| position | nodes G2 -> G3 | elapsed ms G2 -> G3 | completed depth | probes G2 -> G3 | hits G2 -> G3 | cutoffs G2 -> G3 | keys G2 -> G3 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | 128 -> 173 | 47.840 -> 58.243 | 2 -> 2 | 128 -> 173 | 0 -> 45 | 0 -> 0 | 128 -> 173 |
| `tactical-captures` | 299 -> 147 | 58.351 -> 36.434 | 2 -> 2 | 299 -> 147 | 0 -> 33 | 0 -> 0 | 299 -> 147 |
| `drop-branching` | 172 -> 211 | 16.326 -> 24.078 | 2 -> 2 | 172 -> 211 | 0 -> 65 | 0 -> 0 | 172 -> 211 |
| `standalone-ability` | 26 -> 38 | 1.788 -> 2.861 | 2 -> 2 | 26 -> 38 | 0 -> 12 | 0 -> 0 | 26 -> 38 |
| `piece-state-cooldown` | 77 -> 106 | 5.768 -> 8.704 | 2 -> 2 | 77 -> 106 | 0 -> 29 | 0 -> 0 | 77 -> 106 |
| `immediate-king-capture` | 30 -> 45 | 1.801 -> 3.111 | 2 -> 2 | 30 -> 45 | 0 -> 15 | 0 -> 0 | 30 -> 45 |
| `drop-capture` | 50 -> 56 | 3.296 -> 3.825 | 2 -> 2 | 50 -> 56 | 0 -> 18 | 0 -> 0 | 50 -> 56 |
| `airborne-deployment` | 677 -> 974 | 49.295 -> 79.631 | 2 -> 2 | 677 -> 974 | 9 -> 317 | 9 -> 18 | 677 -> 974 |
| `alternating-soldier-pocket-swap` | 102 -> 151 | 3.973 -> 9.191 | 2 -> 2 | 102 -> 151 | 0 -> 49 | 0 -> 0 | 102 -> 151 |

대부분은 depth-1 비용만큼 nodes가 증가하는 정상적인 ID overhead다. `tactical-captures`는 previous iteration root best ordering이 depth-2 alpha-beta 순서를 개선해 depth-1을 포함하고도 fixed-depth보다 nodes가 299에서 147로 감소했다. Airborne의 hits 317 중 cutoff는 18뿐이므로 대부분은 shallow ordering hit다. cross-iteration reuse와 기존 transposition cutoff가 OFF 대비 nodes 10개와 legal generation 47회를 절약했다.

## 실제 difficulty budget

기존 limits는 변경하지 않았다. release TT ON 측정의 `nodes, reached/completed, elapsed ms`다.

| position | Easy | Normal | Hard |
| --- | --- | --- | --- |
| `middlegame` | 45, 1/1, 19.158 | 173, 2/2, 62.655 | 2,049, 3/3, 236.366 |
| `tactical-captures` | 33, 1/1, 4.324 | 147, 2/2, 16.740 | 1,021, 3/3, 101.509 |
| `drop-branching` | 65, 1/1, 8.500 | 211, 2/2, 21.438 | 7,344, 3/3, 729.990 |
| `standalone-ability` | 12, 1/1, 1.257 | 38, 2/2, 2.869 | 177, 3/3, 12.712 |
| `piece-state-cooldown` | 29, 1/1, 3.012 | 106, 2/2, 8.722 | 986, 3/3, 72.041 |
| `immediate-king-capture` | 15, 1/1, 1.414 | 45, 2/2, 3.091 | 315, 3/3, 16.920 |
| `drop-capture` | 18, 1/1, 1.641 | 56, 2/2, 3.876 | 181, 3/3, 11.109 |
| `airborne-deployment` | 308, 1/1, 31.275 | 974, 2/2, 80.805 | 9,456, 3/2, 809.430 |
| `alternating-soldier-pocket-swap` | 49, 1/1, 5.377 | 151, 2/2, 9.212 | 460, 3/3, 33.874 |

Easy는 모두 depth 1, Normal은 모두 depth 2를 완료했다. Hard는 Airborne을 제외하고 depth 3을 완료했다. Airborne Hard는 세 번째 iteration에 진입한 뒤 hard time에 abort되어 depth 2 결과를 반환했다. 809.430 ms가 800 ms hard limit을 조금 넘는 것은 Goal 1부터 알려진 expensive legal/action generation 내부의 hard-limit blind spot이며 이번 Goal에서 action을 cap하거나 삭제하지 않았다. `drop-branching` Hard는 depth 2 완료 시 soft 400 ms 전이어서 depth 3을 시작했고, 진행 중 soft를 넘었더라도 abort하지 않고 729.990 ms에 정상 완료했다.

## 남은 위험과 다음 단계

* owned canonical PositionKey 비용은 iteration마다 누적된다. cross-iteration shallow hits가 많아졌지만 cutoff가 아닌 hit는 직접적인 node 절약을 보장하지 않는다.
* root PV ordering은 tactical position에서 큰 효과를 보였지만 position별로 depth-1 overhead가 더 클 수 있다.
* soft limit은 iteration 경계에서만 작동하므로 soft 직전에 시작한 expensive iteration은 hard limit까지 실행될 수 있다. 이는 의도한 semantics다.
* legal/action generation 내부 hard-limit blind spot 때문에 Airborne은 hard deadline을 소폭 초과할 수 있다. 임의 action truncation 없이 별도 후속 최적화로 다뤄야 한다.
* TT capacity는 전체 node budget 기준으로 bounded되며 shallow entries가 먼저 공간을 차지할 수 있다. 현재 depth-preferred same-key replacement는 유지하지만 full-table replacement 정책은 향후 깊은 ID에서 다시 측정할 필요가 있다.

마지막 completed iteration 보존, shared budget/TT, root PV ordering, ON/OFF 및 variant correctness가 검증됐으므로 Goal 4 — Deck-Chess-aware Quiescence Search로 넘어갈 수 있는 상태다.
