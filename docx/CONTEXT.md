# Brainfuck Chess 웹 게임 개발 프롬프트

너는 웹 기반 전략 보드게임을 설계하고 구현할 수 있는 시니어 게임 개발자이자 룰 엔진 설계자다.

나는 변형 체스 웹 게임 **Brainfuck Chess**를 만들려고 한다.

Brainfuck Chess는 기본 체스의 기물과 보드게임 구조를 바탕으로 하지만, 일반 체스와는 다르게 다음 특징을 가진다.

* 확장 가능한 정사각형 보드
* 게임 시작 전 덱 구성
* 기본 진영 배치 기물과 포켓 기물의 분리
* 게임 중 포켓 기물 착수
* 한 턴에 여러 기물을 움직일 수 있는 이동 스택 시스템
* 체크 / 체크메이트 없음
* 상대 King을 실제로 포획하면 즉시 승리
* 기물의 이동 범위와 공격 범위는 Chessembly 기반으로 계산

이 프롬프트를 바탕으로 Brainfuck Chess의 게임 규칙, 데이터 구조, 룰 엔진, UI/UX, 테스트 전략, MVP 개발 순서를 설계해줘.

---

# 1. 게임 개요

게임 이름은 **Brainfuck Chess**다.

목표는 일반 체스보다 훨씬 많은 선택지와 수읽기를 제공하는 고난도 변형 체스 웹 게임을 만드는 것이다.

기본 체스의 기물 이름과 직관성은 유지하되, 덱 빌딩, 포켓 착수, 확장 보드, 다중 이동 시스템을 통해 전략 복잡도를 크게 높인다.

---

# 2. 보드 규칙

보드는 정사각형이다.

```text
n x n
n >= 8
```

* 최소 보드 크기는 `8x8`이다.
* 플레이어 선택에 따라 `9x9`, `10x10` 등 더 큰 보드로 플레이할 수 있다.
* 보드가 커질수록 플레이어가 사용할 수 있는 기물 점수 상한도 증가한다.

---

# 3. 덱 구성 규칙

플레이어는 게임 시작 전에 자신의 덱을 구성한다.

덱은 두 영역으로 나뉜다.

## 3.1 기본 진영 배치 기물

게임 시작 시 자신의 기본 진영에 미리 놓고 시작하는 기물이다.

기본 진영은 기본 체스에서 자기 기물들이 놓이는 영역에 해당한다.

추천 기본안:

* `8x8` 보드에서는 일반 체스처럼 각 플레이어의 첫 2개 랭크를 기본 진영으로 사용한다.
* `n x n` 보드에서도 각 플레이어의 첫 2개 랭크 전체를 기본 진영으로 사용한다.
* White 기본 진영: `rank 0`, `rank 1`
* Black 기본 진영: `rank n-2`, `rank n-1`

기본 진영 배치 규칙:

* 기본 진영 안의 빈 칸에만 배치할 수 있다.
* 각 플레이어는 반드시 King 1개를 기본 진영에 배치해야 한다.
* King은 포켓에 넣을 수 없다.
* King은 복수로 가질 수 없다.
* King은 덱 점수 계산에서 제외한다.

## 3.2 포켓 기물

포켓 기물은 게임 시작 시 보드 위에 없고, 게임 중 착수할 수 있는 기물이다.

착수란 포켓에 있는 기물을 보드 위의 빈 칸에 새로 놓는 행동이다.

포켓 규칙:

* 포켓 기물도 덱 점수 상한에 포함된다.
* 포켓 기물은 정해진 착수 가능 칸에만 놓을 수 있다.
* King은 포켓에 넣을 수 없다.

---

# 4. 덱 점수 상한

`8x8` 보드 기준 덱 점수 상한은 `39점`이다.

`n x n` 보드의 점수 상한은 다음 식으로 계산한다.

```text
scoreLimit = 39 + n * n - 8 * 8
```

정리하면 다음과 같다.

```text
scoreLimit = n * n - 25
```

예시:

```text
8x8   => 39점
9x9   => 56점
10x10 => 75점
```

기본 기물 점수 추천안:

```text
Pawn   = 1
Knight = 3
Bishop = 3
Rook   = 5
Queen  = 9
King   = 점수 계산 제외
```

커스텀 기물이 추가될 수 있으므로, 기물 점수는 `PieceDefinition`에 저장할 수 있게 설계해라.

---

# 5. 승리 조건

