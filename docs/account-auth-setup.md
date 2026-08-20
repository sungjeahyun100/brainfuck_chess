# Deck Chess 계정, Identity Platform, PostgreSQL 설정

이 문서는 Google Cloud project `var-chess-bfc`의 2026-08-20 설정을 기준으로 한다. Production Cloud Run service는 `brainfuck-chess` (`us-central1`), 계정/DB 전용 runtime service account는 `deck-chess-runtime@var-chess-bfc.iam.gserviceaccount.com`이다.

## Architecture

```text
Browser -- Google popup --> Identity Platform
   |                         |
   +---- verified ID token --+
                |
                v
        Deck Chess backend
        - RS256/JWKS + claim verification
        - guest upgrade / opt-in import
        - expiring HttpOnly session
                |
                v
           PostgreSQL
 shared.users -> shared.auth_identities
       |       UNIQUE(issuer, subject)
       +-> prod/test.custom_piece_versions
       +-> prod/test.custom_piece_images
```

Google email은 user ID가 아니다. 모든 소유권은 Deck Chess `users.id`를 사용하고 Google identity는 `auth_identities(issuer, subject)`로 별도 저장한다. ID/access/refresh token은 DB에 저장하지 않는다.

## Guest migration과 login

- migration은 기존 custom piece/image `owner_id`를 같은 ID의 guest `users` row로 backfill한 뒤 FK를 추가한다.
- 아직 연결되지 않은 Google identity로 처음 login하면 현재 guest ID를 그대로 registered user로 승격한다.
- 기존 account에 연결된 identity로 login하고 guest data가 있으면 backend은 `guest_import_required` 409를 반환한다.
- 가져오기를 선택해야만 모든 immutable version과 image ownership을 하나의 PostgreSQL transaction에서 이전한다.
- 가져오지 않으면 guest row/data를 삭제하지 않는다. 단 account cookie로 교체된 후 해당 guest를 다시 열는 UI는 현재 없으므로, 기존 cookie/browser profile이 없으면 접근하기 어렵다.
- `users.id`는 세션과 소유권 FK가 참조하는 내부 식별자이므로 사용자 설정에서 변경하지 않는다.
- 사용자가 변경하는 개인 ID는 `users.public_id`에 별도로 저장한다. 영문 소문자, 숫자, 밑줄 3~20자이며 첫 글자는 영문 또는 숫자여야 하고, 시스템 예약어와 중복 ID는 허용하지 않는다.
- 개인 ID 변경은 로그인된 동일 origin 요청인 `PATCH /api/auth/profile`로만 처리한다.

## 환경 변수

| 이름 | 범위 | 비밀 | production | 용도 |
|---|---|---:|---:|---|
| `APP_ENV` | backend/public | 아님 | 필수 | `local`, `test`, `prod` |
| `DATABASE_URL` | backend | 비밀 | 필수 | PostgreSQL URL |
| `AUTH_SIGNING_KEY` | backend | 비밀 | 필수 | 32 bytes 이상 HMAC key; 회전 시 session 무효화 |
| `SESSION_TTL_SECONDS` | backend | 아님 | 선택 | 300초~90일, 기본 30일 |
| `IDENTITY_PLATFORM_PROJECT_ID` | backend/public | 아님 | 필수 | token audience/issuer |
| `FIREBASE_API_KEY` | public | 아님 | UI 필수 | Identity Platform Web API key |
| `FIREBASE_AUTH_DOMAIN` | public | 아님 | UI 필수 | 보통 `<PROJECT_ID>.firebaseapp.com` |
| `FIREBASE_APP_ID` | public | 아님 | UI 필수 | Web app ID |

`FIREBASE_*`는 공개 web config이다. OAuth client secret, service-account JSON, DB password, `AUTH_SIGNING_KEY`는 `/config.js`에 넣지 않는다.

## Local development

DB 없이 실행하면 guest/account/custom-piece data는 restart 시 사라진다.

```bash
APP_ENV=local AUTH_SIGNING_KEY='at-least-32-random-characters-here' \
  cargo run -p brainfuck-chess-server
```

Google login을 테스트하려면 Identity Platform authorized domain에 `localhost`를 추가하고 frontend/backend 모두에 같은 project ID를 설정한다.

