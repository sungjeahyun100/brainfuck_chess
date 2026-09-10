# Deck Chess — Standard Format Implementation Plan

> 목적: Codex Desktop의 Goal mode와 Worktree를 이용해 Standard 포맷을 단계적으로 구현하기 위한 기준 문서.
>
> 이 문서는 **전체 목표와 현재 확정된 규칙, 작업 분할, 의존성, 각 Goal의 완료 조건**을 기록한다.
> Codex는 각 작업을 시작하기 전에 이 문서와 프로젝트의 `AGENTS.md`를 읽어야 한다.

---

## 0. 핵심 원칙

### 0.1 이번 작업의 최상위 목표

기존 Deck Chess의 동작을 **Legacy** 룰셋으로 보존하면서,
새로운 **Standard** 룰셋을 추가한다.

보드 크기와 룰셋은 서로 독립적인 설정이다.

예:

- 8×8 + Legacy
- 8×8 + Standard
- 10×10 + Legacy
- 10×10 + Standard
- 12×12 + Legacy
- 12×12 + Standard

### 0.2 호환성 원칙

- 기존 덱과 기존 게임이 깨지지 않아야 한다.
- 기존 데이터에 ruleset 정보가 없다면 기본적으로 Legacy로 취급하는 방향을 우선 검토한다.
- Standard를 구현하기 위해 Legacy 로직을 불필요하게 재작성하지 않는다.
- 새 규칙은 가능한 한 ruleset 경계를 통해 분리한다.
- 클라이언트 입력을 권위 있는 게임 상태로 신뢰하지 않는다.
- 랜덤 드로우와 특수 소환의 최종 판정은 서버가 담당한다.
- 한 Goal에서는 그 Goal에 필요한 범위만 수정한다.
- 이후 Goal에서 해결할 문제를 현재 Goal에서 임의로 확장 구현하지 않는다.

---

# 1. 확정된 포맷 구조

## 1.1 룰셋

현재 룰셋은 두 종류다.

- `Legacy`
- `Standard`

내부적으로는 UI 표시 문자열과 독립적인 안정적인 ID를 사용한다.

권장 예:

```text
legacy
standard
```

덱 빌더에서는 보드 선택과 룰 선택을 서로 분리한다.

예:

```text
보드 크기          룰
[ 8×8 ▼ ]        [ Standard ▼ ]
```

---

# 2. Standard — 현재 확정된 규칙

## 2.1 진영 범위

**G2 확정 (2026-09-10):** 새 진영은 초기 배치뿐 아니라 Standard의 모든 Base/Home 규칙(Drop, 진영 참조 능력, 폭격기 착륙/귀환, 탄약 회복)에 적용한다. Legacy는 기존 진영을 유지한다.

0-based 보드 크기 `n`에서 `w = 짝수 2 / 홀수 3`, `s = (n-w)/2`, `e = s+w-1`이다. White Back은 `rank=0, file=s..e`; White Front는 `(s-1,0)`, `(e+1,0)` 및 `rank=1, file=s-1..e+1`이다. Black은 각 rank를 `n-1-rank`로 반전한다. Base/Home은 두 영역의 합집합이다. Front 전 칸을 Front 기물로 채워야 게임 가능한 덱이며 Back 전 칸 채움은 요구하지 않는다. 일반 Drop은 새 Base ∪ 기존 Attack Map이다. Map/Terrain과 이 좌표 공식은 독립이다.

구행/폭격기의 Extra-only는 G4에서 적용한다. G2 단계에서는 기존 Back 정의와 Pocket 사용을 유지한다.

### 짝수 크기 보드

- 중앙의 2칸이 기존 뒷줄 기물이 들어갈 영역이다.
- 그 중앙 2칸을 감싸는 칸들이 기존 앞줄 기물이 들어갈 영역이다.

### 홀수 크기 보드

- 중앙의 3칸이 기존 뒷줄 기물이 들어갈 영역이다.
- 그 중앙 3칸을 감싸는 칸들이 기존 앞줄 기물이 들어갈 영역이다.

구현 시 특정 보드 크기를 하드코딩하지 말고,
보드 크기로부터 배치 영역을 계산할 수 있는 일반화된 방식으로 설계한다.

Standard에서 Extra Deck 전용으로 지정된 기존 뒷줄 기물은
초기 배치 영역에 배치할 수 없다.

---

## 2.2 손패와 착수

Standard에서는 Pocket과 Hand를 구분한다.

게임 시작 시 각 플레이어는:

