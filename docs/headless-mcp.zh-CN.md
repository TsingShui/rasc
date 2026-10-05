# rasc Headless MCP

状态：首版实现与验收记录。

## 1. 定位

`rasc mcp` 是原生 Rust stdio MCP server。它不是“让 pi code mode 能调用 rasc”的必要层：
code mode 本来就可以通过 `tools.bash()` 调用现有 CLI；Pi extension 也可以用
`pi.registerTool()` 注册 Pi 专用工具。

MCP 只增加以下能力：

- 标准 MCP tool discovery、input/output schema 和 structured content；
- 显式、长寿命的 APK/DEX target；
- 私有不可变输入快照；
- 同一压缩 APK 多轮查询间复用真正的 ZIP deflate 输出；
- 可供 Pi 之外 MCP client 使用的标准协议。

一次性查询、shell pipeline、超大/流式结果，以及 `fields-plan`、`member-by-index`、
`getclass --members` 等 CLI-only 能力继续直接使用 CLI。CLI 与 MCP 相互独立，只共享 native
底层代码；MCP 不启动 CLI 子进程。

## 2. code mode 职责边界

MCP 不实现 JavaScript 已经能完成的通用数据处理：分页、filter、sort、slice、group、map、
聚合和跨工具编排均不属于 server interface。`classes`、`strings`、`entries` 一次返回完整
structured dataset。code mode 在 QuickJS 内缩减结果，只有显式 `return`、`text()` 或
`console.log()` 的内容进入模型上下文。

`findrefs` 的 string/type/method/field 解析、class exact/fuzzy 约束，以及 `getclass` 的类身份和
歧义判断属于 DEX 领域操作，仍由 native scanner 完成。

服务端不会静默截断。完整结果超过 item 或 serialized-byte budget 时，原子返回
`RESOURCE_LIMIT`；调用者应改用更精确的领域查询，或用 CLI 的流式/filter/limit 能力。

## 3. 工具

首版恰好九个工具：

| 工具 | 参数 | 完整结果 |
| --- | --- | --- |
| `open` | `path` | `target_id`、类型、大小、entry 计数 |
| `close` | `target_id` | 是否关闭 |
| `status` | 可选 `target_id` | target 或 session/inflate-cache 状态 |
| `classes` | `target_id` | 所有类定义及精确 `class_id` |
| `strings` | `target_id` | 所有 DEX 字符串及 string index |
| `findrefs` | `target_id, kind, value?, class?, fuzzy_class?` | 所有匹配引用方法 |
| `getclass` | `target_id`，`class_id`/`class_name` 二选一 | 完整 Java-like source |
| `manifest` | `target_id` | 完整 XML |
| `entries` | `target_id` | 所有物理 archive entries |

不存在 `read_text`，也不存在 cursor、offset、limit、next cursor、text handle 或 preview 协议。
每个工具发布 JSON Schema 2020-12 input/output schema。成功 envelope：

```json
{"schema_version":1,"ok":true,"data":{}}
```

业务错误 envelope 使用 `ok:false`、`error:{code,message}`，同时设置 MCP `isError:true`。
常见 code 包括 `TARGET_NOT_FOUND`、`PATH_NOT_ALLOWED`、`INPUT_CHANGED`、`INVALID_INPUT`、
`AMBIGUOUS_CLASS`、`CLASS_NOT_FOUND`、`RESOURCE_LIMIT` 和 `CANCELLED`。

## 4. 会话、快照和身份

`open(path)` 仅接受配置 root 下的普通文件，通过 capability-relative open 阻止符号链接逃逸。
输入在 open 时流式复制到进程私有临时文件并只读 mmap；复制前后长度/修改状态不一致则返回
`INPUT_CHANGED`。原文件之后被删除或修改不影响 target。

每次 open 返回新的 opaque UUID `target_id`，不存在全局 current file。首版默认最多 2 个
simultaneous targets。`close` 撤销 ID、触发 target cancellation、清除该 target 的 inflate
cache；重复 close 幂等。target 不能跨 server 进程恢复。

物理 ZIP entry 和 DEX 041 logical member 保留各自身份。`class_id` 编码 physical entry、logical
member 和 class-def index。按名字遇到多个定义返回 `AMBIGUOUS_CLASS`，不任意选择。保留 CLI 的
root DEX、重复 ZIP entry 和最后一个 `AndroidManifest.xml` 选择规则。

## 5. 唯一缓存：deflate 输出

首版刻意不缓存 parser/index、类定位、dexdec metadata、emitter、source、manifest、查询结果或
serialized response。

