# Diana 整体项目：CLI 优先的产品架构

**最新范围**：整个项目叫 **Diana**，不再把它视为“rasc 里的新反编译器”。用户明确要求 **MCP 暂时不设计**：本轮不定义新 MCP schema、工具、协议、迁移或发布合同，也不以 MCP 完成作为 CLI 的验收条件。已有 MCP 源码暂不删除；本文只设计 CLI、APK/DEX 查询、输入安全与内部反编译流水线。本次不修改生产代码。

## 1. 唯一产品命名

| 层 | 目标名称 / 路径 | 职责 |
|---|---|---|
| 产品与 root package | Diana / `diana` | APK/DEX 查询、manifest、原entry提取与反编译；公开命令以CLI.md指定的11项为准 |
| 用户二进制 | `diana`，Windows `diana.exe` | 一个 CLI；最终不另发 rasc 二进制或长期别名 |
| 内部反编译 crate | `diana-decomp`，`crates/diana-decomp/`，导入 `diana_decomp` | 同仓 P0–P9；不含文件/ZIP/命令入口 |
| 轻量查询 module | root `analysis::dex` 等现有领域代码 | borrowed tables、按需扫描，不先建完整反编译对象模型 |
| 输入与执行 module | root `analysis` 的 input/archive/request scope | 字节持有权、快照、解压、身份、请求级限额与取消 |
| 文档与 agent 使用说明 | `docs/diana/` | 整体设计、CLI帮助和agent用法；不保留公开skill安装命令 |

反编译算法从零设计；原工具已有的有用查询/安全实现可以复用，不要求从零重写它们。**一个项目不意味着一套完整解析对象服务所有查询**：保留两条具有不同成本模型的内部路径。

最终品牌不再是 rasc，但历史 release/tag、copyright、源码出处不全局替换。当前 root Cargo/binary 仍为 rasc，这是未实施状态；正式更名和发行名在产品任务中完成，不手工改版本或发布。

## 2. 总体架构（本轮无 MCP 路径）

```text
                         Diana CLI / agent 文档
                                    │
                      输入策略与请求 scope
                文件权限 · 私有快照 · byte/work/output limits
                                    │
                    APK/ZIP · borrowed/stored/deflate
                   原始 physical entry / logical DEX 身份
                                    │
                  ┌─────────────────┴──────────────────┐
                  │                                    │
              轻量查询路径                        反编译路径
      classes / fields / methods / strings  get-class / get-method
      entries / extract / xref / manifest   exact at / owner-name-proto
      borrowed scanner / native AXML       selected DexImage + declaration facts
                  │                                    │
                  │                           diana-decomp P0–P9
                  │                          Java / Smali / Unavailable
                  └─────────────────┬──────────────────┘
                              领域结果适配
                      CLI text/JSON · diagnostics · spans
```

MCP 属于以后可以接入的 adapter，不是 pipeline phase，也不是本轮预先冻结的协议。原有会话代码中可借鉴的生命周期、取消、缓存纪律可以用于 CLI 请求 scope；这不要求新建 CLI `open/close/status` 命令或设计长期会话产品。

## 3. 现有设计的复用清单

观察固定在本仓 `ef8a4c92c6b89e8afed9ccbc1552feae67b6982d`。这些是源码依据，不是重新测量的性能承诺。