1. 자신의 Pocket에서 기물 3개를 무작위로 뽑는다.
2. 뽑힌 기물을 Hand로 이동시킨다.

각 플레이어는 자신의 턴 시작 시:

1. Pocket에서 기물 1개를 무작위로 뽑는다.
2. 뽑힌 기물을 Hand로 이동시킨다.

Standard에서는 그 턴에 Hand에 존재하는 기물만 착수할 수 있다.

즉 기본 흐름은:

```text
Pocket
  ↓ Draw
Hand
  ↓ Drop
Board
```

Legacy의 기존 Pocket 착수 방식은 그대로 보존한다.

랜덤 결과는 서버가 결정해야 한다.

**G3 분할 확정 (2026-09-10):** G3-A는 명시적 runtime Hand, Standard 일반 Drop의 Hand 출처, 서버의 상대 Hand 비공개 투영만 구현한다. Draw/RNG는 G3-B에서 구현하며 초기 3기와 실제 턴 시작 1기를 분리한다. White/Black 모두 첫 실제 턴에 +1 Draw가 있다. Hand 상한은 없고 Pocket이 비면 정상 진행한다. 자기 Hand는 내용을, 상대 Hand는 장수만 공개한다. 기존 Pocket 조작 Ability는 Standard에서도 Pocket을 직접 사용하며, Pocket 복귀는 자동 Hand 이동을 일으키지 않는다. 저장 덱에는 Hand를 추가하지 않는다. G3-C/G7은 Draw 기록/재현 및 완료 기록의 공개 정책을 다룬다.

---

**G3-C 확정 (2026-09-10):** 기존 기록 접근 정책으로 열람이 허용된 완료 Replay에서는 양측 Hand 및 initial/turn Draw의 실제 PieceId를 공개한다. 진행 중 Hand/Pocket privacy와 Extra 공개 처리는 그대로 유지한다. Draw는 player action이 아닌 automatic transition이다. Standard Analysis preview는 RNG를 소비하지 않으며 필요한 Draw는 `draw_pending: true`로 표시하고 final hash를 생략한다. 분기 저장의 멱등성/버전 검사 이후에만 서버 Draw를 한 번 확정하고 `AnalysisNode.draws`와 최종 state/hash를 함께 저장한다. 재조회와 Replay는 확정 resolution을 재적용하며 RNG를 실행하지 않는다. 결과와 검증은 `standard-format-g3c-implementation.md`에 기록한다.

---

## 2.3 Extra Deck

Standard에는 Main Deck과 별도의 Extra Deck이 존재한다.

Extra Deck의 기본 규칙:

- 최대 3기의 기물만 넣을 수 있다.
- Extra Deck의 기물은 기본적으로 Main Deck의 기물 점수 상한에 포함되지 않는다.
- 그러나 Extra Deck 기물도 자신의 기물 점수(piece score)를 가진다.
- 이 점수는 특수 소환 시 필요한 제물 가치 계산에 사용된다.
- Extra Deck의 기물은 자신의 기물을 제물로 바쳐 필드에 특수 소환할 수 있다.
- 아무 별도 설명이 없는 경우 제물은 필드 위의 자신의 기물을 기준으로 한다.
- 기물에 따라 Hand / Pocket / Board 중 허용되는 제물 위치를 별도로 지정할 수 있는 구조여야 한다.
- 제물 가치의 합은 소환 대상 기물의 기존 score 이상이어야 한다 (`>=`). 선택한 초과 제물도 전부 제거하며 환급하지 않는다.
- Extra Deck의 기물 최대 개수는 현재 기준으로 `3종류`가 아니라 `3기`다.

Extra Deck의 기물은 Main Deck과 별도의 zone으로 취급한다.

---

**G5 확정 (2026-09-10):** Extra는 양측 공개 zone이고 동일 타입 여러 인스턴스를 총 3기까지 허용한다. 구행은 자기 Hand/Board, 폭격기는 자기 Board의 non-King 기물만 제물로 사용한다. 중복 PieceId/상대/잘못된 zone은 거부한다. `ExtraSummon` 하나로 선택 목록 전체를 제거하고 Extra→Board를 원자 적용한다. 소환 후 해당 ID는 Extra에서 제거되며 포획·Pocket 복귀·Draw로 자동 복원되지 않는다. 소환 위치는 일반 Standard Drop의 Base ∪ Attack Map 및 target 규칙을 재사용한다. 실제 턴 전환 후에만 기존 Draw를 적용한다. G5에는 최소 게임/분석 소환 UI와 exact Record/Replay/Analysis를 포함하며 전체 HUD는 G6, Deck Code는 G7, Bot 소환 전략은 G8이다.

