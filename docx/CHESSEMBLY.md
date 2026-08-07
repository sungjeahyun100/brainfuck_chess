# Chessembly 공식 문서 오프라인 정리본

출처: https://docs.en-passant.org/

이 파일은 Chessembly 인터프리터를 구현하는 AI가 공식 문서에 접근하지 못하는 상황에서 참고할 수 있도록, 공식 문서에서 확인된 내용을 섹션별로 모은 자료다.

주의:
- 이 문서는 Chessembly 코드를 작성하게 하는 예시 프롬프트가 아니라, Chessembly DSL의 공식 동작을 오프라인으로 전달하기 위한 자료다.
- 공식 문서에서 확인된 의미만 적는다.
- 문서에서 확인되지 않은 동작은 `Unknown / 문서 확인 필요`로 표시한다.
- 구현자가 체스 일반 상식으로 DSL 의미를 임의 보완하지 않도록 한다.

---

## 0. 공식 문서 구조

공식 문서는 다음 큰 섹션으로 구성되어 있다.

1. Tutorial
   - 1.1 Getting Started
   - 1.2 Slide
   - 1.3 move, take, catch, shift
2. Concepts
   - 2.1 Anchor
   - 2.2 Termination
   - 2.3 Scope
3. Controls
   - 3.1 Procedure & flow
   - 3.2 States
   - 3.3 Look around
   - 3.4 Jump
4. More Examples
   - 4.1 Example: Bouncing-Bishop
   - 4.2 Example: Beacon (1.0)
5. Reference

---

# 1. Tutorial

## 1.1 Getting Started

Chessembly는 체스와 변형 체스 기물의 행마법을 기술하는 표현법이다.

행마법은 가능한 움직임에 대한 설명을 한 줄 한 줄 적어 내려가는 방식이다. 예를 들어 “오른쪽으로 1칸”, “대각선으로 1칸” 같은 설명을 작성한다. Chessembly는 작성된 모든 설명을 읽고, 기물이 현재 갈 수 있는 모든 유효한 칸들을 활성화한다.

대부분의 기물, 예를 들어 나이트, 비숍, 룩, 퀸은 이동과 공격을 동시에 할 수 있다. 이러한 일반적인 동작을 `take-move`라고 설명한다.

`take-move`는 `(dx, dy)` 방향을 받는다.

좌표 예:

- `(1, 0)`: 오른쪽으로 1칸
- `(0, 1)`: 위로 1칸, 전진
- `(-1, 0)`: 왼쪽으로 1칸
- `(1, 1)`: 오른쪽 위 대각선 1칸

각 설명은 세미콜론 `;`으로 끝나야 한다. 세미콜론은 하나의 독립된 설명이 끝났다는 표시다.

띄어쓰기는 올바르게 입력되어야 한다. 각 표현식마다 반드시 띄어쓰기가 되어 있어야 하고, 여는 괄호는 앞의 표현식 이름과 붙여 쓴다. Chessembly는 대소문자에 민감하다. 예를 들어 `Take-Move(1, 0)`은 올바르지 않다.

Chessembly는 올바르지 않은 식을 일반적으로 `end`로 변환하여 인식한다. `end`가 중간에 등장하면 해당 식은 중단된다. 다만 간혹 오류가 발생할 수도 있다.

### Wazir 예시

Wazir는 상하좌우, 즉 직교 방향으로 한 칸만 이동 또는 잡기를 할 수 있는 기물이다.

```chessembly
take-move(1, 0);
take-move(-1, 0);
take-move(0, 1);
take-move(0, -1);
```

---

## 1.2 Slide

Rook이나 Bishop처럼 장애물이 없을 때까지 계속 움직이는 기물도 표현할 수 있다.

`repeat(1)`은 바로 앞의 식을 반복한다. 바로 앞에 있는 `take-move` 설명에 붙으면, 해당 움직임을 한 칸이 아니라 벽이나 다른 기물을 만날 때까지 계속 반복하라는 의미가 된다.

`repeat(2)`는 앞 2개의 식을 반복한다. 단, 앞에 2개의 식이 반드시 있어야 한다.

### Rook 예시

Rook은 상하좌우 네 방향으로 미끄러지듯 움직인다.

```chessembly
take-move(1, 0) repeat(1);
take-move(-1, 0) repeat(1);
take-move(0, 1) repeat(1);
take-move(0, -1) repeat(1);
```

