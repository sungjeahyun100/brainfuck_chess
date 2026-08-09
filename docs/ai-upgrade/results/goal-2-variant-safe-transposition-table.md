# Goal 2 Variant-safe Transposition Table 결과

## 완료 범위

Goal 1의 `choose_bot_action -> search_root -> alpha_beta` 구조에 search-local bounded Transposition Table을 추가했다. TT는 board-only fingerprint가 아니라 Deck Chess와 Chessembly의 현재 rule-relevant state를 canonical representation으로 보관한다.

이번 Goal에서는 Iterative Deepening, Quiescence Search, Aspiration Window, Null Move, PVS, LMR, Killer/History heuristic, 평가 함수 튜닝, drop dedup 및 Airborne pruning을 구현하지 않았다.

## 변경 파일

* `engine/src/ai/transposition_table.rs`
  * canonical `PositionKey`, `TranspositionEntry`, bound type 및 bounded table 구현
  * key canonicalization과 replacement policy 단위 테스트
* `engine/src/ai/search.rs`
  * alpha-beta TT probe/store/cutoff와 TT best-action ordering 통합
  * TT ON/OFF, bound semantics 및 abort 안전성 테스트
* `engine/src/ai/types.rs`
  * `SearchOptions`, `tt_probes`, `tt_stores` 추가
* `engine/src/ai/mod.rs`
  * TT 모듈 및 검색 옵션 API 연결
* `engine/src/ai/move_ordering.rs`
  * 동일 tactical priority action에 deterministic canonical tie-break 추가
* `engine/tests/ai_benchmark.rs`
  * 실제 9개 benchmark position의 TT ON/OFF correctness 비교
  * TT probe/hit/cutoff/store 출력 및 TT-off 비교 benchmark 추가
* `server/src/main.rs`
  * bot turn 통계에 TT probe/hit/cutoff/store 노출

## TT architecture

```text
choose_bot_action
  -> search-local SearchContext
       -> optional TranspositionTable
       -> search_root
            -> alpha_beta
                 -> PositionKey 생성
                 -> TT probe / bound 적용
                 -> canonical action generation
                 -> TT best action ordering
                 -> completed result만 TT store
```

`PositionKey` 구조체 자체가 `HashMap` key다. 축약된 64-bit fingerprint만 저장하지 않으므로 Rust `HashMap` hash가 충돌하더라도 전체 canonical key의 `Eq` 비교로 서로 다른 position을 구분한다.

board의 명시적인 empty entry와 누락 entry는 모두 empty square라는 동일 의미이므로 occupied square만 key에 기록한다. 모든 map/set 성격의 값과 player membership vector는 stable ordering으로 정렬한다.

## Position key에 포함한 state

다음 값을 포함한다.

* board size와 occupied square별 concrete piece ID
* `current_player`
* `phase`
* terminal `result`의 winner와 reason
* 각 concrete piece의:
  * piece ID
  * owner
  * 현재 type ID
  * current square
  * pocket/captured flag
  * `has_moved`
  * stable-sorted piece state key/value
  * stable-sorted move-option cooldown과 remaining 값
* stable-sorted player 정보:
  * player/deck owner ID
  * starting piece membership
  * pocket membership
  * captured-piece membership
  * score limit과 total score
* `en_passant_target`
* `en_passant_available_to`
* stable-sorted Chessembly `global_state`

board와 `Piece.current_square`, piece pocket flag와 player pocket membership처럼 정상 state에서는 중복되는 정보도 포함했다. 외부에서 역직렬화된 불일치 state가 들어온 경우 서로 다른 legal behavior를 낼 수 있으므로 correctness를 위해 별도로 구분한다.

## 제외한 state와 이유

* `GameState.id`: 게임 식별자일 뿐 legal generation/evaluation에서 읽지 않는다.
* `turn_number`: 현재 legal move, placement, evaluation 및 Chessembly execution context에서 읽지 않는다. cooldown semantics는 각 piece의 현재 `remaining` 값에 반영된다.
* `history`: 현재 engine 검색 결과에 영향을 주는 코드가 없다. 기록은 `apply_and_advance_turn`에서 append되지만 legal move, evaluation, Chessembly interpreter는 읽지 않는다.
* 전체 piece definitions/custom manifest/program cache: 아래 ruleset scope 불변조건으로 처리한다.
* visual asset, 이름, 설명: 검색 규칙과 평가에서 관찰하지 않는다.

향후 repetition rule, history 조건 또는 turn-number 기반 Chessembly 기능을 추가할 경우 이 제외 판단을 함께 변경해야 한다.

## Piece definitions와 custom ruleset

TT는 `choose_bot_action` 한 번의 `SearchContext` 안에서 생성되고 호출 종료 시 폐기된다. 검색 중 `piece_definitions`, custom manifest와 compiled Chessembly catalog는 immutable이며, 모든 descendant state는 root state를 clone한 뒤 canonical action mutation만 적용한다.

