# fq_Rust

番茄小说纯 Rust 实现。

当前主线已经统一成单一 Rust 运行链：

- Rust 负责对外 HTTP API、上游请求编排、缓存和内容解密
- Rust 负责 `registerkey` 请求、缓存和解密 key 解析
- Rust 原生 `rnidbg` signer 已内嵌进 `fq-api`
- signer 资源和 `sdk23` 也已编进 `fq-api`，启动时自动解包到临时目录
- Java signer、Maven 构建链、`unidbg` jar 回退路径已删除

## 代码结构

- `crates/api`: Rust API 服务
- `crates/signer-native`: Rust 原生 signer 库
- `assets/fq-signer`: 构建期嵌入的 signer 资源
- `vendor/rnidbg`: vendored Rust 原生 Android 模拟运行时
- `local/rnidbg/sdk31`: 本机私有导入的 Android 12 / API 31 运行时目录
- `configs/config.yaml`: 默认配置
- `.github/workflows/ci.yml`: 编译与测试

当前目录分层约定：

- `crates/`: 所有 Rust 源码
- `assets/`: 会被打进二进制的静态资源
- `vendor/`: 提交到仓库的第三方源码
- `local/`: 仅本机使用、不提交的私有运行时资源
- `tools/`: 导入和维护脚本

## 对外接口

- `GET /search`
- `GET /book/{book_id}`
- `GET /toc/{book_id}`
- `GET /chapter/{book_id}/{chapter_id}`

## 配置

配置加载顺序：

1. `configs/config.yaml`

关键项：

- `fq.upstream`: 番茄上游地址与超时
- `fq.signer.restart_cooldown_ms`: 内嵌 signer 重建节流
- `fq.signer.android_sdk_api`: 模拟上报给库的 Android SDK level
- `fq.cache.postgres_url`: 可选 PostgreSQL 章节主缓存
- `fq.prefetch`: 章节分桶预取
- `fq.auto_heal`: 连续错误后的 registerkey 失效、设备轮换、signer 重启自愈
- `fq.device_profile`: 当前生效设备信息
- `fq.device_pool`: 可选设备池

可用环境变量：

- `FQRS_DB_URL`
- `DB_URL`
- `FQRS_SIGNER_ANDROID_SDK_API`
- `FQ_SIGNER_RESOURCE_ROOT`
- `RNIDBG_BASE_PATH`

兼容保留：

- 默认不需要配置任何资源路径
- `UNIDBG_RESOURCE_ROOT` 仍可用，但只是 `FQ_SIGNER_RESOURCE_ROOT` 的旧名字兼容
- 当前二进制默认内嵌一套 Android 运行时文件：
  本地存在 `local/rnidbg/sdk31` 时优先嵌入 `sdk31`，否则回退到仓库里的 `sdk23`
- `fq.signer.android_sdk_api: 31` 只会改变上报的 SDK level，不等于真正切到 `sdk31`
- `RNIDBG_BASE_PATH` 只在你明确指定外部运行时目录时才需要

## 本地运行

1. 修改 `configs/config.yaml`
2. 构建：

```bash
cargo build --release --workspace
```

3. 启动：

```bash
./target/release/fq-api
```

启动后可直接请求：

```bash
curl "http://127.0.0.1:9999/search?key=斗破苍穹&page=1&size=20&tabType=3"
curl "http://127.0.0.1:9999/book/7185502456775208503"
curl "http://127.0.0.1:9999/toc/7185502456775208503"
curl "http://127.0.0.1:9999/chapter/7185502456775208503/7185502456775209001"
```

## 生成 sdk31

仓库当前内嵌的是 `sdk23`。如果你要尝试真 `sdk31`，推荐流程是：

1. 从官方 Android 12 / API 31 GSI 解压出 `system.img`
2. 把 `system.img` 挂载成只读目录
3. 运行脚本生成 rnidbg 目录：

```bash
tools/import_rnidbg_sdk.sh /path/to/mounted/system local/rnidbg/sdk31
```

生成后运行：

```bash
cargo build --release --workspace
./target/release/fq-api
```

如果构建时存在 `local/rnidbg/sdk31`，它会直接被嵌进 `fq-api`；不需要再额外配置 `RNIDBG_BASE_PATH`。

如果你仍然想强制用某个外部目录覆盖内嵌版本，再显式指定：

```bash
RNIDBG_BASE_PATH="$PWD/local/rnidbg/sdk31" ./target/release/fq-api
```

脚本只会复制当前项目需要的最小文件集：

- `system/bin/ls`
- `system/bin/sh`
- `system/lib64/libc++.so`
- `system/lib64/libc.so`
- `system/lib64/libcrypto.so`
- `system/lib64/libdl.so`
- `system/lib64/liblog.so`
- `system/lib64/libm.so`
- `system/lib64/libssl.so`
- `system/lib64/libstdc++.so`
- `system/lib64/libz.so`

常见挂载方式：

```bash
simg2img system.img system.raw.img
mkdir -p /tmp/android12-system
sudo mount -o loop,ro system.raw.img /tmp/android12-system
tools/import_rnidbg_sdk.sh /tmp/android12-system local/rnidbg/sdk31
sudo umount /tmp/android12-system
```

## GitHub Actions 生成 sdk31

仓库还带了一个手动 workflow：

- [build-sdk-from-system-image.yml](/home/mengying/文档/code/fq_Rust/.github/workflows/build-sdk-from-system-image.yml)

用法：

1. 在 GitHub Actions 页面手动运行 `Build rnidbg SDK`
2. 填入 `system_image_url`
3. 如果输入是 zip，必要时再填 `image_entry`
4. workflow 会：
   - 下载 system image
   - 自动解 zip
   - 自动把 sparse image 转成 raw
   - loop 只读挂载
   - 调用 `tools/import_rnidbg_sdk.sh`
   - 上传 `${sdk_name}.tar.gz` artifact

这条 workflow 适合生成私有 `sdk31` artifact，不适合把 Google GSI 内容直接提交回仓库。

## GitHub Actions

主工作流是 `.github/workflows/ci.yml`：

- `cargo test --workspace`
- `cargo build --workspace --release`
- 上传 `fq-api`

## Docker

当前按单镜像部署：

- 只构建 Rust
- 运行阶段不再需要 Java
- 运行阶段只包含 `fq-api` 和配置文件
- signer 资源与 `sdk23` 由二进制自解包
- 运行阶段使用 `gcr.io/distroless/cc-debian12:nonroot`

本地启动：

```bash
docker compose up --build
```

相关文件：

- [Dockerfile](/home/mengying/文档/code/fq_Rust/Dockerfile)
- [docker-compose.yml](/home/mengying/文档/code/fq_Rust/docker-compose.yml)
- [signer.rs](/home/mengying/文档/code/fq_Rust/crates/api/src/signer.rs)
- [lib.rs](/home/mengying/文档/code/fq_Rust/crates/signer-native/src/lib.rs)
- [runtime.rs](/home/mengying/文档/code/fq_Rust/crates/signer-native/src/runtime.rs)
- [idle_fq_native.rs](/home/mengying/文档/code/fq_Rust/crates/signer-native/src/worker/idle_fq_native.rs)

## Docker Hub 发布

工作流在 [docker-publish.yml](/home/mengying/文档/code/fq_Rust/.github/workflows/docker-publish.yml)。

会推送多架构镜像：

- `<DOCKERHUB_USERNAME>/fq-rust`