`take-move(1, 0) repeat(1);`은 오른쪽으로 끝까지 이동하는 의미가 된다. Rook은 이 한 줄을 보고 `(1, 0)`, `(2, 0)`, `(3, 0)` 위치에 있는 모든 유효한 칸을 활성화한다.

### Knightrider 예시

`repeat(1)`은 `(1, 0)` 같은 직선 방향뿐 아니라 모든 방향에 적용할 수 있다.

Knightrider는 나이트처럼 L자 방향으로, 룩처럼 미끄러지는 변형 체스 기물이다.

```chessembly
take-move(1, 2) repeat(1);
take-move(2, 1) repeat(1);
take-move(1, -2) repeat(1);
take-move(2, -1) repeat(1);
take-move(-1, 2) repeat(1);
take-move(-2, 1) repeat(1);
take-move(-1, -2) repeat(1);
take-move(-2, -1) repeat(1);
```

예를 들어 `take-move(1, 2) repeat(1);`은 `(1, 2)`, `(2, 4)`, `(3, 6)` 방향으로 계속 탐색하며 모든 유효한 칸을 활성화한다.

---

## 1.3 move, take, catch, shift

`take-move`는 이동 또는 잡기가 모두 가능한 표현이다. Chessembly는 이 외에도 이동만 하거나, 잡기만 하거나, 다른 기물과 자리를 바꾸기 위한 표현을 제공한다.

### move

`move`는 해당 칸이 비어 있을 때만 칸을 활성화한다. 해당 칸에 적 기물이 있다면 그 칸은 활성화되지 않는다.

예:

```chessembly
move(1, 0) repeat(1);
move(-1, 0) repeat(1);
move(0, 1) repeat(1);
move(0, -1) repeat(1);
```

`take-move` Rook은 적을 만나면 그 칸을 잡을 수 있는 칸으로 활성화한다. `move` Rook은 적을 만나면 그 칸을 활성화하지 않고, 그 너머로 가지도 못한다.

### take

`take`는 `move`의 정반대다. 해당 칸에 적 기물이 있을 때만 칸을 활성화한다. 해당 칸이 비어 있다면 칸은 활성화되지 않는다.

예:

```chessembly
take(1, 1);
take(1, -1);
take(-1, 1);
take(-1, -1);
```

이 기물은 비어 있는 대각선 칸으로 움직일 수 없다.

### catch

`catch`는 `take`와 마찬가지로 해당 칸에 적이 있을 때만 칸을 활성화한다.

`catch`로 활성화된 칸을 클릭하면, 기물은 제자리에 머무른 채 해당 칸의 적 기물만 잡는다. 즉 원거리 공격이다.

`catch`는 기물이 위험한 칸으로 직접 이동하지 않는다. 따라서 공격한 직후 이동한 위치에서 적에게 되잡힐 위험이 없다.

### shift

`shift`는 `take-move`, `take`, `move`, `catch`와 달리 2개의 기물을 움직인다.

Reference 기준으로 `shift`는 빈 칸, 적 기물, 아군 기물에 대해 각각 활성화 및 기준 위치 이동을 수행할 수 있으며, 벽인 경우 `false`가 된다. 정확한 구현은 Reference의 행마식 표를 따른다.

---

# 2. Concepts

## 2.1 Anchor

세미콜론 `;`으로 구분되는 하나의 블록을 `식 연쇄`라고 부른다.

식 연쇄가 중요한 이유는, 연쇄 내부에서는 기준 위치가 계속 이동하기 때문이다.

기준 위치는 행마를 계산하기 시작하는 출발점이다.

규칙:

- 식 연쇄가 처음 시작될 때, 기준 위치는 현재 기물이 있는 칸이다.
- `move(1, 0)`과 같은 행마식이 실행되면, 기준 위치는 `(1, 0)`만큼 이동한다.
- 세미콜론 `;`을 만나 식 연쇄가 끝나면, 기준 위치는 다음 연쇄를 위해 다시 현재 기물이 있는 칸으로 초기화된다.

### 하나의 식 연쇄 예시

```chessembly
move(1, 0) move(1, 0);
```

기물이 `[c2]`에 있다고 가정하면:

1. 첫 번째 `move(1, 0)`은 `[d2]`를 활성화하고 기준 위치를 `[d2]`로 이동한다.
2. 두 번째 `move(1, 0)`은 새 기준 위치 `[d2]`에서 `[e2]`를 활성화하고 기준 위치를 `[e2]`로 이동한다.