따라서 definition 전체를 매 node key에 반복 복제하지 않는다. TT는 다른 game, bot search 또는 ruleset 사이에서 재사용되지 않으며 bot player 관점도 한 table lifetime 동안 고정된다.

## Lifetime과 memory bound

* lifetime: bot action search 1회
* 전역/static cache 없음
* 최대 entries: `min(search max_nodes, 65,536)`
* 동일 key replacement: 새 entry depth가 기존 depth 이상일 때만 교체
* table이 가득 찬 경우 새로운 key 저장을 생략

현재 difficulty의 최대 node budget은 Hard의 10,000이므로 기본 검색에서는 최대 10,000 entries다.

## Entry와 bound semantics

각 entry는 다음을 저장한다.

```text
depth
score
Exact | LowerBound | UpperBound
best_action
```

lookup 의미:

* `tt_probes`: TT가 활성화된 alpha-beta node에서 lookup을 시도한 횟수
* `tt_hits`: 같은 canonical position entry를 찾은 횟수. 얕은 entry도 best-action ordering에는 사용할 수 있으므로 hit에 포함한다.
* entry depth가 요청 depth 이상일 때만 score/bound를 사용한다.
* `Exact`: 즉시 score 반환
* `LowerBound`: alpha 갱신
* `UpperBound`: beta 갱신
* bound 적용 후 `alpha >= beta`면 cutoff
* `tt_cutoffs`: Exact 반환 또는 bound window closure로 subtree를 생략한 횟수
* `tt_stores`: 새 entry 삽입 또는 허용된 depth replacement 횟수

store bound는 TT lookup 전 caller가 전달한 original alpha/beta window를 기준으로 결정한다.

```text
score <= original alpha -> UpperBound
score >= original beta  -> LowerBound
otherwise               -> Exact
```

현재 King capture score는 ply 보정 없는 고정 `WIN_SCORE`이므로 TT 저장 시 mate-distance normalization은 필요하지 않으며 기존 score semantics를 유지했다.

TT best action은 현재 state에서 canonical actions를 먼저 생성한 뒤 그 목록에 동일 action이 있을 때만 앞으로 이동한다. entry action을 trusted apply에 직접 전달하지 않는다.

## Abort 저장 정책

hard time/node limit으로 현재 node가 `Aborted`되면 해당 node entry를 저장하지 않는다. abort는 기존 Goal 1과 동일하게 root까지 전파되며 incomplete score는 TT bound나 root score로 승격되지 않는다.

abort 전에 완전히 끝난 별도 child node entry는 그 자체로 완전한 결과이므로 table에 남을 수 있다. 회귀 테스트에서는 budget 1로 parent가 중단됐을 때 parent entry와 store counter가 생성되지 않음을 확인했다.

## 추가 regression tests

다음을 검증한다.

* 동일 semantic state와 map insertion order가 다른 state의 key equality
* empty board-map entry 존재 여부가 key에 영향을 주지 않음
* player pocket vector 순서 canonicalization
* side to move 차이
* board square 차이
* pocket membership 및 pocket piece type 차이
* concrete piece `has_moved` 차이
* captured flag 차이
* Windmill-style piece state 차이
* cooldown remaining 차이
* en passant target/available player 차이
* Chessembly global state 차이
* table maximum size와 depth-preferred replacement
* Exact/LowerBound/UpperBound depth 및 cutoff semantics
* aborted parent가 TT entry를 저장하지 않음
* representative variant position TT ON/OFF action/score 일치
* Goal 0의 실제 9개 benchmark position TT ON/OFF action/score/completed depth 일치

마지막 비교에는 요구된 `piece-state-cooldown`, `drop-capture`, `airborne-deployment`, `alternating-soldier-pocket-swap`, `immediate-king-capture`가 모두 포함된다. 충분한 비교 budget으로 양쪽 모두 depth 2를 완료한다.

## 전체 검증 결과

```sh
cargo test --workspace --all-features
```

* 실행된 테스트 135개 통과
* 실패 0개
* profiling benchmark 2개는 의도대로 ignored

```sh
cargo clippy --workspace --all-features -- -D warnings
```

* warning/error 없이 통과

Debug TT ON benchmark:

```sh
cargo test -p brainfuck-chess-engine --features profiling --test ai_benchmark ai_search_baseline -- --ignored --nocapture --test-threads=1
```

TT OFF 비교:

```sh
cargo test -p brainfuck-chess-engine --features profiling --test ai_benchmark ai_search_tt_off_comparison -- --ignored --nocapture --test-threads=1
```

두 명령 모두 `--release`를 추가하여 release profile에서도 실행했다.

## Goal 1 대비 debug benchmark

Goal 1 값은 Goal 2 변경 직전 같은 workspace와 명령으로 다시 측정했다. elapsed는 단일 실행 참고값이며 strict 성능 assertion이 아니다.

