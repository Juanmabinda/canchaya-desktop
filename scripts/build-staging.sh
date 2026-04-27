#!/usr/bin/env bash
# Build staging del wrapper desktop. Hostname y bundle ID separados de prod
# para que las dos versiones puedan convivir en la misma maquina sin
# pisarse el token storage ni los datos del WebView.
#
# Uso: ./scripts/build-staging.sh
#
# Requiere keypair de updater en ~/.tauri/canchaya-desktop. Si no existe,
# generala con: CI=true npx @tauri-apps/cli signer generate -p "" -w ~/.tauri/canchaya-desktop -f
# (la pubkey va en tauri.conf.json — manténganlas alineadas).

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/canchaya-desktop}"

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  if [[ ! -f "$KEY_PATH" ]]; then
    echo "ERROR: no se encontro la signing key en $KEY_PATH"
    echo "Generala con:"
    echo "  CI=true npx @tauri-apps/cli signer generate -p '' -w \"$KEY_PATH\" -f"
    echo "Y actualizá la pubkey en src-tauri/tauri.conf.json."
    exit 1
  fi
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_PATH")"
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

CANCHAYA_SERVER_URL="${CANCHAYA_SERVER_URL:-https://staging.canchaya.ar}"

echo "==> Build staging contra $CANCHAYA_SERVER_URL"

CANCHAYA_SERVER_URL="$CANCHAYA_SERVER_URL" \
  npm run tauri build -- \
    --config src-tauri/tauri.staging.json \
    --bundles app

echo
echo "==> Listo:"
echo "  $ROOT_DIR/src-tauri/target/release/bundle/macos/CanchaYa POS (Staging).app"
