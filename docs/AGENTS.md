## 一、 Rust 开发规范

### 1. 代码架构与设计原则 (Design & Architecture)

* **必须：** 遵循 OCP 和 SRP，保持干净代码。
* **必须：** 所有 pub 类型必须 `#[non_exhaustive]`。除非你确定永远不会增加变体或字段。
* **审慎：** 游离函数。只允许放到私有的 mod helper 中。如果只被使用一次，且逻辑短小，考虑放到局部闭包。
* **禁止：** 为了“共享”而抽取薄封装。只包一行标准库/第三方库调用、没有承载明确边界语义、没有实质减少重复复杂度的函数不得放入 shared/common 包；直接在调用处写原表达式，或保留在具体 adapter 的错误语义附近。
* **禁止：** 文件膨胀。一个源文件不得超过 700 行。

### 2. 类型系统与数据建模 (Type System & Modeling)

* **偏好：** 使用 From trait，一般不允许单独写个函数用于转换。
* **必须：** 所有 struct Trait 都必须有明确的理由，禁止滥用 Clone/Copy 等。
* **必须：** 能使用 strum 则尽量使用。避免手动映射。
* **必须：** 业务类型实现 Eq 和 Hash 时必须显式声明等价性规则。
* **禁止：** 对原生/其它类型只做简单的无业务语义封装。禁止封装一个自定义的 DecimalValue，而应该尽量使用官方的 Decimal.
* **禁止：** 禁止 as 用于数值转换。 强制使用 From/TryFrom
* **禁止：** 禁止 pub use *; 所有导出的类型都需要判断必要性。所有 pub 修饰都需要判断必要性。
* **禁止：** 除非确实是 bool 语义，否则不要用 bool 做状态。 `reduce_only: bool → PositionEffect::ReduceOnly`，`side: bool → Side::Buy / Side::Sell`。
* **规则：** 时间类型统一使用 `time::OffsetDateTime`。 新代码禁止引入 `chrono`；公共模型时间字段用 `OffsetDateTime`，按 Unix 毫秒序列化时用 `time::serde::timestamp::milliseconds`。
* **规则：** 外部可构造且字段较多的 `pub struct` 必须提供 builder，禁止长期保留多参数 `new(...)`。默认使用 `derive_builder`，并把 `too_many_arguments` 的阈值固定在 5。
* **规则：** 一般情况，游离函数只能出现在私有工具区文件中，例如 `internal.rs` 或 `helper.rs`；它们必须是纯工具函数，不得承载交易所业务语义。特殊情况下允许全局函数但必须在文件头部做 RATIONALE (例如边界适配：convert/mod.rs). 不要通过 Impl struct 伪装，因为那样依然是（带命名空间的）全局函数。
* **规则：** 能使用 `Into<&'static str>` 的，不要写 `fn as_str`。
* **规则：** 类型别名仅用于简化复杂泛型签名。 `type Result<T> = std::result::Result<T, Error>`，不用于业务语义。
* **规则：** Copy 只用于平凡 POD 类型。例如：包含 Decimal 的订单结构不应该 Copy。
* **规则：** 避免静态生命周期依赖。 `&'static str` 只用于常量和配置，不用于数据传递。

### 3. 错误处理与健壮性 (Error Handling & Robustness)

* **规则：** 使用 thiserror 派生业务错误，anyhow 仅用于顶层二进制。 库代码禁止暴露 anyhow。
* **规则：** 禁止在 `map_err` 里丢弃错误信息。 每层转换只增加信息，不减少。
* **规则：** 超时必须是独立错误变体。 `Error::Timeout` 和 `Error::Network` 区分开，让上层能判断是否需要重试。
* **规则：** `unwrap()` 只允许在测试和不可恢复初始化里使用。 生产代码里每个 unwrap 必须有注释解释为什么不会失败。
* **规则：** `expect()` 的消息必须包含失败后无法恢复的原因。 不是描述"应该是什么"，而是描述"如果失败意味着什么"。
* **严禁：** 低质量测试。例如：冗余模糊、弱智检查：`set+get+assert`。eg `let x = Obj {y: 5}; assert_eq(x.y, 5);`

### 4. 异步与并发 (Async & Concurrency)

* **禁止：** `block_on`。 异步上下文里不能同步等待另一个异步任务.
* **规则：** 所有 `select!` 分支必须公平。
* **规则：** 取消安全：所有 `async fn` 必须在 `.await` 点持有可丢弃状态。 不能假设 future 一定会执行完成。
* **规则：** `CancellationToken` 传递到所有子任务。 关闭时优雅取消所有 WS 读取、定时器、重连循环。

### 5. 性能、序列化与 IO (Performance & IO)

* **禁止：** 在 hot path 里 clone 有堆分配的类型。
* **规则：** 用 serde 做所有外部边界序列化。不涉及边界的地方禁止用 serde. 如果类型确实公共，则尽量让序列化成为一个 feature 可开关。
* **规则：** Display 用于用户，Debug 用于开发者。Debug 必须脱敏，Display 用于格式化输出。禁止通过随手新建函数来做 Display 的事情。

---

## 二、 代码分析工具 (`ast-outline`) 使用指南

### 1. 核心命令 (Core Commands)

* `map`: 映射文件或目录 — 签名与行范围，无方法体。
* `show`: 提取符号源码。
* `digest`: 一页式模块地图。
* `implements`: 查找子类 / 实现。
* `search`: 仓库级混合 BM25 + 深度语义搜索。
* `find-related`: 查找与给定文件行语义相似的代码块。
* `surface`: 解析真实的公共 API 表面（处理 `pub use`）。
* `deps / reverse-deps`: 正向/反向导入图遍历。
* `cycles`: 通过 Tarjan SCC 查找导入循环。
* `graph`: 发射依赖图（文本或 JSON）。
* `index`: 构建、刷新或检查搜索索引。
* `prompt`: 打印 agent 提示片段。
* `status`: 报告安装情况。
* `hook / mcp`: 内部调用与协议支持。

### 2. 探索代码的推荐步骤 (Exploration Workflow)

1. **陌生目录：** `ast-outline digest <dir>`
2. **单文件轮廓：** `ast-outline map <file>`
3. **具体符号：** `ast-outline show <file> <Symbol>`
4. **查找实现：** `ast-outline implements <Type> <dir>`
5. **模糊搜索：** `ast-outline search "<query>"`
6. **相似代码：** `ast-outline find-related <file>:<line>`
7. **发布接口：** `ast-outline surface <dir>`
8. **依赖分析：** 使用 `deps`, `reverse-deps`, `cycles`, `graph`。

---

## 第三方项目使用

- 有时候外部项目的 API 你不确定，你可以把第三方项目，例如 binance-sdk 克隆到 .tmp 以便随时查看。

## 三、 项目特殊要求

* **文档参考：** `AGENTGS_MKT.md`
