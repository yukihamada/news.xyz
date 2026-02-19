#!/usr/bin/env bash
set -euo pipefail

STACK_NAME="${1:-hypernews}"
REGION="${AWS_DEFAULT_REGION:-ap-northeast-1}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKEND_DIR="$(dirname "$SCRIPT_DIR")"
FRONTEND_DIR="$BACKEND_DIR/../frontend"

echo "=== HyperNews Deploy ==="
echo "Stack: $STACK_NAME"
echo "Region: $REGION"

# --- 1. Build Rust lambdas ---
echo ""
echo ">>> Building Rust lambdas (ARM64)..."
cd "$BACKEND_DIR"

# Use cargo-lambda for building Lambda-compatible binaries
cargo lambda build --release --arm64

echo "Build complete."

# --- 2. SAM Deploy ---
echo ""
echo ">>> Deploying SAM stack..."
cd "$SCRIPT_DIR"

sam deploy \
  --template-file template.yaml \
  --stack-name "$STACK_NAME" \
  --region "$REGION" \
  --capabilities CAPABILITY_IAM \
  --resolve-s3 \
  --no-confirm-changeset \
  --no-fail-on-empty-changeset

# --- 3. Get outputs ---
echo ""
echo ">>> Fetching stack outputs..."
BUCKET=$(aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --region "$REGION" \
  --query "Stacks[0].Outputs[?OutputKey=='FrontendBucket'].OutputValue" \
  --output text)

CF_URL=$(aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --region "$REGION" \
  --query "Stacks[0].Outputs[?OutputKey=='CloudFrontUrl'].OutputValue" \
  --output text)

API_URL=$(aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --region "$REGION" \
  --query "Stacks[0].Outputs[?OutputKey=='ApiUrl'].OutputValue" \
  --output text)

CONFIG_TABLE=$(aws cloudformation describe-stacks \
  --stack-name "$STACK_NAME" \
  --region "$REGION" \
  --query "Stacks[0].Outputs[?OutputKey=='ConfigTableName'].OutputValue" \
  --output text)

# --- 4. Seed ConfigTable from feeds.toml (if empty) ---
echo ""
echo ">>> Checking ConfigTable seed data..."
EXISTING=$(aws dynamodb query \
  --table-name "$CONFIG_TABLE" \
  --region "$REGION" \
  --key-condition-expression "pk = :pk AND begins_with(sk, :prefix)" \
  --expression-attribute-values '{":pk":{"S":"CONFIG"},":prefix":{"S":"FEEDS#"}}' \
  --select COUNT \
  --query "Count" \
  --output text 2>/dev/null || echo "0")

if [ "$EXISTING" = "0" ]; then
  echo ">>> Seeding ConfigTable from feeds.toml..."
  FEEDS_FILE="$BACKEND_DIR/feeds.toml"
  FEED_INDEX=0

  # Parse feeds.toml and insert into DynamoDB
  while IFS= read -r line; do
    if [[ "$line" =~ ^url\ =\ \"(.+)\" ]]; then
      FEED_URL="${BASH_REMATCH[1]}"
    elif [[ "$line" =~ ^source\ =\ \"(.+)\" ]]; then
      FEED_SOURCE="${BASH_REMATCH[1]}"
    elif [[ "$line" =~ ^category\ =\ \"(.+)\" ]]; then
      FEED_CATEGORY="${BASH_REMATCH[1]}"
      FEED_ID="seed-$(printf '%03d' $FEED_INDEX)"
      FEED_INDEX=$((FEED_INDEX + 1))

      aws dynamodb put-item \
        --table-name "$CONFIG_TABLE" \
        --region "$REGION" \
        --item "{
          \"pk\": {\"S\": \"CONFIG\"},
          \"sk\": {\"S\": \"FEEDS#$FEED_ID\"},
          \"feed_id\": {\"S\": \"$FEED_ID\"},
          \"url\": {\"S\": \"$FEED_URL\"},
          \"source\": {\"S\": \"$FEED_SOURCE\"},
          \"category\": {\"S\": \"$FEED_CATEGORY\"},
          \"enabled\": {\"BOOL\": true},
          \"added_by\": {\"S\": \"seed\"}
        }"
      echo "  Seeded: $FEED_SOURCE ($FEED_CATEGORY)"
    fi
  done < "$FEEDS_FILE"

  echo ">>> Seed complete: $FEED_INDEX feeds"
else
  echo ">>> ConfigTable already has $EXISTING feeds, skipping seed."
fi

# --- 5. Deploy frontend to S3 ---
echo ""
echo ">>> Syncing frontend to S3..."
aws s3 sync "$FRONTEND_DIR" "s3://$BUCKET" \
  --region "$REGION" \
  --delete \
  --cache-control "public, max-age=31536000, immutable" \
  --exclude "*.html" \
  --exclude "manifest.json" \
  --exclude "sw.js"

# HTML, manifest, SW — shorter cache
aws s3 sync "$FRONTEND_DIR" "s3://$BUCKET" \
  --region "$REGION" \
  --cache-control "public, max-age=300" \
  --exclude "*" \
  --include "*.html" \
  --include "manifest.json" \
  --include "sw.js"

# --- 6. Invalidate CloudFront ---
echo ""
echo ">>> Invalidating CloudFront cache..."
DIST_ID=$(aws cloudfront list-distributions \
  --query "DistributionList.Items[?DomainName=='$(echo $CF_URL | sed 's|https://||')'].Id" \
  --output text)

if [ -n "$DIST_ID" ]; then
  aws cloudfront create-invalidation \
    --distribution-id "$DIST_ID" \
    --paths "/*" > /dev/null
  echo "CloudFront invalidation created."
fi

# --- Done ---
echo ""
echo "=== Deploy Complete ==="
echo "Frontend: $CF_URL"
echo "API:      $API_URL"
echo "S3:       s3://$BUCKET"
echo "Config:   $CONFIG_TABLE"
echo ""
echo "NOTE: Set your Anthropic API key with:"
echo "  aws secretsmanager put-secret-value \\"
echo "    --secret-id $STACK_NAME/anthropic-api-key \\"
echo "    --secret-string 'sk-ant-...' \\"
echo "    --region $REGION"
