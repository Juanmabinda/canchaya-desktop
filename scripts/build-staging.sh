#!/usr/bin/env bash
# Build staging del wrapper desktop. Hostname y bundle ID separados de prod
# para que las dos versiones puedan convivir en la misma maquina sin
# pisarse el token storage ni los datos del WebView.
#
# Uso: ./scripts/build-staging.sh

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CANCHAYA_SERVER_URL="${CANCHAYA_SERVER_URL:-https://staging.canchaya.ar}"

echo "==> Build staging contra $CANCHAYA_SERVER_URL"

CANCHAYA_SERVER_URL="$CANCHAYA_SERVER_URL" \
  npm run tauri build -- \
    --config src-tauri/tauri.staging.json \
    --bundles app

echo
echo "==> Listo:"
echo "  $ROOT_DIR/src-tauri/target/release/bundle/macos/CanchaYa POS (Staging).app"