## 2.4 현재 Extra Deck 전용 기물

### 구행

- 기존 분류: 뒷줄 기물
- Standard에서는 Extra Deck 전용
- 제물 가능 위치: Hand / Board
- 초기 배치 불가

### 폭격기

- 기존 분류: 뒷줄 기물
- Standard에서는 Extra Deck 전용
- 제물 가능 위치: Board
- 초기 배치 불가

---

# 3. 아직 확정하지 않은 규칙

아래 항목은 구현 과정에서 Codex가 임의로 결정하지 않는다.

필요한 Goal에 도달했을 때 사용자에게 결정이 필요하다고 보고하거나,
코드 구조상 여러 선택지를 지원할 수 있도록 확장 지점만 만든다.

- Pocket이 비었을 때의 정확한 UI 동작
- 포켓 제물을 사용하는 기물의 구체적 규칙
- Standard 전용 봇 평가 방식

현재 구현에 반드시 필요한 경우 기존 확정 규칙과 충돌하지 않는 최소 구조만 만든다.

---

# 4. Goal 의존성

전체 작업은 다음 순서로 진행한다.

```text
G0  기존 구조 분석
 ↓
G1  Ruleset / Board Size 분리
 ↓
├── G2  Standard 배치 규칙
├── G3  Hand / Draw
└── G4  Extra Deck 데이터 모델 + 덱 빌더
         │
G3 ──────┤
         ↓
G5  Extra Summon 엔진
 ↓
G6  Standard 게임 UI
 ↓
G7  저장 / API / 대국 기록 / 복기 호환
 ↓
G8  Bot / Challenge 대응
```

G2, G3, G4는 G1이 안정적으로 병합된 뒤에는 가능한 한 독립 작업으로 취급한다.
서로 같은 파일을 크게 수정하지 않는다는 것이 확인되면 별도 Worktree에서 병렬 실행할 수 있다.

---

# 5. Goal 0 — 기존 구조 분석

## 목적

Standard 구현에 앞서 현재 코드에서 관련 데이터와 로직이 어디를 통과하는지 정확히 파악한다.

## 이 Goal에서는 코드 수정 금지

코드를 분석하고 문서만 작성한다.

## 조사 대상

최소한 다음을 추적한다.

1. Deck 데이터 모델
2. Board size 저장 위치
3. 덱 빌더의 보드 선택 UI
4. 보드 크기에 따른 점수 상한
5. 앞줄 / 뒷줄 기물 분류
6. 초기 배치 validation
7. 덱 validation의 진입점
8. 덱 저장 / 로딩
9. Deck Code serialize / deserialize
10. 게임 생성 요청
11. 서버의 덱 재검증 여부
12. GameState 생성
13. Pocket 표현 방식
14. Drop action 표현 방식
15. 턴 전환 / 턴 시작 처리
16. RNG가 이미 존재하는지와 사용 위치
17. canonical TurnAction / history
18. GameRecord
19. Replay
20. Bot이 Board / Pocket을 평가하는 위치
21. Challenge가 덱 또는 게임 포맷을 생성하는 방식

## 산출물

`docs/standard-format-codebase-analysis.md`를 작성한다.

문서에는 다음을 포함한다.

- 관련 파일 목록
- 각 파일의 책임
- 데이터 흐름
- Standard 구현 시 수정 가능성이 높은 부분
- G1~G8별 예상 수정 파일
- G2/G3/G4 병렬 작업 시 충돌 가능성이 높은 파일
- 기존 테스트 위치
- 부족한 테스트 영역
- 위험 요소 / 하위호환성 위험
- 현재 구조를 최대한 유지하면서 구현할 권장 방향

## 완료 조건

- 실제 코드 수정이 없어야 한다.
- 분석 문서만 변경되어야 한다.
- G1을 바로 시작할 수 있을 만큼 데이터 흐름이 구체적이어야 한다.
- 추측과 실제 코드에서 확인한 사실을 구분해서 기록해야 한다.

---

# 6. Goal 1 — Board Size와 Ruleset 분리

## 목적

기존 보드 선택과 게임 규칙 개념을 분리하고
Legacy / Standard 룰셋을 표현할 기반을 만든다.

## 구현 범위