| 来源 | 已有行为 | Diana 当前决策 |
|---|---|---|
| `src/lib.rs`、`src/cli/mod.rs` | 参数/诊断/输出与底层 analysis 分离；不启动外部反编译进程 | 保留 module 分工，root 更名，不造第二套 CLI |
| `src/analysis/session/mod.rs`：`open`、`Target` | capability-relative open、普通文件限制、私有不可变快照、owner 与取消 | 复用安全读取思路到 CLI request scope；不将用户可变 mmap 作为长寿命输入；不强制复制旧协议类型 |
| `src/analysis/archive.rs`、`apk.rs` | central-directory/root DEX、stored/deflate、按需读取、稳定合并 | 复用 ZIP/native 读取；取消为输出稳定顺序而做的排序、按序等待和重排缓冲；安全策略与性能变化单独验收 |
| `src/analysis/session/cache.rs`：`InflateCache` | 解压字节 single-flight、reservation、LRU/pin、关闭与发布竞态 | 若 CLI 同请求复用解压字节，沿用有界 owner/reservation 纪律；不承诺跨 CLI 进程热缓存 |
| `src/analysis/dex/mod.rs`、`query.rs` | borrowed scanner、ASCII/raw 预筛、exact/fuzzy、无需完整 DEX 模型 | 保留快查询，结果顺序不保证；列表与引用查询不能先运行 Engine::open/SSA |
| `src/analysis/dex/container.rs` | 041 logical member、旧 emitter 的 header overlay | 新 engine 接未改写原 container + header_offset；不把旧 overlay 当 raw truth |
| `src/analysis/session/query.rs`：`getclass_cancellable` | physical/logical/class-def 身份、名字歧义、取消检查 | 借鉴精确定义选择；新 CLI/engine 的内部 key 按原输入传递，不设计 MCP 返回结构 |
| `src/cli/mod.rs`、`args.rs` | stdout payload、stderr diagnostics、输出文件字节一致、BrokenPipe、member table | 保留通道；新 Mixed/JSON 输出与退出码显式定义 |
| `src/cli/agent_skill.rs`、`skill/SKILL.md`、release workflow | 品牌影响安装目录、帮助、version、下载产物 | 独立品牌任务，逐消费者改名，不重写历史许可/旧 tag |

原 `src/mcp/` 与 `docs/headless-mcp.zh-CN.md` 只作为现有能力记录；本轮不扩展它们，不提出 schema_version=2、九工具新 contract 或发布迁移任务。

## 4. 输入 interface 与生命周期

反编译库的 `Vec<Arc<[u8]>>` 方案更新为 borrowed views，避免将所有 snapshot/stored DEX 复制进完整 engine。

```rust
pub struct DexImage<'a> {
    pub bytes: &'a [u8],       // 未修改的 physical DEX/container
    pub header_offset: usize, // 普通 DEX 为 0；041 选定 logical header
    pub origin: ImageOrigin,  // 请求内 opaque 原始身份
}
pub struct Engine<'a> { /* 只读 ProgramIndex<'a> + 请求配置 */ }
pub struct RunContext<'a> { pub is_cancelled: &'a dyn Fn() -> bool }
// Engine::open(images: Vec<DexImage<'a>>, options, catalog)
// Engine::decompile(request, &RunContext) -> Result<Report, RequestError>
```

root request scope 持 snapshot/mapping/inflated buffers 的 owner lease，先加载必要字节，再在同 scope 内借用并用完 Engine。内层不持文件、ZIP、tokio/cap-std 或 JSON 协议对象，不做不安全 `'static` 生命周期延长。跨 DEX 声明依赖由 root adapter 轻量定位、在总预算内按需加载；不能先解压/解析整个 APK 后称“零拷贝”。

内部算法 product 的生命周期参数在示意 schema 中可省略，实际 Rust 类型沿 `MethodInput<'a> → FlowBody<'a> → …` 传播；原始 bytes 不由 ValueRecovery/AST 重写。无更长寿命 parser/source cache；需要时另设性能与失效任务。

## 5. 原始身份贯通与 041

- 请求内定位：`InputScopeId + PhysicalEntryIndex + LogicalMemberIndex + ClassDefIndex`；方法再关联原 MethodId 和精确 name/proto。名字只用于查找，不是定义身份。
- 列表`at`使用CLI.md的`e#:l#:c#[:m#|:f#]::descriptor`规范，string是`e#:l#:s#`；作用于同一显式输入，不是跨输入变化后的永久key。索引与描述符交叉核验，失效错误，不挑另一个同名定义；不再有`--class-id`。
- engine DexId只在本次请求内一致，ImageOrigin映射回物理/逻辑原身份。ClassKey含`class_def_idx`与descriptor，同DEX内重复descriptor也不能任意取第一项。
- fields/methods列表和get-class/get-method源码来自同原始定义身份；符号清单独立提供，不再把`--members`索引表前缀塞进Java source，原始index/proto与合成LocalId/span分开。
- 041 以原 container 为地址空间，传选定 logical header_offset，校验 container_size、header_offset、成员边界/共享 section 范围。禁止重写 header_size/checksum 后称它是原始字节。
- native 041 未验收前明确 UnsupportedContainer，不偷偷用旧 normalization。CLI 产品验收需补齐该既有输入能力，但不因此假设所有新版 opcode 都 Java 支持。

## 6. CLI 控制、限额与缓存

### 输出顺序策略：不提供稳定顺序保证

