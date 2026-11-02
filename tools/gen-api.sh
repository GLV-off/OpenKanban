#!/usr/bin/env bash
# Regenerates frontend API types from the running service OpenAPI document.
# Usage: tools/gen-api.sh [base-url]
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
API_URL="${1:-${OK_API_URL:-http://127.0.0.1:8080}}"
WEB_DIR="$ROOT_DIR/frontend/open_kanban_web"
SPEC_PATH="$ROOT_DIR/out/openapi.json"
OUTPUT_PATH="$WEB_DIR/src/shared/api/schema.d.ts"

mkdir -p "$ROOT_DIR/out"

echo "Fetching $API_URL/api/v1/openapi.json"
curl --fail --silent --show-error "$API_URL/api/v1/openapi.json" --output "$SPEC_PATH"

echo "Generating TypeScript types"
cd "$WEB_DIR"
npx openapi-typescript "$SPEC_PATH" --output "$OUTPUT_PATH"

echo "Generated: $OUTPUT_PATH"