최종적으로 `[d2]`와 `[e2]`가 활성화된다. 즉 `(1, 0)`과 `(2, 0)` 두 칸이 모두 활성화된다.

### 두 개의 식 연쇄 예시

```chessembly
move(1, 0);
move(1, 0);
```

세미콜론 때문에 두 식 연쇄가 분리된다. 각 연쇄는 현재 기물이 있는 칸에서 다시 시작하므로, 결과적으로 `move(1, 0);` 한 번만 입력한 것과 같은 행마법이 된다.

### anchor 식

공식 문서에는 `anchor` 식이 Concepts와 More Examples에서 사용된다. Beacon 예시에서는 `anchor(dx, dy)`가 기준 위치를 이동시키는 용도로 사용된다.

예:

```chessembly
anchor(0, -1)
anchor(-1, 0)
anchor(1, 0)
anchor(0, 1)
```

정확한 실패 조건과 반환값은 Reference에서 확인해야 한다.

---

## 2.2 Termination

행마식은 칸을 활성화하고 기준 위치를 옮길 뿐 아니라, 실행 후 자신의 상태를 보고한다.

반환값:

- `true`: 계획대로 칸을 활성화하거나 이동했고, 다음 식을 계속 진행해도 좋다.
- `false`: 벽, 아군, 또는 규칙에 막혀 아무것도 못 했다. 멈춰야 한다.

식 연쇄의 생명 규칙:

> 하나의 식 연쇄 안에서 일반 식이 `false` 값을 보고하면, 해당 식 연쇄의 실행이 중단된다.

일반 식이란 `jmp`, `not` 등 특별한 제어식을 제외한 대부분의 식을 의미한다.

### take-move 종료 규칙

`takes-move`가 아니라 `take-move`다.

`take-move`는 적을 잡는 칸을 활성화한 뒤 `false`를 반환하여 식 연쇄 중단을 알린다.

`take-move`가 만난 대상별 동작:

| 대상 칸 | 활성화 | 기준 위치 이동 | 반환값 |
|---|---|---|---|
| 빈 칸 | 함 | 함 | `true` |
| 적 기물 | 함 | 함 | `false` |
| 아군/벽 | 안 함 | 안 함 | `false` |

이 규칙 때문에 `take-move(1, 0) repeat(1);` 같은 슬라이드는 벽이나 기물을 만나면 멈춘다.

---

## 2.3 Scope

`{ }` 블록은 식 연쇄 안에 작은 스코프를 만든다.

`{`와 `}` 자체도 식이기 때문에, 앞뒤의 다른 표현과 띄어 써야 한다.

틀린 예:

```chessembly
move(1, 0) {move(1,0)};
```

올바른 예:

```chessembly
move(1, 0) { move(1,0) };
```

블록의 두 가지 핵심 규칙:

1. 종료의 격리, Failure Isolation
2. 기준 위치의 복원, Anchor Scoping

### 종료의 격리

`{ }` 블록 안에서 식이 `false`를 반환하더라도 식 연쇄 전체가 종료되지 않는다. 대신 해당 블록의 실행만 즉시 중단되고, 실행 순서는 닫는 괄호 `}` 바로 다음 식으로 점프한다.

### 기준 위치의 복원

`{ }` 블록은 시작할 때의 기준 위치를 체크포인트처럼 저장한다. 블록 안에서 `move` 등으로 기준 위치가 아무리 멀리 이동했더라도, 블록이 끝나면 기준 위치는 저장해 둔 체크포인트 위치로 복원된다.

### Y자 행마 예시

```chessembly
move(0, 1) { move(1, 1) } move(-1, 1);
```

초기 위치가 `[c2]`라고 하면:

1. `move(0, 1)`은 `[c3]`을 활성화하고 기준 위치를 `[c3]`으로 이동한다.
2. `{`에서 기준 위치 `[c3]`을 체크포인트로 저장한다.
3. 블록 내부 `move(1, 1)`은 `[c3]` 기준 `[d4]`를 활성화하고 기준 위치를 `[d4]`로 이동한다.
4. 블록이 끝나면 기준 위치는 `[c3]`으로 복원된다.
5. 이후 `move(-1, 1)`은 `[c3]` 기준 `[b4]`를 활성화한다.

---

# 3. Controls

## 3.1 Procedure & flow

`false` 값 종료 규칙을 무시하거나, 오히려 `false`를 이용하는 특별한 제어식들이 있다. 이 식들은 식 연쇄의 실행 흐름을 직접 제어한다.