- Ruleset 타입 추가
- `Legacy`
- `Standard`
- Deck 또는 적절한 format 데이터에 ruleset 저장
- Board size와 ruleset의 독립 표현
- 덱 빌더에서 Board selector 옆에 Ruleset selector 추가
- 기존 저장 데이터의 Legacy 하위호환
- 필요한 serialize / deserialize 기반 수정
- validation이 ruleset을 받을 수 있는 확장 지점 마련

## 이번 Goal에서 하지 않을 것

- Standard 새로운 배치 규칙
- Hand
- Draw
- Extra Deck
- Extra Summon
- Standard 게임 UI

Standard를 선택할 수는 있지만 아직 실제 Standard 규칙을 구현하지 않는다.

## 완료 조건

가장 중요한 완료 조건:

**Legacy에서의 게임 동작이 작업 전과 동일해야 한다.**

추가로:

- 기존 덱을 읽을 수 있다.
- 기존 덱은 Legacy로 동작한다.
- UI에서 Board Size와 Ruleset을 별도로 선택할 수 있다.
- 보드 크기를 바꿔도 ruleset이 암묵적으로 바뀌지 않는다.
- ruleset을 바꿔도 board size가 암묵적으로 바뀌지 않는다.
- 관련 테스트가 통과한다.

---

# 7. Goal 2 — Standard Base/Home Zone 및 배치 규칙

## 목적

Standard에서 새로운 중앙 Base/Home을 초기 배치와 모든 기존 진영 기반 규칙에 사용한다.

## 구현 범위

- 짝수 보드 중앙 2칸 계산
- 홀수 보드 중앙 3칸 계산
- Backline 배치 영역 계산
- Frontline 배치 영역 계산
- Standard Front 전 칸 채움 validation, 좌표 기반 DeckEditor 및 최소 유효 초기 배치
- 일반 Drop/박격포/폭격기 귀환·착륙/탄약의 새 Base 연결
- Legacy validation과 runtime Base/Home 보존
- 여러 보드 크기에 대한 테스트

## 이번 Goal에서 하지 않을 것

- Hand
- Draw
- Extra Deck 저장 구조
- Extra Summon

Extra-only 기물이 아직 데이터 모델에 없으면
G4와 충돌하지 않는 확장 지점까지만 만들고 임의로 큰 구조를 추가하지 않는다.

## 완료 조건

최소 테스트:

- 8×8
- 9×9
- 10×10
- 11×11
- 12×12

에서 양 진영의 정확한 Back/Front/Base 집합과 rank mirror, 배치 영역이 기대한 위치로 계산된다.

Legacy의 기존 배치 영역은 변하지 않는다.

---

# 8. Goal 3 — Hand / Draw

G3-A/B/C로 나누어 구현한다. G3-A 결과 및 현재 서버 공개 계약은 `standard-format-g3a-implementation.md`를 참조한다. 아래 Draw 완료 조건은 G3 전체의 조건이며 G3-A에 적용하지 않는다.

## 목적

Standard에 Pocket → Hand → Board 흐름을 추가한다.

## 구현 범위

- Hand zone 추가
- 게임 시작 3기 draw
- 턴 시작 1기 draw
- Standard에서는 Hand 기물만 Drop 허용
- Legacy에서는 기존 Pocket Drop 유지
- 서버 authoritative RNG
- 빈 Pocket 처리
- 필요한 테스트

## 설계 원칙

랜덤 결과를 클라이언트가 지정하지 않는다.

가능하면 랜덤 결과가 이후 Replay에서 재현될 수 있도록
이벤트 또는 authoritative state 기록과 연결할 수 있는 구조를 마련한다.
단, GameRecord 전체 통합은 G7에서 수행한다.

## 완료 조건

- Standard 게임 시작 시 양측 Hand가 초기 draw 결과를 가진다.
- Standard 턴 시작 draw가 서버에서 실행된다.
- Standard에서 Pocket 직접 Drop이 거부된다.
- Standard에서 Hand → Board Drop이 가능하다.
- Legacy Pocket Drop은 기존처럼 동작한다.

---

# 9. Goal 4 — Extra Deck 데이터 모델 + 덱 빌더

## 목적

특수 소환 로직 없이 Extra Deck을 구성하고 저장할 수 있게 한다.

## 구현 범위

- Main Deck / Extra Deck 구분
- Extra Deck 최대 3기 validation
- Extra Deck 기물의 score 유지
- Extra Deck 기본 deck-limit cost 제외
- Extra Deck 전용 기물 표현
- 구행: Extra-only
- 폭격기: Extra-only
- Standard 덱 빌더의 Extra Deck 영역
- 덱 저장 / 로드에 필요한 모델 변경
- Legacy에서 Extra Deck 사용 금지

