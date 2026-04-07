#!/usr/bin/env bash
set -euo pipefail

export INTERNAL_API_TOKEN="${INTERNAL_API_TOKEN:-change-me-in-production}"
export FQRS_SIDECAR_BASE_URL="${FQRS_SIDECAR_BASE_URL:-http://127.0.0.1:18080}"
export FQRS_SIDECAR_INTERNAL_TOKEN="${FQRS_SIDECAR_INTERNAL_TOKEN:-$INTERNAL_API_TOKEN}"

java --enable-native-access=ALL-UNNAMED -jar /app/fq-sidecar.jar &
SIDECAR_PID=$!

cleanup() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "${API_PID}" 2>/dev/null || true
    wait "${API_PID}" 2>/dev/null || true
  fi
  kill "${SIDECAR_PID}" 2>/dev/null || true
  wait "${SIDECAR_PID}" 2>/dev/null || true
}

trap cleanup INT TERM EXIT

sleep 2
if ! kill -0 "${SIDECAR_PID}" 2>/dev/null; then
  echo "sidecar failed to start" >&2
  wait "${SIDECAR_PID}"
  exit 1
fi

fq-api &
API_PID=$!

set +e
wait -n "${SIDECAR_PID}" "${API_PID}"
STATUS=$?
set -e

exit "${STATUS}"