문서의 Procedure & flow 섹션에서 설명하는 연쇄 종료 규칙의 예외:

1. `while`
2. `jmp(n)`
3. `jne(n)`
4. `not`
5. `label(n)`

Reference에서는 여기에 `true`, `false`, `read`, `read-and`, `read-or`, `read-xor`, `write`도 예외로 포함한다.

### while, jmp, jne

이 식들은 `false`를 받아도 연쇄를 멈추는 대신 항상 `true`를 반환하여 연쇄의 생명을 이어간다.

- `jmp(n)`: 직전 값이 `true`이면 `label(n)`으로 점프한다. 직전 값이 `false`이면 아무것도 하지 않는다.
- `jne(n)`: 직전 값이 `false`이면 `label(n)`으로 점프한다. 직전 값이 `true`이면 아무것도 하지 않는다.
- `while`: 직전 값이 `true`이면 `do`로 점프한다. 직전 값이 `false`이면 아무것도 하지 않는다.

`jne`는 프로그래밍의 `if`처럼 사용할 수 있다. 바로 앞의 식이 `false`면 특정 구간 뒤로 점프하도록 하여, 그 사이의 식들이 조건이 만족될 때만 실행되게 할 수 있다.

### not

`not`은 논리를 반전하는 식이다.

- 직전 값이 `true`이면 `false`를 반환한다.
- 직전 값이 `false`이면 `true`를 반환한다.

---

## 3.2 States

체스판만 보고는 캐슬링 가능 여부 같은 정보를 알 수 없는 경우가 있다. Chessembly는 게임 상태를 기억하고, 그 상태에 따라 행마를 바꾸는 기능을 제공한다.

상태를 관리하는 식은 두 종류로 나뉜다.

1. 조건식: 현재 상태를 읽고 `true` / `false`를 반환한다. 예: `if-state`, `piece`
2. 수식어 Modifier: 이후에 활성화될 칸에 특별한 액션을 부착한다. 예: `set-state`, `transition`

### 상태 조건식

#### piece(piece_name)

이 코드를 실행하는 기물 자체의 종류가 `piece_name`과 일치하면 `true`를 반환한다.

예: `piece(windmill-rook)`, `piece(rook)`, `piece(beacon)`

`piece(rook)`이 `false`를 반환하면 그 즉시 연쇄가 종료되므로, 특정 기물 전용 행마를 정의할 때 사용할 수 있다.

#### if-state(key, n)

게임에 저장된 전역 상태 `key`의 값이 `n`과 같으면 `true`를 반환한다.

만약 `key`가 한 번도 설정된 적 없다면 기본값 `0`으로 간주한다.

### 상태 수식어

`set-state`나 `transition`은 다음에 실행될 `move`/`take` 등이 활성화할 칸에 액션 태그를 미리 붙여두는 식이다.

이 식들 자체는 실패하지 않는 한 항상 `true`를 반환하여 연쇄를 계속 진행시킨다.

#### transition(piece_name)

이 식 이후에 활성화되는 모든 칸에, 클릭 시 `piece_name`으로 기물을 교체하는 액션 태그를 부착한다.

#### set-state(key, n)

이 식 이후에 활성화되는 모든 칸에, 클릭 시 전역 변수 `key`의 값을 `n`으로 설정하는 액션 태그를 부착한다.

#### set-state

`set-state`를 인자 없이 단독으로 사용하면, 이전에 설정된 마지막 액션 태그를 하나 지운다. 이 식 이후에 활성화되는 칸은 아무 상태도 변경하지 않는다.

---

## 3.3 Look around

조건식은 칸을 활성화하기 전에 그곳의 상태를 미리 확인하는 데 사용된다.

조건식은 기준 위치를 옮기거나, `peek` 제외, 칸을 활성화하지 않고, 보드를 관찰한 뒤 `true` 또는 `false`만 반환한다.

### observe vs peek

두 식 모두 해당 칸이 비어 있는지 확인한다. 비어 있으면 `true`, 기물이 있으면 `false`를 반환한다.

#### observe(dx, dy)

- `(dx, dy)` 위치를 확인하고 `true` 또는 `false`를 반환한다.
- 기준 위치는 움직이지 않는다.

#### peek(dx, dy)

- `(dx, dy)` 위치를 확인한다.
- 해당 위치가 비어 있어서 `true`를 반환한다면, 기준 위치도 `(dx, dy)`만큼 이동한다.
- 벽에 막혀서 `false`를 반환하면 기준 위치는 움직이지 않는다.