Brainfuck Chess에는 기본 체스의 체크와 체크메이트가 없다.

승리 조건은 다음 하나를 기본으로 한다.

```text
상대 King을 포획하면 즉시 승리한다.
```

규칙:

* King은 일반 기물처럼 공격받을 수 있다.
* King은 상대 공격 범위 안의 칸으로 이동할 수 있다.
* King이 공격받는 상태여도 불법 상태가 아니다.
* 상대가 실제로 King을 포획해야 승리한다.
* King이 포획되는 순간 게임은 즉시 종료된다.
* King을 포획한 플레이어가 승리한다.
* King을 잡은 뒤 같은 턴에 이동 스택이 남아 있어도 추가 행동은 하지 않는다.

사용하지 않는 개념:

* 체크
* 체크메이트
* 스테일메이트
* 체크 해소
* King이 공격받는 칸으로 이동 금지
* 자기 King을 노출시키는 이동 금지
* 착수로 체크를 막아야 하는 규칙

---

# 6. 착수 규칙

착수는 포켓에 있는 기물을 보드 위의 빈 칸에 새로 놓는 행동이다.

착수 가능한 칸은 다음 두 집합의 합집합이다.

```text
placementSquares = baseZoneSquares ∪ playerAttackMap
```

즉, 포켓 기물은 다음 중 하나에 해당하는 빈 칸에 착수할 수 있다.

1. 자신의 기본 진영 칸
2. 현재 자신의 기물들이 공격하거나 위협하는 칸

착수 제한:

* 이미 기물이 있는 칸에는 착수할 수 없다.
* 보드 밖에는 착수할 수 없다.
* 자신의 포켓에 존재하는 기물만 착수할 수 있다.
* King은 착수할 수 없다.
* 착수 후 자기 King이 공격받는 상태여도 합법이다.
* 착수 가능 범위 계산에는 Chessembly 기반 Attack Map을 사용한다.

---

# 7. Chessembly 통합 요구사항

Brainfuck Chess의 기물 이동 가능 범위와 공격 / 위협 범위는 Chessembly를 사용해 계산한다.

Chessembly는 기물의 이동 방식과 공격 방식을 코드 형태로 정의하는 DSL 또는 해석 가능한 명령 체계로 취급한다.

Chessembly는 기능 확장은 가능하나, 조건은 다음과 같다.

* `https://docs.en-passant.org/`에 기재된 Chessembly 기능을 기준으로 하위호환성을 유지해야 한다. 
* 공식 문서에 있는 기존 문법, 명령어, 예제 동작을 깨뜨리면 안 된다.
* 기존 Chessembly 코드가 Brainfuck Chess 엔진에서도 동일한 의미로 실행되어야 한다.
* Brainfuck Chess 전용 확장은 기존 Chessembly 명령어의 의미를 바꾸는 방식으로 구현하면 안 된다.
* 확장은 버전, dialect, extension flag, 또는 별도 네임스페이스로 분리해야 한다.
* 만약 공식 문서에 접근 불가능하다면, `CHESSEMBLY.md` 파일을 참고하라.

예시:

```ts
interface PieceDefinition {
  id: string;
  name: string;
  score: number;
  chessemblyCode: string;
  chessemblyVersion: string;
  dialect?: "classic" | "brainfuck-chess";
  extensions?: string[];
  isKing?: boolean;
}
```

---

# 8. Chessembly의 책임과 Brainfuck Chess 룰 엔진의 책임 분리

Chessembly의 책임:

* 기물 단위의 이동 후보 계산
* 기물 단위의 공격 / 위협 칸 계산
* 커스텀 기물 행마법 정의
* `movementSquares`와 `attackSquares` 산출

Brainfuck Chess 룰 엔진의 책임:

* 보드 크기 제한
* 아군 기물 충돌 판정
* 적 기물 포획 처리
* King 포획 시 승리 처리
* 이동 스택 소비
* 턴 모드 관리
* 포켓 착수 처리
* 덱 점수 제한
* 기본 진영 계산
* 서버 검증
* 게임 종료 처리

중요한 구조 원칙:

```text
Chessembly = 기물 단위의 이동 / 공격 / 위협범위 계산
BrainfuckChessRuleEngine = 게임 전체 규칙, 턴, 착수, 스택, 승패 판정
```

---

# 9. 이동 범위와 공격 범위의 분리