```bash
APP_ENV=local \
IDENTITY_PLATFORM_PROJECT_ID='var-chess-bfc' \
FIREBASE_API_KEY='public-web-api-key' \
FIREBASE_AUTH_DOMAIN='var-chess-bfc.firebaseapp.com' \
FIREBASE_APP_ID='public-web-app-id' \
npm run dev
```

Backend는 Google secure-token public cert를 `Cache-Control: max-age`동안 cache하므로 login 시 외부 network가 필요하다.

## Identity Platform / Google provider Console 설정

현재 UI 명칭은 [Identity Platform Google login 공식 문서](https://cloud.google.com/identity-platform/docs/web/google)를 기준으로 한다.

1. Google Cloud Console에서 project `var-chess-bfc`를 선택한다.
2. **Google Auth Platform** > **Overview** > **Get started**에서 OAuth 동의 화면을 구성한다.
   - App name: `Deck Chess`
   - User support email과 developer contact email: 실제 관리 이메일
   - Audience: 외부 사용자에게 제공할 서비스이면 **External**
   - 게시 전에는 **Audience**에서 실제 로그인할 계정을 test user로 추가한다. 전체 사용자에게 제공할 때만 영향 범위를 확인하고 **Publish app**을 선택한다.
3. **Google Auth Platform** > **Clients** > **Create client**에서 **Web application** client를 만든다.
   - Authorized JavaScript origins:
     - `https://brainfuck-chess-nipfcne3sq-uc.a.run.app`
     - `https://var-chess-bfc.firebaseapp.com`
     - `https://var-chess-bfc.web.app`
     - local 개발이 필요하면 `http://localhost:5173`
   - Authorized redirect URIs:
     - `https://var-chess-bfc.firebaseapp.com/__/auth/handler`
     - `https://var-chess-bfc.web.app/__/auth/handler`
4. 생성된 Web Client ID와 Web Client Secret을 안전하게 보관한다. Secret은 source, frontend, `/config.js`, 문서, log에 기록하지 않는다.
5. **Identity Platform** > **Providers** > **Add a provider** > **Google**을 선택한다.
6. Google provider를 **Enabled**로 바꾸고 3단계의 Web Client ID와 Web Client Secret을 입력한 뒤 저장한다.
7. **Settings** > **Authorized domains**에 scheme/path 없이 다음을 추가한다.
   - `brainfuck-chess-nipfcne3sq-uc.a.run.app`
   - 실제 production custom domain
   - local test용 `localhost`
8. Project settings에 Web app(`Deck Chess Web`)을 등록한다. Firebase Hosting은 필요 없다.
9. `apiKey`, `authDomain`, `projectId`, `appId`만 frontend public config에 복사한다. OAuth client secret은 frontend에 넣지 않는다.
10. Gmail/Drive/Calendar scope를 추가하지 않는다.

Firebase Authentication Console에서 Google sign-in을 활성화하면 기반 OAuth client가 자동 생성되는 경로도 있지만, Identity Platform Console과 REST API의 Google provider 설정은 Web Client ID와 Secret을 요구한다. `var-chess-bfc`에서는 2026-08-20 확인 당시 Web App만 등록되어 있었고 OAuth client와 `google.com` provider는 없었으므로 위 순서로 Web client를 명시적으로 생성해 연결했다.

Backend는 [Firebase/Identity Platform ID token 공식 검증 규칙](https://firebase.google.com/docs/auth/admin/verify-id-tokens)에 따라 RS256, `kid`, Google cert, `exp`, `iat`, `auth_time`, `aud`, `iss`, `sub`, `firebase.sign_in_provider=google.com`을 검사한다.

## Cloud SQL 생성(비용 발생)

생성 전 다음 target과 Console의 월 예상 비용을 다시 확인한다.

```text
project: var-chess-bfc
region: us-central1
Cloud Run service: brainfuck-chess
instance: deck-chess-postgres
database: deck_chess
application DB user: deck_chess
connection: /cloudsql/var-chess-bfc:us-central1:deck-chess-postgres
```

1. **Cloud SQL** > **Create instance** > **PostgreSQL**로 이동한다.
2. 초기 트래픽에는 보수적 shared-core 급을 고려할 수 있지만 Console의 가격, SLA, backup/PITR 제약을 확인한다. 비용만을 위해 backup을 무조건 끄지 않는다.
3. **Databases**에 `deck_chess`, **Users**에 `deck_chess`를 생성한다. `postgres` 관리자를 app runtime user로 쓰지 않는다.
4. password는 강한 random value를 사용하고 terminal history/source/issue에 기록하지 않는다.

Cloud Run은 [Cloud Run–Cloud SQL PostgreSQL 공식 문서](https://cloud.google.com/sql/docs/postgres/connect-run)의 Unix socket을 사용한다. 전용 runtime service account에 `roles/cloudsql.client`가 필요하며, default Compute service account에 추가 운영 권한을 부여하지 않는다.

## Secret Manager / Cloud Run

1. Secret `deck-chess-database-url`, `deck-chess-auth-signing-key`를 생성한다.
2. `DATABASE_URL` 예시(실제 password는 보고서/log에 출력하지 않는다):

   ```text
   postgresql://deck_chess:PERCENT_ENCODED_PASSWORD@localhost/deck_chess?host=/cloudsql/var-chess-bfc:us-central1:deck-chess-postgres
   ```

3. Runtime service account에 secret의 `roles/secretmanager.secretAccessor`를 최소 범위로 부여한다.
4. Cloud Run **Edit and deploy new revision** > **Container(s), Volumes, Networking, Security**에서 Cloud SQL connection을 추가한다.
5. `DATABASE_URL`, `AUTH_SIGNING_KEY`를 secret reference로, public Firebase config와 `IDENTITY_PLATFORM_PROJECT_ID=var-chess-bfc`를 일반 env로 추가한다.

Google은 secret env에 [Secret Manager version pinning](https://cloud.google.com/run/docs/configuring/services/secrets)을 권장한다. `cloudbuild.yaml`의 `latest`는 bootstrap 편의용이므로 운영 정책 확정 후 version number로 고정하는 것을 권장한다.

## Migration / deployment

- startup migration은 실행하지 않는다. shared/prod/test 전환과 별도 release
  절차는 [database-environment-isolation.md](database-environment-isolation.md)를 따른다.
- production과 배포형 test는 `DATABASE_URL`, `AUTH_SIGNING_KEY`,
  `IDENTITY_PLATFORM_PROJECT_ID`가 누락되면 startup을 차단한다. local만 명시적으로
  개발용 fallback을 허용한다.
- 현재 GCP trigger는 repository `cloudbuild.yaml`을 명시적으로 사용하는 trigger로 확인되지 않았다. Console source-deploy를 계속 쓰면 위 connection/env/secret을 Cloud Run revision에 직접 설정한다.
- Production migration 전 backup을 확인하고 `DROP`, `TRUNCATE`, 전체 table 재생성을 하지 않는다.

## Verification

```bash
cargo test --workspace
cargo build --release
cd frontend
npm test
npm run typecheck
npm run build
```

격리된 PostgreSQL test DB가 있을 때:

```bash
TEST_DATABASE_URL='postgresql://...' \
  cargo test -p brainfuck-chess-server postgres_repository_survives_reconnection -- --ignored
```

Production smoke test:

1. `/api/health` 200과 `/config.js`의 public field만 확인한다.
2. 새 browser에서 guest piece를 만들고 첫 login 후 그대로 보이는지 확인한다.
3. 다른 browser에서 같은 Google account로 login해 동일 piece를 확인한다.
4. Guest import modal의 두 선택을 각각 확인한다.
5. Logout 후 `/api/auth/me`가 authenticated user를 반환하지 않는지 확인한다.
6. 조작 cookie, `X-User-Id`, 다른 user의 piece/image ID, malformed/expired/wrong-audience token을 거부하는지 확인한다.

## Troubleshooting

- `DATABASE_URL is required for APP_ENV=...`: Cloud Run secret mapping과 secret accessor IAM을 확인한다.
- `failed to connect to PostgreSQL`: Cloud SQL connection, `roles/cloudsql.client`, instance name, URL-encoded password를 확인한다.
- Google button disabled: `/config.js`의 네 public field가 실제 값인지 확인한다.
- `unauthorized-domain`: Identity Platform Authorized domains에 현재 hostname을 scheme 없이 추가한다.
- `invalid_id_token`: frontend/backend project ID, token expiry, Google provider, Google public-key endpoint network를 확인한다. Token 원문은 log에 남기지 않는다.