### 기물/상태 확인

- `enemy(dx, dy)`: `(dx, dy)` 위치에 적 기물이 있으면 `true`를 반환한다.
- `friendly(dx, dy)`: `(dx, dy)` 위치에 아군이 있으면 `true`를 반환한다.
- `piece-on(piece, dx, dy)`: `(dx, dy)` 위치에 `piece`, 예를 들어 `rook`, 기물이 있으면 `true`를 반환한다.
- `danger(dx, dy)`: `(dx, dy)` 위치가 현재 적에게 공격받고 있으면 `true`를 반환한다.
- `check`: 위치와 상관없이 현재 아군이 체크 상태이면 `true`를 반환한다.

### 경계 확인 Board Bounds

Bouncing-Bishop 예시에서 핵심적으로 사용된다.

- `bound(dx, dy)`: `(dx, dy)` 위치가 보드 밖이면, 어느 방향이든, `true`를 반환한다.
- `edge(dx, dy)`: `(dx, dy)` 위치가 보드의 변을 벗어나면 `true`를 반환한다.
- `corner(dx, dy)`: `(dx, dy)` 위치가 보드의 모서리를 벗어나면 `true`를 반환한다.

공식 문서의 추출 가능한 부분에는 “세부 경계 확인”이라는 항목이 있으나, 이 오프라인 추출본에서는 그 세부 목록의 본문이 완전히 확보되지 않았다. 프로젝트에서 `edge-right`, `edge-left`, `edge-top`, `edge-bottom` 등을 사용한다면 공식 Reference 또는 실제 구현을 추가 확인해야 한다.

---

## 3.4 Jump

`jump` 식은 11월 25일 업데이트로 새로 생긴 행마식이다.

`jump` 식은 `jmp` 식 및 `jne` 식과 다르다.

- `jmp`, `jne`: 제어식
- `jump`: 행마식

`jump` 식은 `take` 식과 짝을 이룬다.

`jump` 식이 후행하는 `take` 식은 일반 `take` 식과 다르게 동작한다. `take` 식이 실행된 직후에 `jump` 식을 만난다면, `jump` 식이 `true`를 내놓건 `false`를 내놓건, 마지막에 실행된 `take` 식은 `take` 행마법을 활성화하지 않는다.

`jump` 이전에 쓰인 `take` 식은 `take-jump` 식의 일부로 볼 수 있다.

앞에 `take` 식이 없는데 `jump` 식이 있었다면, `false`를 내놓고 아무것도 하지 않는다.

이 설계는 `take-jump`를 별도 식으로 쓰는 대신, 뒤에 `jump`가 쓰인 `take` 식을 `jump` 식과 함께 묶어 `take-jump` 식으로 만들기 위한 것이다. `take-jump`는 보드상에서 2개의 위치를 필요로 하므로, 한 번에 4개의 좌표를 입력하기보다 `take`와 `jump`가 좌표를 따로 지정하게 하기 위한 설계다.

`jump` 행마법의 기준 위치 이동 방법과 `true`, `false` 반환 조건은 `move` 식과 동일하다.

### Cannon 예시

```chessembly
do take(1, 0) enemy(0, 0) not while jump(1, 0) repeat(1);
do take(-1, 0) enemy(0, 0) not while jump(-1, 0) repeat(1);
do take(0, 1) enemy(0, 0) not while jump(0, 1) repeat(1);
do take(0, -1) enemy(0, 0) not while jump(0, -1) repeat(1);
do peek(1, 0) while friendly(0, 0) move(1, 0) repeat(1);
do peek(-1, 0) while friendly(0, 0) move(-1, 0) repeat(1);
do peek(0, 1) while friendly(0, 0) move(0, 1) repeat(1);
do peek(0, -1) while friendly(0, 0) move(0, -1) repeat(1);
```

공식 문서의 Cannon 설명 요지:

1. `do ... while` 문의 시작점인 `do` 식을 둔다.
2. 한 칸 앞에 `take`를 설치한다. 적이 없으면 `anchor`만 이동하고 `take` 행마는 설치되지 않는다.
3. 그곳에 적이 있어 `enemy(0, 0)`이 `true`이면,
4. `not`을 거쳐 `false`로 반전되고 `do ~ while` 루프를 탈출한다.
5. 적이 없었다면 루프를 반복한다.
6. `jump` 식은 `take` 식으로 지정된 위치를 `take`하면서 자기 칸으로 `jump`하는 행마를 대신 설치한다.
7. 이 과정을 반복한다.

