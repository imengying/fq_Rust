# fq_Rust

番茄小说纯 Rust 实现。

当前主线已经统一成单一 Rust 运行链：

- Rust 负责对外 HTTP API、上游请求编排、缓存和内容解密
- Rust 负责 `registerkey` 请求、缓存和解密 key 解析
- Rust 原生 `rnidbg` signer 已内嵌进 `fq-api`
- signer 资源和 `sdk23` 也已编进 `fq-api`，启动时自动解包到临时目录
- Java signer、Maven 构建链、`unidbg` jar 回退路径已删除

## 代码结构

- `api`: Rust API 服务
- `signer-native`: Rust 原生 signer 库
- `resources`: 构建期嵌入的 signer 资源源码
- `third_party/rnidbg`: Rust 原生 Android 模拟运行时
- `configs/config.yaml`: 默认配置
- `.github/workflows/ci.yml`: 编译与测试
- `.github/workflows/docker-publish.yml`: Docker Hub 发布

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
- 当前仓库内嵌的底层系统文件仍来自 `sdk23`
- `fq.signer.android_sdk_api: 31` 只会改变上报的 SDK level，不等于真正切到 `sdk31`

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
- [signer.rs](/home/mengying/文档/code/fq_Rust/api/src/signer.rs)
- [lib.rs](/home/mengying/文档/code/fq_Rust/signer-native/src/lib.rs)
- [runtime.rs](/home/mengying/文档/code/fq_Rust/signer-native/src/runtime.rs)
- [idle_fq_native.rs](/home/mengying/文档/code/fq_Rust/signer-native/src/worker/idle_fq_native.rs)

## Docker Hub 发布

工作流在 [docker-publish.yml](/home/mengying/文档/code/fq_Rust/.github/workflows/docker-publish.yml)。

会推送多架构镜像：

- `<DOCKERHUB_USERNAME>/fq-rust`