Brainfuck Chess에서는 `movementSquares`와 `attackSquares`를 반드시 분리해야 한다.

```ts
interface ChessemblyResult {
  movementSquares: Square[];
  attackSquares: Square[];
  metadata?: Record<string, unknown>;
}
```

이유:

* Pawn처럼 이동 칸과 공격 칸이 다른 기물이 존재한다.
* 포켓 착수 가능 범위는 이동 가능 칸이 아니라 공격 / 위협 칸 기준이다.
* UI에서 이동 가능 칸과 공격 범위를 다르게 표시해야 한다.
* AI 평가에서 이동성과 위협도를 따로 계산할 수 있어야 한다.

---

# 10. Attack Map 계산 방식

Attack Map은 특정 플레이어의 모든 기물이 현재 보드 상태에서 공격하거나 위협하는 칸들의 집합이다.

계산 절차:

1. 현재 플레이어의 보드 위 기물을 모두 가져온다.
2. 각 기물의 Chessembly 코드를 실행한다.
3. 각 기물의 `attackSquares`를 가져온다.
4. 모든 `attackSquares`를 합집합으로 합친다.
5. 각 칸을 어떤 기물들이 공격하는지도 `sourceMap`에 저장한다.

자료구조 예시:

```ts
interface AttackMap {
  playerId: PlayerId;
  attackedSquares: Set<SquareId>;
  sourceMap: Map<SquareId, PieceId[]>;
}
```

Attack Map의 용도:

* 포켓 기물 착수 가능 칸 계산
* 공격 범위 UI 하이라이트
* 위험 칸 표시
* AI 평가 함수
* 전술 분석

중요:

```text
Attack Map은 직접 승패를 판정하지 않는다.
실제 승패는 King이 포획되었을 때만 발생한다.
```

---

# 11. 이동 스택 시스템

이번 버전에는 별도의 스턴 스택이 없다.

모든 기물은 기본적으로 한 턴마다 이동 스택 1개를 받는다.

규칙:

* 각 턴 시작 시 현재 플레이어의 보드 위 기물들은 이동 스택 1개를 가진다.
* 기물이 한 번 이동하면 해당 기물의 이동 스택을 1개 소비한다.
* 이동 스택이 0인 기물은 그 턴에 더 이상 이동할 수 없다.
* 한 턴에 여러 기물을 움직일 수 있다.
* 같은 기물은 기본적으로 한 턴에 한 번만 움직일 수 있다.
* 한 턴에 최소 하나의 행동은 반드시 해야 한다.
* 패스는 불가능하다.

---

# 12. 턴 진행 방식

MVP에서는 다음 턴 구조를 사용한다.

```text
턴 시작
↓
현재 플레이어의 모든 보드 위 기물에 이동 스택 1 부여
↓
플레이어가 이동 모드 또는 착수 모드 선택
↓
선택한 모드의 행동을 1개 이상 수행
↓
King 포획 여부 확인
↓
게임이 끝나지 않았다면 턴 종료
```

추천 기본 규칙:

* 한 턴에는 “이동만” 하거나 “착수만” 할 수 있다.
* 이동과 착수를 같은 턴에 섞지 않는다.
* 이동 턴에는 여러 기물을 이동할 수 있다.
* 착수 턴에는 포켓 기물 1개만 착수할 수 있다.
* 패스는 불가능하다.

이 방식을 추천하는 이유:

* 합법 수 생성기가 단순해진다.
* UI에서 현재 턴 모드를 명확히 보여줄 수 있다.
* 착수 후 새로 생긴 공격 범위로 추가 행동을 하는 연쇄 문제를 피할 수 있다.
* 서버 검증이 쉬워진다.
* 다중 이동 시스템과 King 포획 승리 조건을 안정적으로 처리할 수 있다.

추후 고급 모드에서는 “착수 후 이동 가능, 단 이동 시작 후 착수 불가” 같은 규칙을 실험할 수 있다.
하지만 MVP에서는 이동과 착수를 같은 턴에 섞지 않는 것을 기본으로 설계해라.

---

# 13. 이동 규칙

기물 이동 규칙은 Chessembly 코드로 정의한다.

기본 체스 기물도 가능하면 하드코딩하지 말고, Chessembly 기반 정의로 관리한다.

기본 기물:

* King
* Queen
* Rook
* Bishop
* Knight
* Pawn

기본 이동 규칙은 체스 기물의 움직임을 따른다.

