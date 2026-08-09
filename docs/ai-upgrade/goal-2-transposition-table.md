# Goal 2 — Variant-safe Transposition Table 구현

## 목적

동일한 게임 position을 반복 탐색하는 비용을 줄이기 위해 Transposition Table을 구현한다.

Deck Chess는 일반 체스와 달리 보드 위치만으로 state가 결정되지 않으므로 반드시 variant-safe position identity를 설계한다.

정확성이 성능보다 우선이다.

## 1. Search position identity 정의

TT key에 어떤 state가 포함되어야 하는지 먼저 조사하고 문서화한다.

최소한 다음은 검색 결과에 영향을 줄 수 있으므로 고려한다.

* current player
* 모든 기물의:

  * owner
  * type
  * board square
  * pocket 여부
  * captured 여부
  * has_moved
  * per-piece state
  * move option cooldown
* en passant state
* global state

또한 다음을 조사한다.

* history가 현재 또는 향후 Chessembly/legal move 결과에 영향을 주는가?
* turn number 자체가 규칙에 영향을 주는가?

영향을 준다면 반드시 position key에 반영한다.

반대로 검색 결과와 관계없는 다음 정보는 key에서 제외한다.

예:

* game id
* visual asset
* piece display name
* description

## 2. Immutable definition context 처리

한 번의 search 동안 `piece_definitions`와 custom piece definition이 immutable이라는 전제가 실제 코드에서 안전한지 확인한다.

안전하다면 매 TT key마다 전체 definition을 hash할 필요는 없다.

대신 TT가 다른 규칙 집합의 게임 사이에서 재사용되지 않도록 lifetime을 한 search 또는 한 game context로 제한한다.

이 전제가 불안전하면 definition fingerprint를 도입한다.

정확성을 우선한다.

## 3. Key 구현

초기 구현은 반드시 incremental Zobrist일 필요는 없다.

현재 복잡한 state를 놓칠 위험이 있으므로:

* canonical search position representation
* deterministic hash

방식으로 시작해도 된다.

HashMap iteration order에 따라 key가 달라지지 않도록 반드시 deterministic ordering 또는 hash combination을 사용한다.

동일 의미의 state는 항상 동일 key가 나와야 한다.

## 4. TT Entry

최소 다음 정보를 가진다.

```rust
depth
score
bound
best_action
```

bound는 최소:

```text
Exact
LowerBound
UpperBound
```

를 표현한다.

## 5. Alpha-beta integration

TT lookup 시:

* 저장 depth가 현재 요구 depth 이상일 경우 bound를 사용할 수 있다.
* Exact면 즉시 score 반환 가능
* Lower/Upper bound는 alpha/beta를 조정하거나 cutoff에 사용

store 시 원래 alpha/beta window와 최종 결과를 바탕으로 bound type을 정확히 기록한다.

mate/king-capture score와 ply를 나중에 조정할 필요가 있는지 현재 scoring 구조를 확인한다.

현재 WIN_SCORE가 고정이라면 일단 기존 semantics를 유지하되 잘못된 TT reuse가 생기지 않는지 테스트한다.

## 6. TT best action move ordering

TT entry에 저장된 best action이 현재 legal candidate 목록에 존재한다면 해당 action을 가장 먼저 검색한다.

없다면 무시한다.

TT의 stale/invalid action 때문에 검색이 실패해서는 안 된다.

## 7. Replacement policy

초기 버전은 간단하게 구현한다.

예:

* deeper entry 우선
* 동일 key면 깊은 결과로 교체

필요하다면 최대 entry 수를 둔다.

무제한 메모리 증가를 허용하지 않는다.

## 8. SearchStats

최소 추가:

* tt_probes
* tt_hits
* tt_cutoffs
* tt_stores

프로젝트 API에 너무 많은 필드를 노출하는 것이 부담되면 내부 stats와 외부 stats를 분리해도 된다.

## 테스트

### Position key equality

완전히 동일한 logical position은 동일 key.

### Position key inequality

다음 중 하나만 달라도 필요하면 다른 key여야 한다.

* current player
* square
* pocket
* captured
* has_moved
* piece state
* cooldown
* en passant
* global state

history가 규칙에 영향을 준다면 history 차이도 포함한다.

### TT correctness

TT enabled/disabled 검색이 동일한 충분한 depth에서 동일 best move/score를 내는 representative positions를 만든다.

### Transposition hit

서로 다른 move order로 동일 position에 도달할 수 있는 테스트 position을 만들 수 있다면 TT hit가 실제 발생하는지 확인한다.

## 하지 말아야 할 것

이번 Goal에서:

* Quiescence를 구현하지 않는다.
* Aspiration을 구현하지 않는다.
* 공격적인 TT compression을 하지 않는다.
* board-only key를 사용하지 않는다.

## 완료 조건

1. variant state를 고려한 deterministic position key가 있다.
2. TT Exact/Lower/Upper semantics가 동작한다.
3. TT best action이 ordering에 활용된다.
4. TT를 껐을 때와 켰을 때 correctness가 유지된다.
5. TT stats를 관찰할 수 있다.
6. benchmark에서 hit/cutoff 효과를 확인할 수 있다.

마지막에 Goal 0 benchmark와 비교하여 결과를 보고한다.
