#!/usr/bin/env bash
# GCP Cloud Run 이미지 수동 재설치용 스크립트.
# 사용법:
#   ./reinstall-image.sh test
#   ./reinstall-image.sh prod
#   ./reinstall-image.sh test custom-tag

set -Eeuo pipefail

PROJECT_ID="var-chess-bfc"
REPOSITORY="cloud-run-source-deploy"
IMAGE_NAME="brainfuck_chess/brainfuck-chess"

TARGET="${1:-test}"
CUSTOM_TAG="${2:-}"

case "$TARGET" in
  prod|main)
    SERVICE="brainfuck-chess"
    RUN_REGION="us-central1"
    AR_REGION="us-central1"
    APP_ENV="prod"
    SHOW_TEST_UI="false"
    TAG_PREFIX="prod"
    EXPECTED_BRANCH="main"
    ;;

  test|dev|develop)
    SERVICE="brainfuck-chess-test"
    RUN_REGION="europe-west1"
    AR_REGION="europe-west1"
    APP_ENV="test"
    SHOW_TEST_UI="true"
    TAG_PREFIX="test"
    EXPECTED_BRANCH="develop"
    ;;

  *)
    echo "사용법: $0 [test|prod] [tag]"
    exit 1
    ;;
esac

CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"

if [[ "$CURRENT_BRANCH" != "$EXPECTED_BRANCH" ]]; then
  echo "현재 브랜치: $CURRENT_BRANCH"
  echo "예상 브랜치: $EXPECTED_BRANCH"
  echo "실수 방지를 위해 중단함."
  exit 1
fi

SHORT_SHA="$(git rev-parse --short HEAD 2>/dev/null || date +%Y%m%d-%H%M%S)"
TAG="${CUSTOM_TAG:-$TAG_PREFIX-$SHORT_SHA-$(date +%Y%m%d-%H%M%S)}"

IMAGE="$AR_REGION-docker.pkg.dev/$PROJECT_ID/$REPOSITORY/$IMAGE_NAME:$TAG"
LATEST_IMAGE="$AR_REGION-docker.pkg.dev/$PROJECT_ID/$REPOSITORY/$IMAGE_NAME:$TARGET-latest"

echo "TARGET=$TARGET"
echo "SERVICE=$SERVICE"
echo "RUN_REGION=$RUN_REGION"
echo "AR_REGION=$AR_REGION"
echo "APP_ENV=$APP_ENV"
echo "IMAGE=$IMAGE"
echo "LATEST_IMAGE=$LATEST_IMAGE"

echo "== Artifact Registry 로그인 =="
gcloud auth print-access-token | docker login \
  -u oauth2accesstoken \
  --password-stdin "https://$AR_REGION-docker.pkg.dev"

echo "== Docker 이미지 새로 빌드 =="
docker build \
  -t "$IMAGE" \
  -t "$LATEST_IMAGE" \
  .

echo "== Docker 이미지 push =="
docker push "$IMAGE"
docker push "$LATEST_IMAGE"

echo "== Cloud Run 재배포 =="
gcloud run deploy "$SERVICE" \
  --image "$IMAGE" \
  --region "$RUN_REGION" \
  --allow-unauthenticated \
  --ingress all \
  --default-url \
  --update-env-vars "APP_ENV=$APP_ENV,SHOW_TEST_UI=$SHOW_TEST_UI" \
  --quiet

echo "== 완료 =="
gcloud run services describe "$SERVICE" \
  --region "$RUN_REGION" \
  --format='table(status.url,spec.template.spec.containers[0].image,status.latestReadyRevisionName)'