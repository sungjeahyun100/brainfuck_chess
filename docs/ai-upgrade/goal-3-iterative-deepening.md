
# Goal 3 — Iterative Deepening 구현

## 목적

기존의 한 번짜리 fixed-depth search를 Iterative Deepening 방식으로 변경한다.

검색은:

```text
depth 1
depth 2
depth 3
...
```

순으로 진행한다.

시간 또는 node budget이 끝났을 때는 반드시 **마지막으로 완전히 완료된 depth의 결과**를 사용한다.

Goal 1의 abort propagation과 Goal 2의 TT를 활용한다.

## 1. Iteration loop

`choose_bot_action()` 또는 적절한 search controller에 iterative loop를 만든다.

개념:

```text
for depth in 1..=max_depth
    search_root(depth)

    if completed:
        save best action/score
    else:
        stop
```

최소 depth 1이 완료되었다면 이후 iteration이 abort되어도 depth 1 결과를 반환한다.

아무 depth도 완료하지 못한 극단적 상황에는 기존의 legal fallback action을 사용할 수 있다.

## 2. completed depth

`depth_reached`와 `completed_depth`를 혼동하지 않는다.

예:

depth 4를 탐색하다 abort했다면:

```text
depth_reached = 4일 수 있음
completed_depth = 3
```

이어야 한다.

최종 BotDecision에 어느 값을 노출할지 현재 API 호환성을 고려하되 두 의미를 내부적으로 구분한다.

## 3. Time management

### Hard limit

현재 탐색 중이라도 즉시 abort할 수 있다.

### Soft limit

완료된 iteration 뒤:

```text
soft limit을 넘었으면 다음 depth를 시작하지 않는다.
```

방식으로 사용하는 것을 기본으로 한다.

현재 iteration을 soft limit 때문에 중간에서 버리는 방식은 지양한다.

단 hard limit은 항상 우선한다.

## 4. Previous iteration best move ordering

직전 완료 iteration의 best action을 다음 iteration에서 root ordering 최우선으로 둔다.

TT best move와 겹치면 중복 처리하지 않는다.

권장 우선순위:

1. previous iteration/PV best move
2. TT move
3. immediate King capture
4. tactical/capture ordering
5. 나머지

구체적 구현은 현재 move ordering module에 맞게 한다.

## 5. Easy difficulty behavior

현재 Easy가 상위 후보 중 임의 선택 같은 의도적 약화를 사용하고 있다면 그 게임 디자인을 유지한다.

단 검색 iteration 결과를 불완전하게 섞지는 않는다.

Easy의 randomness는 completed root result 후보를 대상으로 적용하거나 기존 semantics와 최대한 비슷하게 유지한다.

## 6. SearchLimits 의미

기존:

* max_depth_actions
* max_nodes
* soft_time_ms
* hard_time_ms

를 유지할 수 있다.

`max_depth_actions`는 iterative deepening의 최대 depth로 해석한다.

## 테스트

### Completed-depth fallback

의도적으로 depth N 탐색 중 budget을 소진시킨다.

결과가 depth N의 partial result가 아니라 depth N-1의 completed result인지 확인한다.

### Plenty-of-time case

충분한 budget에서는 설정된 max depth까지 완료하는지 확인한다.

### Immediate win

모든 difficulty에서 즉시 King capture를 계속 선택해야 한다.

### deterministic strong difficulties

Normal/Hard에서 동일 state와 동일 설정이면 randomness가 없다면 안정적으로 동일 수를 선택해야 한다.

## Benchmark

Goal 0의 모든 position에서:

* completed depth
* nodes
* elapsed
* tt hits
* selected move

를 기록한다.

특히 fixed-depth 방식보다 제한시간 내에서 안정적인 결과가 나오는지 비교한다.

## 하지 말아야 할 것

이번 단계에서:

* Quiescence 구현하지 않음
* Aspiration 구현하지 않음
* LMR/PVS 구현하지 않음

## 완료 조건

1. depth 1부터 순차적으로 검색한다.
2. partial iteration을 최종 결과로 사용하지 않는다.
3. soft/hard time semantics가 분리되어 있다.
4. 이전 iteration best move가 다음 search ordering에 사용된다.
5. TT와 정상적으로 함께 동작한다.
6. 기존 AI correctness가 유지된다.
