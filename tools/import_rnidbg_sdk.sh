#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  tools/import_rnidbg_sdk.sh <mounted-system-dir> [output-dir]

Examples:
  tools/import_rnidbg_sdk.sh /mnt/android12_system third_party/local-sdk/sdk31
  tools/import_rnidbg_sdk.sh /mnt/android12_system/system third_party/local-sdk/sdk31

Input:
  <mounted-system-dir> can be either:
  - a directory containing system/bin and system/lib64
  - the system directory itself containing bin and lib64

Output:
  A rnidbg-compatible sdk directory, for example:
    third_party/local-sdk/sdk31

Notes:
  - This script does not download or mount images for you.
  - It only copies the minimum files currently used by fq_Rust.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

SRC_INPUT="${1:-}"
DEST_DIR="${2:-third_party/local-sdk/sdk31}"

[[ -n "${SRC_INPUT}" ]] || {
  usage
  exit 1
}

if [[ -d "${SRC_INPUT}/system" ]]; then
  ROOT_BASE="${SRC_INPUT}"
  SYSTEM_ROOT="${SRC_INPUT}/system"
elif [[ -d "${SRC_INPUT}/bin" || -d "${SRC_INPUT}/lib64" || -d "${SRC_INPUT}/apex" ]]; then
  ROOT_BASE="${SRC_INPUT}"
  SYSTEM_ROOT="${SRC_INPUT}"
else
  echo "ERROR: 无法识别 system 根目录: ${SRC_INPUT}" >&2
  echo "需要满足以下之一：" >&2
  echo "  - <dir>/system/..." >&2
  echo "  - <dir>/bin 或 <dir>/lib64 或 <dir>/apex" >&2
  exit 1
fi

mkdir -p "${DEST_DIR}/system"

copy_file() {
  local dest_relative="$1"
  shift
  local dest="${DEST_DIR}/system/${dest_relative}"
  local candidate
  for candidate in "$@"; do
    if [[ -e "${candidate}" ]]; then
      mkdir -p "$(dirname "${dest}")"
      cp -a "${candidate}" "${dest}"
      return 0
    fi
  done
  return 1
}

copy_or_fail() {
  local label="$1"
  local dest_relative="$2"
  shift 2
  if ! copy_file "${dest_relative}" "$@"; then
    echo "ERROR: 缺少必需文件(${label})，已尝试路径：" >&2
    printf '  %s\n' "$@" >&2
    exit 1
  fi
}

ensure_placeholder() {
  local dest="${DEST_DIR}/system/$1"
  mkdir -p "$(dirname "${dest}")"
  : > "${dest}"
}

copy_or_fail "ls" "bin/ls" \
  "${SYSTEM_ROOT}/bin/ls" \
  "${ROOT_BASE}/system/bin/ls"

copy_or_fail "sh" "bin/sh" \
  "${SYSTEM_ROOT}/bin/sh" \
  "${ROOT_BASE}/system/bin/sh"

copy_or_fail "libc++.so" "lib64/libc++.so" \
  "${SYSTEM_ROOT}/lib64/libc++.so" \
  "${ROOT_BASE}/system/lib64/libc++.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libc++.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libc++.so"

copy_or_fail "libc.so" "lib64/libc.so" \
  "${SYSTEM_ROOT}/lib64/libc.so" \
  "${ROOT_BASE}/system/lib64/libc.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libc.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libc.so"

copy_or_fail "libcrypto.so" "lib64/libcrypto.so" \
  "${SYSTEM_ROOT}/lib64/libcrypto.so" \
  "${ROOT_BASE}/system/lib64/libcrypto.so" \
  "${ROOT_BASE}/apex/com.android.conscrypt/lib64/libcrypto.so" \
  "${ROOT_BASE}/system/apex/com.android.conscrypt/lib64/libcrypto.so"

copy_or_fail "libdl.so" "lib64/libdl.so" \
  "${SYSTEM_ROOT}/lib64/libdl.so" \
  "${ROOT_BASE}/system/lib64/libdl.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libdl.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libdl.so"

copy_or_fail "liblog.so" "lib64/liblog.so" \
  "${SYSTEM_ROOT}/lib64/liblog.so" \
  "${ROOT_BASE}/system/lib64/liblog.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/liblog.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/liblog.so"

copy_or_fail "libm.so" "lib64/libm.so" \
  "${SYSTEM_ROOT}/lib64/libm.so" \
  "${ROOT_BASE}/system/lib64/libm.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libm.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libm.so"

copy_or_fail "libssl.so" "lib64/libssl.so" \
  "${SYSTEM_ROOT}/lib64/libssl.so" \
  "${ROOT_BASE}/system/lib64/libssl.so" \
  "${ROOT_BASE}/apex/com.android.conscrypt/lib64/libssl.so" \
  "${ROOT_BASE}/system/apex/com.android.conscrypt/lib64/libssl.so"

if ! copy_file "lib64/libstdc++.so" \
  "${SYSTEM_ROOT}/lib64/libstdc++.so" \
  "${ROOT_BASE}/system/lib64/libstdc++.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libstdc++.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libstdc++.so"; then
  ensure_placeholder "lib64/libstdc++.so"
fi

copy_or_fail "libz.so" "lib64/libz.so" \
  "${SYSTEM_ROOT}/lib64/libz.so" \
  "${ROOT_BASE}/system/lib64/libz.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libz.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libz.so"

chmod 755 "${DEST_DIR}/system/bin/ls" "${DEST_DIR}/system/bin/sh"

manifest="${DEST_DIR}/MANIFEST.sha256"
(
  cd "${DEST_DIR}"
  find system -type f | sort | while read -r file; do
    sha256sum "${file}"
  done
) > "${manifest}"

echo "Generated rnidbg sdk directory: ${DEST_DIR}"
echo "Manifest written to: ${manifest}"