단, 다음 특수 규칙은 MVP에서 단순화한다.

## Pawn

추천안:

* 각 플레이어의 전진 방향을 가진다.
* White는 rank가 증가하는 방향으로 전진한다.
* Black은 rank가 감소하는 방향으로 전진한다.
* Pawn의 전진 이동과 공격 범위는 분리한다.
* Pawn의 공격 범위는 대각선 전방 1칸이다.
* Pawn이 자기 기본 진영의 두 번째 랭크에 있을 때만 2칸 전진할 수 있다.

## Promotion

추천안:

* Pawn이 상대편 끝 랭크에 도달하면 승격한다.
* 승격 가능 기물은 Queen, Rook, Bishop, Knight로 제한한다.
* 커스텀 기물 승격은 추후 확장 기능으로 둔다.

## Castling

MVP에서는 비활성화한다.

이유:

* 자유로운 덱 구성과 기본 진영 배치가 존재한다.
* Rook과 King의 고정 시작 위치를 보장하기 어렵다.
* 확장 보드에서 캐슬링 거리를 정의하기 어렵다.

## En Passant

MVP에서는 비활성화한다.

이유:

* 한 턴에 여러 기물이 이동할 수 있으므로 “직전 한 수” 개념이 복잡하다.
* 포켓 착수와 조합될 때 판정이 복잡해진다.

---

# 14. 합법 이동 판정

이동 행동의 합법성은 다음 조건으로 판단한다.

1. 이동하려는 기물이 현재 플레이어의 기물인가?
2. 해당 기물이 보드 위에 존재하는가?
3. 해당 기물의 이동 스택이 남아 있는가?
4. Chessembly 기준으로 해당 목적지가 `movementSquares`에 포함되는가?
5. 목적지가 보드 안인가?
6. 목적지에 아군 기물이 없는가?
7. 목적지에 적 기물이 있다면 포획 가능한가?
8. 현재 턴 모드가 이동 모드인가?

검사하지 않는 조건:

```text
이동 후 자기 King이 공격받는가?
이동 후 자기 King이 체크 상태인가?
이동으로 체크를 해소했는가?
```

Brainfuck Chess에는 체크가 없기 때문이다.

---

# 15. King 포획 처리

이동 결과 상대 King을 포획한 경우 즉시 게임을 종료한다.

추천 처리 방식:

```ts
function applyMoveAction(gameState: GameState, action: MoveAction): GameState {
  const targetPiece = getPieceAt(gameState.board, action.to);

  const nextState = movePieceAndCaptureIfNeeded(gameState, action);

  if (targetPiece && isRoyalPiece(targetPiece, getPieceDefinition(targetPiece.typeId))) {
    return {
      ...nextState,
      phase: "ended",
      result: {
        winner: action.playerId,
        reason: "king_capture",
      },
    };
  }

  return nextState;
}
```

---

# 16. 착수 가능 칸 계산

착수 가능 칸은 Chessembly 기반 Attack Map을 사용한다.

```text
placementSquares = baseZoneSquares ∪ playerAttackMap
```

절차:

1. 현재 플레이어의 포켓 기물 목록을 가져온다.
2. 현재 보드 상태 기준으로 playerAttackMap을 계산한다.
3. 현재 플레이어의 baseZoneSquares를 계산한다.
4. 두 집합의 합집합을 만든다.
5. 이미 기물이 있는 칸을 제거한다.
6. 보드 밖 칸을 제거한다.
7. 남은 칸을 착수 가능 칸으로 사용한다.

착수 행동의 합법성:

1. 포켓에 해당 기물이 존재하는가?
2. 착수하려는 칸이 보드 안인가?
3. 착수하려는 칸이 비어 있는가?
4. 착수하려는 칸이 `placementSquares`에 포함되는가?
5. 현재 턴 모드가 착수 모드인가?
6. 해당 기물이 King이 아닌가?

---

# 17. 데이터 구조 설계

TypeScript 기준으로 다음과 같은 구조를 추천한다.

