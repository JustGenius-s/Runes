# Runes

Runes 是一个基于 Rust/rustc 的可观测性语言实验：用 `rune` 标记一个普通 Rust 值，编译器随后追踪它的局部来源派生和函数调用，并在原生程序运行时记录事件与耗时。

```rust
rune user = load_user();

let normalized = normalize_name(&user.name);
println!("Hello, {normalized}");
```

`user` 的类型仍是 `User`，不会被替换成 `Tracked<User>`。所有权、生命周期、借用检查、trait 和机器码生成仍由 rustc 负责。

> 当前状态：可运行的多 root MVP，用于验证语法、MIR 来源传播和原生运行时插桩；尚不是生产级追踪器。

## 当前能力

- 源文件直接使用 `.rs`，支持 `rune name = expression;` 根声明。
- 根值支持普通 Rust 可按值绑定的 `Sized` 类型，包括标量、结构体、枚举、容器、引用、闭包及 `Box<dyn Trait>`。
- 保守追踪同一函数内的赋值、引用和字段投影。
- 追踪依赖 root 的直接函数调用，并在被调函数内部继承追踪上下文。
- 记录 `root`、`value.derive`、`call.enter` 和 `call.exit` 事件，并为事件传播每个值来源的完整 root 集合。
- 记录正常返回调用的持续时间，并提供纳秒单位的时间戳。
- unwind 时通过 panic hook 排空线程调用栈，为每个被跳过的调用补发 `call.exit`（`unwind=true`），事件成对完整。
- 间接调用、动态 dispatch、新线程和 async 任务可用 `runes_runtime::current_roots()` / `with_roots(roots, f)` 手动传播追踪上下文。
- 自动打印事件（`RUNES_PRINT=0` 关闭）；事件为结构化 `TraceEvent` 枚举，可用 `runes_runtime::take_trace_events()` 收集，或用 `write_trace_file(path)` / `write_trace_file_default()`（`RUNES_OUT` 指定路径，默认 `trace.json`）在运行结束后一次性写入 JSON，供前端绘制。
- 事件写入线程本地无锁 ring buffer（容量 `RUNES_BUFFER_CAPACITY`，默认 4096），满时丢弃并计数；线程退出时自动归集，`overflowed_event_count()` 可查丢弃数。
- `Runes.toml` 可以把 `rune` 换成其他标识符或单字符别名。

纳秒是输出单位，不代表测量误差达到 1 ns。当前结果适合观察顺序、调用关系和耗时量级，不应直接替代严谨的性能基准。

## Trace 文件格式

`write_trace_file` 输出一个 JSON 文档（schema v3），事件按时间戳升序，前端可直接消费：

```json
{"schema_version":3,"overflowed_events":0,"events":[
  {"event":"root","at_ns":125,"binding":"user","file":"src/main.rs","line":11,"column":5},
  {"event":"call_enter","at_ns":590042,"call_id":2,"label":"normalize_name","file":"src/main.rs","line":15,"column":22,"roots":["user"]},
  {"event":"call_exit","at_ns":625792,"call_id":2,"label":"normalize_name","roots":["user"],"duration_ns":29833,"unwind":false}
]}
```

事件类型：

- `root`：`{ at_ns, binding, file, line, column }`
- `call_enter`：`{ at_ns, call_id, label, file, line, column, roots }`
- `call_exit`：`{ at_ns, call_id, label, roots, duration_ns, unwind }` —— `call_id` 与 `call_enter` 配对
- `value_derive`：`{ at_ns, value_id, label, file, line, column, roots }`

## 可视化

仓库自带一个零依赖的单文件 viewer（`viewer.html`），把 `trace.json` 拖进页面即可渲染**火焰图**（调用栈按时间展开）和**甘特时间线**（按 root 着色）：

```bash
./scripts/serve-viewer          # 启动静态服务器，默认端口 8642
# 打开 http://127.0.0.1:8642/viewer.html
```

也可以直接双击 `viewer.html`（`file://` 下拖拽导入同样可用）。两种视图都按 root 自动配色，悬停显示函数、源码位置、耗时与 root 集合。

## 快速运行

项目需要 Rust nightly、`rustc-dev`、`rust-src` 和 `llvm-tools-preview`。compiler 目录中的 `rust-toolchain.toml` 会选择所需工具链。

```bash
./scripts/run
```

输出类似：

