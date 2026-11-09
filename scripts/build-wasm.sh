#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET="wasm32-unknown-unknown"
PROFILE="wasm-release"
APP_NAME="earth_orbit"
OUT_DIR="dist"

if ! rustup target list --installed | grep -q "^${TARGET}$"; then
  rustup target add "${TARGET}"
fi

# Cargo создаёт Cargo.lock при первой сборке, если его ещё нет.
cargo build --profile "${PROFILE}" --target "${TARGET}"

# wasm-bindgen-cli должен иметь ту же версию, что и библиотека wasm-bindgen,
# с которой собран исполняемый WASM-файл.
WASM_BINDGEN_VERSION="$(awk '
  /^\[\[package\]\]$/ { in_package = 1; is_wasm_bindgen = 0; next }
  in_package && /^name = "wasm-bindgen"$/ { is_wasm_bindgen = 1; next }
  in_package && is_wasm_bindgen && /^version = / {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' Cargo.lock)"

if [[ -z "${WASM_BINDGEN_VERSION}" ]]; then
  echo "Could not find wasm-bindgen in Cargo.lock" >&2
  exit 1
fi

if ! command -v wasm-bindgen >/dev/null 2>&1 \
   || ! wasm-bindgen --version | grep -q "${WASM_BINDGEN_VERSION}"; then
  cargo install wasm-bindgen-cli --version "${WASM_BINDGEN_VERSION}" --locked
fi

rm -rf "${OUT_DIR}"
mkdir -p "${OUT_DIR}/assets"

wasm-bindgen \
  "target/${TARGET}/${PROFILE}/${APP_NAME}.wasm" \
  --target web \
  --out-dir "${OUT_DIR}" \
  --out-name "${APP_NAME}"

cp index.html "${OUT_DIR}/index.html"
cp -R assets/. "${OUT_DIR}/assets/"
touch "${OUT_DIR}/.nojekyll"

echo "WASM site was generated in ./${OUT_DIR}/"
echo "Test locally with: python3 -m http.server 8000 --directory ${OUT_DIR}"