---

# 4. More Examples

## 4.1 Example: Bouncing-Bishop

Bouncing-Bishop 예제는 3단계까지 배운 내용을 사용하여 벽에 한 번 튕기는 비숍을 단계별로 구축하는 과정이다.

목표:

1. 비숍의 슬라이드를 구현한다.
2. 슬라이드가 멈췄을 때, 적을 잡아서 멈춘 것인지, 벽에 막혀서 멈춘 것인지 구분한다.
3. 벽에 막혔을 경우에만 바운스 로직을 실행한다.
4. 오른쪽 벽에 막혔다면 정해진 방향, 예: 북서, 으로 튕겨 나간다.
5. 오른쪽 벽이 아니고 위쪽 벽에 막혔다면 다른 정해진 방향, 예: 남동, 으로 튕겨 나간다.

### 1단계: 멈추지 않는 슬라이드

```chessembly
do
  take-move(1, 1)
while
```

실행 설명:

1. `do`는 `do ... while`을 시작하는 위치다.
2. `take-move(1, 1)`이 실행된다.
   - 빈 칸: 칸을 활성화하고, 기준 위치를 이동하고, `true`를 반환한다.
   - 적 기물: 칸을 활성화하고, 기준 위치를 이동하고, `false`를 반환한다.
   - 벽/아군: 아무것도 하지 않고, 기준 위치는 유지되고, `false`를 반환한다.
3. `while`은 직전 식인 `take-move`의 값을 확인한다.
   - `true`이면 `do`로 돌아가 슬라이드를 계속한다.
   - `false`이면 `do`로 돌아가지 않고 루프를 이탈한다.
4. `while`은 연쇄 종료의 예외다. `false`를 받아도 연쇄를 종료시키지 않고 다음 식으로 실행을 넘긴다.

### 2단계: 바운스 관문

문서에는 `peek(0, 0)`을 사용해 슬라이드가 멈춘 이유를 구분하는 단계가 이어진다. 슬라이드가 `false`로 멈춘 이유는 적을 잡았거나, 벽에 막혔거나, 아군에 막혔거나 셋 중 하나다.

이 오프라인 추출본에서는 Bouncing-Bishop의 나머지 전체 코드가 완전히 확보되지 않았다. `edge-right`, `edge-top` 등 세부 경계식이 필요하면 공식 문서 또는 실제 예제 코드를 추가해야 한다.

---

## 4.2 Example: Beacon (1.0)

Beacon은 아군 기물이 어디에 있든 자리를 바꿀 수 있어야 하므로 체스판 전체를 탐색해야 한다.

공식 문서의 Beacon 예시 코드:

```chessembly
piece(beacon) do
  anchor(0, -1)
  {
    do anchor(-1, 0) while
    friendly(0, 0) jne(0)
    piece-on(pawn, 0, 0) jmp(0)
    piece-on(beacon, 0, 0) jmp(0)
    shift(0, 0) label(0)
    do
      friendly(1, 0) jne(2)
      piece-on(pawn, 1, 0) jmp(2)
      piece-on(beacon, 1, 0) jmp(2)
      shift(1, 0) jmp(1) label(2)
      anchor(1, 0)
      label(1)
    while
  }
while;
piece(beacon) do
  {
    do anchor(-1, 0) while
    friendly(0, 0) jne(0)
    piece-on(pawn, 0, 0) jmp(0)
    piece-on(beacon, 0, 0) jmp(0)
    shift(0, 0) label(0)
    do
      friendly(1, 0) jne(2)
      piece-on(pawn, 1, 0) jmp(2)
      piece-on(beacon, 1, 0) jmp(2)
      shift(1, 0) jmp(1) label(2)
      anchor(1, 0)
      label(1)
    while
  }
  anchor(0, 1)
while;
```

### absolute 식 사용

공식 문서에는 Beacon 탐색을 위해 `absolute-y`와 `absolute-x`를 사용하는 버전도 제시되어 있다.

```chessembly
piece(beacon)
absolute-y(0)
do
  absolute-x(0)
  do
    friendly(0, 0)
    jne(0)
    piece-on(beacon, 0, 0)
    jmp(0)
    piece-on(pawn, 0, 0)
    jmp(0)
    shift(0, 0)
    label(0)
    anchor(1, 0)
  while
  anchor(0, 1)
while;
```

---

# 5. Reference

Reference는 Chessembly의 모든 식과 핵심 규칙을 요약한 문서다.

