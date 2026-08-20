# 커스텀 기물 PostgreSQL 설정

Google 계정, Identity Platform, guest 승격/import, `users` FK를 포함한 현재 설정은 [account-auth-setup.md](account-auth-setup.md)를 참고한다.

## 필요한 환경 변수

- `APP_ENV`: 로컬은 `local`, Cloud Run은 `prod`
- `DATABASE_URL`: PostgreSQL 접속 URL. 코드나 Git에 커밋하지 않는다.
- `AUTH_SIGNING_KEY`: 32바이트 이상의 장기 고정 무작위 문자열. 이 값을 바꾸면 기존 게스트 세션이 모두 무효화된다.

`.env.example`은 형식만 보여 준다. 실제 `.env`는 `.gitignore`에서 제외된다.

## 로컬에서 처음 설정

1. PostgreSQL 14 이상을 실행한다.
2. 데이터베이스와 전용 사용자를 만든다.

   ```sql
   CREATE USER deck_chess WITH PASSWORD 'replace_this_password';
   CREATE DATABASE deck_chess OWNER deck_chess;
   ```

3. 터미널에서 secret을 직접 기록하지 않고 현재 shell에만 설정한다.

   ```bash
   export APP_ENV=local
   export DATABASE_URL='postgresql://deck_chess:replace_this_password@127.0.0.1:5432/deck_chess'
   export AUTH_SIGNING_KEY='replace-with-a-random-string-of-at-least-32-bytes'
   cargo run -p brainfuck-chess-server
   ```

4. 격리 schema는 서버 시작 전에 관리자가 적용해야 한다. 서버는 migration을
   자동 실행하지 않으며 schema 계약이 없으면 시작을 차단한다. 자세한 절차는
   [database-environment-isolation.md](database-environment-isolation.md)를 따른다.
5. 세션 발급은 브라우저가 첫 커스텀 기물 API의 401을 받을 때 자동으로 수행한다.

## Cloud Run에서 처음 설정

1. GCP Console에서 Cloud SQL for PostgreSQL 인스턴스를 만들고 DB/사용자를 추가한다. 또는 Cloud Run에서 접속 가능한 관리형 PostgreSQL을 준비한다.
2. Secret Manager에 `deck-chess-database-url`과 `deck-chess-auth-signing-key` secret을 만든다.
3. Cloud SQL을 쓰는 경우 `deck-chess-database-url`의 값을 다음 형식으로 저장한다. 사용자, 비밀번호, DB명과 connection name을 실제 값으로 바꾼다.

   ```text
   postgresql://deck_chess:PASSWORD@localhost/deck_chess?host=/cloudsql/PROJECT_ID:REGION:INSTANCE_NAME
   ```

   비밀번호에 `@`, `:`, `/`, `?` 등 URL 특수 문자가 있으면 percent-encoding한다.

   다른 관리형 PostgreSQL을 쓰면 provider가 제공하는 TLS PostgreSQL URL을 저장한다.
4. `deck-chess-auth-signing-key`에는 다음처럼 생성한 값을 저장한다.

   ```bash
   openssl rand -base64 48
   ```

5. Cloud Run runtime service account에 두 secret의 Secret Manager Secret Accessor 권한과 DB 접속에 필요한 권한을 준다.
6. Cloud Build trigger의 substitution에서 실제 secret 이름이 다르면 `_DATABASE_URL_SECRET`, `_AUTH_SIGNING_KEY_SECRET`를 바꾼다. Cloud SQL connection name에 맞게 `_CLOUD_SQL_INSTANCE`도 바꾼다.
7. 배포한다. `cloudbuild.yaml`이 두 secret을 Cloud Run 환경 변수로 매핑한다.
8. Cloud Run revision 로그에 `server startup blocked` 오류가 없는지 확인한다.
   DB 연결, schema 계약 또는 반대 환경 권한 검사가 실패하면 안전하게 시작이 차단된다.

## 재시작 영속성 검증

1. 커스텀 기물을 하나 저장한다.
2. 서버를 종료한 뒤 같은 `DATABASE_URL`로 다시 시작한다.
3. 커스텀 기물 목록에 기물이 남아 있는지 확인한다.
4. 자동화된 통합 테스트는 격리된 DB에서 다음과 같이 실행한다.

   ```bash
   TEST_DATABASE_URL='postgresql://...' cargo test -p brainfuck-chess-server postgres_repository_survives_reconnection -- --ignored
   ```

## 현재 세션의 제한

이 버전은 기존에 계정 시스템이 없었기 때문에 서버가 서명한 게스트 세션을
사용한다. 클라이언트는 타인으로 가장할 수 없지만, 브라우저 쿠키를 삭제하면
기존 소유권을 다시 회복할 수 없고 다른 기기와 자동 동기화되지 않는다. 향후 실제
계정 provider를 도입할 때는 서명 principal을 계정 ID로 교체해야 한다.
