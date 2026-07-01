# 프로그래밍 또는 그 기획에서 필수적으로 지켜야 하는 원칙

## 유저에게 맡겨라
안전한 기본값을 제공하고, 위험하거나 권한 있는 기능은 명시적 opt-in 뒤에 사용자에게 선택권을 준다. 선택지가 있다는 사실만으로 신뢰 경계나 안전 정책을 우회하지 않는다.

## 유저에게 알려라
현재 primary lane의 상태, 격리 여부, degraded/fallback 경로, export 차단, Last Known Good의 읽기 전용 제공, confirmed fix 보류 사유를 원문 비밀 없이 상태 기반으로 알린다.

## 사용자의 입력을 신뢰하지 않는다
외부 입력은 처음에는 unknown/untrusted로 받고, 구조 확인, 범위 검사, 정규화, redaction 경계를 통과한 뒤에만 내부 normalized object로 승격한다. optional 값은 undefined로 기록하지 않고, 값이 없으면 필드를 생략해 상태 의미를 보존한다.

## 하드코딩을 지양한다
정적인 값은 책임별 registry 또는 policy 파일에서 관리한다. 사용자 문구, fixed term, safety grade, quorum policy, retry policy, error code를 하나의 거대 registry에 섞지 않으며, 런타임 입력으로 보안 기본값을 약화시키지 않는다.

## SoC(관심사 분리)와 SRP(단일 책임 원칙)를 따른다
기획 혹은 체크리스트에서 파일 구조를 잡을 때 SoC와 SRP가 지켜지고 있는지 계속 검토해 이 둘이 100% 나뉘어질 수 있도록 한다

## 모든 시스템을 객체화한다
class 수를 늘리는 뜻이 아니라 responsibility, boundary, input, output, failure, evidence, recovery action, safety decision을 각자의 계약 객체로 분리한다.

## 다중화된 시스템을 구축한다
동일한 책임을 서로 다른 구현, 판단 순서, source, failure mode로 수행하는 보조 시스템을 둔다. 동일 parser 결과, HTTP mapper, pattern list를 반복해 가짜 독립성을 만들지 않고, conservative lane, shadow audit, 독립 voter와 monitor를 구분한다.

## Fail-Operational 시스템을 구축한다
고장이 발생해도 confirmed 기준을 낮추지 않은 채 metadata-only degraded output, 안전한 fallback, read-only Last Known Good로 기능 연속성을 유지한다. 외부 Provider, Docker daemon, GPU driver, worker process를 Nodra가 몰래 수정하거나 재시작하지 않는다.

## 오류 시 인간 개입을 최소화한다
오류를 상태와 안전 경계로 알리고, Nodra 내부에서 가능한 격리, bounded retry, 안전한 backup 선택, fail-safe export 차단을 먼저 수행한다. 사람에게는 자동 재수집, 파일 선택, 외부 Provider 확인, opt-in처럼 마지막에 필요한 최소 행동만 요청한다.

## 재시도 불가능한 오류를 대비한다
구조적으로 잘못된 input, redaction 실패, unsafe output, 정책/권한 차단, quorum conflict는 retry 금지와 함께 fallback 가능 여부, backup/LKG 사용 여부, fail-safe 전환 기준을 명시한다.

## 재시도 가능한 일시적 고장을 즉시 고친다
"즉시"는 무한 재시도가 아니라 idempotency 확인 뒤의 bounded retry를 뜻한다. max attempts, timeout, backoff, jitter, abort, retry budget, circuit breaker, audit를 적용하고, 내부 reset과 redacted health audit을 통과한 lane만 voting에 재참여시킨다.

## Fail-Operational에 실패할 경우 대안으로 Fail-Safe를 사용한다
고치거나 백업할 수 없는 고장 발생 시 안전하게 정지한다

## 심층 방어 시스템을 설계한다
계층을 여러 개 두는 데서 멈추지 않고, 각 계층이 다른 정보만 보유해 한 계층 또는 두 계층이 침해되어도 원문, secret, unsafe output을 재구성할 수 없게 한다. 방어 계층은 정규화된 입력만 받고, redaction 실패 시 degraded/fallback/fail-safe 경로를 정책으로 선택한다.