## 5.1 식의 값 규칙

1. `true`, 계속: 식이 성공하면 `true`를 반환하고, 식 연쇄는 다음 식을 실행한다.
2. `false`, 종료: 일반 식이 `false`를 반환하면, 식 연쇄 전체가 종료된다.
3. 예외: `while`, `jmp`, `jne`, `not`, `label`, `true`, `false`, `read`, `read-and`, `read-or`, `read-xor`, `write`는 `false`를 받아도 연쇄를 종료시키지 않는다.

---

## 5.2 행마식 Movement Expressions

행마식은 칸을 활성화하고 기준 위치를 이동시킨다.

| 식 | 대상: 빈 칸 | 대상: 적 기물 | 대상: 아군/벽 |
|---|---|---|---|
| `move` | 활성화, 기준 위치 이동, `true` | `false`, 종료 | `false`, 종료 |
| `take` | 기준 위치 이동, `true` | 활성화, 기준 위치 이동, `true` | `false`, 종료 |
| `take-move` | 활성화, 기준 위치 이동, `true` | 활성화, 기준 위치 이동, `false`, 종료 | `false`, 종료 |
| `catch` | 기준 위치 이동, `true` | 활성화, 기준 위치 이동, `true` | `false`, 종료 |
| `jump` | 활성화, 기준 위치 이동, `true` | `false`, 종료 | `false`, 종료 |
| `shift` | 활성화, 기준 위치 이동, `true` | 활성화, 기준 위치 이동, `true` | 아군인 경우 활성화, 기준 위치 이동 및 `true`; 벽인 경우 `false` |

주의: 이 표의 `catch`는 Tutorial의 “기물은 제자리에 머무른 채 해당 칸의 적 기물만 잡는다”는 설명과 함께 해석해야 한다. 인터프리터 구현 시 실제 기물 이동과 기준 위치 이동을 구분해야 할 수 있다.

---

## 5.3 제어식 Control Expressions

제어식은 식 연쇄의 실행 흐름, 즉 어떤 식이 다음에 실행될지를 직접 제어한다.

| 식 | 직전 값이 false일 때 | 반환 값 | 설명 |
|---|---|---|---|
| `repeat(n)` | 연쇄 종료 | 직전 값 | `true`일 때만 `n`칸 뒤로 점프한다. |
| `{ ... }` | 연쇄 종료 | 블록 마지막 값 | `false`를 격리하고 기준 위치를 복원한다. Y자 행마, 템페스트-룩 등에 쓰인다. |
| `end` | 해당 없음 | 없음 | `{}` 블록 안에서도 식 연쇄를 무조건 종료한다. |
| `do` | 연쇄 종료 | `true` | `while`과 쌍을 이루는 루프의 시작점. 일반 식이다. |
| `while` | 연쇄 계속 | `true` | 예외식. `true`일 때만 `do`로 점프한다. |
| `label(n)` | 연쇄 계속 | 직전 값 | 예외식. `jmp`/`jne`의 목적지. 직전 값을 그대로 전달한다. |
| `jmp(n)` | 연쇄 계속 | `true` | 예외식. `true`일 때만 `label(n)`으로 점프한다. |
| `jne(n)` | 연쇄 계속 | `true` | 예외식. `false`일 때만 `label(n)`으로 점프한다. |
| `not` | 연쇄 계속 | `!(직전 값)` | 예외식. `true`를 `false`로, `false`를 `true`로 뒤집는다. |
| `true` | 연쇄 계속 | `true` | 예외식. 직전 값과 상관없이 `true`를 반환한다. |
| `false` | 연쇄 계속 | `false` | 예외식. 직전 값과 상관없이 `false`를 반환한다. |
| `read(n)` | 연쇄 계속 | `bits[n]` | 예외식. 직전 값과 상관없이 n번째 비트에 저장된 값을 반환한다. 단, `0 <= n < 16`. |
| `read-and(n)` | 연쇄 계속 | `bits[n] & 직전 값` | 예외식. n번째 비트와 직전 값의 AND를 반환한다. 단, `0 <= n < 16`. |
| `read-or(n)` | 연쇄 계속 | `bits[n] | 직전 값` | 예외식. n번째 비트와 직전 값의 OR를 반환한다. 단, `0 <= n < 16`. |
| `read-xor(n)` | 연쇄 계속 | `bits[n] ^ 직전 값` | 예외식. n번째 비트와 직전 값의 XOR를 반환한다. 단, `0 <= n < 16`. |
| `write(n)` | 연쇄 계속 | `true` | 예외식. n번째 비트에 직전 값을 저장한다. 단, `0 <= n < 16`. |