用户明确取消稳定结果顺序要求。这是对原 rasc 可观察行为与仓库确定性指导的**有意设计变更**，H01/H03需要记录并更新未来实现的合同与测试；本次不修改现有代码或AGENTS。

- 查询、独立方法/类的计算完成顺序、诊断与trace的跨任务交错不作跨运行保证；并行结果按完成/到达顺序提交。不引入全局排序、固定tie-break或仅为还原输入顺序的队头等待/重排缓冲，也不增加stable-output模式。
- 消费者按原始identity关联记录，不依赖行号/数组位置。成功查询返回全部匹配记录，完整、不重不漏；不同线程只比较按身份对应的内容/多重集合，不比较序列。
- 保留 `filter` 匹配语义；移除用户查询的 `--offset`、`--limit`，不提供分页、截取子集或 cursor。`entries`同样不保证展示顺序，但每条保留真实physical-entry index，重复entry/class歧义与manifest选择规则不能被到达次序覆盖。
- 原始指令顺序、控制转移、调用参数/副作用、catch优先级、锁释放与初始化顺序是**程序语义**，不能取消。raw smali按原指令位置表达；Java effect/initializer保持语义顺序。互不依赖的声明可以换展示次序，不能以“无序输出”改变运行行为。
- ID仅要求单次请求内一致、原始definition可追溯，不保证合成ID/局部名或生成文本跨运行一致。算法需要RPO/拓扑/支配遍历时保留其正确性要求，但不额外排序以追求相同打印结果。文档看板的phase/任务编号排列只是阅读导航，不是运行时排序要求。

取消排序/等待是否提高性能要用release测量；不承诺无序必然更快或所有排序都应删除。

取消由 root CLI/request scope 提供 callback，包含用户中断与 caller stop；到 P1 decode、P3/P5 worklist、P6 copy、P7/P8 graph、P9 emit 检查点。既有 libdeflate 不可抢占，取消是协作式，不宣称硬超时。BrokenPipe/下游关闭应停止查询/输出，不把它当新的 smali fallback 请求。

限额分层、显式组合：

| 层 | 本轮合同 |
|---|---|
| 文件/ZIP | 以现有安全读取默认 2GiB input、100k directory entries、64MiB directory、256MiB inflated entry 为工程基线；移到 CLI 若改变旧限制需说明并测量 |
| CLI 请求/解压 | 持有字节、解压 reservation、queued query rows、声明依赖与输出 scratch 合计有界；只保留同进程/同请求必要的字节复用，不新增常驻服务 |
| Engine | DESIGN 内 64MiB/DEX、256MiB/request 和 method work/graph/output 初值保持独立；超过引擎限额的输入仍可轻量查询，反编译明确 Resource |
| CLI 输出 | 现有查询可流式；单反编译报告完整渲染后提交。text/JSON 分别计最终字节，JSON escape、diagnostics、spans 要算；不截断后标完整 |

这些是不同预算，不将当前会话的 16MiB MCP envelope 限额硬套为新 CLI 产品合同。JSON 报告默认先采用 engine report 上限的 64MiB **最终序列化**上限作为待测工程初值，按剩余请求预算收紧；最终值在产品任务测量确定，不声称硬 RSS 上限。

扫描worker可按完成顺序把完整row/batch交给有界collector；只为byte/row预算、背压和取消保留缓冲，不为physical→logical→row还原顺序等待。取消按输入ordinal限制领先窗口或按序drain的调度；并发admission按资源权重/可用额度控制，慢entry不阻塞已经完成的其他entry发布。反编译首版可单线程简化状态，但不承诺文本/结果稳定顺序，也不为了打乱输出刻意并行；不创建嵌套Rayon pool或自动加 parser/IR/source cache。

## 7. CLI目标合同：以CLI.md为唯一规范

用户指定的新公开命令为：`get-class`、`entries`、`extract`、`strings`、`xref`、`manifest`、`classes`、`get-method`、`fields`、`methods`、`help`。具体说明、参数、TSV/JSON列、at语法、转义、失败与退出码在[CLI.md](CLI.md)，本页不保留另一份不同规范。