- 裸 DEX：从私有 snapshot mmap 借用 bytes，不进入缓存；
- ZIP stored entry（method 0）：从 snapshot mmap 借用 payload，不复制、不进入缓存；
- ZIP deflated entry（method 8）：第一次访问解压成 bytes，之后跨工具/请求复用；
- 同 `(target_id, physical_entry)` 的 concurrent miss single-flight；
- inflate cache 有全进程 byte budget 和 LRU；仍被读者 pin 的 entry 不强制回收；
- 失败/取消不会发布半成品或泄漏 reservation；close 清除 target entries；
- DEX 041 logical view 按需生成，不永久缓存 member-sized copies。

`status` 报告配置的 `analysis_threads`、`max_concurrent_requests`，以及
`inflate_cache_bytes`、`inflate_cache_entries`、`inflate_cache_loads`、`inflate_cache_hits`。普通
snapshot 借用不伪装成 cache hit。

`classes`、`strings` 和 `findrefs` 在 process-wide Rayon pool 上并行处理相互独立的 physical DEX
entry。每个 worker 在 entry 内顺序处理 DEX 041 logical member 和 row；coordinator 仍按
`physical entry → logical member → row` 顺序提交结果，并且只有 coordinator 消耗完整结果预算。
worker 到 coordinator 使用有界 row channel，in-flight entry 还受 inflate-cache byte budget 加权
限制；admission window 最多领先 coordinator 一个 `analysis_threads` 窗口，所有 entry 共用一个有界
channel，不会为任意多 entry 预建 channel。queued rows 按实际 serialized bytes 计入 result-byte 上限；
`findrefs` 的 target set 和单 class hit/name scratch 也受同一上限限制。实现不使用无界
`par_iter().collect()` 放大中间结果。DEX 041 的 nonzero logical view 可能复制
完整 container，所以 logical members 故意不并行，也不永久缓存副本。

`getclass` 每次通过 `DEX entry bytes → emitter` seam 创建隔离 emitter；不同调用不会共享 dexdec
state。其精确 class discovery、`manifest` 的单 entry 解码，以及纯 metadata 的 `entries` 没有额外并行，
避免为不明确的收益引入排序、歧义或内存复杂度。故热请求只承诺省去重复 inflate，不承诺
parser/decompiler 热加速。

## 6. 默认资源限制

- open targets：2；
- 单 input snapshot：2 GiB；
- ZIP central directory：100,000 entries、64 MiB；
- 单 inflated entry：256 MiB；
- process-wide analysis pool：默认使用可用 CPU 数，可用 `--analysis-threads` 调整；
- 同时执行的 open/analysis requests：2；超过时 fail-fast 返回 `RESOURCE_LIMIT`，不形成无界队列；
- inflate cache：512 MiB；
- 单完整 result：100,000 elements、16 MiB serialized envelope。

`close` 和 `status` 不占 analysis slot，确保过载时仍能取消工作和观察状态。`open`、`classes`、
`strings`、`findrefs`、`getclass`、`manifest`、`entries` 共用同一个 process-wide semaphore；permit
覆盖 `spawn_blocking` 计算和返回值在 handler 中的 JSON envelope 构造。
`--analysis-threads` 控制单个 process-wide Rayon pool 的 worker 数，不限制请求数；
`--max-concurrent-requests` 控制同时占用 handler permit 的请求数，不为每个请求创建新的 pool。CLI 可配置
`--max-targets`、`--max-input-bytes`、`--analysis-threads`、`--max-concurrent-requests`、
`--inflate-cache-bytes`、`--max-result-items` 和 `--max-result-bytes`。缓存预算不是硬 RSS 上限；snapshot mmap、in-flight
scratch、serde 和 dexdec 自身内存另计。release 使用 `panic=abort`，未知 panic 会断开连接；首版
没有 crash-isolation worker。取消是协作式：entry/row 等检查点可停止，libdeflate 或 dexdec 内部
不可抢占阶段不是硬超时。

## 7. Pi 配置和 code mode

项目级 `.pi/mcp.json` 示例（不会由 rasc 自动写入）：

```json
{
  "mcpServers": {
    "rasc": {
      "command": "rasc",
      "args": ["mcp", "--root", "/absolute/path/to/authorized-inputs"],
      "exposure": "codemode",
      "description": "Persistent typed APK/DEX analysis with deflate-byte reuse"
    }
  }
}
```

MCP tool 返回完整 `CallToolResult`。下面的 filter/sort/slice/map 和并行编排全部发生在 code mode，
不是 MCP 参数：

