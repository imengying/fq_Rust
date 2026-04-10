#!/usr/bin/env bash
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

log() {
  printf '\n[%s] %s\n' "$(date '+%F %T')" "$*"
}

die() {
  echo "ERROR: $*" >&2
  exit 1
}

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="${PROJECT_DIR:-$SCRIPT_DIR}"
MODE="${1:-all}"

case "${MODE}" in
  deps|build|test|run|all)
    if [[ $# -gt 0 ]]; then
      shift
    fi
    ;;
  *)
    MODE="all"
    ;;
esac

RUN_BIN="${PROJECT_DIR}/target/release/fq-api"
CONFIG_PATH="${PROJECT_DIR}/configs/config.yaml"

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"
SYSTEM_RUST_INSTALL="${SYSTEM_RUST_INSTALL:-auto}"
if [[ -z "${CARGO_HOME:-}" ]]; then
  if [[ "$(id -u)" -eq 0 && "${SYSTEM_RUST_INSTALL}" != "false" ]]; then
    CARGO_HOME="/usr/local/cargo"
  else
    CARGO_HOME="$HOME/.cargo"
  fi
fi
if [[ -z "${RUSTUP_HOME:-}" ]]; then
  if [[ "$(id -u)" -eq 0 && "${SYSTEM_RUST_INSTALL}" != "false" ]]; then
    RUSTUP_HOME="/usr/local/rustup"
  else
    RUSTUP_HOME="$HOME/.rustup"
  fi
fi
RUSTUP_DIST_SERVER="${RUSTUP_DIST_SERVER:-https://mirrors.aliyun.com/rustup}"
RUSTUP_UPDATE_ROOT="${RUSTUP_UPDATE_ROOT:-https://mirrors.aliyun.com/rustup/rustup}"
RUSTUP_INIT_URL="${RUSTUP_INIT_URL:-https://mirrors.aliyun.com/repo/rust/rustup-init.sh}"
CARGO_REGISTRY_MIRROR="${CARGO_REGISTRY_MIRROR:-sparse+https://mirrors.aliyun.com/crates.io-index/}"
CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-true}"
CARGO_BUILD_LOCKED="${CARGO_BUILD_LOCKED:-false}"
RUST_LOG="${RUST_LOG:-info}"

if [[ ! -f "${PROJECT_DIR}/Cargo.toml" ]]; then
  die "未找到 ${PROJECT_DIR}/Cargo.toml"
fi

if [[ ! -f "${CONFIG_PATH}" ]]; then
  die "未找到配置文件 ${CONFIG_PATH}"
fi

if [[ "$(id -u)" -eq 0 ]]; then
  SUDO=""
else
  command -v sudo >/dev/null 2>&1 || die "当前不是 root，且系统未安装 sudo"
  SUDO="sudo"
fi

APT_UPDATED=0

