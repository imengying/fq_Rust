# fq_Rust

番茄小说混合架构实现，当前形态是单项目、单镜像、双语言运行时：

- Rust 负责对外 HTTP API、上游请求编排、缓存和内容解密
- Rust 负责 `registerkey` 请求、缓存和解密 key 解析
- Java 只保留 `unidbg signer`，作为 Rust 拉起的内部 worker
- 容器主进程只有 `fq-api`，Java worker 通过极简 `stdin/stdout` 行协议和 Rust 通信
- unidbg 资源不再从 jar 内临时解压，运行时直接读取镜像内 `/app/unidbg`

## 代码结构

- `api`: Rust API 服务
- `signer`: Java signer worker 与 unidbg 资源
- `configs/config.yaml`: 默认配置示例
- `.github/workflows/ci.yml`: 编译与测试
- `.github/workflows/docker-publish.yml`: Docker Hub 发布

## 对外接口

当前暴露四个接口：

- `GET /search`
- `GET /book/{book_id}`
- `GET /toc/{book_id}`
- `GET /chapter/{book_id}/{chapter_id}`

Java worker 不对外提供 HTTP 接口。

## 配置

配置加载顺序：

1. `configs/config.yaml`

关键项：

- `fq.upstream`: 番茄上游地址与超时
- `fq.signer.command`: Rust 拉起 Java worker 的命令
- `fq.signer.restart_cooldown_ms`: Rust 侧 signer 进程重启节流
- `fq.device_profile`: 当前生效设备信息
- `fq.device_pool`: 可选设备池；如果配置了，会按 `fq.device_pool_startup_name` 或首项继承到运行设备
- `fq.device_rotate_cooldown_ms`: 运行时设备轮换冷却时间

默认 worker 命令是：

```yaml
fq:
  signer:
    command:
      - java
      - --enable-native-access=ALL-UNNAMED
      - -jar
      - /app/fq-signer.jar
```

也可以用环境变量 `FQRS_SIGNER_COMMAND` 覆盖。

## 本地运行

本地有 Rust / Java / Maven 时，最短路径如下：

1. 直接修改 `configs/config.yaml`，按需调整设备池/设备信息、上游配置，以及 `fq.signer.command` 里的 jar 路径。
2. 构建 Java worker：`mvn -f signer/pom.xml -DskipTests package`
3. 构建 Rust API：`cargo build --release`
4. 以源码资源目录启动：`UNIDBG_RESOURCE_ROOT="$PWD/signer/src/main/resources" ./target/release/fq-api`

如果本地没有环境，也可以直接依赖 GitHub Actions 产物或 Docker。

如需关闭 `unidbg/libmetasec` 控制台噪音过滤，可在 `fq.signer.command` 里给 JVM 加 `-Dfq.log.filterConsoleNoise=false`。

启动后可以直接请求：

```bash
curl "http://127.0.0.1:9999/search?key=斗破苍穹&page=1&size=20&tabType=3"
curl "http://127.0.0.1:9999/book/7185502456775208503"
curl "http://127.0.0.1:9999/toc/7185502456775208503"
curl "http://127.0.0.1:9999/chapter/7185502456775208503/7185502456775209001"
```

## GitHub Actions

主工作流是 `.github/workflows/ci.yml`：

- Rust：`cargo test`、`cargo build --release`
- Java：`mvn -B -DskipTests package`
- `fq-signer` artifact 会同时带上 jar 和 `signer/src/main/resources` 资源目录
- 构建产物会作为 artifact 上传

## Docker

当前按单镜像部署：

- 构建阶段分别编译 Rust 与 Java
- unidbg 资源目录会直接拷到 `/app/unidbg`
- 运行阶段使用 `gcr.io/distroless/java25-debian13:nonroot`
- 镜像入口是 `fq-api`

本地启动：

```bash
docker compose up --build
```

相关文件：

- [Dockerfile](/home/mengying/文档/code/fq_Rust/Dockerfile)
- [docker-compose.yml](/home/mengying/文档/code/fq_Rust/docker-compose.yml)
- [signer.rs](/home/mengying/文档/code/fq_Rust/api/src/signer.rs)
- [SignerWorker.java](/home/mengying/文档/code/fq_Rust/signer/src/main/java/com/mengying/fqnovel/SignerWorker.java)

## Docker Hub 发布

工作流在 [docker-publish.yml](/home/mengying/文档/code/fq_Rust/.github/workflows/docker-publish.yml)。

触发方式：

- push tag：`v*.*.*`
- GitHub Actions 页面手动 `Run workflow`

需要在仓库 `Settings -> Secrets and variables -> Actions` 配置：

- `DOCKERHUB_USERNAME`
- `DOCKERHUB_TOKEN`

会推送一个多架构镜像：

- `<DOCKERHUB_USERNAME>/fq-rust`
