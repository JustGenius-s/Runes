# Runes (runes-ts)

Runes 是一个基于 TypeScript 的可观测性语言实验：用 `rune` 标记一个普通 TS 值，编译器随后追踪它的局部来源派生和函数调用，并在程序运行时记录事件与耗时。

```ts
rune user = loadUser();

const normalized = normalizeName(user.name);
console.log(`Hello, ${normalized}`);
```

`user` 的类型仍是 `User`，不会被替换成 `Tracked<User>`。类型检查、模块解析和代码生成仍由 TypeScript / Node 负责。

> 当前状态：可运行的多 root MVP，用于验证语法降级、AST 来源传播和运行时插桩；尚不是生产级追踪器。

## 当前能力

- 源文件直接使用 `.ts`，支持 `rune name = expression;` 根声明。
- 根值支持任意 TypeScript 值：标量、对象、数组、Map/Set、函数、Promise 等。
- 保守追踪同一作用域内的赋值、引用和属性访问，闭包捕获外层 root 时自动继承。
- 追踪依赖 root 的函数调用（callee 或参数引用被追踪的名字），并记录进入/退出。
- 记录 `root`、`value.derive`、`call.enter` 和 `call.exit` 事件，并为事件传播每个值来源的完整 root 集合。
- 记录调用的持续时间（纳秒）；Promise 结果测量到 settle 为止。
- 同步异常和 Promise rejection 都会补发 `call.exit`（`unwind=true`），事件始终成对。
- 间接调用、动态 dispatch 和 async 边界可用 runtime 的 `currentRoots()` / `withRoots(roots, f)` 手动传播追踪上下文。
- 自动打印事件（`RUNES_PRINT=0` 关闭）；事件为结构化 `TraceEvent` 联合类型，运行结束后一次性写入 JSON（schema v3），供前端绘制。
- 事件写入进程内 ring buffer（容量 `RUNES_BUFFER_CAPACITY`，默认 4096），满时丢弃并计数（`overflowed_event_count` 可查）。
- 插件选项 `keyword` 可以把 `rune` 换成其他标识符。

纳秒是输出单位，不代表测量误差达到 1 ns。当前结果适合观察顺序、调用关系和耗时量级，不应直接替代严谨的性能基准。

## Trace 文件格式

`writeTraceFile` 输出一个 JSON 文档（schema v3），事件按时间戳升序，前端可直接消费：

```json
{
  "schema_version": 3,
  "overflowed_events": 0,
  "events": [
    {
      "event": "root",
      "at_ns": 26083,
      "binding": "count",
      "file": "examples/demo.ts",
      "line": 30,
      "column": 1
    },
    {
      "event": "call_enter",
      "at_ns": 1214041,
      "call_id": 1,
      "label": "double",
      "file": "examples/demo.ts",
      "line": 31,
      "column": 17,
      "roots": ["count"]
    },
    {
      "event": "call_exit",
      "at_ns": 1254458,
      "call_id": 1,
      "label": "double",
      "roots": ["count"],
      "duration_ns": 20333,
      "unwind": false
    }
  ]
}
```

事件类型：

- `root`：`{ at_ns, binding, file, line, column }`
- `call_enter`：`{ at_ns, call_id, label, file, line, column, roots }`
- `call_exit`：`{ at_ns, call_id, label, roots, duration_ns, unwind }` —— `call_id` 与 `call_enter` 配对
- `value_derive`：`{ at_ns, value_id, label, file, line, column, roots }`

## 快速运行