```ts
type PlayerId = "white" | "black";
type SquareId = string;
type PieceId = string;
type PieceTypeId = string;

interface Square {
  file: number;
  rank: number;
}

interface Board {
  size: number;
  squares: Map<SquareId, PieceId | null>;
}

interface PieceDefinition {
  id: PieceTypeId;
  name: string;
  score: number;
  chessemblyCode: string;
  chessemblyVersion: string;
  dialect?: "classic" | "brainfuck-chess";
  extensions?: string[];
  isKing?: boolean;
}

interface Piece {
  id: PieceId;
  owner: PlayerId;
  typeId: PieceTypeId;
  currentSquare?: Square;
  inPocket: boolean;
  captured: boolean;
  moveStack: number;
  hasMoved: boolean;
}

interface Deck {
  playerId: PlayerId;
  startingPieces: Piece[];
  pocketPieces: Piece[];
  scoreLimit: number;
  totalScore: number;
}

interface Player {
  id: PlayerId;
  deck: Deck;
  capturedPieces: Piece[];
}

interface GameState {
  board: Board;
  players: Record<PlayerId, Player>;
  currentPlayer: PlayerId;
  turnNumber: number;
  phase: "setup" | "playing" | "ended";
  turnState: TurnState;
  result?: GameResult;
}

interface TurnState {
  mode: "undecided" | "move" | "drop";
  actions: TurnAction[];
  movedPieceIds: Set<PieceId>;
}

type TurnAction = MoveAction | DropAction;

interface MoveAction {
  type: "move";
  playerId: PlayerId;
  pieceId: PieceId;
  from: Square;
  to: Square;
  capturedPieceId?: PieceId;
}

interface DropAction {
  type: "drop";
  playerId: PlayerId;
  pieceId: PieceId;
  to: Square;
}

interface GameResult {
  winner?: PlayerId;
  reason:
    | "king_capture"
    | "resignation"
    | "timeout"
    | "draw";
}
```

---

# 18. Chessembly 실행 관련 데이터 구조

```ts
interface ChessemblyExecutionContext {
  board: Board;
  piece: Piece;
  pieceDefinition: PieceDefinition;
  position: Square;
  player: PlayerId;
  mode: "movement" | "attack";
}

interface ChessemblyResult {
  movementSquares: Square[];
  attackSquares: Square[];
  metadata?: Record<string, unknown>;
}

interface AttackMap {
  playerId: PlayerId;
  attackedSquares: Set<SquareId>;
  sourceMap: Map<SquareId, PieceId[]>;
}
```

---

# 19. 엔진 계층 구조

다음 계층으로 구현하는 것을 추천한다.

## 19.1 ChessemblyParser

역할:

* Chessembly 코드를 파싱한다.
* 공식 문서 기준 기존 문법을 지원한다.
* Brainfuck Chess 전용 확장을 기존 문법과 충돌하지 않게 처리한다.

## 19.2 ChessemblyInterpreter

역할:

* Chessembly 명령을 실행한다.
* 특정 기물이 특정 보드 상태에서 이동 가능한 칸과 공격 가능한 칸을 계산한다.
* 기존 Chessembly 코드의 의미를 유지한다.

## 19.3 AttackMapGenerator

역할:

* 플레이어별 공격 맵을 계산한다.
* Chessembly 결과의 `attackSquares`를 합산한다.
* 착수 가능 칸과 UI 하이라이트에 사용한다.

## 19.4 BrainfuckChessRuleEngine

역할:

* 전체 게임 규칙을 관리한다.
* Chessembly 결과를 받아 합법 행동으로 필터링한다.
* 이동 스택, 착수, 턴 종료, King 포획, 게임 종료를 처리한다.

## 19.5 GameServerValidator

역할:

* 클라이언트가 보낸 행동이 합법인지 서버에서 재검증한다.
* 멀티플레이에서 부정 조작을 막는다.
* 가능하다면 클라이언트와 서버가 동일한 룰 엔진 코드를 공유한다.

---

# 20. 주요 함수 목록

다음 함수를 설계해라.

```ts
createBoard(size: number): Board

calculateScoreLimit(boardSize: number): number

calculateDeckScore(deck: Deck): number

validateDeck(deck: Deck, boardSize: number): ValidationResult

getBaseZoneSquares(playerId: PlayerId, boardSize: number): Square[]

runChessembly(
  context: ChessemblyExecutionContext
): ChessemblyResult

generateAttackMap(
  gameState: GameState,
  playerId: PlayerId
): AttackMap

getPlacementSquares(
  gameState: GameState,
  playerId: PlayerId
): Square[]

generateLegalMoveActions(
  gameState: GameState,
  playerId: PlayerId
): MoveAction[]

generateLegalDropActions(
  gameState: GameState,
  playerId: PlayerId
): DropAction[]

applyMoveAction(
  gameState: GameState,
  action: MoveAction
): GameState

applyDropAction(
  gameState: GameState,
  action: DropAction
): GameState

isRoyalPiece(
  piece: Piece,
  definition: PieceDefinition
): boolean

hasLivingKing(
  gameState: GameState,
  playerId: PlayerId
): boolean

isKingCaptureMove(
  gameState: GameState,
  action: MoveAction
): boolean

canEndTurn(
  gameState: GameState
): boolean

endTurn(
  gameState: GameState
): GameState
```

