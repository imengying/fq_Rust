# Rust + Java Sidecar 新项目蓝图

## 目标

新项目采用混合架构：

- Rust 作为主服务，负责对外 HTTP API、上游编排、缓存、预取、当前上游设备管理、错误整形和可观测性
- Java 收缩为 sidecar，只保留 `unidbg signer` 和 `registerkey` 核心能力

核心原则：

- 不在 Rust 中重做 `unidbg`
- 不让 Java sidecar 再承担公开 API、章节缓存、搜索流程编排
- 设备风控策略、章节预取、接口兼容层统一收口到 Rust

## 非目标

- 不做当前 Java 项目的逐文件翻译
- 不让 Java 继续承担 `/search`、`/toc`、`/chapter` 这些业务接口
- 不在第一阶段追求 Java 完全无框架，最小可维护优先

## 默认假设

新项目默认按下面的运行形态设计：

- 单用户
- 单实例 Rust
- 单实例 Java sidecar
- 单活跃上游设备

说明：

- 这里的“单用户”指默认只服务一个使用者，不为多租户、多账号、多用户隔离做额外设计
- 这里的“单活跃上游设备”指默认只有一套正在使用的 `device_profile`
- 设备池和设备轮换保留为可选扩展，不作为第一版的默认复杂度

## 总体架构

```text
Legado / Client
    |
    v
Rust API Gateway
    |
    |-- PostgreSQL / local cache
    |
    |-- Java Signer Sidecar
    |     |- unidbg signer
     |     |- registerkey cache
    |     |- signer reset
    |
    `-- FQ upstream API
