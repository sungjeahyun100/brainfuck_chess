# Deck Chess 덱 코드 구조 설명서

> 구현 기준일: 2026-08-16
>
> 대상 포맷: `DC1`
>
> 기준 구현: [`useDeckCodeCodec.ts`](../frontend/src/composables/useDeckCodeCodec.ts)

이 문서는 덱 편집기의 **덱 코드 복사·공유·불러오기 기능**이 사용하는 외부 문자열 형식, 내부 JSON 스키마, 검증 경계와 하위호환성 원칙을 현재 구현 기준으로 설명한다.

## 목차

1. [핵심 요약](#1-핵심-요약)
2. [외부 문자열 형식](#2-외부-문자열-형식)
3. [DC1 JSON 페이로드](#3-dc1-json-페이로드)
4. [인코딩 과정](#4-인코딩-과정)
5. [디코딩 과정](#5-디코딩-과정)
6. [검증 계층과 안전 경계](#6-검증-계층과-안전-경계)
7. [가져오기 동작](#7-가져오기-동작)
8. [오류 모델](#8-오류-모델)
9. [커스텀 기물과 버전 고정](#9-커스텀-기물과-버전-고정)
10. [예제](#10-예제)
11. [하위호환성 및 유지보수 지침](#11-하위호환성-및-유지보수-지침)
12. [구현 파일 지도](#12-구현-파일-지도)

## 1. 핵심 요약

DC1 덱 코드는 아래 구조를 가진 버전형 문자열이다.

```text
DC1. + Base64URL(UTF-8(JSON(DeckCodeV1)))
```

데이터 흐름은 다음과 같다.

```text
현재 덱
  → 공유 대상 필드 추출
  → DeckCodeV1 객체
  → JSON 문자열
  → UTF-8 바이트
  → Base64URL
  → "DC1." 접두사 결합
```

공유하는 정보:

- 보드 크기
- 시작 기물의 종류와 좌표
- 포켓 기물의 종류와 수량

공유하지 않는 정보:

- 덱 ID와 이름
- 생성·수정 시각
- 점수와 점수 제한 계산 결과
- 화면 및 편집 상태
- 수량이 0인 포켓 항목
- 별도의 커스텀 기물 정의 또는 패키지

> [!WARNING]
> Base64URL은 암호화가 아니다. 누구나 내용을 읽거나 변조할 수 있으므로 가져온 코드는 항상 신뢰할 수 없는 외부 입력으로 취급한다.

## 2. 외부 문자열 형식

예시:

```text
DC1.<base64url-payload>
```

| 요소 | 현재 값 | 의미 |
|---|---|---|
| 제품 식별자 | `DC` | Deck Code임을 나타낸다. 대소문자를 구분한다. |
| 봉투 버전 | `1` | 현재 지원하는 외부 포맷 버전이다. |
| 구분자 | `.` | 헤더와 페이로드를 구분한다. |
| 페이로드 문자 | `A-Z`, `a-z`, `0-9`, `_`, `-` | 패딩 없는 Base64URL 알파벳이다. |
| 최대 입력 길이 | 65,536자 | 공백 제거 전 원본 입력에 적용한다. |

봉투는 다음 정규식으로 식별한다.

```regex
^DC(\d+)\.(.*)$
```

- `DC`는 대문자여야 한다.
- 버전 숫자가 `1`이 아니면 `unsupported_version` 오류가 된다.
- 복사 과정에서 생긴 줄바꿈, 스페이스, 탭 등의 공백은 디코딩 전에 제거한다.
- 빈 입력 검사와 최대 길이 검사는 공백을 제거하기 전에 수행한다.

## 3. DC1 JSON 페이로드

디코딩된 JSON은 다음 네 개의 최상위 키를 **정확히** 가져야 한다.

```json
{
  "v": 1,
  "boardSize": 8,
  "starting": [
    { "pieceId": "king", "file": 4, "rank": 0 },
    { "pieceId": "pawn", "file": 0, "rank": 1 }
  ],
  "pocket": [
    { "pieceId": "knight", "count": 2 }
  ]
}
```

누락된 키뿐 아니라 알 수 없는 추가 키도 허용하지 않는다.

### 3.1 최상위 필드

| 필드 | 타입 | 코덱 단계 제약 | 의미 |
|---|---|---|---|
| `v` | 정수 리터럴 | 반드시 `1` | JSON 내부 스키마 버전 |
| `boardSize` | 정수 | 코덱은 정수 여부만 확인 | 정사각형 보드의 한 변 길이 |
| `starting` | 배열 | 최대 144개 | White 기준 시작 배치 |
| `pocket` | 배열 | 최대 256종 | 게임 중 착수할 포켓 기물 |

`boardSize`의 실제 지원 범위는 코덱이 아니라 도메인 검증에서 확인한다. 현재 지원 크기는 `8`, `9`, `10`, `11`, `12`이다.

### 3.2 `starting` 항목

각 시작 기물은 다음 세 키만 가져야 한다.

```json
{
  "pieceId": "king",
  "file": 4,
  "rank": 0
}
```

| 필드 | 타입 | 코덱 검사 | 도메인 검사 |
|---|---|---|---|
| `pieceId` | 문자열 | 길이 1~256자 | 현재 또는 보관된 기물 카탈로그에서 존재 여부 확인 |
| `file` | 정수 | 정수 여부 | 보드 범위와 기본 진영 여부 확인 |
| `rank` | 정수 | 정수 여부 | 보드 범위, 기본 진영, front/back 배치 제한 확인 |

같은 칸의 중복, King 개수, 기물별 배치 줄 제한은 코덱 이후의 도메인 검증에서 처리한다.

좌표는 0부터 시작한다.

### 3.3 `pocket` 항목

각 포켓 항목은 다음 두 키만 가져야 한다.

```json
{
  "pieceId": "knight",
  "count": 2
}
```

| 필드 | 타입 | 제약 |
|---|---|---|
| `pieceId` | 문자열 | 길이 1~256자이며 배열 안에서 중복될 수 없다. |
| `count` | 정수 | 1 이상 1,024 이하이다. |

추가 제한:

- 모든 포켓 항목의 `count` 합은 최대 4,096이다.
- 수량이 0인 기물은 인코딩할 때 생략한다.
- King의 포켓 사용 가능 여부와 덱 점수 제한은 도메인 검증에서 판정한다.

## 4. 인코딩 과정

인코딩은 [`encodeDeckCode()`](../frontend/src/composables/useDeckCodeCodec.ts)에서 수행한다.

1. `LobbyDeck`과 `boardSize`에서 공유 대상 필드만 추출한다.
2. 시작 기물의 필드명을 다음과 같이 변환한다.

   ```text
   pieceType  → pieceId
   square.file → file
   square.rank → rank
   ```

3. 시작 기물을 `rank → file → pieceId` 순으로 정렬한다.
4. 포켓에서 수량이 양수인 항목만 남긴다.
5. 포켓 항목을 `pieceId` 순으로 정렬한다.
6. `JSON.stringify()`로 직렬화한다.
7. 문자열을 UTF-8 바이트로 변환한다.
8. 표준 Base64를 Base64URL 형식으로 변환한다.

   ```text
   + → -
   / → _
   끝의 = 제거
   ```

9. 앞에 `DC1.`을 붙인다.

### 결정적 출력

인코딩 전에 시작 기물과 포켓 항목을 정렬하므로 같은 구성의 덱은 원본 배열이나 객체의 삽입 순서가 달라도 안정적으로 같은 코드가 된다.

## 5. 디코딩 과정

디코딩은 [`decodeDeckCode()`](../frontend/src/composables/useDeckCodeCodec.ts)에서 수행한다.

```text
외부 문자열
  → 입력 크기 검사
  → 공백 제거
  → 봉투와 버전 검사
  → Base64URL 디코딩
  → 엄격한 UTF-8 디코딩
  → JSON 파싱
  → DC1 스키마 검사
  → 명시적인 성공 또는 실패 결과
```

세부 순서:

1. 입력이 비어 있는지 확인한다.
2. 원본 입력이 65,536자를 넘는지 확인한다.
3. 모든 공백을 제거한다.
4. `DC<숫자>.` 봉투인지 확인한다.
5. 봉투 버전이 `1`인지 확인한다.
6. Base64URL 문자와 길이를 확인한다.
7. 필요한 Base64 패딩을 복원하고 디코딩한다.
8. `TextDecoder('utf-8', { fatal: true })`로 UTF-8을 엄격하게 디코딩한다.
9. JSON을 파싱한다.
10. 일반 객체 여부, 정확한 키, 타입과 상한을 확인한다.

반환 타입은 boolean 하나가 아니라 성공과 실패의 의미를 구분하는 판별 유니온이다.

```ts
type DeckCodeDecodeResult =
  | { ok: true; value: DeckCodeV1 }
  | { ok: false; error: DeckCodeDecodeError }
```

## 6. 검증 계층과 안전 경계

검증은 두 단계로 나뉜다.

| 계층 | 담당 파일 | 책임 |
|---|---|---|
| 코덱 검증 | [`useDeckCodeCodec.ts`](../frontend/src/composables/useDeckCodeCodec.ts) | 입력 크기, 봉투, 버전, Base64URL, UTF-8, JSON, 정확한 키, 타입, 배열과 수량 상한, 포켓 종류 중복 |
| 가져오기·도메인 검증 | [`useDeckCode.ts`](../frontend/src/composables/useDeckCode.ts), [`useDeckValidation.ts`](../frontend/src/composables/useDeckValidation.ts) | 기물 존재 여부, 커스텀 기물 참조 복원, 보드 크기, 좌표, 배치 규칙, King, 포켓 허용 여부, 점수 제한, 커스텀 버전 활성 상태 |

> [!IMPORTANT]
> `decodeDeckCode()`의 성공은 “스키마가 안전한 DC1 데이터”라는 뜻이다. 실제 게임에서 사용할 수 있다는 보장은 `importDeckCode()`가 현재 카탈로그와 `validateLobbyDeck()`을 통과한 뒤에만 성립한다.

### 6.1 코덱이 확인하는 항목

- 입력 문자열의 크기
- 봉투 형식과 지원 버전
- Base64URL 문자 집합과 길이
- 올바른 UTF-8과 JSON 여부
- 객체가 `null`이나 배열이 아닌지
- 객체의 프로토타입이 `Object.prototype` 또는 `null`인지
- 최상위 및 배열 항목의 정확한 키 집합
- 필드 타입
- 배열 길이와 포켓 수량 상한
- 포켓 `pieceId` 중복

예상하지 않은 키도 거부하므로 `__proto__` 같은 삽입 데이터가 조용히 내부 모델로 승격되지 않는다.

### 6.2 도메인 계층이 확인하는 항목

- 현재 또는 보관 카탈로그에 기물이 존재하는지
- 지원하는 보드 크기인지
- 시작 좌표가 정수이고 기본 진영 안인지
- 같은 칸에 여러 시작 기물이 없는지
- 기물의 front/back 배치 제한을 만족하는지
- 시작 기물에 King이 정확히 하나 있는지
- King이 포켓에 없는지
- 포켓 수량이 유효하고 해당 기물을 포켓에 넣을 수 있는지
- 덱 점수가 보드별 제한을 넘지 않는지
- 고유 시작 기물이 중복되지 않았는지
- 사용한 커스텀 기물 버전이 활성 상태인지

## 7. 가져오기 동작

가져오기는 [`importDeckCode()`](../frontend/src/composables/useDeckCode.ts)에서 수행한다.

성공 후보를 새 객체로 만든 뒤 검증하므로 실패한 덱 코드는 현재 편집 중인 덱을 수정하지 않는다.

### 7.1 데이터 처리

| 코드에서 교체 | 현재 덱에서 보존 | 카탈로그에서 재구성 |
|---|---|---|
| `boardSize`, `starting`, `pocket` | `id`, `name`, `createdAt`, `updatedAt` 등 기존 `SavedDeck`의 나머지 정보 | `customPieces` 참조 목록 |

가져오기 순서:

1. 덱 코드를 디코딩한다.
2. `pocket` 배열을 현재 덱의 포켓 객체 형식으로 변환한다.
3. 시작 배치와 포켓에서 사용한 모든 `pieceId`를 수집한다.
4. 각 ID를 현재 또는 보관된 카탈로그에서 찾는다.
5. 하나라도 찾을 수 없으면 전체 가져오기를 거부한다.
6. 커스텀 기물의 참조 정보를 재구성한다.
7. 후보 `SavedDeck`을 만든다.
8. 기존 `validateLobbyDeck()` 규칙으로 후보를 검증한다.
9. 성공 시 후보 덱, 총점, 점수 제한을 반환한다.

### 7.2 편집기 적용

[`DeckEditor.vue`](../frontend/src/views/DeckEditor.vue)는 가져온 덱을 즉시 덮어쓰지 않는다.

```text
코드 입력
  → 가져오기 및 검증
  → 후보 미리보기
  → 사용자가 "적용" 선택
  → 편집 중인 덱 교체
  → 사용자가 별도로 저장
```

따라서 적용 후에도 사용자가 저장하기 전까지 기존 저장본은 유지된다.

## 8. 오류 모델

### 8.1 코덱 오류

| 오류 | 조건 | 사용자에게 전달되는 의미 |
|---|---|---|
| `empty` | 입력이 비어 있거나 공백뿐임 | 덱 코드를 입력해야 한다. |
| `too_large` | 원본 입력이 65,536자 초과 | 허용된 최대 길이를 초과했다. |
| `invalid_format` | `DC<숫자>.` 봉투가 아님 | 올바른 덱 코드 형식이 아니다. |
| `unsupported_version` | 봉투 버전이 `1`이 아님 | 현재 클라이언트가 지원하지 않는 버전이다. |
| `invalid_payload` | Base64URL, UTF-8 또는 JSON이 잘못됨 | 데이터가 손상되었거나 올바른 JSON이 아니다. |
| `invalid_schema` | JSON 구조, 키, 타입 또는 상한 위반 | DC1 데이터 구조가 올바르지 않다. |

### 8.2 도메인 오류

코덱 이후의 실패는 사용자에게 표시할 구체적인 메시지로 변환한다.

예시:

- 존재하지 않거나 현재 사용할 수 없는 기물
- 같은 시작 칸의 중복
- 시작 배치 구역 위반
- 덱 점수 제한 초과
- King의 포켓 포함
- 지원하지 않는 보드 크기
- 비활성화된 커스텀 기물 버전

결정적인 입력·스키마·도메인 오류는 같은 입력으로 자동 재시도해도 결과가 달라지지 않으므로 자동 재시도 대상이 아니다.

## 9. 커스텀 기물과 버전 고정

DC1은 커스텀 기물 패키지 자체를 포함하지 않는다. 대신 `pieceId`가 버전이 포함된 카탈로그 키를 가리킨다.

```text
custom:<custom-piece-id>:v<version>:<exposed-piece-key>
```

가져오기 환경에서 이 키를 현재 또는 보관된 카탈로그에서 찾으면 다음 참조를 재구성한다.

```json
{
  "id": "custom-piece-id",
  "version": 3,
  "contentHash": "...",
  "exposedPieceKey": "..."
}
```

적용 원칙:

- 동일한 이름의 최신 기물로 조용히 치환하지 않는다.
- 코드가 가리키는 정확한 버전을 유지한다.
- 참조 버전을 찾을 수 없으면 가져오기를 거부한다.
- 찾을 수 있어도 비활성화된 버전은 새 게임용 덱 검증에서 거부될 수 있다.
- 덱 코드만으로 커스텀 기물 정의를 다른 환경에 설치하거나 복구할 수는 없다.

## 10. 예제

### 10.1 완성된 덱 코드

```text
DC1.eyJ2IjoxLCJib2FyZFNpemUiOjgsInN0YXJ0aW5nIjpbeyJwaWVjZUlkIjoia2luZyIsImZpbGUiOjQsInJhbmsiOjB9LHsicGllY2VJZCI6InBhd24iLCJmaWxlIjowLCJyYW5rIjoxfV0sInBvY2tldCI6W3sicGllY2VJZCI6ImtuaWdodCIsImNvdW50IjoyfV19
```

디코딩 결과:

```json
{
  "v": 1,
  "boardSize": 8,
  "starting": [
    { "pieceId": "king", "file": 4, "rank": 0 },
    { "pieceId": "pawn", "file": 0, "rank": 1 }
  ],
  "pocket": [
    { "pieceId": "knight", "count": 2 }
  ]
}
```

이는 다음 덱 구성을 의미한다.

- 8×8 보드
- `(file: 4, rank: 0)`의 King
- `(file: 0, rank: 1)`의 Pawn
- 포켓의 Knight 2개

### 10.2 경계 사례

| 상황 | 결과 |
|---|---|
| 코드 중간에 줄바꿈이 들어감 | 전체 길이 제한 안이면 공백 제거 후 정상 해석할 수 있다. |
| 최상위에 `score` 필드를 추가함 | 정확한 키 집합 위반으로 `invalid_schema`가 된다. |
| 포켓에 같은 `pieceId`를 두 번 기재함 | `invalid_schema`가 된다. |
| `boardSize`가 `99`임 | 코덱은 정수로 수용하지만 가져오기 도메인 검증에서 거부한다. |
| 존재하지 않는 `pieceId`가 있음 | 가져오기 전체를 거부하며 현재 덱은 변경되지 않는다. |
| 유효하지 않은 현재 덱을 복사하려 함 | 편집기가 현재 덱 검증 결과를 확인하고 복사를 차단한다. |

## 11. 하위호환성 및 유지보수 지침

1. **DC1의 의미를 조용히 바꾸지 않는다.** 기존 DC1 코드가 계속 같은 덱 구성을 의미해야 한다.
2. **DC1 객체에 키를 임의로 추가하지 않는다.** 현재 디코더는 정확한 키 집합을 요구하므로 새 필드는 기존 구현에서 거부된다.
3. **새로운 구조가 필요하면 새 버전을 검토한다.** 예를 들어 `DC2.`를 추가하되 기존 `DC1.` 디코더와 회귀 테스트를 보존한다.
4. **정규 출력 순서를 유지한다.** 정렬 규칙 변경은 같은 덱의 코드 문자열을 바꾼다.
5. **입력 안전 상한을 임의로 제거하지 않는다.** 문자열, 배열, 항목 수량 제한 변경은 안전성 영향을 검토해야 한다.
6. **게임 규칙과 코드 스키마를 구분한다.** 점수나 배치 정책처럼 변할 수 있는 계산 결과를 코드에 저장하지 않고 가져오기 시 현재 규칙으로 다시 검증한다.
7. **버전 간 모호한 자동 추론을 피한다.** 봉투 버전을 먼저 판별한 뒤 해당 버전의 스키마로만 파싱한다.
8. **변경 시 회귀 테스트를 추가한다.** 기존 코드의 왕복 보존과 오류 분류가 유지되어야 한다.

현재 테스트가 확인하는 주요 항목:

- 내보내기·가져오기 왕복 보존
- 이름, 점수 등 비공유 데이터 제외
- 복사 과정의 공백 허용
- 잘못된 봉투, 버전, Base64URL, UTF-8과 JSON 거부
- 누락되거나 추가된 키 거부
- `__proto__` 등 예상하지 않은 키 거부
- 포켓 수량 상한과 종류 중복 거부
- 알 수 없는 기물이 있을 때 현재 덱 불변
- 중복 좌표, 보드 범위, 배치 구역, 점수, King과 보드 크기 규칙 재사용

관련 테스트는 [`useDeckCode.test.ts`](../frontend/src/composables/useDeckCode.test.ts)에 있다.

## 12. 구현 파일 지도

| 파일 | 책임 |
|---|---|
| [`frontend/src/composables/useDeckCodeCodec.ts`](../frontend/src/composables/useDeckCodeCodec.ts) | DC1 타입과 상수, Base64URL 변환, 결정적 인코딩, 엄격한 디코딩 |
| [`frontend/src/composables/useDeckCode.ts`](../frontend/src/composables/useDeckCode.ts) | 오류 메시지 변환, 기물 카탈로그 확인, 커스텀 참조 복원, 후보 덱 검증 |
| [`frontend/src/composables/useDeckValidation.ts`](../frontend/src/composables/useDeckValidation.ts) | 지원 보드 크기, 배치·King·포켓·점수·커스텀 활성 상태 등 도메인 규칙 |
| [`frontend/src/types/deck.ts`](../frontend/src/types/deck.ts) | `LobbyDeck`, `SavedDeck`, 배치와 커스텀 기물 참조 타입 |
| [`frontend/src/views/DeckEditor.vue`](../frontend/src/views/DeckEditor.vue) | 복사 차단, 클립보드 쓰기, 가져오기 미리보기와 명시적 적용 UI |
| [`frontend/src/composables/useDeckCode.test.ts`](../frontend/src/composables/useDeckCode.test.ts) | 왕복, 오류, 안전 경계와 도메인 거부 회귀 테스트 |

---

이 문서는 위 구현 파일의 현재 동작을 기준으로 한다. 덱 코드 형식이나 검증 정책을 변경할 때는 코드, 테스트와 이 문서를 함께 갱신한다.