## 이번 Goal에서 하지 않을 것

- 제물 선택
- 특수 소환 action
- 게임 중 Extra Deck UI
- 구행 / 폭격기의 실제 소환

## 완료 조건

- Standard 덱에서 Extra Deck을 최대 3기까지 구성 가능
- 4기째는 validation 실패
- 구행 / 폭격기를 Standard 초기 배치에 넣을 수 없음
- Extra Deck의 점수는 표시/데이터로 존재하지만 Main Deck 점수 상한에는 기본적으로 포함되지 않음
- Legacy 동작에는 영향 없음

---

# 10. Goal 5 — Extra Summon 엔진

## 목적

Extra Deck의 기물을 제물 비용을 지불하고 Board로 특수 소환하는 서버 권위 action을 구현한다.

## 필요한 개념

- ExtraSummon action
- Sacrifice source
- Sacrifice validation
- Summon target validation
- Atomic apply

제물 위치는 최소 다음 zone을 표현할 수 있어야 한다.

```text
Board
Hand
Pocket
```

현재 기물:

```text
구행: Hand / Board
폭격기: Board
```

별도 지정이 없는 Extra Summon 기물의 기본 제물 위치는 Board다.

## validation

최소한 다음을 검사한다.

- 해당 Extra 기물이 실제 자신의 Extra Deck에 존재하는가
- 제물이 실제 존재하는가
- 제물이 자신의 기물인가
- 제물 zone이 허용되는가
- 같은 기물을 중복 선택하지 않았는가
- 제물 가치가 요구 비용을 충족하는가
- target이 합법적인가
- 모든 validation이 통과하기 전에 GameState를 부분 변경하지 않는가

## 완료 조건

제물 제거와 Extra 기물의 소환이 하나의 원자적 action으로 처리된다.

실패한 요청은 GameState를 부분적으로 변경하지 않는다.

---

# 11. Goal 6 — Standard 게임 UI

## 목적

G2~G5에서 구현된 Standard 규칙을 플레이어가 게임 화면에서 사용할 수 있게 한다.

## 구현 범위

- Hand UI
- Pocket 표시
- Extra Deck UI
- Extra 기물 선택
- 제물 선택 mode
- 현재 선택한 제물 가치 / 필요 가치 표시
- 소환 target 선택
- 취소 / 복귀 처리
- 서버 validation 오류 표시
- Legacy UI 보존

상대 Hand 공개 범위처럼 아직 확정되지 않은 게임 규칙은 임의로 결정하지 않는다.

---

# 12. Goal 7 — 저장 / API / 대국 기록 / 복기

## 목적

Standard를 기존 저장 및 기록 시스템에 완전히 통합한다.

## 검토 대상

- Deck serialization
- Deck Code
- 서버 API
- Game creation payload
- GameRecord
- Replay
- ruleset versioning
- Draw 결과
- ExtraSummon
- Sacrifice 목록
- state hash / authoritative state

## 핵심 조건

기존 Legacy Deck Code와 기존 GameRecord를 읽을 수 있어야 한다.

새로운 Standard 게임은 replay 시 서버가 실제 게임에서 결정한 draw 및 summon 결과를 동일하게 재현할 수 있어야 한다.

---

**G7 동결 (2026-09-10):** Standard 및 Extra가 남은 Legacy 초안은 DC4로 공유하고, 빈 Extra의 Legacy 출력은 기존 DC3를 유지한다. Standard의 첫 stable semantic record version은 `deck-chess-standard-1`, Legacy는 `deck-chess-1`이다. 개발 단계 Standard+Legacy-version 및 unknown version은 Replay/Analysis 전에 거부한다. 저장 DB format_version=1, GameRecord format_version=2, snapshot_version=1은 유지한다. 구현·호환성 표·검증은 `standard-format-g7-implementation.md`를 참조한다.

# 13. Goal 8 — Bot / Challenge

## 목적

Standard 게임이 안정화된 뒤 봇과 Challenge를 대응시킨다.

## 이 Goal 전에는

Standard를 PvP 위주로 먼저 완성해도 된다.
Standard 안정화 전에 Bot을 억지로 맞추지 않는다.

## 향후 봇이 고려해야 할 정보

- Board value
- Hand value
- Pocket expected value
- Extra Deck value
- Draw 기대값
- Extra Summon 가능성
- 제물의 opportunity cost
- 소환 후 가치