- `get-class/get-method`默认text只返回Java；默认无法满足时失败且无半输出。显式`--format json`返回清楚标记Java/Mixed/Smali/Unavailable的完整domain report；不自动把smali作为Fetch as Java成功。无`--require-java`，也无旧`getclass`alias。
- `at`携原始entry/logical/class-def/member identity，可直接回传；method selector必须完整owner/name/proto，或method-at，不按裸方法名/独立参数拼猜重载。字段/方法单独列表，无`--members`前缀污染source/span。
- 表格列严格采用用户指定项：entries的4列、strings的3列、xref的3列、classes/fields/methods的一列at；默认UTF8 TSV与header，不擅自附索引/调试列。不保证记录序，不分页。
- `extract`只提取原physical entry的完整解压payload到必需的`-o`，不改header/checksum、不切logicalview；重复name须entry-index消歧，原输入/符号链接/已有输出保护、CRC/资源错误/原子提交必须验收。它不向stdout写二进制副本。
- 查询成功完整返回全部匹配项，保留filter，安全预算超限明确失败而非静默截断。原始语义顺序/identity仍准确；xref按实际指令引用site列，不拿旧方法级结果冒充site。
- 不提供旧getclass/findrefs/skill命令、class-id/memberprefix、分页或稳定输出模式。MCP仍延期，本轮不删其源代码、不新增协议；agent说明作为文档/发行资产，不增加用户未指定的第十二个命令。

以上是对rasc CLI的有意接口重设计，不要求旧命令/列字节兼容；H01记录旧行为作为复用参考、按新规范验收，而不是用旧帮助限制新能力。

最终只叫 Diana。过渡期为不破坏现有使用，可以保留旧 emitter helper 的活动调用；**不得将暂留旧代码解读成另一个长期 rasc 产品或默默跨引擎回退**。不提供长期 `--engine dexdec`。

## 8. 产品任务与验收范围

**先测试准备**：B01–B04的70case规范/source/fixture/toolchain/reference/harness先冻结并认证，B04通过才进入H01/T01；见[TEST_PREPARATION.md](TEST_PREPARATION.md)、[E2E_CASES.md](E2E_CASES.md)。所有必需来源按[REPRODUCIBILITY.md](REPRODUCIBILITY.md)公开URL/commit与checkout相对路径定位，不从作者个人SDK/参考目录取输入。实现不重设计expected、不按当前引擎能力删requiredcase。

**测试边界**：不新增CLI黑盒或帮助/参数/路由/列名等基础接口测试。CLI.md继续约束实现，但以源码/文档审阅和编译确认；H01/H07无自动测试filter。语义E2E从公开反编译库跑Java行为与raw/smali回环；复杂原始身份/041/引用内容、输入/文件安全/资源取消走公开领域module interface。现有测试不在本次删除范围，未来准备fixture也不添加简单CLI矩阵。

产品任务采用 **H01–H10**，避免与已有 `U01–U08` Unsupported fixture 编号冲突，详见 [PROJECT_TASKS.md](PROJECT_TASKS.md)。

- H01 名称/产品合同与 package 落点；T01–T25改为内部 `diana-decomp`。
- H02安全输入/request scope；H03已有轻量查询与manifest能力复用。
- H04原始 view/041/SelectedClass 与依赖catalog；H05在T06级接通新CLI，接线做源码审阅；公开反编译interface的语义回环不等所有phase完成，不通过CLI子进程测试。
- H06取消/字节/输出预算和同请求复用；H09补defined fields/methods与site级xref，H10补安全原entry extract，H07统一11项help/CLI JSON与agent文档。
- H08 CLI产品验收依赖H07/H09/H10及引擎闸门，检查发布配置/旧代码退役，不发布、不建tag、不新增MCP迁移任务。

EngineCoreRC = T01–T23/T25；**CLIProductRC** = H01–H10 + EngineCoreRC：基础11项命令/列/接线/agent说明以代码审阅和构建确认，复杂query/extract/041/身份/安全与引擎行为提供公开module测试证据。不要求CLI黑盒、旧输出序列或跨运行byte-identical。MCP不在该分母里；它的暂缓状态要在产品说明写清楚。

若未设计MCP仍有live调用依赖旧dexdec，H08只能列出依赖与保留理由，不能盲删令现有代码不可编译；全面依赖退役待后续MCP范围决定。CLI新路径必须明确使用Diana Engine，无旧helper静默回退。这是阶段性内部遗留，不是第二产品。

本次仅更新设计和网页，不改 root Cargo/binary、共享 session 或 MCP，不删除引擎源码或发布历史。