```

请求流建议：

1. 客户端请求进入 Rust
2. Rust 完成参数校验、缓存查询、设备选择、上游 URL/Headers 构建
3. Rust 调用 Java sidecar 生成签名头
4. Rust 自己发起上游请求
5. 章节接口需要解密时，Rust 向 Java sidecar 请求 `registerkey` 对应的真实 AES key
6. Rust 完成章节内容解密、GZIP 解压、HTML 提取、响应裁剪和缓存写入

这样拆分后：

- Java 只负责难以替换的 signer 和 registerkey
- Rust 拿回大部分业务控制权
- 两边的状态边界更清楚

## 职责边界

### Rust 主服务负责

- 对外 API
  - `GET /search`
  - `GET /book/:book_id`
  - `GET /toc/:book_id`
  - `GET /chapter/:book_id/:chapter_id`
- 请求参数校验
- 统一响应格式
- 上游 URL、query、headers 构建
- 搜索两阶段流程
- 目录接口调用和响应裁剪
- 单章预取、批量抓取、负缓存、重试退避
- 当前上游设备管理
- 可选的设备轮换与冷却时间
- 上游限流
- PostgreSQL 章节主缓存
- 本地热点缓存
- registerkey 使用策略
  - 何时请求
  - 何时刷新
  - 何时因为 `BadPadding` 触发重试
- 章节内容解密、解压、正文抽取、标题抽取
- metrics / tracing / 日志

### Java sidecar 负责

- `unidbg` 初始化和生命周期管理
- signer 生成签名头
- signer reset
- registerkey 请求与缓存
- 默认按当前上游设备维度缓存 `keyver -> real_key`

### 明确不再留在 Java 的内容

- Controller 对外接口
- 搜索两阶段逻辑
- 章节预取和 dedupe
- 目录缓存和章节缓存
- 设备轮换策略
- 统一异常处理
- DTO 精简和面向 Legado 的响应整形

## 状态归属

这是新项目最重要的边界。

### Rust 持有的状态

- 当前活跃设备
- 可选的备用设备列表与轮换状态
- 上游限流计时
- 搜索结果缓存
- 目录缓存
- 章节缓存
- 负缓存与空章节 backoff
- inflight request 去重
- 自动自愈和失败计数

### Java 持有的状态

- signer 实例
- signer reset epoch
- registerkey 缓存

### 关键决定

Java sidecar 不应再维护“全局当前设备”。

推荐方案：

- Rust 在每次 sidecar 调用时显式传入 `device_profile`
- 默认按“当前上游设备指纹 + keyver”建立索引
- signer 接口按请求内容实时签名，不依赖 sidecar 内部全局设备态

这样可以避免：

- Rust 和 Java 之间出现“当前设备不一致”
- sidecar 重启后丢失业务态
- 后续启用备用设备轮换时缓存语义混乱

## 内部接口设计

建议 sidecar 只监听 `127.0.0.1` 或 Unix Domain Socket。

所有内部接口都带一个共享令牌，例如 `X-Internal-Token`。

接口字段与状态码的当前定稿见 [sidecar-openapi.yaml](/home/mengying/文档/code/fq_Rust/sidecar-openapi.yaml)。

### 1. 生成签名

`POST /internal/v1/sign`

请求：

```json
{
  "url": "https://api5-normal-sinfonlineb.fqnovel.com/reading/bookapi/search/tab/v?...",
  "headers": {
    "accept": "application/json; charset=utf-8,application/x-protobuf",
    "cookie": "store-region=cn-zj; ...",
    "user-agent": "com.dragon.read.oversea.gp/68132 (...)",
    "x-reading-request": "1710000000000-123456789"
  }
}
```

响应：

```json
{
  "code": 0,
  "message": "success",
  "data": {
    "headers": {
      "x-argus": "...",
      "x-gorgon": "...",
      "x-khronos": "..."
    },
    "signer_epoch": 12
  }
}
```

说明：

- Java 只返回 signer 生成的 headers
- Rust 将原 headers 和签名头合并后自己请求上游

### 2. 获取真实解密 key

`POST /internal/v1/register-key/resolve`

请求：

```json
{
  "device_profile": {
    "name": "dev01",
    "user_agent": "com.dragon.read.oversea.gp/68132 (...)",
    "cookie": "store-region=cn-zj; store-region-src=did; install_id=573270579220059",
    "device": {
      "aid": "1967",
      "cdid": "9daf93bf-4dcf-417e-8795-20284ad26a1f",
      "device_id": "1778337441136410",
      "device_type": "Sirius",
      "device_brand": "Xiaomi",
      "install_id": "573270579220059",
      "version_code": "68132",
      "version_name": "6.8.1.32",
      "update_version_code": "68132",
      "resolution": "2244*1080",
      "dpi": "440",
      "rom_version": "V417IR+release-keys",
      "host_abi": "arm64-v8a",
      "os_version": "13",
      "os_api": "33"
    }
  },
  "required_keyver": 123
}
```

响应：

```json
{
  "code": 0,
  "message": "success",
  "data": {
    "device_fingerprint": "a8a8c5d4...",
    "keyver": 123,
    "real_key_hex": "0123456789ABCDEF0123456789ABCDEF",
    "expires_at_ms": 1710003600000,
    "source": "cache"
  }
}
```

说明：

- `source` 取值建议为 `cache` 或 `refresh`
- Rust 不需要知道 registerkey 上游协议，只需要拿到真正解密 key

### 3. 使某设备 registerkey 失效

`POST /internal/v1/register-key/invalidate`

请求：

```json
{
  "device_fingerprint": "a8a8c5d4..."
}
```

用途：

- Rust 解密章节出现 `BadPadding`
- Rust 判断需要主动清空某设备的当前 key

### 4. signer reset

`POST /internal/v1/signer/reset`

请求：

```json
{
  "reason": "UPSTREAM_EMPTY"
}
```

用途：

- Rust 识别 signer 卡死、返回异常头或连续空响应
- 仍保留 Java 内部的 cooldown，避免 reset 风暴

## Rust 侧模块建议

推荐直接用 `axum + tokio + reqwest + sqlx`。

建议目录：

```text
fqnovel-rs/
├── apps/
│   └── api/
│       ├── src/main.rs
│       └── src/http/
├── crates/
│   ├── api-model/
│   ├── app-service/
│   ├── upstream/
│   ├── sidecar-client/
│   ├── cache/
│   ├── device/
│   ├── content-crypto/
│   └── observability/
├── migrations/
└── configs/
```

### `api-model`

- 对外请求/响应 DTO
- 与 Legado 兼容的精简结构

### `app-service`

- `search_service`
- `directory_service`
- `chapter_service`
- `book_service`

### `upstream`

- 上游 URL、参数、headers 构建
- 搜索两阶段编排
- 原始响应解析
- 错误原因归类

### `sidecar-client`

- 调 Java sidecar 的 HTTP client
- token 注入
- 超时、重试、熔断

### `cache`

- 本地缓存
- PostgreSQL 持久化缓存
- inflight dedupe
- negative cache

### `device`

- 当前上游设备
- 可选的备用设备轮换
- 冷却时间
- 启动探测

### `content-crypto`

- AES-CBC
- Base64
- GZIP
- HTML 正文提取
- 标题提取

这部分可直接承接当前 Java 中的：

- `FqCrypto`
- `ChapterContentBuilder`
- `HtmlTextExtractor`

## Java sidecar 模块建议

推荐先保留最小 Spring Boot 外壳，不要为了 sidecar 再单独折腾网络层。

原因：

- 现有 `unidbg` 代码已经是 Java 生态
- sidecar 只暴露少量内部接口
- 维护成本低于重写一层轻量 HTTP server

建议目录：

```text
fqnovel-signer-sidecar/
├── src/main/java/.../sidecar/
│   ├── controller/
│   │   ├── SignController.java
│   │   ├── RegisterKeyController.java
│   ├── signer/
│   │   ├── IdleFQ.java
│   │   ├── SignerService.java
│   │   └── SignerResetService.java
│   ├── registerkey/
│   │   ├── RegisterKeyService.java
│   │   ├── RegisterKeyCache.java
│   │   └── RegisterKeyUpstreamClient.java
│   ├── dto/
│   └── config/
└── src/main/resources/
```

### Java 应保留的现有能力

- `IdleFQ`
- `FQEncryptService`
- `FQEncryptServiceWorker` 的 reset/cooldown 思路
- `FQRegisterKeyService`
- `FQApiUtils` 中与 registerkey 必需的最小子集

### Java 应删除的现有能力

- `FQSearchService`
- `FQDirectoryService`
- `FQNovelService`
- `FQChapterPrefetchService`
- `FQDeviceRotationService`
- `AutoRestartService`
- 所有对外 controller

## 设备模型建议

Rust 和 Java 之间传输统一的 `DeviceProfile`。

字段建议固定为：

- `name`
- `user_agent`
- `cookie`
- `aid`
- `cdid`
- `device_id`
- `install_id`
- `device_type`
- `device_brand`
- `resolution`
- `dpi`
- `rom_version`
- `host_abi`
- `version_code`
- `version_name`
- `update_version_code`
- `os_version`
- `os_api`

Rust 用这个结构做：

- 设备轮换
- 上游 header/query 构建
- sidecar registerkey 请求

Java 用这个结构做：

- registerkey 请求参数生成
- `device_fingerprint` 计算
- key 缓存分片

## 缓存策略建议

### Rust

- 搜索缓存：短 TTL，本地内存
- 目录缓存：中 TTL，本地内存
- 章节缓存：本地内存 + PostgreSQL
- 负缓存：章节解密失败、空内容、上游空响应
- inflight dedupe：搜索、目录、章节预取都做

### Java

- signer 默认单实例
- registerkey 默认按当前上游设备的 `device_fingerprint + keyver` 缓存
- 当前 key 与历史 key 分开管理
- 保留 TTL 和最大条目上限

## 错误归类建议

Rust 统一归类这些错误原因：

- `SIGNER_FAIL`
- `ILLEGAL_ACCESS`
- `UPSTREAM_EMPTY`
- `UPSTREAM_GZIP`
- `UPSTREAM_NON_JSON`
- `REGISTER_KEY_MISMATCH`
- `CHAPTER_EMPTY_OR_SHORT`
- `DECRYPT_BAD_PADDING`

Java sidecar 只负责输出清晰错误码和 message，不负责业务退避。

业务退避、轮换、自愈全部交给 Rust。

## 迁移映射

当前 Java 项目到新架构的映射建议：

### 迁到 Rust

- `FQSearchController`
- `FQNovelController`
- `GlobalExceptionHandler`
- `FQSearchService`
- `FQDirectoryService`
- `FQNovelService`
- `FQChapterPrefetchService`
- `PgChapterCacheService`
- `FQDeviceRotationService`
- `AutoRestartService`
- `FQSearchRequestEnricher`
- `FQApiUtils` 中搜索、目录、batch_full 参数构建
- `FqCrypto`
- `ChapterContentBuilder`

### 保留到 Java sidecar

- `IdleFQ`
- `FQEncryptService`
- `FQEncryptServiceWorker`
- `FQRegisterKeyService`
- registerkey 所需的最小 DTO 与工具类

### 还能优先迁到 Rust 的细粒度代码

- `HtmlTextExtractor`
  - 章节正文提取和标题提取，纯文本处理，适合直接改成 Rust 工具模块
- `FQSearchResponseParser`
  - 上游搜索 JSON 的容错解析与标准化，适合放进 Rust 的 `upstream`/`parser` 层
- `FQDirectoryResponseTransformer`
  - 目录裁剪、章节序号补全、最小响应整形，属于纯 DTO 变换
- `GzipUtils`
  - 上游响应的 gzip/非 gzip 兼容解码，适合 Rust 统一接管
- `CookieUtils`
  - `install_id` 规范化逻辑，纯字符串处理
- `Texts`
  - 空白处理、回退取值、截断等基础字符串工具
- `RetryBackoff`
  - 重试退避和 jitter 算法，直接迁过去可复用在搜索、章节、自愈逻辑
- `RequestCacheHelper`
  - 本地缓存 + inflight dedupe 的核心模式，适合直接按 Rust async 模式重做
- `LocalCacheFactory`
  - 本地 TTL/LRU 缓存策略封装
- `FQApiUtils`
  - 搜索、目录、`batch_full` 的 query/header 构建逻辑都适合 Rust 接手
- `FQSearchRequestEnricher`
  - 搜索默认参数、session id、运行时字段补全，适合放进 Rust 的请求建模层
- `FQNovelResponse` 以及面向 Legado 的 DTO
  - 这些响应结构本来就应该跟着 Rust 对外 API 一起走

### 删除或重写

- Java 中面向 Legado 的 DTO 精简逻辑
- Java 中的统一响应包装
- Java 中所有公开 Web 接口

## 推荐落地顺序

### 阶段 1：先把 Rust API 跑起来

- 建 Rust 基础工程
- 先实现 `sidecar-client`
- 实现对 Java sidecar 的 `sign` 调用
- 用固定设备打通一次 search 请求

验收标准：

- Rust 能独立返回搜索结果
- Java 只暴露内部接口

### 阶段 2：迁搜索和目录

- 完成搜索两阶段
- 完成目录接口
- 引入本地缓存和 inflight dedupe

验收标准：

- `/search`、`/toc`、`/book` 都由 Rust 输出

### 阶段 3：迁章节链路

- Rust 接入 registerkey resolve
- 实现章节内容解密、解压、正文提取
- 实现章节预取和 PostgreSQL 主缓存

验收标准：

- `/chapter` 完整跑通
- `BadPadding` 时 Rust 能驱动 invalidate + refresh

### 阶段 4：迁设备风控和自愈

- 当前上游设备管理
- 可选的备用设备轮换
- cooldown
- 启动探测
- signer reset

验收标准：

- Rust 侧完全接管现有风控编排

## MVP 范围

第一版新项目不要一次追求完全等价。

建议 MVP：

- 单实例 Rust
- 单实例 Java sidecar
- 先只做一个活跃设备
- 先只做本地内存缓存
- 先只做 `/search`、`/toc`、`/chapter`

延后项：

- 多设备轮换
- PostgreSQL 缓存
- 自动自愈
- metrics 大盘
- sidecar 多实例

## 主要风险

### 1. 设备态边界不清

如果 Java 还维护“当前设备”，Rust 也维护“当前设备”，后面一定会漂移。

解决：

- 明确 Rust 是设备态唯一 owner
- Java 只按请求参数工作

### 2. registerkey 缓存按全局维度管理

当前实现偏向单运行时“当前 key”。

单用户、单活跃设备时问题不大，但只要后续启用备用设备轮换，这个设计就会变脆弱。

解决：

- registerkey 缓存按 `device_fingerprint + keyver` 存

### 3. 把所有自愈都留在 Java

这样 Rust 会退化成薄网关，达不到目标。

解决：

- Java 只做“原子能力”
- Rust 做“策略决策”

### 4. 过早优化 sidecar 无框架

收益很小，风险很高。

解决：

- 先用最小 Spring Boot sidecar
- 等稳定后再考虑收缩运行时

## 最终建议

这条路线是可行的，而且比“全量 Rust 重写”靠谱很多。

推荐最终形态：

- Rust 是唯一业务入口和编排层
- Java 是受控的内部 signer/registerkey sidecar
- 当前上游设备管理、缓存、自愈、接口兼容性全部在 Rust
- Java 只保留必须依赖 JVM/unidbg 的核心

如果新项目要立项，第一周只做一件事：

- 先把 `sign` 和 `register-key/resolve` 这两个 sidecar 协议敲定

这两个协议稳定了，后面的 Rust 业务层就能并行推进。
