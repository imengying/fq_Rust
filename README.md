# fq_Rust

Rust 主服务 + Java signer sidecar 的番茄小说混合架构实现。

当前仓库按 [RUST_HYBRID_BLUEPRINT.md](/home/mengying/文档/code/fq_Rust/RUST_HYBRID_BLUEPRINT.md) 落了一个可编译的 MVP：

- Rust 负责对外 API：`/search`、`/book/:book_id`、`/toc/:book_id`、`/chapter/:book_id/:chapter_id`
- Java 负责内部 sidecar：`sign`、`register-key/resolve`、`invalidate`、`signer/reset`
- 默认单设备、单实例、内存缓存
- GitHub Actions 直接构建 Rust 二进制和 Java jar

## 目录

- `apps/api`: Rust HTTP API
- `sidecar`: Java signer/registerkey sidecar
- `configs/api.example.yaml`: Rust 配置示例
- `sidecar-openapi.yaml`: sidecar 内部协议

## 本地运行

本机没有 Rust / Java / Maven 也没关系，推到 GitHub 后可以直接走 Actions 编译。  
如果你后面本地补环境，启动顺序：

1. 复制 `configs/api.example.yaml` 为 `configs/api.yaml`，改掉 token。
2. 在 `sidecar` 目录启动 Java sidecar。
3. 在 `apps/api` 目录启动 Rust API。

## GitHub Actions

工作流位置：`.github/workflows/ci.yml`

- Rust：`cargo fmt --check`、`cargo test`、`cargo build --release`
- Java：`mvn -DskipTests package`
- 构建产物会作为 workflow artifact 上传

## Docker

仓库带了：

- [docker-compose.yml](/home/mengying/文档/code/fq_Rust/docker-compose.yml)
- [apps/api/Dockerfile](/home/mengying/文档/code/fq_Rust/apps/api/Dockerfile)
- [sidecar/Dockerfile](/home/mengying/文档/code/fq_Rust/sidecar/Dockerfile)

启动时只要把 `.env.example` 复制成 `.env` 并改 token，然后执行：

```bash
docker compose up --build
```
