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

if [[ -d "${SRC_INPUT}/system/bin" && -d "${SRC_INPUT}/system/lib64" ]]; then
  SYSTEM_ROOT="${SRC_INPUT}/system"
elif [[ -d "${SRC_INPUT}/bin" && -d "${SRC_INPUT}/lib64" ]]; then
  SYSTEM_ROOT="${SRC_INPUT}"
else
  echo "ERROR: 无法识别 system 根目录: ${SRC_INPUT}" >&2
  echo "需要满足以下之一：" >&2
  echo "  - <dir>/system/bin 和 <dir>/system/lib64" >&2
  echo "  - <dir>/bin 和 <dir>/lib64" >&2
  exit 1
fi

REQUIRED_FILES=(
  "bin/ls"
  "bin/sh"
  "lib64/libc++.so"
  "lib64/libc.so"
  "lib64/libcrypto.so"
  "lib64/libdl.so"
  "lib64/liblog.so"
  "lib64/libm.so"
  "lib64/libssl.so"
  "lib64/libstdc++.so"
  "lib64/libz.so"
)

mkdir -p "${DEST_DIR}/system"

for relative in "${REQUIRED_FILES[@]}"; do
  src="${SYSTEM_ROOT}/${relative}"
  dest="${DEST_DIR}/system/${relative}"
  if [[ ! -e "${src}" ]]; then
    echo "ERROR: 缺少必需文件: ${src}" >&2
    exit 1
  fi
  mkdir -p "$(dirname "${dest}")"
  cp -a "${src}" "${dest}"
done

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
