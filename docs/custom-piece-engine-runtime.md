# 커스텀 기물 엔진 런타임 계약

상태: 2단계 구현  
기준일: 2026-07-27

## 패키지 입력

엔진 공개 진입점은
`validate_and_build_custom_piece_package(CustomPiecePackageInput)`이다.
`raw_script`는 바이트 단위로 보존되며 다음 JSON envelope 형식이다.

```json
{
  "format": "brainfuck-chess-piece-set-v1",
  "definitions": [
    {
      "id": "north",
      "chessembly_code": "transition(east) move(0,1);"
    }
  ]
}
```

실제 `PieceDefinition`에는 기존 엔진 serde 계약이 요구하는 나머지 필드도
포함해야 한다. envelope만 JSON이며 각 정의의 `chessembly_code`와
`move_layers[].chessembly_code`는 기존 체섬블리 문법을 그대로 사용한다.
엔진에 존재하지 않는 새 기물 선언 문법을 체섬블리 파서에 추가하지 않았다.

패키지는 로컬 ID를
`custom:{package_id}:v{version}:{local_piece_key}` 런타임 ID로 변환한다.
promotion과 `transition(local_key)` 참조도 같은 패키지 네임스페이스로
변환한다. 대표 기물만 `deck_selectable_custom_type_ids`에 노출되며 내부
정의는 런타임 참조에만 사용한다.

커스텀 정의는 `is_king`과 `can_capture_on_drop` 권한을 획득할 수 없다.
기본 기물 ID, 다른 패키지 ID, 중복 런타임 ID는 덮어쓰지 않고 오류로
반환한다.

## 게임 런타임과 행동

`install_runtime_catalog(&mut GameState, packages)`가 기본 정의가 이미 들어
있는 게임별 `piece_definitions`에 승인된 패키지 정의를 설치하고 체섬블리
프로그램 캐시를 재구축한다. 기존 legal move, 공격 범위, 포켓 착수, 행동
검증/적용과 AI 경로는 모두 이 `GameState`의 동일한 정의 snapshot을
참조한다.

`transition(target)`은 후보 `MoveAction.effects.piece_type_transition`에만
기록된다. legal move/attack 조회는 불변이며 canonical action이
`submit_action`을 통과해 commit될 때 대상 정의로 타입과 초기 상태가
변경된다. 불법 action에는 어떤 전환도 적용되지 않는다.

## 직렬화와 복원

`serialize_game_snapshot`과 `restore_game_snapshot`을 사용한다. 기존
`GameState` JSON 안에 전체 정의 snapshot과 다음 manifest가 함께 저장된다.

- package ID와 불변 version
- 원문 content hash
- 대표 runtime type ID
- package에 속한 모든 runtime type ID
- 정렬된 정의 snapshot hash

복원은 외부 저장소나 최신 package를 조회하지 않는다. 기물의 정의 누락,
manifest 정의 누락, 대표 정의 누락, 정의 snapshot hash 불일치는
`CustomPieceError`로 차단하고 프로그램 캐시만 검증된 snapshot에서
재생성한다.

## 공개 오류

`CustomPieceError`는 다음 상태를 구분한다.

- `ParseFailure`
- `SemanticValidation`
- `MissingExposedPiece`
- `MissingInternalReference`
- `IdentifierCollision`
- `ExecutionLimitExceeded`
- `UnsupportedFeature`
- `CorruptSnapshot`
- `DefinitionVersionMismatch`

현재 compile 제한은 원문 64 KiB와 패키지당 16개 정의다. 파싱 결과가 비어
있는 비어 있지 않은 프로그램은 오류로 처리한다.
`run_chessembly_layer_for_piece_checked`는 호출자가 지정한 expression step
budget을 강제하고 초과를 `ExecutionLimitExceeded("execution_steps")`로
반환한다. 기존 기본 기물 호환 API는 100,000 step 기본 예산을 사용한다.
위치가 포함된 완전한 parser diagnostic은 아직 구현되지 않은 엔진 제한이다.

## 서버 단계 사용 순서

1. 서버가 원문, package ID, version, 대표 local key와 점수로
   `CustomPiecePackageInput`을 만든다.
2. `validate_and_build_custom_piece_package` 결과만 승인된 package로
   취급한다.
3. 게임 생성 시 기본 정의가 들어 있는 `GameState`에
   `install_runtime_catalog`를 한 번 호출한다.
4. 덱 기물 instance에는 package의 `exposed_type_id`를 사용한다.
5. 게임 저장/복원은 snapshot 함수로 수행한다.

서버 API, DB, 소유권, 이미지와 프론트엔드는 이 단계에서 구현하지 않았다.