---

**G8-B 동결 (2026-09-10):** ChallengeDefinition은 ruleset/canonical map/Starting/Pocket/Extra를 명시한다. 기존 tempest_horde/raining_men/tempest_set은 기존 콘텐츠 그대로 Legacy이며 공개 Standard Challenge를 추가하지 않는다. Standard는 test-only definition으로 production 생성/Draw/fair Bot/ExtraSummon/privacy/Record/Replay/Analysis/clear를 검증한다. 결과와 G1~G8 최종 회귀 및 배포 전 잔여 작업은 `standard-format-g8b-implementation.md`에 기록한다.

# 14. Worktree 운영 규칙

G1은 먼저 단독으로 완료하고 검증한다.

G1이 병합된 뒤,
G2 / G3 / G4를 분석 결과상 충돌이 적다면 별도 Worktree에서 병렬로 실행할 수 있다.

예:

```text
Worktree A → G2 Standard Deployment
Worktree B → G3 Hand / Draw
Worktree C → G4 Extra Deck
```

각 작업은 반드시 동일한 G1 완료 커밋을 base로 시작한다.

두 Worktree가 같은 핵심 파일을 크게 수정하게 된다는 것이 G0 분석에서 확인되면
무리해서 병렬화하지 않는다.

G5는 G3와 G4가 모두 통합된 뒤 시작한다.

---

# 15. 각 Goal에서 Codex가 따라야 할 공통 절차

각 Goal을 시작할 때:

1. `AGENTS.md`를 읽는다.
2. 이 문서를 읽는다.
3. 해당 Goal에 필요한 기존 코드를 다시 확인한다.
4. 현재 Goal의 범위를 요약한다.
5. 범위 밖 변경을 피한다.
6. 구현한다.
7. 관련 테스트를 추가/수정한다.
8. 테스트 / lint / build를 실행한다.
9. diff를 자체 검토한다.
10. 완료 조건과 실제 결과를 비교한다.
11. 다음 Goal에서 해야 할 일을 현재 Goal에 섞지 않는다.

마지막 보고에는 최소한 다음을 포함한다.

- 변경한 파일
- 핵심 설계
- 테스트 결과
- 남은 위험
- 다음 Goal에 영향을 주는 사항
- 사용자가 직접 확인해야 할 항목

---

# 16. Codex에게 주는 Goal 프롬프트 형식

각 Goal은 이 문서 전체를 프롬프트에 다시 복사하지 않는다.

프로젝트에 이 파일이 존재한다는 전제에서 다음처럼 지시한다.

예: G0

```text
AGENTS.md와 docs/STANDARD_FORMAT_IMPLEMENTATION_PLAN.md를 먼저 읽어라.

이번 Goal에서는 문서의 "Goal 0 — 기존 구조 분석"만 수행하라.
코드는 수정하지 말고, 실제 코드베이스를 조사해
docs/standard-format-codebase-analysis.md를 작성하라.

완료 기준은 해당 문서의 Goal 0 섹션을 그대로 따른다.
추측과 코드에서 확인한 사실을 구분하고,
G1~G8별 예상 수정 파일과 G2/G3/G4의 병렬 작업 충돌 가능성을 반드시 분석하라.
```

예: G1

```text
AGENTS.md,
docs/STANDARD_FORMAT_IMPLEMENTATION_PLAN.md,
docs/standard-format-codebase-analysis.md를 먼저 읽어라.

이번 Goal에서는 "Goal 1 — Board Size와 Ruleset 분리"만 수행하라.
Standard의 실제 게임 규칙은 아직 구현하지 마라.

가장 중요한 성공 조건은 Legacy의 동작을 변경하지 않는 것이다.
구현 후 관련 테스트, lint, build를 실행하고
Goal 1의 완료 조건과 결과를 대조해 보고하라.
```

---

# 17. 문서 유지 규칙

사용자가 게임 규칙을 새로 결정했다면
구현 프롬프트에만 남기지 말고 이 문서도 함께 갱신한다.

특히 다음은 이 문서에 기록한다.

- 규칙 변경
- Goal 범위 변경
- 새로운 의존성
- 기존 결정의 폐기
- 새 포맷의 이름/ID
- 하위호환성 정책

단, 실제 코드 구조 분석 결과는
`docs/standard-format-codebase-analysis.md`에 기록한다.

이 문서는 **무엇을 만들 것인가**를 기록하고,
분석 문서는 **현재 코드가 어떻게 생겼는가**를 기록한다.
