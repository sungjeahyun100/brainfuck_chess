
# Goal 0 — AI 검색 Baseline 및 Benchmark 구축

## 목적

향후 Transposition Table, Iterative Deepening, Quiescence Search, Aspiration Window 등의 최적화가 실제로 성능을 개선하는지 비교할 수 있도록 현재 AI 검색의 기준 성능을 측정할 수 있는 benchmark 환경을 만든다.

현재 AI 동작을 변경하는 것이 이번 Goal의 목적이 아니다.

## 현재 확인할 코드

우선 다음 영역을 조사한다.

* `engine/src/ai/search.rs`
* `engine/src/ai/evaluate.rs`
* `engine/src/ai/move_ordering.rs`
* `engine/src/ai/types.rs`
* `engine/src/profiling.rs`
* `engine/tests/ai.rs`
* 합법 수 생성 및 action 적용 관련 코드

이미 profiling infrastructure가 있다면 새 시스템을 중복 구현하지 말고 이를 확장한다.

## 구현 요구사항

### 1. 대표 검색 포지션 준비

최소 다음 유형을 포함하는 AI benchmark position을 만든다.

1. 일반적인 중반 포지션
2. 캡처 후보가 많은 tactical position
3. 포켓에 여러 기물이 존재하여 drop branching factor가 큰 position
4. 특수능력을 사용할 수 있는 position
5. piece state 또는 cooldown을 가진 기물이 존재하는 position
6. 즉시 King capture가 가능한 position

가능하면 실제 게임 생성 API를 이용해 state를 구성한다.

테스트용 state 생성 과정이 지나치게 복잡하면 AI benchmark용 helper를 만들 수 있다.

### 2. 측정값

최소 다음 값을 측정 가능하게 한다.

* elapsed time
* searched nodes
* reached/completed depth
* legal move generation call count
* drop generation call count
* attack map generation call count
* Chessembly execution count

현재 profiling system에서 이미 제공되는 값은 그대로 재사용한다.

추가하기 쉬운 경우 다음도 기록한다.

* evaluation call count
* action application count

### 3. 반복 가능한 benchmark

가능하면 Rust benchmark 또는 별도의 ignored test 형태로 구현한다.

예:

`cargo test --features profiling ... -- --ignored --nocapture`

처럼 개발자가 쉽게 반복 실행할 수 있어야 한다.

CI에서 성능 숫자 자체를 엄격하게 assertion해서는 안 된다.

환경마다 실행 시간이 다르므로 correctness test와 benchmark를 분리한다.

### 4. 기준 결과 기록

benchmark 실행 시 각 position에 대해 읽기 쉬운 형태로 다음 정보를 출력한다.

* position name
* selected action
* score
* searched nodes
* depth
* elapsed
* profiling counters

## 하지 말아야 할 것

이번 단계에서는 다음을 구현하지 않는다.

* TT
* Iterative Deepening
* Quiescence
* Aspiration Window
* Killer heuristic
* History heuristic
* Evaluation tuning

현재 검색 결과가 바뀌는 대규모 리팩터링도 하지 않는다.

## 테스트

기존 AI 테스트가 모두 통과해야 한다.

추가로 benchmark state들이 다음을 만족하는지 검증한다.

* AI가 panic하지 않는다.
* legal action만 선택한다.
* benchmark가 정상적으로 끝난다.

## 완료 조건

다음을 만족하면 Goal을 완료한다.

1. 대표 benchmark position들이 존재한다.
2. 기존 profiling 정보와 AI search 정보를 함께 확인할 수 있다.
3. 이후 최적화 전후를 동일 position으로 비교할 수 있다.
4. 기존 게임 동작 및 AI correctness test가 유지된다.

작업 마지막에 다음을 보고한다.

* 추가/수정한 파일
* benchmark 실행 명령
* baseline 결과 요약
* 현재 가장 큰 비용으로 관찰된 부분