install_system_rust_links() {
  if [[ "${CARGO_HOME}" != /usr/local/* ]]; then
    return
  fi

  mkdir -p /usr/local/bin
  local bin
  for bin in cargo cargo-clippy cargo-fmt rustc rustdoc rustfmt rustup; do
    if [[ -x "${CARGO_HOME}/bin/${bin}" ]]; then
      ln -sf "${CARGO_HOME}/bin/${bin}" "/usr/local/bin/${bin}"
    fi
  done
}

install_system_rust_env() {
  if [[ "${CARGO_HOME}" != /usr/local/* ]]; then
    return
  fi

  mkdir -p /etc/profile.d
  cat > /etc/profile.d/fq-rust-env.sh <<EOF
export CARGO_HOME="${CARGO_HOME}"
export RUSTUP_HOME="${RUSTUP_HOME}"
case ":\$PATH:" in
  *:"${CARGO_HOME}/bin":*) ;;
  *) export PATH="${CARGO_HOME}/bin:\$PATH" ;;
esac
EOF
  chmod 0644 /etc/profile.d/fq-rust-env.sh
}

ensure_apt_packages() {
  if ! command -v apt-get >/dev/null 2>&1; then
    die "当前脚本只处理 Debian/Ubuntu 系统，请手动安装 Rust、cmake、pkg-config、build-essential、git、curl"
  fi

  if [[ "${APT_UPDATED}" -eq 0 ]]; then
    log "更新 apt 索引"
    ${SUDO} apt-get update
    APT_UPDATED=1
  fi

  log "安装基础编译依赖"
  ${SUDO} apt-get install -y \
    build-essential \
    ca-certificates \
    cmake \
    curl \
    git \
    ninja-build \
    pkg-config \
    perl \
    tar \
    xz-utils
}

ensure_rust() {
  export CARGO_HOME RUSTUP_HOME
  export PATH="/usr/local/bin:${CARGO_HOME}/bin:${PATH}"
  export RUSTUP_DIST_SERVER
  export RUSTUP_UPDATE_ROOT
  export CARGO_NET_GIT_FETCH_WITH_CLI

  mkdir -p "${CARGO_HOME}" "${RUSTUP_HOME}"

  if [[ ! -x "${CARGO_HOME}/bin/rustup" ]]; then
    log "安装 rustup"
    curl --proto '=https' --tlsv1.2 -sSf "${RUSTUP_INIT_URL}" | sh -s -- -y --profile minimal --default-toolchain none --no-modify-path
  fi

  install_system_rust_links
  install_system_rust_env

  if ! "${CARGO_HOME}/bin/rustup" run "${RUST_TOOLCHAIN}" rustc --version >/dev/null 2>&1; then
    log "安装 Rust toolchain ${RUST_TOOLCHAIN}"
    "${CARGO_HOME}/bin/rustup" toolchain install "${RUST_TOOLCHAIN}" --profile minimal
  fi

  if ! "${CARGO_HOME}/bin/rustup" component list --toolchain "${RUST_TOOLCHAIN}" | grep -q '^rustfmt-.*(installed)$'; then
    log "安装 rustfmt 组件"
    "${CARGO_HOME}/bin/rustup" component add rustfmt --toolchain "${RUST_TOOLCHAIN}"
  fi

  "${CARGO_HOME}/bin/rustup" default "${RUST_TOOLCHAIN}" >/dev/null
  install_system_rust_links
  install_system_rust_env
  log "使用 Rust: $("${CARGO_HOME}/bin/rustup" run "${RUST_TOOLCHAIN}" rustc --version)"
  log "Rust 安装目录: CARGO_HOME=${CARGO_HOME}, RUSTUP_HOME=${RUSTUP_HOME}"
}

cargo_cmd() {
  cargo \
    --config 'source.crates-io.replace-with="aliyun"' \
    --config "source.aliyun.registry=\"${CARGO_REGISTRY_MIRROR}\"" \
    "$@"
}

install_deps() {
  ensure_apt_packages
  ensure_rust
}

build_project() {
  cd "${PROJECT_DIR}"
  export PATH="/usr/local/bin:${CARGO_HOME}/bin:${PATH}"
  export RUSTUP_DIST_SERVER
  export RUSTUP_UPDATE_ROOT
  export CARGO_NET_GIT_FETCH_WITH_CLI

  log "编译 fq-api"
  if [[ "${CARGO_BUILD_LOCKED}" == "true" ]]; then
    cargo_cmd build --release --workspace --locked
  else
    cargo_cmd build --release --workspace
  fi

  [[ -x "${RUN_BIN}" ]] || die "未生成 ${RUN_BIN}"
}

test_project() {
  cd "${PROJECT_DIR}"
  export PATH="/usr/local/bin:${CARGO_HOME}/bin:${PATH}"
  export RUSTUP_DIST_SERVER
  export RUSTUP_UPDATE_ROOT
  export CARGO_NET_GIT_FETCH_WITH_CLI

  log "运行测试"
  if [[ "${CARGO_BUILD_LOCKED}" == "true" ]]; then
    cargo_cmd test --workspace --locked
  else
    cargo_cmd test --workspace
  fi
}

run_project() {
  cd "${PROJECT_DIR}"
  export PATH="/usr/local/bin:${CARGO_HOME}/bin:${PATH}"
  export RUST_LOG
  unset RNIDBG_BASE_PATH
  unset FQ_SIGNER_RESOURCE_ROOT
  unset UNIDBG_RESOURCE_ROOT

  if [[ ! -x "${RUN_BIN}" ]]; then
    die "未找到 ${RUN_BIN}，请先执行 ./start.sh build 或 ./start.sh all"
  fi

  log "启动 fq-api"
  echo "PROJECT_DIR=${PROJECT_DIR}"
  echo "CONFIG_PATH=${CONFIG_PATH}"
  echo "RUST_LOG=${RUST_LOG}"
  echo "RNIDBG_RUNTIME=<embedded sdk23>"
  echo "SIGNER_ASSETS=<embedded assets>"

  exec "${RUN_BIN}" "$@"
}

case "${MODE}" in
  deps)
    install_deps
    ;;
  build)
    install_deps
    build_project
    ;;
  test)
    install_deps
    test_project
    ;;
  run)
    run_project "$@"
    ;;
  all)
    install_deps
    build_project
    run_project "$@"
    ;;
  *)
    die "不支持的模式: ${MODE}"
    ;;
esac
