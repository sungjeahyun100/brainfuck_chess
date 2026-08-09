# Goal 1 — Search Foundation 리팩터링

## 목적

Transposition Table과 Iterative Deepening을 안전하게 구현할 수 있도록 현재 AI search core를 정리한다.

특히 다음 문제를 해결한다.

1. 검색 시간/노드 제한에 걸린 불완전한 탐색이 정상적인 score처럼 상위 호출자에게 전달되는 문제
2. root search와 recursive alpha-beta가 강하게 결합된 구조
3. 검색 중 이미 생성한 canonical action을 다시 반복 검증하는 비용
4. 검색 통계가 이후 최적화를 분석하기에 부족한 문제

이번 Goal에서 검색 알고리즘의 전략 자체를 크게 변경하지 않는다.

## 1. Search abort를 명시적으로 표현

현재 제한에 걸리면 단순히 `evaluate()` 결과를 반환하는 방식이라면 이를 제거한다.

검색 함수는 최소한 다음 두 상태를 구분할 수 있어야 한다.

* 정상적으로 해당 subtree 탐색 완료
* 시간 또는 node budget 때문에 탐색 중단

예시 개념:

```rust
enum SearchOutcome {
    Complete(i32),
    Aborted,
}
```

정확한 이름과 구조는 현재 코드에 맞게 선택한다.

중요:

중단된 subtree의 score를 정상적인 minimax 결과로 사용해서는 안 된다.

## 2. Root search 분리

현재 `choose_bot_action()`에 직접 들어 있는 root 후보 탐색 로직을 별도의 함수/구조로 분리한다.

개념적으로:

```text
choose_bot_action
  -> search_root(...)
       -> alpha_beta(...)
```

형태가 되도록 한다.

`search_root`는 최소 다음 정보를 반환할 수 있어야 한다.

* best action
* best score
* 해당 depth 탐색 완료 여부

향후 Iterative Deepening이 root search를 반복 호출할 수 있는 구조여야 한다.

## 3. 검색 내부 action 적용 경로 추가

AI가 `generate_ai_actions()`에서 직접 생성한 canonical action을 적용할 때 현재 public validation boundary를 여러 번 통과하면서 합법 수를 재생성한다면 이를 최적화한다.

외부 API용:

```text
submit_action
-> canonical validation
-> apply
```

은 유지한다.

검색 내부에서는:

```text
generated canonical action
-> internal trusted apply
```

가 가능하도록 한다.

조건:

* public API로 노출하지 않거나 최소한 `pub(crate)` 범위로 제한한다.
* AI가 임의의 malformed action을 이 경로로 적용하지 않도록 호출 구조를 제한한다.
* public `submit_action()`의 validation을 삭제하거나 약화하지 않는다.
* 내부 적용 결과와 public canonical 적용 결과가 동일한지 regression test를 작성한다.

가능하면 실제 state mutation logic 자체는 하나의 공통 구현을 공유하고 validation boundary만 분리한다.

## 4. SearchStats 확장

현재 필드를 유지하면서 향후 검색 분석에 필요한 통계를 추가할 기반을 만든다.

최소:

* searched_nodes
* depth_reached
* completed_depth 또는 이에 대응하는 개념
* beta_cutoffs

향후 사용할 수 있도록 기본값을 둬도 되는 항목:

* qnodes
* tt_hits
* tt_cutoffs
* aspiration_researches

아직 구현되지 않은 기능의 카운터는 항상 0이어도 된다.

기존 serialization/API와 호환성이 필요한지 확인하고 필요하면 serde default를 사용한다.

## 5. soft/hard timeout 의미 정리

hard limit은 즉시 search abort를 발생시켜야 한다.

soft limit은 현재 root iteration을 무조건 망가뜨리는 방식으로 사용하지 않는다.

이번 단계에서는 Iterative Deepening을 구현하지 않으므로 현재 behavior와 최대한 비슷하게 유지하되, 이후 ID에서 다음 형태로 사용할 수 있게 구조화한다.

* hard limit: 진행 중인 탐색도 중단
* soft limit: 새로운 iteration을 시작하지 않는 판단에 사용 가능

## 테스트

반드시 다음 테스트를 추가한다.

### Abort propagation

아주 낮은 node budget 또는 time budget에서:

* search가 중단됨을 상위 호출자가 알 수 있어야 한다.
* 중단된 결과가 completed result로 기록되지 않아야 한다.

### Internal apply parity

같은 canonical action에 대해:

* public validation 경로
* search internal trusted apply 경로

의 최종 state가 동일해야 한다.

Move, Drop, Ability 각각 가능한 범위에서 테스트한다.

### 기존 correctness

다음 기존 특성이 유지되어야 한다.

* 즉시 King capture 선택
* illegal action 미선택
* special move 지원
* ability 지원
* cooldown 적용
* drop 정상 적용