```text
[Runes +83ns] root       count @ src/main.rs:46:5
[Runes +181166ns] call.enter #1 double roots=[count] @ src/main.rs:47:19
[Runes +188208ns] root       user @ src/main.rs:50:5
[Runes +196333ns] value.derive _29 <- &_4 roots=[user] @ src/main.rs:63:50
[Runes +202166ns] root       user_ref @ src/main.rs:63:5
[Runes +203416ns] call.enter #2 describe_user roots=[user_ref] @ src/main.rs:64:16
...
[Runes +221666ns] call.enter #6 std::ops::Fn::call roots=[bump, count] @ src/main.rs:68:18
...
doubled: 42
name: ADA LOVELACE
total score: 282 (active)
```

统一的 `examples/demo` 覆盖所有受支持的 root 值形态：标量、结构体、枚举、容器（`Vec`/`HashMap`）、引用、闭包和 `Box<dyn Trait>`，并演示多 root 汇合（闭包调用 `roots=[bump, count]`）。

示例业务代码没有手工调用追踪 API；事件由编译器插入的 runtime hook 产生。运行结束后示例会调用 `write_trace_file_default()` 把结构化事件写入 `trace.json`（`RUNES_OUT` 可覆盖路径）。

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

降级为保持原类型的 `runes_runtime::mark_root_at(...)` 调用，并携带绑定名与源码位置。随后 driver 覆写 rustc 的 `optimized_mir` query，在最终 MIR 中传播 shadow provenance（每个 local 携带其来源的 root 集合），并拆分调用边以插入进入/退出 hook。

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
- 多个 root 汇合时，事件会传播并显示完整的 root 集合（`roots=[order, user]`）；集合通过 MIR 上的不动点并集传播，跨函数调用时由被调方继承。
- 只为 rustc 能静态解析到目标的直接调用记录完整函数名；函数指针和动态 trait dispatch 尚不完整。
- panic/unwind、`async` suspend/resume、生成器和跨线程传播尚未实现。
- unsafe、原始指针和内部可变性可以作为 root，但其内存写入尚不能被精确建图。
- 宏展开内部默认不插桩，避免标准库实现细节淹没业务链路。
- driver 使用不稳定的 `rustc_private` API，需要跟随并固定 nightly 版本。
- 事件在 ring buffer 满时会被丢弃（`overflowed_event_count()` 可查），丢弃策略是「保留旧、丢新」。

## 规划

### 1. 来源语义正确性

- [x] 为 shadow value 分配稳定的 `value_id`（当前按源码位置 FNV-1a 派生，版本化语义待做）。
- [x] 支持多个 root 的集合传播、汇合与继承（MIR 不动点并集 + runtime 字符串集合）。
- [ ] 完善 move、copy、borrow、reborrow、projection、mutation 和 return 的版本化语义（侦察结论：当前 nightly 的 `mir_borrowck` query 不再暴露 `MoveData`，精确版本化需引入 `rustc_mir_dataflow` 并在优化后 body 上重建 move path，语义会弱化，暂缓）。
- [x] 为间接调用、trait dispatch、跨线程和 async 提供运行时手动传播降级方案（`current_roots()` / `with_roots()`）。
- [x] 为正常返回与 unwind 建立成对事件（panic hook 排空调用栈，补发 `call.exit unwind=true`），检测不完整 span 的机制待做。

### 2. 并发与异步

- [x] 将追踪上下文显式传播到新线程和任务（`current_roots()` / `with_roots()` 手动传播）。
- [ ] 记录 `async` suspend/resume、任务切换与跨任务因果关系。
- [ ] 为共享可变状态设计低成本、可解释的并发事件模型。

### 3. 低开销记录器

- [x] 用线程本地无锁 ring buffer 替代热路径直接打印（`RUNES_PRINT=0` 可关打印，`RUNES_BUFFER_CAPACITY` 控制容量）。
- [ ] 批量写出二进制事件，提供采样、过滤和容量上限。
- [ ] 校准空 hook 开销，同时输出原始时间与校准时间。
- [ ] 在 release 构建中建立开销、丢事件率和时钟稳定性基准。

### 4. 查询与可视化

- [x] 稳定 trace schema（v3：`TraceEvent` 结构化枚举 + `write_trace_file` JSON 导出，前端可直接消费）。
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
examples/demo           统一示例：全类型 root + 多 root 汇合
viewer.html             零依赖 trace 可视化（火焰图 + 甘特图）
scripts/run             构建 driver 并运行 demo 示例
scripts/serve-viewer    启动 viewer 静态服务器
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

端到端（构建 driver 并运行统一示例）：

```bash
./scripts/run
```