---

## 5.4 조건식 Conditional Expressions

조건식은 칸을 활성화하지 않고, 엿보기를 통해 `true` 또는 `false`만 반환한다. 모두 일반 식이므로 `false` 반환 시 연쇄가 종료된다.

- `peek(dx, dy)`: `(dx, dy)`가 비어 있으면 `true`를 반환하고, 기준 위치도 `(dx, dy)`만큼 이동한다.
- `observe(dx, dy)`: `(dx, dy)`가 비어 있으면 `true`를 반환한다. 기준 위치는 이동하지 않는다.
- `enemy(dx, dy)`: `(dx, dy)`에 적이 있으면 `true`를 반환한다.
- `friendly(dx, dy)`: `(dx, dy)`에 아군이 있으면 `true`를 반환한다.

Reference의 조건식 목록은 추출본에서 여기까지 확인되었다. Controls의 Look around 섹션에서는 추가로 `piece-on`, `danger`, `check`, `bound`, `edge`, `corner`를 설명한다.

---

# 6. 공식 문서 기반 인터프리터 구현 시 반드시 보존해야 할 의미

이 절은 구현 지시가 아니라, 위 공식 문서 내용에서 직접 도출되는 보존 사항이다.

## 6.1 식 연쇄

- 세미콜론 `;`은 독립된 식 연쇄의 종료를 의미한다.
- 각 식 연쇄의 시작 기준 위치는 현재 기물 위치다.
- 식 연쇄 내부에서는 행마식 실행에 따라 기준 위치가 계속 이동한다.
- 식 연쇄가 끝나면 다음 식 연쇄는 다시 현재 기물 위치에서 시작한다.

## 6.2 반환값

- 식은 `true` 또는 `false`를 반환한다.
- 일반 식이 `false`를 반환하면 해당 식 연쇄가 종료된다.
- 예외 제어식은 `false`를 받아도 식 연쇄를 종료시키지 않는다.

## 6.3 블록

- `{ }`는 false 격리와 기준 위치 복원을 수행한다.
- 블록 안의 액션 생성 자체는 유지되어야 한다.
- 블록이 끝나면 기준 위치는 블록 시작 전 위치로 복원된다.

## 6.4 행마식

- 행마식은 칸을 활성화하고 기준 위치를 이동시킨다.
- `move`, `take`, `take-move`, `catch`, `jump`, `shift`는 Reference 표의 대상별 동작을 따른다.
- `catch`는 Tutorial에서 원거리 공격으로 설명된다.
- `jump`는 `jmp`/`jne`과 다른 행마식이다.
- `jump`는 앞선 `take`와 결합해 `take-jump`처럼 동작한다.

## 6.5 상태식

- `piece`와 `if-state`는 상태를 읽는 조건식이다.
- `transition`과 `set-state`는 이후 활성화될 칸에 액션 태그를 붙이는 수식어다.
- 인자 없는 `set-state`는 마지막 액션 태그를 하나 지운다.

## 6.6 조건식

- 조건식은 칸을 활성화하지 않고 `true`/`false`만 반환한다.
- `peek`은 예외적으로 성공 시 기준 위치도 이동한다.
- 조건식은 일반 식이므로 `false` 반환 시 식 연쇄가 종료된다.

---

# 7. 문서 확인 필요 / 추출본 한계

다음 항목은 공식 문서에 존재하거나 예제에서 사용되는 것으로 확인되지만, 이 오프라인 추출본만으로는 세부 의미를 완전히 확정하기 어렵다.

- `anchor(dx, dy)`의 정확한 반환값과 실패 조건
- `absolute-x(n)`, `absolute-y(n)`의 정확한 반환값과 실패 조건
- `edge-right`, `edge-left`, `edge-top`, `edge-bottom` 등 세부 경계식의 정확한 목록과 의미
- Bouncing-Bishop 예제의 전체 최종 코드
- `catch`의 “기준 위치 이동”과 실제 기물 위치 유지의 인터프리터 내부 모델링 방식
- `shift`가 빈 칸을 활성화한다는 Reference 표의 의미와 실제 자리 바꾸기 규칙의 세부사항
- `danger`와 `check`의 공격 판정 세부 방식

구현자는 위 항목을 임의로 확정하지 말고, 프로젝트 기존 구현 또는 공식 문서를 추가 확인해야 한다.