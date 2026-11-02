#!/usr/bin/env bash
# Verifies that a running service answers the baseline endpoints.
# Usage: tools/smoke.sh [base-url]
set -euo pipefail

BASE_URL="${1:-${OK_BASE_URL:-http://127.0.0.1:8080}}"

check() {
  local path="$1"
  local expected="${2:-200}"
  local code

  code="$(curl --silent --output /dev/null --write-out '%{http_code}' "$BASE_URL$path")"
  if [[ "$code" != "$expected" ]]; then
    echo "FAIL $path: expected $expected, got $code" >&2
    exit 1
  fi
  echo "OK   $path -> $code"
}

check "/healthz"
check "/readyz"
check "/api/v1/openapi.json"
