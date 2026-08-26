# Runes

Runes 是基于 Rust/rustc 的可观测性原生语言实验。源码直接使用 `.rs`，只扩展一个上下文关键字：

```rust
rune user = load_user();

let name = normalize_name(&user.name);
save(name);
```

`rune` 创建一个追踪根；普通 Rust 语法、类型检查、借用检查和机器码生成继续由 rustc 完成。

## 当前结构

```text
crates/runes-syntax     rune 关键字、符号别名和源码降级
crates/runes-runtime    原生 root marker 与后续事件运行时
crates/runes-trace      后端无关的 TraceId/ValueId/事件协议
compiler/runes-driver   nightly rustc_driver 与 MIR 分析入口
examples/hello          直接使用 rune 关键字的 .rs 示例
```

项目根目录的 `Runes.toml` 控制表层标记：

```toml
[syntax]
root_marker = "rune"
aliases = []
```

可以增加单字符或标识符别名；所有别名都会降级成同一个 `mark_root` intrinsic。

## 运行

```bash
./scripts/run-hello
```

预期输出包含：

```text
RUNES_ROOT main at src/main.rs:...
Hello, ADA LOVELACE
native rune root: user
```

脚本会先构建 Runes driver，再以 `RUSTC_WRAPPER` 方式调用固定 nightly 工具链。driver 在 rustc 解析前将 `rune name = expression;` 降级成普通 Rust，然后在优化后的 MIR `Call` 中识别 `runes_runtime::mark_root`。

## 验证

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

当前已经完成 `.rs` 关键字降级、rustc 原生编译、MIR root 识别和 runtime root 事件。下一阶段是在 MIR 中为 assignment、field projection 和 call/return 注入 shadow provenance，使原生路径形成完整值来源图。
