#!/usr/bin/env bash
#GCP cloud run 이미지 수동 재설치용 스크립트.
set -Eeuo pipefail

PROJECT_ID="var-chess-bfc"
REGION="us-central1"
SERVICE="brainfuck-chess"
REPOSITORY="cloud-run-source-deploy"
IMAGE_NAME="brainfuck_chess/brainfuck-chess"

# 태그를 인자로 받음. 없으면 reinstall-날짜시간
TAG="${1:-reinstall-$(date +%Y%m%d-%H%M%S)}"

IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/$REPOSITORY/$IMAGE_NAME:$TAG"
LATEST_IMAGE="$REGION-docker.pkg.dev/$PROJECT_ID/$REPOSITORY/$IMAGE_NAME:latest"

echo "IMAGE=$IMAGE"
echo "LATEST_IMAGE=$LATEST_IMAGE"

echo "== Artifact Registry 로그인 =="
gcloud auth print-access-token | docker login \
  -u oauth2accesstoken \
  --password-stdin "https://$REGION-docker.pkg.dev"

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
  --region "$REGION" \
  --allow-unauthenticated \
  --ingress all \
  --default-url \
  --quiet

echo "== 완료 =="
gcloud run services describe "$SERVICE" \
  --region "$REGION" \
  --format='table(status.url,spec.template.spec.containers[0].image,status.latestReadyRevisionName)'