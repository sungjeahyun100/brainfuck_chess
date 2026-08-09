
# Goal 5 — Aspiration Window 구현

## 목적

Iterative Deepening의 이전 iteration score를 이용해 다음 depth alpha-beta search window를 좁혀 탐색 효율을 높인다.

Goal 3의 Iterative Deepening과 Goal 4의 Quiescence가 안정적으로 동작한다는 전제에서 구현한다.

## 1. 적용 범위

depth 1은 full window를 사용한다.

depth 2 이상에서는 직전 완료 iteration의 score를 중심으로 aspiration window를 설정한다.

개념:

```text
previous_score - delta
previous_score + delta
```

정확한 delta는 상수 또는 SearchConfig로 둔다.

WIN_SCORE와 일반 evaluation score scale을 고려해서 합리적인 초기값을 선택한다.

## 2. Fail-low / Fail-high 처리

검색 결과가 window 밖으로 벗어나면 결과를 그대로 채택하지 않는다.

### Fail-low

score가 alpha 이하라면 lower side를 확장하여 재탐색한다.

### Fail-high

score가 beta 이상이라면 upper side를 확장하여 재탐색한다.

필요하면 window를 지수적으로 확장한다.

최종적으로 충분히 넓은 window 또는 full window까지 fallback할 수 있어야 한다.

## 3. Timeout 처리

aspiration re-search 중 hard timeout이 발생할 수 있다.

이 경우 불완전한 현재 depth 결과를 사용하지 않는다.

반드시 직전 completed depth의 결과로 돌아간다.

즉:

```text
depth 4 aspiration search fail-high
-> re-search 시작
-> hard timeout
```

이면 depth 3 결과를 사용한다.

Goal 1/3의 abort semantics를 그대로 활용한다.

## 4. TT 활용

첫 aspiration search에서 생성된 TT entries가 re-search에 활용될 수 있어야 한다.

단 bound semantics가 정확해야 한다.

fail-high/fail-low 결과를 Exact로 저장해서는 안 된다.

## 5. SearchStats

최소:

* aspiration_searches
* aspiration_researches
* fail_high_count
* fail_low_count

를 기록한다.

필드 이름은 현재 구조에 맞게 조정 가능하다.

## 6. 설정

초기 aspiration delta를 magic number로 search 함수 여러 곳에 흩뿌리지 않는다.

한 설정 위치에 둔다.

예:

```text
SearchLimits
SearchConfig
상수
```

중 현재 구조에 적합한 방식을 선택한다.

난이도별로 다르게 설정할 필요는 초기에는 없다.

## 테스트

### Stable score

이전 depth와 다음 depth score가 비슷한 position에서 re-search 없이 완료되는지 확인한다.

### Fail-high

의도적으로 작은 window를 테스트 설정에서 사용해 fail-high가 발생하도록 만들고 정상적으로 window를 확장하는지 확인한다.

### Fail-low

동일하게 fail-low를 테스트한다.

### Timeout during re-search

re-search 중 abort가 발생하면 직전 completed iteration 결과를 반환하는지 검증한다.

### TT bound correctness

aspiration 검색 후에도 TT enabled/disabled 결과가 충분한 budget에서 동일해야 한다.

## Benchmark

Goal 0의 benchmark position에 대해:

* searched nodes
* elapsed
* aspiration re-search count
* TT hits
* completed depth

를 비교한다.

Aspiration 때문에 오히려 re-search가 과도한 position이 있는지도 보고한다.

그런 경우 delta 조정 근거를 제시한다.

## 하지 말아야 할 것

이번 Goal에서는:

* PVS
* LMR
* Null Move Pruning
* Futility Pruning

을 같이 구현하지 않는다.

## 완료 조건

1. depth 2 이상에서 이전 score 기반 aspiration window를 사용한다.
2. fail-low/high 시 안전하게 re-search한다.
3. timeout 시 partial current iteration을 사용하지 않는다.
4. TT bound semantics와 호환된다.
5. 통계로 aspiration 효과를 측정할 수 있다.
6. benchmark 결과를 baseline 및 ID-only 결과와 비교할 수 있다.