## 하지 말아야 할 것

이번 Goal에서는 다음을 구현하지 않는다.

* TT
* Iterative Deepening 전체 구현
* Quiescence
* Aspiration
* 평가 함수 대규모 변경

## 완료 조건

다음을 모두 만족해야 한다.

1. recursive search가 완료/중단을 명시적으로 구분한다.
2. root search가 별도 단위로 분리되어 있다.
3. 검색 내부 canonical action 적용 시 불필요한 전체 legal regeneration이 감소한다.
4. 외부 action validation은 유지된다.
5. 이후 Iterative Deepening이 root search를 반복 호출할 수 있다.
6. 모든 기존 테스트가 통과한다.

마지막에 Goal 0 benchmark와 비교하여:

* legal move generation 호출 수
* searched nodes
* elapsed time

변화를 보고한다.

# Goal 1 추가 요구사항 — Airborne 다중 deployment 지원

현재 `generate_legal_ability_actions()`에서 Airborne의 `airdrop`은 실제 게임 규칙상 하나의 `AbilityAction`에 여러 `AbilityDeployment`를 포함할 수 있음에도, AI 검색 후보에서는 사실상 deployment 1개짜리 action만 생성되고 있다.

이 문제를 Goal 1의 search foundation 작업과 함께 해결한다.

## 요구사항

1. AI가 검색하는 action space와 실제 플레이어가 제출할 수 있는 canonical legal action space가 불필요하게 달라지지 않도록 한다.

2. Airborne의 `airdrop`에 대해 실제 규칙이 허용하는 다중 deployment 조합을 AI도 검색할 수 있어야 한다.

3. 생성되는 multi-deployment action은 기존 `is_legal_ability_action()` 및 public `submit_action()`에서 정상적으로 accepted 되는 canonical action이어야 한다.

4. 다음 제약을 모두 지켜야 한다.

   * 동일 pocket piece를 한 action에서 두 번 사용할 수 없음
   * 동일 square에 두 deployment를 둘 수 없음
   * 각 pocket piece는 원래 `airdrop` 조건을 만족해야 함
   * 각 square는 해당 Airborne 능력의 합법 영역이어야 함
   * King이나 점수 제한을 초과하는 기물 등 기존 eligibility 규칙을 우회하지 않음

5. 조합 폭발을 고려한다.
   단순히 가능한 모든 deployment 부분집합과 순열을 중복 생성하지 않는다.
   순서만 다른 동일 의미 action은 하나로 canonicalize한다.

예:

```text
Bishop -> c5
Knight -> d6
```

와

```text
Knight -> d6
Bishop -> c5
```

가 동일한 게임 결과를 의미한다면 AI 후보에서는 중복되지 않아야 한다.

6. 현재 `generate_ai_actions()`가 public gameplay legal generation을 재사용하는 구조라면,
   Airborne만 AI 전용 하드코딩으로 우회하기보다 legal ability generation 자체의 canonical representation을 정리하는 쪽을 우선 검토한다.

단, UI/API에서 필요로 하는 single-deployment 후보와 실제 한 턴의 multi-deployment action이 서로 다른 개념이라면 역할을 명확히 분리해도 된다.

7. 이 변경 때문에 일반 Ability, Green Camp, Alternating Soldier 등의 기존 action generation semantics가 변경되어서는 안 된다.

## 테스트

최소 다음을 추가한다.

### multi-deployment generation

Airborne + pocket Bishop + Knight 상태에서 AI legal actions에 다음과 같은 action이 존재해야 한다.

```text
airdrop:
- Bishop -> 합법 칸 A
- Knight -> 합법 칸 B
```

즉 `deployments.len() >= 2`인 legal `AiAction::Ability`가 실제 생성되어야 한다.

### canonical validity

생성된 multi-deployment action을:

```text
submit_action(state, TurnAction::Ability(action))
```

으로 제출했을 때 성공해야 한다.

### no duplicate permutations

동일 deployment 집합을 순서만 바꾼 action이 중복 생성되지 않는지 테스트한다.

### uniqueness constraints

* 같은 piece를 두 번 배치하는 action 없음
* 같은 square를 두 번 사용하는 action 없음

### benchmark coverage

Goal 0의 `airborne-deployment` benchmark correctness assertion을 강화하여 단순히 `deployments`가 비어 있지 않은지만 확인하지 말고, 최소 하나의 `deployments.len() >= 2` action이 존재하는지 확인한다.

기존 baseline position은 유지한다.

## 완료 조건

Goal 1 완료 시 Airborne은 실제 게임에서 허용되는 다중 deployment 능력을 AI 검색에서도 고려할 수 있어야 하며, 생성된 action은 기존 canonical validation 경계를 그대로 통과해야 한다.


