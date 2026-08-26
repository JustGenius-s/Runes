# Runes

Runes 是一个基于 Rust/rustc 的可观测性语言实验：用 `rune` 标记一个普通 Rust 值，编译器随后追踪它的局部来源派生和函数调用，并在原生程序运行时记录事件与耗时。

```rust
rune user = load_user();

let normalized = normalize_name(&user.name);
println!("Hello, {normalized}");
```

`user` 的类型仍是 `User`，不会被替换成 `Tracked<User>`。所有权、生命周期、借用检查、trait 和机器码生成仍由 rustc 负责。

> 当前状态：可运行的单 root MVP，用于验证语法、MIR 来源传播和原生运行时插桩；尚不是生产级追踪器。

## 当前能力

- 源文件直接使用 `.rs`，支持 `rune name = expression;` 根声明。
- 根值支持普通 Rust 可按值绑定的 `Sized` 类型，包括标量、结构体、枚举、容器、引用、闭包及 `Box<dyn Trait>`。
- 保守追踪同一函数内的赋值、引用和字段投影。
- 追踪依赖 root 的直接函数调用，并在被调函数内部继承追踪上下文。
- 记录 `root`、`value.derive`、`call.enter` 和 `call.exit` 事件。
- 记录正常返回调用的持续时间，并提供纳秒单位的时间戳。
- 自动打印事件，也可通过 `runes_runtime::take_trace_events()` 获取结构化 `Vec<TraceEvent>`。
- `Runes.toml` 可以把 `rune` 换成其他标识符或单字符别名。

纳秒是输出单位，不代表测量误差达到 1 ns。当前结果适合观察顺序、调用关系和耗时量级，不应直接替代严谨的性能基准。

## 快速运行

项目需要 Rust nightly、`rustc-dev`、`rust-src` 和 `llvm-tools-preview`。compiler 目录中的 `rust-toolchain.toml` 会选择所需工具链。

```bash
./scripts/run-hello
```

输出类似：

```text
[Runes +541ns] root       user @ src/main.rs:11:5
[Runes +66833ns] value.derive _8 <- &(_1.0: std::string::String) roots=[user] @ src/main.rs:15:37
[Runes +71583ns] call.enter #2 normalize_name roots=[user] @ src/main.rs:15:22
[Runes +73166ns] call.enter #3 core::str::<impl str>::trim roots=[user] @ src/main.rs:7:5
[Runes +75083ns] call.exit  #3 core::str::<impl str>::trim roots=[user] duration=959ns
[Runes +78625ns] call.exit  #2 normalize_name roots=[user] duration=5750ns
Hello, ADA LOVELACE
```

示例业务代码没有手工调用追踪 API；事件由编译器插入的 runtime hook 产生。

## 工作方式

```text
.rs 源码
  → 仅降级 rune 根声明
  → rustc 解析、类型检查与借用检查
  → optimized MIR 来源分析与插桩
  → 原生二进制 + 运行时事件
```

Runes 不重新实现完整 Rust 解析器。driver 只在 rustc 解析前把：

```rust
rune user = make_user();
```

降级为保持原类型的 `runes_runtime::mark_root(...)` 调用。随后 driver 覆写 rustc 的 `optimized_mir` query，在最终 MIR 中传播 shadow provenance，并拆分调用边以插入进入/退出 hook。

## 语法与配置

当前根声明只支持：

```rust
rune name = expression;
```

暂不支持 `rune mut name`、显式类型标注和解构 pattern。

根标记由 `Runes.toml` 配置：

```toml
[syntax]
root_marker = "rune"
aliases = []
```

例如可以添加符号别名：

```toml
aliases = ["◇"]
```

所有标记最终具有相同的 runtime 语义。

## 当前边界

- 来源分析是保守的局部 MIR 分析，尚未对每次覆写建立完整的值版本。
- 多个 root 汇合时，runtime 暂时使用最近创建的 root 命名事件，还没有传播 root bitset。
- 只为 rustc 能静态解析到目标的直接调用记录完整函数名；函数指针和动态 trait dispatch 尚不完整。
- panic/unwind、`async` suspend/resume、生成器和跨线程传播尚未实现。
- unsafe、原始指针和内部可变性可以作为 root，但其内存写入尚不能被精确建图。
- 宏展开内部默认不插桩，避免标准库实现细节淹没业务链路。
- driver 使用不稳定的 `rustc_private` API，需要跟随并固定 nightly 版本。
- 当前热路径仍会直接格式化和打印日志，耗时中存在插桩扰动。

## 规划

### 1. 来源语义正确性

- [ ] 为每个 shadow value 分配稳定的 `ValueId`，记录 parent、operation 和 source span。
- [ ] 支持多个 root 的集合传播、汇合与分离。
- [ ] 完善 move、copy、borrow、reborrow、projection、mutation 和 return 的版本化语义。
- [ ] 为间接调用、trait dispatch、闭包和泛型单态化建立统一调用记录。
- [ ] 为正常返回与 unwind 建立成对事件，检测不完整 span。

### 2. 并发与异步

- [ ] 将追踪上下文显式传播到新线程和任务。
- [ ] 记录 `async` suspend/resume、任务切换与跨任务因果关系。
- [ ] 为共享可变状态设计低成本、可解释的并发事件模型。

### 3. 低开销记录器

- [ ] 用线程本地无锁 ring buffer 替代热路径直接打印。
- [ ] 批量写出二进制事件，提供采样、过滤和容量上限。
- [ ] 校准空 hook 开销，同时输出原始时间与校准时间。
- [ ] 在 release 构建中建立开销、丢事件率和时钟稳定性基准。

### 4. 查询与可视化

- [ ] 稳定 trace schema，并支持 JSON/二进制导出。
- [ ] 按 root、值、函数、源码位置和时间范围查询。
- [ ] 展示值来源图、调用瀑布图和关键路径。
- [ ] 提供事件过滤、脱敏和 preview 策略，避免记录敏感数据。

### 5. 工具链与发布

- [ ] 固定并自动验证支持的 nightly/rustc commit。
- [ ] 增加编译器集成测试、错误诊断和失败回退。
- [ ] 支持普通 Cargo 项目接入，而不依赖示例脚本。
- [ ] 评估 rustc fork、稳定编译器插件接口或其他后端的长期维护成本。

## 项目结构

```text
crates/runes-syntax     rune 语法配置与源码降级
crates/runes-runtime    原生事件记录和 runtime hook
crates/runes-trace      后端无关的 trace 数据结构
compiler/runes-driver   rustc_driver、MIR 分析与插桩
examples/hello          最小 .rs 示例
scripts/run-hello       构建 driver 并运行示例
```

## 开发验证

稳定层：

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

nightly compiler driver：

```bash
cd compiler/runes-driver
cargo check
```
