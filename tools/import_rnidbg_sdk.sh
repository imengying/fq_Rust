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
TMP_WORK="$(mktemp -d /tmp/fq-sdk-import.XXXXXX)"

cleanup() {
  if [[ -d "${TMP_WORK}" ]]; then
    find "${TMP_WORK}" -type d -path '*/mnt' | while read -r mountpoint; do
      sudo umount "${mountpoint}" >/dev/null 2>&1 || true
    done
    rm -rf "${TMP_WORK}"
  fi
}
trap cleanup EXIT

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

find_apex_package() {
  local prefix="$1"
  find "${ROOT_BASE}" -type f \( -name "${prefix}*.apex" -o -name "${prefix}*.capex" \) | head -n 1
}

mount_apex_payload() {
  local package="$1"
  local cache_key
  cache_key="$(basename "${package}")"
  local workdir="${TMP_WORK}/${cache_key}"
  local apex_file="${package}"
  local mount_dir="${workdir}/mnt"

  if [[ -d "${mount_dir}" ]]; then
    printf '%s\n' "${mount_dir}"
    return 0
  fi

  mkdir -p "${workdir}"

  if [[ "${package}" == *.capex ]]; then
    local extracted_apex="${workdir}/original.apex"
    if unzip -p "${package}" original_apex > "${extracted_apex}" 2>/dev/null; then
      apex_file="${extracted_apex}"
    elif unzip -p "${package}" apex_payload.img > "${workdir}/apex_payload.img" 2>/dev/null; then
      apex_file=""
    else
      return 1
    fi
  fi

  local payload="${workdir}/apex_payload.img"
  if [[ -n "${apex_file}" ]]; then
    unzip -p "${apex_file}" apex_payload.img > "${payload}" 2>/dev/null || return 1
  fi

  mkdir -p "${mount_dir}"
  sudo mount -o loop,ro "${payload}" "${mount_dir}" >/dev/null 2>&1 || return 1
  printf '%s\n' "${mount_dir}"
}

copy_from_apex() {
  local dest_relative="$1"
  local apex_prefix="$2"
  local apex_inner="$3"
  local dest="${DEST_DIR}/system/${dest_relative}"
  local package
  package="$(find_apex_package "${apex_prefix}")"
  [[ -n "${package}" ]] || return 1

  local mount_dir
  mount_dir="$(mount_apex_payload "${package}")" || return 1
  local source="${mount_dir}/${apex_inner}"
  [[ -e "${source}" ]] || return 1

  mkdir -p "$(dirname "${dest}")"
  cp -a "${source}" "${dest}"
  return 0
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

if ! copy_file "lib64/libc++.so" \
  "${SYSTEM_ROOT}/lib64/libc++.so" \
  "${ROOT_BASE}/system/lib64/libc++.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libc++.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libc++.so"; then
  copy_from_apex "lib64/libc++.so" "com.android.runtime" "lib64/bionic/libc++.so" || {
    echo "ERROR: 缺少必需文件(libc++.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/libc.so" \
  "${SYSTEM_ROOT}/lib64/libc.so" \
  "${ROOT_BASE}/system/lib64/libc.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libc.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libc.so"; then
  copy_from_apex "lib64/libc.so" "com.android.runtime" "lib64/bionic/libc.so" || {
    echo "ERROR: 缺少必需文件(libc.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/libcrypto.so" \
  "${SYSTEM_ROOT}/lib64/libcrypto.so" \
  "${ROOT_BASE}/system/lib64/libcrypto.so" \
  "${ROOT_BASE}/apex/com.android.conscrypt/lib64/libcrypto.so" \
  "${ROOT_BASE}/system/apex/com.android.conscrypt/lib64/libcrypto.so"; then
  copy_from_apex "lib64/libcrypto.so" "com.android.conscrypt" "lib64/libcrypto.so" || {
    echo "ERROR: 缺少必需文件(libcrypto.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/libdl.so" \
  "${SYSTEM_ROOT}/lib64/libdl.so" \
  "${ROOT_BASE}/system/lib64/libdl.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libdl.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libdl.so"; then
  copy_from_apex "lib64/libdl.so" "com.android.runtime" "lib64/bionic/libdl.so" || {
    echo "ERROR: 缺少必需文件(libdl.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/liblog.so" \
  "${SYSTEM_ROOT}/lib64/liblog.so" \
  "${ROOT_BASE}/system/lib64/liblog.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/liblog.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/liblog.so"; then
  copy_from_apex "lib64/liblog.so" "com.android.runtime" "lib64/bionic/liblog.so" || {
    echo "ERROR: 缺少必需文件(liblog.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/libm.so" \
  "${SYSTEM_ROOT}/lib64/libm.so" \
  "${ROOT_BASE}/system/lib64/libm.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libm.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libm.so"; then
  copy_from_apex "lib64/libm.so" "com.android.runtime" "lib64/bionic/libm.so" || {
    echo "ERROR: 缺少必需文件(libm.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/libssl.so" \
  "${SYSTEM_ROOT}/lib64/libssl.so" \
  "${ROOT_BASE}/system/lib64/libssl.so" \
  "${ROOT_BASE}/apex/com.android.conscrypt/lib64/libssl.so" \
  "${ROOT_BASE}/system/apex/com.android.conscrypt/lib64/libssl.so"; then
  copy_from_apex "lib64/libssl.so" "com.android.conscrypt" "lib64/libssl.so" || {
    echo "ERROR: 缺少必需文件(libssl.so)" >&2
    exit 1
  }
fi

if ! copy_file "lib64/libstdc++.so" \
  "${SYSTEM_ROOT}/lib64/libstdc++.so" \
  "${ROOT_BASE}/system/lib64/libstdc++.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libstdc++.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libstdc++.so"; then
  copy_from_apex "lib64/libstdc++.so" "com.android.runtime" "lib64/bionic/libstdc++.so" || true
fi
if [[ ! -e "${DEST_DIR}/system/lib64/libstdc++.so" ]]; then
  ensure_placeholder "lib64/libstdc++.so"
fi

if ! copy_file "lib64/libz.so" \
  "${SYSTEM_ROOT}/lib64/libz.so" \
  "${ROOT_BASE}/system/lib64/libz.so" \
  "${ROOT_BASE}/apex/com.android.runtime/lib64/bionic/libz.so" \
  "${ROOT_BASE}/system/apex/com.android.runtime/lib64/bionic/libz.so"; then
  copy_from_apex "lib64/libz.so" "com.android.runtime" "lib64/bionic/libz.so" || {
    echo "ERROR: 缺少必需文件(libz.so)" >&2
    exit 1
  }
fi

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