工具链基于 [Vite+](https://viteplus.dev)（`vp`），Node 24+ 直接以 type-stripping 方式运行 TS 源码，无需预编译：

```bash
pnpm install
pnpm demo            # = node examples/run.ts（Vite + unplugin 管线）
```

在宿主项目里通过 bundler 插件使用（支持 Vite / Rollup / esbuild / webpack）：

```ts
// vite.config.ts
import { defineConfig } from "vite";
import runes from "runes-ts";

export default defineConfig({
  plugins: [runes.vite()],
});
```

插桩在 bundler 的 transform 管线内完成，runtime 以虚拟模块 `virtual:runes-runtime` 注入，不产生任何中间产物；设置 `RUNES_OUT` 后，进程退出时自动写入 trace。

输出类似：

```text
[Runes +26083ns] root       count @ examples/demo.ts:30:1
[Runes +1214041ns] call.enter #1 double roots=[count] @ examples/demo.ts:31:17
[Runes +1254458ns] call.exit  #1 double roots=[count] duration=20333ns
[Runes +1278416ns] value.derive doubled roots=[count] @ examples/demo.ts:31:7
[Runes +1298875ns] root       user @ examples/demo.ts:33:1
...
doubled: 42
name: ADA LOVELACE
total score: 282 (active)
```

`examples/demo.ts` 覆盖标量、结构体、容器、闭包捕获和多 root 汇合（`roots=[count, user]`）。示例业务代码没有手工调用追踪 API；事件由编译器插入的 runtime hook 产生。

## 工作方式

```text
.ts 源码
  → 文本级降级 rune 根声明（字符串/注释感知扫描器）
  → TypeScript Compiler API 做来源传播 + 调用插桩
  → 宿主 bundler 继续编译（runtime 由虚拟模块注入）
  → 运行，产出 trace.json（schema v3）
```

Runes 不重新实现完整 TS 解析器。降级阶段只把：

```ts
rune user = loadUser();
```

改写为保持原类型的 `__runes.markRoot("user", loadUser(), file, line, column)` 调用。随后 `src/core/transform.ts` 用 TypeScript Compiler API 遍历 AST：在每个作用域内保守传播 root 集合（初始化器引用被追踪名字的绑定继承其 root 集），把引用了被追踪名字的调用包成 `__runes.call(label, roots, () => original, ...)`（thunk 保留 `this`，runtime 负责记录 enter/exit/duration 并保证 unwind 安全），派生绑定包成 `__runes.derive(...)` 发出 `value_derive` 事件。

runtime（`src/core/runtime.ts`）是一个普通 TS 模块，由插件 transpile 后以虚拟模块 `virtual:runes-runtime` 注入模块图——业务代码不需要安装或 import 任何追踪包。

## 语法与配置

当前根声明只支持：

```ts
rune name = expression;
```

暂不支持显式类型标注（`rune x: T = ...`）和解构 pattern。

根标记关键字默认是 `rune`，可用插件选项 `keyword` 更换：

```ts
runes.vite({ keyword: "trace" });
```

## 当前边界

- 来源分析是保守的局部 AST 分析，按作用域传播；尚未对每次覆写建立完整的值版本。
- 多个 root 汇合时，事件会传播并显示完整的 root 集合（`roots=[count, user]`）。
- 只插桩编译期可见的调用表达式；`eval`、`Function` 构造器等动态代码不在追踪范围。
- 对象属性写入（`obj.field = x`）目前不更新追踪状态，只有标识符赋值会被追踪。
- 跨 async 边界的上下文继承需要 `withRoots()` 手动传播。
- 事件在 ring buffer 满时会被丢弃（保留旧、丢新），`overflowed_event_count` 可查。

## 项目结构

```text
src/core/unplugin.ts  bundler 插件入口（Vite/Rollup/esbuild/webpack，虚拟 runtime 模块）
src/core/syntax.ts    文本级降级：rune 根声明 → __runes.markRoot(...)
src/core/transform.ts AST 来源追踪与调用插桩（TypeScript Compiler API）
src/core/runtime.ts   内联 runtime（schema v3 事件记录，由插件注入）
src/core/index.ts     公共 API（导出插件）
src/app/              trace 前端（Svelte + shadcn-svelte）
examples/demo.ts      统一示例：标量/结构体/闭包/多 root 汇合
examples/run.ts       demo 运行器：Vite dev server + ssrLoadModule
tests/            vitest 测试（语法降级、AST 变换、端到端 trace 校验）
```

## 开发验证

```bash
vp check    # 格式（oxfmt）+ lint（oxlint）+ 类型检查（tsgo）
vp test     # vitest
pnpm demo   # 端到端运行示例
pnpm app    # 启动 trace 前端（src/app，Vite + Svelte + shadcn-svelte）
```