```js
function data(result) {
  const body = result.structuredContent;
  if (result.isError || !body?.ok) {
    throw new Error(body?.error?.message ?? "rasc request failed");
  }
  return body.data;
}

const opened = data(await tools.mcp__rasc__open({
  path: "/absolute/path/to/authorized-inputs/sample.apk"
}));
store("rasc.target", opened.target_id); // 只保存小型 ID

const [classes, strings] = await Promise.all([
  tools.mcp__rasc__classes({ target_id: opened.target_id }),
  tools.mcp__rasc__strings({ target_id: opened.target_id })
]);

const interesting = data(classes).items
  .filter(x => x.dotted_name.includes(".crypto."))
  .sort((a, b) => a.dotted_name.localeCompare(b.dotted_name))
  .slice(0, 1)
  .map(({ class_id, dotted_name }) => ({ class_id, dotted_name }));

const authCount = data(strings).items
  .filter(x => x.value.toLowerCase().includes("authorization"))
  .reduce(count => count + 1, 0);
return { interesting, authCount };
```

一次性或大输出可直接走 CLI，例如：

```js
const result = await tools.bash({
  command: "rasc strings --filter Authorization --limit 100 app.apk"
});
return result.output;
```

也可以用 Pi extension `pi.registerTool({ exposure: "codemode", ... })` 做 Pi-only wrapper；MCP 的选择
理由是标准协议、typed discovery、跨 client 和长寿命 inflate reuse，而不是 code mode 的基本接入。

## 8. 实现与验证记录

实现使用 `rmcp 3.5` stdio transport，兼容项目 Rust 1.93。自制 fixtures 覆盖：

- initialize、tools/list、九工具 schema、structured success/error、stdout 纯协议和 EOF shutdown；
- schema 无分页、文本句柄或 classes/strings/entries 通用 filter；
- 可审计的 `tools/verify_codemode.mjs` 直接 import 当前 `pi` 安装的
  `createCodemodeTool`，在真实 Pi QuickJS sandbox 执行与上例相同的
  `Promise.all`、`structuredContent` envelope、filter/sort/slice/map/reduce；运行
  `node tools/verify_codemode.mjs` 输出
  `{"interesting":[{"class_id":"0:0:2","dotted_name":"app.crypto.Cipher"}],"authCount":2}`；
- 私有 snapshot、原文件删除、root/symlink 逃逸、target 限制；
- bare/stored 输入不会产生 inflate cache entry/load/hit；
- deflated DEX 冷请求 load 一次，另一工具热请求 hit，close 后 cache entry/bytes 为零；
- concurrent miss single-flight、LRU、失败/取消 reservation cleanup；
- process-wide analysis concurrency hard budget，超限 structured `RESOURCE_LIMIT`，permit 释放后可重试；
- `analysis_threads=1/4` 的 `classes`、`strings`、`findrefs` typed output 完全相同且顺序稳定；
- physical-entry 并行下 stored/deflated cache load/hit 计数、预取消、完整预算及后续畸形 entry 的错误优先级；
- 裸 DEX、多 root DEX、重复 physical entry、DEX 041、歧义类、MUTF-8 和畸形 offset/length；
- item/byte response budget，且预算耗尽会在读取后续畸形 class row 前停止；
- `A → B → A` 每次重建隔离 emitter，输出一致；
- 现有 CLI 与 self-contained/no-subprocess contract。

Rust 1.93 的 workspace all-features tests 已通过；需要授权 APK 的测试保持显式 ignored。下文记录了
一个授权真实 APK 的有限验证；该单一样本及其放大衍生 workload 不能外推到其他 APK。

## 9. 性能验收口径

只比较与首版设计相关的工作：

1. `open` 的 snapshot cold cost；
2. deflated DEX 首次查询（`inflate_cache_loads +1`）；
3. 同 target 的不同后续查询（loads 不变、hits 增加）；
4. close 后缓存归零；
5. bare/stored 查询始终零 inflate cache load/hit。

release 测量记录 wall time、cache counters，以及平台可获得的 current/peak/steady RSS。不能把
OS page cache 称为 MCP cache，也不能把 parser/decompiler 时间减少归因于本实现。授权真实输入及
原始记录放在未跟踪 `.cache/`；无授权样本时真实 APK 性能明确标记“未评估”。

2026-03-16 在本机 release build 上运行自制、匿名的 20,000-class deflated DEX workflow（单次
样本，仅用于验证计数器和测量 harness，不是性能承诺）：

| 阶段 | wall time |
| --- | ---: |
| private snapshot `open` | 2.279 ms |
| cold `classes`（含一次 inflate、DEX 扫描和 JSON） | 57.424 ms |
| warm `strings`（命中 bytes，仍重新解析/扫描/序列化） | 44.526 ms |

