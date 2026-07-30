# 커스텀 기물 백엔드 계약

상태: 3단계 시제품 구현  
인증 임시 계약: 모든 API는 `X-User-Id` 헤더를 요구한다. 이 값은 계정
미들웨어가 도입될 때 인증 principal extractor로 교체해야 하며, 클라이언트가
임의로 정한 값을 운영 인증으로 간주해서는 안 된다.

## 저장, 버전과 역방향 절차

현재 프로젝트에는 DB, 계정 provider 또는 마이그레이션 도구가 없다. 따라서
`CustomPieceRepository` 경계 뒤의 격리된 인메모리 저장소가 시제품
구현이다. 서버 재시작 시 커스텀 기물과 업로드 이미지가 사라지는 것이
알려진 제한이다. 브라우저 저장소에는 영속화하지 않는다.

정의 수정은 기존 레코드를 덮어쓰지 않고 단조 증가하는 불변 버전을 추가한다.
서버는 엔진이 원문으로 계산한 콘텐츠 해시를 저장하며 클라이언트 버전/해시는
입력으로 받지 않는다. 삭제는 최신 기물을 목록과 최신 조회에서 숨기는 soft
delete이며 과거 버전은 버전 API와 이후 덱/게임 복원을 위해 보존한다.

현 단계에는 적용할 DB schema가 없으므로 DB migration 명령도 없다. 역방향
절차는 새 라우트를 제거하고 `AppState.custom_pieces`를 제거하는 것이며 기존
게임/방 데이터 형식에는 migration이 없다. 실제 DB adapter를 추가하는
단계에서는 이 문서의 필드를 immutable version 및 image owner 테이블로
옮기고 up/down migration을 함께 추가해야 한다.

## API

모든 경로는 `/api` 기준이다.

- `GET /custom-pieces`: 인증 사용자 활성 기물 목록
- `POST /custom-pieces`: 생성
- `GET /custom-pieces/:id`: 활성 최신 버전 상세
- `PUT /custom-pieces/:id`: 전체 수정. `expected_version` 필수
- `DELETE /custom-pieces/:id`: soft delete. JSON `expected_version` 필수
- `GET /custom-pieces/:id/versions/:version`: 소유자의 불변 과거 버전
- `POST /custom-pieces/validate`: 저장 전 엔진 검증
- `POST /custom-piece-images`: `{filename, media_type, bytes}` 이미지 등록
- `POST /custom-pieces/test/options`: 가능한 행동 조회(상태 불변)
- `POST /custom-pieces/test/actions`: canonical 행동 적용

기물 입력은 `{name, description, score, image, raw_script,
exposed_piece_key}`이다. `image`는 `{kind:"built_in",asset_key}` 또는
`{kind:"uploaded",asset_id}`이다. 생성/수정 시 이름 공백, 길이, 점수,
원문 크기와 대표 ID를 검사하고 엔진의
`validate_and_build_custom_piece_package`를 다시 실행한다.

테스트 API의 `definition`은 위 draft 입력 또는
`{custom_piece_id,version}`이다. `board`는 8~12 크기, 현재 플레이어와
`{id,piece_key,owner,square,state}` 기물 목록이다. 서버는 좌표, 중복 ID,
owner, 정의 참조와 상태 schema를 검사한 뒤 엔진 카탈로그를 설치한다.

## 이미지 정책

업로드 바이트는 파일명과 분리된 UUID asset ID로 저장한다. 512 KiB,
각 변 2048 이하만 허용한다. PNG signature/IHDR/IEND, JPEG SOF/EOI,
SVG UTF-8/viewBox를 확인하고 선언 MIME 및 확장자와 실제 형식을 비교한다.
SVG는 script, 이벤트 속성, 외부 URL/resource, style, foreignObject 및
HTML embed 계열을 거부한다. 기본 이미지는 고정 allowlist key로만 참조한다.

## 오류

오류는 `{error, code}`이며 인증 실패 401, 미존재/타 소유 리소스 404,
낙관적 버전 충돌 409, 입력/엔진 검증 실패 422를 사용한다. 엔진의 상세
내부 상태나 원문/이미지는 로그 또는 오류에 포함하지 않는다.
