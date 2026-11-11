#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# --------------------------------------------------
# Configuration
# --------------------------------------------------

TOOLCHAIN="stable"
TARGET="wasm32-unknown-unknown"
PROFILE="wasm-release"
APP_NAME="earth_orbit"
OUT_DIR="dist"

# --------------------------------------------------
# 1. Configure Homebrew rustup
# --------------------------------------------------

export PATH="/opt/homebrew/bin:$PATH"

RUSTUP="$(command -v rustup)"

if [[ -z "${RUSTUP}" ]]; then
  echo "ERROR: rustup not found." >&2
  exit 1
fi

echo "======================================"
echo " Building ${APP_NAME} for WebAssembly"
echo "======================================"

echo ""
echo "==> Detecting Rust toolchain..."

# Get actual rustc and cargo from rustup.
RUSTC="$("${RUSTUP}" which --toolchain "${TOOLCHAIN}" rustc)"
CARGO="$("${RUSTUP}" which --toolchain "${TOOLCHAIN}" cargo)"

if [[ ! -x "${RUSTC}" || ! -x "${CARGO}" ]]; then
  echo "ERROR: Rust toolchain binaries not found." >&2
  exit 1
fi

# Force Cargo to use the correct compiler.
export RUSTC

echo "Rustup:   ${RUSTUP}"
echo "Rustc:    ${RUSTC}"
echo "Cargo:    ${CARGO}"

"${RUSTC}" --version
"${CARGO}" --version

# --------------------------------------------------
# 2. Check WASM target
# --------------------------------------------------

echo ""
echo "==> Checking WASM target..."

if ! "${RUSTUP}" target list \
  --installed \
  --toolchain "${TOOLCHAIN}" | grep -Fxq "${TARGET}"; then

  echo "==> Installing ${TARGET}..."

  "${RUSTUP}" target add "${TARGET}" \
    --toolchain "${TOOLCHAIN}"
fi

# Verify target libraries actually exist.
TARGET_LIBDIR="$("${RUSTC}" \
  --print target-libdir \
  --target "${TARGET}")"

if [[ ! -d "${TARGET_LIBDIR}" ]]; then
  echo "ERROR: WASM target libraries not found:" >&2
  echo "${TARGET_LIBDIR}" >&2
  exit 1
fi

echo "==> WASM target ready:"
echo "${TARGET_LIBDIR}"

# --------------------------------------------------
# 3. Build WASM
# --------------------------------------------------

echo ""
echo "==> Building WASM binary..."

"${CARGO}" build \
  --profile "${PROFILE}" \
  --target "${TARGET}"

WASM_FILE="target/${TARGET}/${PROFILE}/${APP_NAME}.wasm"

if [[ ! -f "${WASM_FILE}" ]]; then
  echo "ERROR: WASM binary not found: ${WASM_FILE}" >&2
  exit 1
fi

echo "==> WASM binary created successfully."

# --------------------------------------------------
# 4. Detect wasm-bindgen version
# --------------------------------------------------

echo ""
echo "==> Detecting wasm-bindgen version..."

if [[ ! -f Cargo.lock ]]; then
  echo "ERROR: Cargo.lock not found." >&2
  exit 1
fi

WASM_BINDGEN_VERSION="$(awk '
  /^\[\[package\]\]$/ {
    in_package = 1
    is_wasm_bindgen = 0
    next
  }

  in_package && /^name = "wasm-bindgen"$/ {
    is_wasm_bindgen = 1
    next
  }

  in_package && is_wasm_bindgen && /^version = / {
    gsub(/"/, "", $3)
    print $3
    exit
  }
' Cargo.lock)"

if [[ -z "${WASM_BINDGEN_VERSION}" ]]; then
  echo "ERROR: Could not find wasm-bindgen in Cargo.lock." >&2
  exit 1
fi

echo "==> Required version: ${WASM_BINDGEN_VERSION}"

# --------------------------------------------------
# 5. Check wasm-bindgen-cli
# --------------------------------------------------

echo ""
echo "==> Checking wasm-bindgen-cli..."

if ! command -v wasm-bindgen >/dev/null 2>&1 || \
   ! wasm-bindgen --version | grep -Fq "${WASM_BINDGEN_VERSION}"; then

  echo "==> Installing wasm-bindgen-cli ${WASM_BINDGEN_VERSION}..."

  "${CARGO}" install wasm-bindgen-cli \
    --version "${WASM_BINDGEN_VERSION}" \
    --locked
fi

# --------------------------------------------------
# 6. Prepare output directory
# --------------------------------------------------

echo ""
echo "==> Preparing output directory..."

rm -rf "${OUT_DIR}"
mkdir -p "${OUT_DIR}/assets"

# --------------------------------------------------
# 7. Generate JavaScript bindings
# --------------------------------------------------

echo ""
echo "==> Running wasm-bindgen..."

wasm-bindgen \
  "${WASM_FILE}" \
  --target web \
  --out-dir "${OUT_DIR}" \
  --out-name "${APP_NAME}"

# --------------------------------------------------
# 8. Copy static files
# --------------------------------------------------

echo ""
echo "==> Copying static files..."

if [[ ! -f index.html ]]; then
  echo "ERROR: index.html not found." >&2
  exit 1
fi

cp index.html "${OUT_DIR}/index.html"

if [[ -d assets ]]; then
  cp -R assets/. "${OUT_DIR}/assets/"
fi

touch "${OUT_DIR}/.nojekyll"

# --------------------------------------------------
# 9. Done
# --------------------------------------------------

echo ""
echo "======================================"
echo " BUILD COMPLETED SUCCESSFULLY!"
echo "======================================"
echo ""
echo "Output directory: ./${OUT_DIR}/"
echo ""
echo "To test locally:"
echo "python3 -m http.server 8000 --directory ${OUT_DIR}"
echo ""
echo "Open: http://localhost:8000"
echo ""