cold 后 `loads=1,hits=0`；warm 后 `loads=1,hits=1`；缓存 2,152,402 bytes；close 后 entries/bytes
归零。查询类型不同，以上 wall time **不能**分解为纯 inflate 加速或作 cold/warm speedup 比值；它只
证明第二次没有重复 inflate。本机 `ps` 在 close 后测得 current RSS 60,304 KiB；该值不是 peak RSS，
也不是 cache-only 内存。可通过
`cargo test --release --test mcp_inflate_benchmark -- --ignored --nocapture` 复测。

同一 harness 还提供 8 个 deflated physical DEX entry、每个 8,000 个 synthetic class/string row 的
`analysis_threads=1` 对 `4` 比较；每个配置独立启动 MCP process，并取 5 次 end-to-end tool call
（包含扫描、structured result 和 JSON-RPC serialization）的中位数。2026-10-05 本机一次 release
记录如下（该 fixture 约 6.45 MiB：`classes` 当时走并行分支，而当前 32 MiB admission 下的 `strings`
走 threshold fallback，因此后一行不是 concurrent-strings 性能证据）：

| tool | 1 thread median | 4 threads median | 4/1 |
| --- | ---: | ---: | ---: |
| `classes` | 137.806 ms | 141.430 ms | 1.026 |
| `strings` | 122.072 ms | 121.639 ms | 0.996 |

结果没有显示显著 speedup。为避免小输入和高度偏斜输入上的调度开销，当前 admission 使用
`总 selected DEX uncompressed bytes - 最大 entry bytes` 近似真正可与最大 entry 重叠的工作：
`classes`/`findrefs` 至少 4 MiB、`strings` 至少 32 MiB 才启用 entry parallelism；阈值以下仍复用同一
session pool 配置但采用有序串行 traversal。单元测试通过隐藏的 test seam 强制覆盖 concurrent
classes/strings/findrefs，并另外覆盖正常 threshold fallback。这里不作普遍性能保证，并应同时考虑
`--max-concurrent-requests` 造成的总 CPU 竞争。

2026-10-05 还通过 ADB 从本机授权测试设备拉取公开发行的 OrganicMaps（package
`app.organicmaps.web`，61 MiB base APK）到未跟踪 `.cache/` 做端到端验证。原 APK 有 2 个 root DEX：
MCP 返回 7,124 classes、48,846 strings，`analysis_threads=1/4` 的完整 typed arrays byte-identical；
cold classes/strings 分别约 28.7/67.1 ms 和 30.1/72.1 ms，说明该高度偏斜的两-entry 输入不受益；
最终 admission 以最大 entry 之外的可并行 bytes 判断，因此会对它回退为串行。由这两个 DEX 构造的
本地 8-entry benchmark（仅用于放大同一授权 workload，不作为兼容性样本）在每次重启 server 的
cold `classes` 7 次中位数从 117.772 ms 降到 110.703 ms（约 6.0%）；cold `strings` 从 303.416 ms
增到 306.271 ms（约 0.9%）。后续 16-entry（约 49.7 MiB uncompressed DEX，确保两工具均越过
最终阈值）交替冷启动各 7 次；当前保留的最终样本中，`classes` 中位数 245.546 → 209.001 ms
（约 14.9%），`strings` 634.383 → 592.066 ms（约 6.7%）。因此只声称：该放大 workload 上的 cold
class scan 有明确收益，strings 有较小收益；两者都不能当作普遍加速保证，也不能把差值单独归因于
batching。峰值 RSS 在
原 APK 独立 process 测量约 102.7 MiB（1 thread）和 103.1 MiB（4 threads）。

已评估上述一个授权 OrganicMaps build；不能外推到其他真实 APK。更广泛 corpus 的 cold/warm
latency、多线程 speedup、兼容性、peak/steady RSS 仍**未评估**。本次记录使用 Rust 1.93 release
build、1/4 analysis threads、完整 MCP typed result 和独立 process cold call；原始样本与 JSON
记录仅保存在未跟踪 `.cache/`，不提交设备标识。

## 10. 依据

- 本地：`src/analysis/session/`、`src/mcp/`、`src/analysis/apk.rs`、
  `src/analysis/archive.rs`、`src/analysis/dex/`、`src/analysis/emitter.rs`、
  `crates/dexdec/FORK.md`；
- Pi：`docs/codemode.md`、`docs/mcp.md`、`docs/extensions.md`；
- 协议：[MCP tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)、
  [cancellation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation)；
- SDK：[official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)。