체크와 체크메이트가 없으므로 다음 함수는 만들지 않는다.

```ts
isKingInCheck(...)
isCheckmate(...)
```

---

# 21. UI / UX 요구사항

웹 UI는 다음 화면을 포함해야 한다.

## 21.1 보드 크기 선택 화면

* `8x8` 이상 선택 가능
* 선택한 보드 크기에 따른 점수 상한 표시
* 예: `10x10 보드: 덱 점수 상한 75점`

## 21.2 덱 빌딩 화면

* 기본 기물 목록
* 각 기물 점수 표시
* 현재 사용 점수 / 점수 상한 표시
* 기본 진영 배치 기물 선택
* 포켓 기물 선택
* King 필수 포함 검증
* King 포켓 배치 금지 검증

## 21.3 기본 진영 배치 화면

* 자신의 기본 진영 칸 하이라이트
* 선택한 기물을 기본 진영에 배치
* 불법 배치 방지

## 21.4 게임 화면

* 보드 표시
* 현재 턴 표시
* 현재 플레이어 표시
* 이동 모드 / 착수 모드 선택
* 이동 가능한 칸 하이라이트
* 공격 범위 하이라이트
* 착수 가능한 칸 하이라이트
* 포켓 기물 목록
* 이동 스택 표시
* King 위험 상태 보조 표시
* King 포획 시 게임 종료 연출
* 턴 로그 표시

주의:

* “체크”라는 룰은 없으므로 UI에서 체크 상태를 강제 규칙처럼 표시하지 않는다.
* 단, King이 상대 Attack Map 안에 있을 때 “위험” 정도의 보조 표시를 할 수는 있다.

---

# 22. 테스트 전략

## 22.1 Chessembly 하위호환성 테스트

공식 문서 기준 기능을 테스트한다.

테스트 목표:

* 기존 Chessembly 코드가 파싱되는지 확인
* 기존 명령어의 의미가 바뀌지 않았는지 확인
* 예제 기물의 이동 / 공격 결과가 기존 기대값과 일치하는지 확인
* Brainfuck Chess 확장을 켜지 않았을 때 기존 동작이 그대로 유지되는지 확인

## 22.2 Brainfuck Chess 룰 테스트

테스트 항목:

* 보드 크기 생성
* 점수 상한 계산
* 덱 점수 검증
* 기본 진영 계산
* 포켓 착수 가능 칸 계산
* Attack Map 계산
* 이동 스택 소비
* 이동 턴 검증
* 착수 턴 검증
* King 포획 판정
* 게임 종료 판정
* 턴 종료 조건

## 22.3 서버 검증 테스트

* 클라이언트에서 불법 행동을 보내도 서버가 거부해야 한다.
* 서버와 클라이언트의 룰 엔진 결과가 일치해야 한다.
* 동일한 GameState와 Action을 넣으면 항상 같은 결과가 나와야 한다.

---

# 23. MVP 개발 순서

## 1단계: 순수 룰 엔진

* Board 구현
* Piece 구현
* Deck 구현
* 점수 상한 계산
* 기본 진영 계산
* 턴 상태 구현

## 2단계: Chessembly 통합

* Chessembly 파서 또는 기존 구현 연결
* 기본 기물 이동 코드 정의
* `movementSquares`와 `attackSquares` 분리
* Attack Map 생성
* 하위호환성 테스트 추가

## 3단계: 로컬 2인 플레이

* 보드 UI
* 기물 이동
* 포켓 착수
* 턴 종료
* King 포획 승리 처리

## 4단계: 덱 빌더

* 보드 크기 선택
* 점수 상한 표시
* 기본 진영 배치
* 포켓 기물 선택
* 덱 유효성 검사

## 5단계: 게임 종료와 로그

* King 포획 종료
* 기권
* 무승부 조건
* 게임 결과 표시
* 턴 로그 저장

