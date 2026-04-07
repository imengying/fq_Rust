# fq_Rust

Rust 主服务 + Java signer sidecar 的番茄小说混合架构实现。

当前仓库按 [RUST_HYBRID_BLUEPRINT.md](/home/mengying/文档/code/fq_Rust/RUST_HYBRID_BLUEPRINT.md) 落了一个可编译的 MVP：

- Rust 负责对外 API：`/search`、`/book/:book_id`、`/toc/:book_id`、`/chapter/:book_id/:chapter_id`
- Java 收缩成内部 worker：`sign`、`register-key-resolve`、`register-key-invalidate`、`signer-reset`
- 默认单设备、单实例、内存缓存
- GitHub Actions 直接构建 Rust 二进制和 Java jar

## 目录

- `apps/api`: Rust HTTP API
- `sidecar`: Java signer/registerkey sidecar
- `configs/api.example.yaml`: Rust 配置示例
- `sidecar-openapi.yaml`: sidecar 内部协议

## 本地运行

本机没有 Rust / Java / Maven 也没关系，推到 GitHub 后可以直接走 Actions 编译。  
如果你后面本地补环境，默认是 Rust 主进程自动拉起 Java worker。
本地最短路径：

1. 复制 `configs/api.example.yaml` 为 `configs/api.yaml`，改掉 token。
2. 先构建 Java worker：`mvn -f sidecar/pom.xml -DskipTests package`
3. 再启动 Rust API，它会按 `fq.sidecar.command` 自动拉起 `fq-sidecar.jar`

## GitHub Actions

工作流位置：`.github/workflows/ci.yml`

- Rust：`cargo test`、`cargo build --release`
- Java：`mvn -DskipTests package`
- 构建产物会作为 workflow artifact 上传

## Docker

当前按单镜像方式部署：容器主进程只有 Rust API，Java worker 由 Rust 在进程内拉起。
最终层走 `gcr.io/distroless/java25-debian13:nonroot`。

关键文件：

- [Dockerfile](/home/mengying/文档/code/fq_Rust/Dockerfile)
- [sidecar.rs](/home/mengying/文档/code/fq_Rust/apps/api/src/sidecar.rs)
- [SidecarWorker.java](/home/mengying/文档/code/fq_Rust/sidecar/src/main/java/com/mengying/fqnovel/SidecarWorker.java)
- [docker-compose.yml](/home/mengying/文档/code/fq_Rust/docker-compose.yml)

启动时直接执行：

```bash
docker compose up --build
```

## Docker Hub 发布

工作流在 [docker-publish.yml](/home/mengying/文档/code/fq_Rust/.github/workflows/docker-publish.yml)。

触发方式：

- push tag：`v*.*.*`
- GitHub Actions 页面手动 `Run workflow`

需要先在仓库 `Settings -> Secrets and variables -> Actions` 配置：

- `DOCKERHUB_USERNAME`
- `DOCKERHUB_TOKEN`

会推送一个多架构镜像：

- `<DOCKERHUB_USERNAME>/fq-rust`

标签规则：

- tag push 时：推 `latest` 和当前 git tag
- 手动触发时：默认推 `latest`，也可以额外填一个 `version_tag`