| position | nodes | elapsed ms | legal gen | action apply | beta cutoffs | TT hits | TT cutoffs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `middlegame` | 128 → 128 | 116.636 → 166.370 | 212 → 212 | 128 → 128 | 44 → 44 | 0 | 0 |
| `tactical-captures` | 373 → 299 | 125.801 → 255.710 | 714 → 566 | 373 → 299 | 29 → 30 | 0 | 0 |
| `drop-branching` | 159 → 172 | 44.341 → 55.006 | 254 → 280 | 159 → 172 | 62 → 61 | 0 | 0 |
| `standalone-ability` | 26 → 26 | 4.004 → 4.754 | 41 → 41 | 26 → 26 | 11 → 11 | 0 | 0 |
| `piece-state-cooldown` | 105 → 77 | 19.912 → 15.839 | 182 → 126 | 105 → 77 | 27 → 25 | 0 | 0 |
| `immediate-king-capture` | 30 → 30 | 3.841 → 4.500 | 46 → 46 | 31 → 31 | 15 → 15 | 0 | 0 |
| `drop-capture` | 64 → 50 | 10.229 → 8.561 | 109 → 81 | 64 → 50 | 16 → 15 | 0 | 0 |
| `airborne-deployment` | 685 → 677 | 137.989 → 153.077 | 1,063 → 1,038 | 685 → 677 | 302 → 292 | 9 | 9 |
| `alternating-soldier-pocket-swap` | 102 → 102 | 12.406 → 11.875 | 66 → 66 | 102 → 102 | 48 → 48 | 0 | 0 |

TT ON의 probe/store 수는 hit가 없는 position에서 각각 nodes와 동일하다. Airborne은 677 probes, 9 hits, 9 cutoffs, 668 stores였다.

TT 자체 효과를 deterministic ordering이 동일한 Goal 2 TT OFF 실행과 비교하면 Airborne은 다음과 같다.

| metric | TT OFF | TT ON |
| --- | ---: | ---: |
| nodes | 687 | 677 |
| legal generation | 1,067 | 1,038 |
| action applications | 687 | 677 |
| beta cutoffs | 301 | 292 |
| TT hits/cutoffs | 0/0 | 9/9 |
| elapsed ms | 138.865 | 153.077 |

나머지 8개 depth-2 position에서는 transposition hit가 없었다. Goal 1 대비 node 변화 중 이 8개에서 발생한 변화는 TT cutoff가 아니라 새 deterministic equal-priority ordering의 탐색 순서 효과다.

## Release benchmark 보존

Goal 1 release baseline과 Goal 2 TT ON elapsed는 다음과 같다.

| position | Goal 1 release ms | Goal 2 TT ON release ms |
| --- | ---: | ---: |
| `middlegame` | 47.079 | 86.082 |
| `tactical-captures` | 105.204 | 171.412 |
| `drop-branching` | 13.195 | 49.376 |
| `standalone-ability` | 1.700 | 2.802 |
| `piece-state-cooldown` | 8.250 | 7.443 |
| `immediate-king-capture` | 1.767 | 1.946 |
| `drop-capture` | 3.959 | 3.332 |
| `airborne-deployment` | 48.010 | 49.661 |
| `alternating-soldier-pocket-swap` | 3.584 | 3.966 |

Goal 2 release TT OFF/ON에서 Airborne은 49.953 ms/49.661 ms였으며 nodes와 legal generation 감소가 key overhead를 근소하게 상쇄했다. 다른 position은 hit가 없어 canonical key 생성 비용만 추가됐다.

## 발견된 correctness/performance 위험

* 현재 exact canonical key는 state를 빠뜨리지 않는 대신 각 probe에서 정렬된 owned representation을 만든다. depth 2 hit rate가 낮은 position에서는 이 비용 때문에 elapsed가 증가한다.
* deterministic action tie-break는 TT ON/OFF 결과 재현성을 보장하지만, Goal 1의 HashSet iteration-order 기반 동점 순서와 달라져 일부 position의 alpha-beta node 수가 변했다.
* Airborne multi-deployment에서만 현재 benchmark상 실제 TT benefit이 측정됐다. 더 깊은 검색/Iterative Deepening이 추가되면 shallow entry의 best-action ordering과 cross-iteration reuse 효과를 다시 측정해야 한다.
* 향후 history/turn-number 기반 rule 또는 repetition evaluation을 추가하면 PositionKey도 동시에 확장해야 한다. 이를 누락하면 variant-unsafe reuse가 된다.
* custom definitions는 search lifetime 동안 immutable이라는 불변조건에 의존한다. TT를 game/global lifetime으로 승격하는 변경은 ruleset fingerprint 없이는 안전하지 않다.

현재 구현의 우선순위는 raw speed보다 collision-safe canonical identity와 incomplete-search 비저장을 통한 correctness다. 후속 최적화에서는 canonical key의 allocation/정렬 비용을 줄이되 동일 key regression suite를 유지해야 한다.
