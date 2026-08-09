# Goal 0 AI search baseline

## 실행 방법

Correctness 검증:

```sh
cargo test -p brainfuck-chess-engine --test ai_benchmark benchmark_positions_produce_legal_ai_decisions
```

반복 가능한 profiling baseline:

```sh
cargo test -p brainfuck-chess-engine --features profiling --test ai_benchmark ai_search_baseline -- --ignored --nocapture --test-threads=1
```

카운터가 process-global이므로 position별 delta가 다른 테스트의 영향을 받지 않도록 baseline은 단일 test thread로 실행한다. elapsed 값은 머신과 build profile에 따라 달라지므로 성능 회귀의 strict assertion으로 사용하지 않는다.

## 대표 포지션

동일한 position builder를 correctness test와 baseline test가 공유한다.

| 이름 | 대표 특성 |
| --- | --- |
| `middlegame` | 양측 주요 기물과 폰이 전개된 일반 중반 |
| `tactical-captures` | 여러 캡처 후보가 동시에 존재 |
| `drop-branching` | 포켓의 Queen, Rook, Bishop, Knight로 인한 drop 분기 |
| `standalone-ability` | Green Camp의 standalone ability 사용 가능 |
| `piece-state-cooldown` | Windmill state와 Cannon Rook cooldown 존재 |
| `immediate-king-capture` | 즉시 King capture 가능 |

## 기준 결과

2026-08-10 debug test profile, `BotDifficulty::Normal`에서 측정한 예시다. 정확한 elapsed 수치가 아니라 동일 환경에서의 전후 상대 비교를 위한 기준이다.

| position | nodes | depth | elapsed (ms) | 관찰된 특성 |
| --- | ---: | ---: | ---: | --- |
| middlegame | 207 | 2 | 187.96 | 일반 move branching과 반복 평가 |
| tactical-captures | 230 | 2 | 156.85 | legal generation/Chessembly 실행이 많은 편 |
| drop-branching | 343 | 2 | 152.50 | 가장 많은 node와 attack-map 호출 |
| standalone-ability | 187 | 2 | 51.75 | move와 standalone ability 후보를 함께 생성 |
| piece-state-cooldown | 174 | 2 | 47.03 | state predicate와 cooldown을 포함 |
| immediate-king-capture | 90 | 2 | 19.55 | 즉시 승리 후보가 존재하는 기준 |

각 실행은 selected action, score, searched nodes, reached/completed depth, elapsed와 다음 profiling counter를 한 줄로 출력한다.

* legal move generation calls
* drop generation calls
* attack map generation calls
* Chessembly executions
* evaluation calls
* action application calls

## 초기 관찰

호출 횟수 기준으로는 legal move generation 과정의 Chessembly 실행이 가장 반복적인 비용이다. 특히 drop branching position은 placement 검증 때문에 attack-map 생성도 크게 증가한다. 이는 시간 비율을 직접 측정한 결론이 아니라 baseline counter에 근거한 우선 조사 대상이며, 후속 최적화는 동일 position과 명령으로 전후를 비교해야 한다.