## 6단계: 온라인 멀티플레이

* 서버 상태 저장
* Action 기반 동기화
* 서버 검증
* 재접속 처리
* 게임 로그 저장

## 7단계: AI 대전

* 합법 수 생성 최적화
* Attack Map 캐싱
* 평가 함수
* 탐색 알고리즘
* 포켓 착수 평가
* King 포획 위협 평가

---

# 24. 확정 필요 항목

다음 항목은 구현 전에 최종 결정이 필요하다.

1. `n x n` 보드에서 기본 진영을 정확히 첫 2개 랭크 전체로 확정할 것인가?
2. 한 턴에 착수는 1회만 가능한가, 여러 번 가능한가?
3. 이동과 착수를 같은 턴에 완전히 금지할 것인가?
4. Pawn의 초기 2칸 이동 조건을 어떻게 확정할 것인가?
5. Castling을 완전히 제거할 것인가?
6. En Passant를 완전히 제거할 것인가?
7. 커스텀 기물의 점수 산정 방식은 어떻게 할 것인가?
8. 포켓 기물 착수 후 즉시 다음 턴부터만 공격 범위에 반영할 것인가, 아니면 같은 턴 내 연쇄 규칙이 생길 여지가 있는가?
9. 다중 이동 중 King을 잡으면 즉시 종료하는 현재 규칙을 확정할 것인가?
10. 무승부 조건을 둘 것인가?

---

# 25. 추천 기본 룰셋 v0.3

MVP에서는 다음 기본 룰셋을 추천한다.

```text
보드 크기:
n x n, n >= 8

점수 상한:
scoreLimit = n * n - 25

기본 진영:
각 플레이어의 첫 2개 랭크 전체

King:
반드시 기본 진영에 1개 배치
포켓 불가
복수 King 불가
점수 계산 제외

승리 조건:
상대 King을 포획하면 즉시 승리

체크:
없음

체크메이트:
없음

스테일메이트:
없음

착수:
자기 기본 진영 또는 자기 Attack Map 안의 빈 칸에 가능
착수 후 자기 King이 공격받는 상태여도 합법
착수 턴에는 포켓 기물 1개만 착수

턴:
한 턴에는 이동 또는 착수 중 하나만 선택
이동 턴에는 여러 기물을 이동 가능
착수 턴에는 포켓 기물 1개 착수 가능
패스 불가

이동 스택:
모든 기물은 턴마다 이동 스택 1개
한 기물은 한 턴에 최대 1번 이동 가능

King 이동:
King은 공격받는 칸으로 이동 가능
King이 실제로 포획될 때만 패배

Castling:
MVP에서는 비활성화

En Passant:
MVP에서는 비활성화

Promotion:
Pawn이 상대 끝 랭크에 도달하면 Queen, Rook, Bishop, Knight 중 하나로 승격

공격 / 이동 계산:
Chessembly 기반
movementSquares와 attackSquares 분리

Attack Map 용도:
착수 가능 칸 계산
UI 하이라이트
AI 평가
위험 칸 표시
```

---

# 26. 원하는 답변 형식

답변은 한국어로 작성해라.

다음 순서로 결과물을 제시해라.

1. Brainfuck Chess 핵심 규칙 요약
2. 추천 룰셋 v0.3
3. Chessembly 통합 구조
4. Chessembly 하위호환성 유지 전략
5. 데이터 구조 설계
6. 룰 엔진 계층 구조
7. 합법 이동 생성 알고리즘
8. 착수 가능 칸 계산 알고리즘
9. King 포획 승리 처리 방식
10. 이동 스택 시스템 구현 방식
11. UI/UX 설계
12. 테스트 전략
13. MVP 개발 순서
14. 확정 필요 질문 목록

중요한 점:

* 체스의 체크 / 체크메이트 개념을 사용하지 마라.
* “체크 상태라서 불법” 같은 판정을 넣지 마라.
* King은 실제로 포획되었을 때만 패배 조건이 된다.
* Chessembly는 기물 이동 / 공격 / 위협범위 계산 엔진으로 사용한다.
* Brainfuck Chess 룰 엔진은 Chessembly 결과를 받아 턴, 스택, 착수, 포획, 승패를 처리한다.
* 기존 Chessembly 문법과 공식 문서 기준 기능의 하위호환성을 깨뜨리지 마라.
* 구현 가능한 웹 게임 구조로 설계하라.
