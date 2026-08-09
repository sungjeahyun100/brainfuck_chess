# Goal 4 — Deck Chess용 Quiescence Search 구현

## 목적

고정 depth leaf에서 캡처 직전/직후 같은 불안정한 position을 정적 평가하여 발생하는 horizon effect를 줄인다.

일반 체스의 capture-only quiescence를 그대로 구현하지 않는다.

Deck Chess에서는 capture 외에도 상태를 크게 변화시키는 여러 action이 존재하므로 `noisy action` 개념을 정의한다.

## 1. Quiescence entry

regular alpha-beta에서:

```text
depth == 0
```

이면 즉시 `evaluate()` 하지 않고:

```text
quiescence(...)
```

를 호출한다.

terminal game state는 기존 terminal evaluation을 유지한다.

## 2. Stand pat

quiescence 시작 시 현재 state의 static evaluation을 계산한다.

이를 stand-pat score로 사용한다.

일반적인 alpha-beta quiescence semantics를 사용하되 현재 maximizing/minimizing 구조 또는 negamax 여부에 맞게 구현한다.

검색 관점이 bot player 기준으로 고정되어 있다는 기존 evaluation semantics를 깨뜨리지 않는다.

## 3. Noisy action 정의

초기 구현에서 최소 다음을 noisy로 분류한다.

### 반드시 포함

* King capture
* 일반 capture
* capture-on-drop
* promotion

### 현재 엔진 구조를 조사해서 포함

* piece type transition을 일으키는 move
* 상대 기물을 제거하거나 pocket으로 돌리는 standalone ability
* 즉각적으로 material/state를 크게 바꾸는 action effect

예를 들어 Green Camp recall처럼 상대 기물을 보드에서 제거해 pocket으로 보내는 능력은 tactical/noisy 후보로 취급하는 것이 합리적인지 실제 effect를 확인한다.

반대로 단순 quiet drop이나 위치 이동은 초기 qsearch에서 제외한다.

## 4. 중앙 분류 함수

`is_noisy_action(state, action)` 또는 이에 대응하는 단일 정책 지점을 만든다.

quiescence 내부 곳곳에 piece id 문자열을 직접 하드코딩하는 방식은 피한다.

가능하면 action의 구조적 effect를 보고 판단한다.

능력처럼 현재 구조상 generic effect 정보가 부족한 경우 최소한의 명시적 분류를 허용하되 한 곳에 모은다.

## 5. Q-search limits

변형체스에서는 tactical action chain이 길거나 반복될 가능성이 있으므로 반드시 별도 제한을 둔다.

최소:

* max q depth
* q node count

hard search timeout/node limit도 공유한다.

예:

```text
max_q_depth = 보수적인 작은 값
```

으로 시작한다.

정확한 기본값은 benchmark로 정한다.

무한 tactical recursion이 절대 발생하지 않아야 한다.

## 6. Move ordering

qsearch에서는 최소:

1. King capture
2. 높은 가치 capture
3. promotion
4. 다른 noisy effect

정도로 우선순위를 둔다.

가능하면 기존 move ordering infrastructure를 확장한다.

## 7. TT 연동

초기 구현에서는 TT의 regular depth entry와 qsearch entry를 혼동해서 잘못 reuse하지 않도록 한다.

두 방법 중 안전한 쪽을 선택한다.

* qsearch 전용 depth semantics 정의
* 또는 첫 구현에서는 qsearch 결과를 TT에 저장하지 않음

정확성이 명확하지 않다면 후자를 선택한다.

## 8. SearchStats

추가:

* qnodes
* qdepth_reached

가능하면:

* noisy actions searched

## 테스트

### Horizon regression

예를 들어:

```text
봇이 가치 높은 기물을 잡을 수 있지만
바로 다음 tactical reply에서 더 큰 손해를 보는 position
```

을 구성한다.

fixed static leaf보다 qsearch가 안정적인 선택을 하는지 검증한다.

### Promotion

depth boundary 바로 뒤 promotion이 있을 때 qsearch가 이를 고려하는지 확인한다.

### Capture-on-drop

가능한 경우 drop capture가 qsearch 후보에 포함되는지 테스트한다.

### Ability

noisy로 분류한 standalone ability가 실제 qsearch에 포함되는지 테스트한다.

### Quiet action exclusion

일반 quiet move/drop이 qsearch branching에 무조건 포함되지 않는지 확인한다.

### Limits

인위적으로 tactical sequence가 긴 state에서도 qsearch limit과 hard limit 안에 종료해야 한다.

## 하지 말아야 할 것

이번 Goal에서:

* 모든 ability를 무조건 qsearch에 포함하지 않는다.
* 모든 drop을 noisy로 취급하지 않는다.
* Aspiration Window를 아직 구현하지 않는다.
* SEE 같은 복잡한 체스 전용 알고리즘을 억지로 넣지 않는다.

## 완료 조건

1. depth 0에서 qsearch로 연결된다.
2. Deck Chess에 맞는 noisy action 정책이 중앙화되어 있다.
3. capture/drop capture/promotion/중요 ability를 처리할 수 있다.
4. qsearch 자체 depth/node limit이 있다.
5. TT와 잘못된 entry reuse가 발생하지 않는다.
6. qnodes를 측정할 수 있다.
7. 기존 correctness test가 유지된다.
