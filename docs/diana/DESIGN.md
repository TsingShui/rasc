# Diana 内部反编译流水线：架构与语义契约

Diana 是整个项目名，根产品包/命令为 `diana`；本页设计内部 `diana-decomp`（`crates/diana-decomp/`）。CLI、查询与输入整合见 [PROJECT.md](PROJECT.md)、[PROJECT_TASKS.md](PROJECT_TASKS.md)。**MCP 暂不设计，不进入本轮验收。**

状态：**从零设计、尚未实现**。先按[TEST_PREPARATION.md](TEST_PREPARATION.md)完成B01–B04/E2E规范与资产冻结再实现；团队来源/工具/相对路径由[REPRODUCIBILITY.md](REPRODUCIBILITY.md)规定，不以作者机器目录为前提。接口中的Rust是契约草案，不是已存在代码。阶段编号是本设计定义，不继承既有实验分支；不要求兼容或迁移它。证据出处见 [SOURCE_MAP.md](SOURCE_MAP.md)。执行任务见 [TASKS.md](TASKS.md)。

## 1. 目标与非目标

目标是一个原生 Rust DEX → Java/smali 分析引擎：用 Kuna 的产物所有权、显式调度、预算和可观测性承载小型、保守算法；吸收 dexdec 对类型、异常、Phi、源码合法性的处理义务，以及 radguard 的 dense ID、精简 worklist、文本成员溯源。

**优先级**：不丢语义 > 明确失败 > 可实施与易验证 > 覆盖率 > 可读性 > 输出紧凑。允许多 locals、raw generic types、显式数值转换、重复 cleanup、`while (true)`/dispatcher。不能接受错类型、漏锁、提前求值、错误 catch 顺序、未知 opcode 当 NOP 或方法抛异常桩冒充成功。

不承诺还原源码、所有合法 DEX 都能映射 Java、全库可重编译、二进制同构或具体删行/性能比例。内部反编译库不负责 native code、APK 资源/签名、JAR 隐式转换、Kotlin 源码后端、SLEIGH、插件框架或 CLI。APK/ZIP、manifest 与轻量查询由同项目 root modules 复用现有能力；最终 CLI 的 `get-class/get-method` 切换按 H 任务验收，而不是永久保留 rasc 作为宿主。

## 1.1 不保证结果稳定顺序

用户取消稳定输出要求：查询记录、独立方法/类的报告、诊断/trace跨任务交错、合成名字/ID及源码文本不承诺跨运行相同。按完成/到达顺序发布，不额外sort、稳定tie-break或按输入ordinal等待，也不增加稳定输出模式。原始definition identity与本次报告的span映射必须一致，但不把数组位置当identity。

这不取消**程序语义顺序**：原始指令、控制流转移、receiver/argument与副作用求值、catch优先级、monitor、初始化、raw smali指令位置仍保留。RPO/支配/有限worklist若为算法正确性所需可以使用，不为生成同一文本额外排序。每次输出仍须合法、完整且具备对应证据；实现时的性能影响要测量，不能推断“所有排序都无用”。

## 2. 成功与降级必须在类型上分开

### 2.1 可观察语义

在匹配依赖环境和有效程序输入上，Java 成功恢复应保留：

- 返回值（含整型溢出、FP NaN/有符号零与位模式相关操作）、终止/循环行为。
- 字段/数组/调用的顺序、接收者与成员原始身份、分派种类、class initialization 触发。
- 异常类型、抛出和 handler 选择顺序、finally 的可见效果、抛出指令**失败前**的寄存器状态。
- monitor 获取/释放对象、次数、顺序和异常路径；声明的 synchronized 语义。
- 字符串 UTF-16 code units，包括孤立 surrogate；static encoded value 与初始化顺序。

源码位置、局部名、临时量数量、栈帧形状不是承诺。Java/JVM 与 ART 的实现相关行为（例如具体 OOME 时机、栈轨迹行号、GC 时机）不能泛称已完全等价；算法不得因此任意跨越可能抛出、分配或同步操作，验证报告须注明运行环境局限。成功仅指恢复满足本设计的已支持语义 profile，不是数学上证明任意真实应用等价。

### 2.2 结果模型

```rust
pub struct DexImage<'a> { pub bytes: &'a [u8], pub header_offset: usize, pub origin: ImageOrigin }
pub struct RunContext<'a> { pub is_cancelled: &'a dyn Fn() -> bool }
pub struct Engine<'a> { /* 不可变 ProgramIndex<'a> + 请求配置；字节由 root scope 持有 */ }
pub struct Options { pub limits: Limits, pub recovery: RecoveryPolicy }
pub struct Request { pub class: ClassKey, pub method: Option<MethodSelector> }
// MethodSelector = { name, proto }，解析后才得到本地 MethodKey；不按 name 单独选。
pub struct Report {
    pub class: ClassOutput,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: RunStats,
}
pub enum MethodOutput {
    Java { declaration: VerifiedJavaMethod, evidence: MethodEvidence },
    DeclarationOnly { declaration: JavaMethodDecl }, // abstract/native，仅声明
    Smali { source: String, spans: Vec<OriginSpan>, reason: Diagnostic },
    Unavailable { reason: Diagnostic, excerpt: Option<RawExcerpt> },
}
pub enum ClassOutput {
    JavaUnit { unit: VerifiedJavaUnit, members: Vec<MethodOutput> },
    Mixed { shell: DeclarationOutput, members: Vec<MethodOutput> },
}
pub enum DeclarationOutput { Java(JavaClassDecl), Smali(String), Unavailable(Diagnostic) }
```

内部库 interface 最小化：`Engine::open(images: Vec<DexImage<'a>>, options: Options, catalog: DeclarationCatalog) -> Result<Engine<'a>, InputError>`；`Engine::decompile(request: Request, control: &RunContext) -> Result<Report, RequestError>`。单 DEX 用长度为 1 的 images；DexId在请求内分配并以ImageOrigin映射回root的physical/logical身份，不要求不同到达顺序得到相同数字，也不为文件名/输入顺序额外排序。catalog 允许 empty；类型/调用证据不足时局部降级，不猜测。库无磁盘、进程或网络访问，root request scope 持字节 owner 至 Engine 使用结束；普通 DEX header_offset=0，041 原始 view 由 H04 验收。

`VerifiedJavaMethod/Unit` 字段私有，仅 P8 验证器能构造。证据记录依赖的 `EnvironmentId` 与支持 profile；**生产引擎不调用 javac**。外部环境满足声明事实时才声称合法 Java；测试必须使用真实配套 classpath。不能把 Unknown 外部方法完整声明集当 Complete。

`JavaUnit` 要求合法 class shell、所有成员为 Java 或 DeclarationOnly、成员绑定/初始化/构造约束全通过。只要某方法 smali/unavailable 或 class shell 无法表达，结果为 Mixed；renderer 顶部必须标明非完整 Java，不能用 `.java` 后缀暗示可编译。方法级成功允许存在于 Mixed，但编译测试必须用可表示的上下文，不注入改变行为的 stub。

### 2.3 四类失败与取消

| 分类 | 例子 | 处理 |
|---|---|---|
| `Malformed` | 越界 ID、截断 opcode、错误分支边界、无效 LEB、使用未定义寄存器 | 全局 header/catalog 无法定位则 RequestError；局部 method Invalid → Unavailable，可附明确 raw excerpt。不是普通 Unsupported，不输出声称有效的 smali |
| `Unsupported` | 已知合法 invoke-custom、无法合法化的 constructor、无法证明的 monitor/异常结构、缺外部声明证据 | 从完整校验的 raw method 重新生成 smali；raw 不足则 Unavailable |
| `BudgetExceeded` | 方法 blocks/worklist/copies/输出超过限额 | 停止该方法分析；在独立小额 fallback 预算内 smali，否则 Unavailable；不得重试增加限制 |
| `InternalInvariant` | verifier 前后身份不一致、证书漏 edge、算法 impossible 状态 | 诊断显式 BUG、dev test 必败；请求报告可保留方法级 raw fallback，但状态仍为内部错误，不能算正常 Unsupported 或 Java 成功 |
| `Cancelled` | caller 的取消 token | 停止请求，RequestError::Cancelled；不为取消逐个生成 smali |

任何分析失败后的 Java AST/文本全部丢弃，fallback 永远只读原始字节。一个方法失败不修改其他方法配置。Unknown opcode 若无法可靠识别格式，只能 raw excerpt（例如 offset+hex），不能发明非标准 `.word` 并称 smali。

## 3. 深 Module 与三个设计 seam

1. **Input seam**：`ProgramIndex::open` 隐藏 header、LEB、MUTF-8、catalog、method raw range 校验。只发布 checked 只读视图，未校验 slice 不跨 seam。
2. **Method seam**：`analyze_method(MethodInput, &RequestFacts, &mut RunControl)` 返回 `VerifiedJavaMethod` 或分类错误。caller 不学习 SSA、Phi、region 細节。各 phase 有内部 typed product，避免公开十几种配置/trait。
3. **Emit seam**：`render(Report) -> RenderedReport` 只读已验证模型，输出 text+OriginSpan。Java 与 smali 是实际变化点，所以有两个 adapter；不为只有一个实现的每个分析造插件 trait。

`substrate/` 只放稳定 IDs、位置、类型、CFG/AST 容器及通用只读遍历；不藏算法、配置读取、全局缓存。不可用一个 “Context” 暴露所有阶段的可变字段。Rust 本地函数和具体 product 优先；只对预算、trace、读取视图使用必要 interface。

## 4. 阶段、产物与实际顺序

目录采用 Kuna 熟悉的 P0–P9，但语义和执行由 Diana 自己定义。

| Phase / Module | 拥有的产物 | 输入与职责 | 不拥有 |
|---|---|---|---|
| P0 `p0_knowledge` | `RequestFacts`、配置快照、`EnvironmentId` | DEX/catalog 事实来源、回合 profile、trace/catalog 元数据 | 不生成字节码事实、不改 IR、不放持久学习引擎 |
| P1 `p1_input` | `ProgramIndex`、`MethodInput`、`RawMethodView` → `ValidatedRawMethod` | 安全容器与 metadata、UTF-16、版本/opcode 格式原始覆盖；range 校验与完整结构校验分级 | 不决定控制结构、不以源码难表达拒绝整个合法 class |
| P2 `p2_flow` | `FlowBody` | decode、move-result 绑定、block、normal/ordered exceptional edge、effect sites | 不优化副作用、不结构化、不推测新调用目标 |
| P3 `p3_ssa` | `SsaBody` | 输入参数/this、每 definition 稳定 ValueId、normal/exception edge environments、Phi | 不合并源码 locals、不消除异常边 |
| P4 `p4_calls` | `ResolvedBody`、`CallEvidence` | 原始 owner/proto/kind、声明候选/访问/throws/class-init 事实与待验证的 Java 绑定义务；allocation/init 约束 | 不恢复 native calling conventions；不因 DEX 有 proto 就成为空阶段 |
| P5 `p5_types` | `TypedBody` | primitive/reference/null/raw constant bits、use conversions、Phi join、有限单调工作队列 | 不必重建完整泛型源码；不修改 CFG topology |
| P6 `p6_variables` | `StorageBody`、`CopyPlan` | 一 definition 一 LocalId、不做通用 coalescing；正常边平行拷贝、异常 shadow slots；DA certificate | 不做词法结构推断、不把 SSA construction 放 Repeat 反复生成身份 |
| P7 `p7_regions` | `RegionPlan`、`ProtectionCertificate` | 只读 StorageBody 分析 normal SCC/SESE、try 区间/handler、monitor | 不改原始 CFG、不得把 catch-all 自动当 finally |
| P8 `p8_java` | `VerifiedJavaMethod/Unit` | block/edge 精确覆盖、结构化/dispatcher、表达式合法化、class/constructor/initializer 与 throws 验证 | 不猜未解析字节、不直接按 visited 跳过 block |
| P9 `p9_emit` | `RenderedReport`、最终 UTF-8 `OriginSpan` | AST printer、raw smali adapter、diagnostic renderer | 不做值/类型/控制流恢复 |

**实际首版调度**：

```text
ProgramIndex(P1) ──→ RequestFacts(P0，冻结)
每个方法：ValidatedRawMethod(P1)
  → decode_and_bind(P2) → verify_flow
  → build_ssa(P3)       → verify_ssa
  → resolve_calls(P4)   → verify_declaration_evidence
  → solve_types(P5)     → verify_types
  → allocate_and_copy(P6) → verify_storage
  → identify_regions(P7)
  → lower_java(P8)      → verify_method
类装配(P8)             → verify_unit_or_mark_mixed
render(P9)
任意分类失败 ──→ 原始方法 + 独立 fallback 预算 ──→ smali / unavailable(P9)
```

初版仅有阶段内局部 fixed points：P3 的固定 ValueId/PhiId 环境传播、live-in/定义可用性，以及 P5 的有限类型域 worklist。它们只收集有限 facts，不创建无限新身份。P2 的 topo normalization 先收集编辑，再原子应用一次。可选纯表达式内联以后加入 P8 的有限 rewrite group，不改变 SSA/CFG。**不把 P3–P6 整段塞入互不动点**，也不强制所有 phase 必须各有相同数量 pass。

选择理由：dexdec 的 ValueRecovery 依赖 structured Semantic IR [DX-01]；本设计不采用它，而保留显式三地址式计算，先在 CFG 上解决 SSA/type/storage，避免 BDD/gated Phi/语言语法多轮互相调度。这是新的算法设计决策，不是声称 DEX 天生只允许某种顺序。代价是输出更长、额外拷贝多，收益尚未测量。

## 5. 身份、来源与产物不变量

### 5.1 ID 与不可变 raw

本节“固定/稳定身份”仅指**单次分析内引用一致、arena ID不重用**，不是结果排序或跨运行确定性。不同worker到达顺序可以生成不同合成ID和局部名，所有引用/证书/最终span按本次identity核对。

```rust
struct DexId(u32);
struct ClassKey { dex: DexId, class_def_idx: u32, descriptor: String }
struct MethodSelector { name: String, proto: String }
struct MethodKey { dex: DexId, method_idx: u32 } // method_id 唯一解析到 owner/name/proto
struct FieldKey { dex: DexId, field_idx: u32 }
struct InsnId(u32);    // 方法内 decode 次序，永不重用
struct BlockId(u32);   // 同请求split后身份不重用；不保证跨运行数字相同
struct EdgeId(u32);    // 两个到同 block 的不同 switch/异常边不合并
struct ValueId(u32);   // 定义身份，不等于物理寄存器
struct LocalId(u32);   // 源变量身份，默认一 ValueId 一 LocalId
struct Site { method: MethodKey, code_unit: u32 }
struct Origin { sites: Vec<Site>, synthetic_reason: Option<SyntheticReason> }
```

`RawMethodView` 仅证明 code_item/try/debug 原始范围可安全读取；它不能喂给标准 smali adapter。P1 `validate_raw` 在格式表/metadata/目标/register/index 检查齐全后才产生 `ValidatedRawMethod`。这仍不是宣称通过完整 ART verifier；后续 SSA/type 阶段发现确定的非法输入时按 Malformed 降为 Unavailable，而不是因为此前 raw 结构通过就无条件 Smali。

offset 单位必须在类型/字段名上表达：DEX code_unit 与 file_byte、UTF-16 code unit 与输出 UTF-8 byte 不混用。debug local name 只是 hint，不能决定身份或类型。原始字节由 root request scope 持有，P1 借用未经 header 改写的 DexImage；method 的 code/try/debug range 校验后独立记录。不用第三方 `Instruction::bytes()` 拼回所谓原始输入 [DX-11]。

每条 DEX instruction 由一条 Op/Terminator 或明确 metadata（payload）消费；move-result 被 producer 覆盖两 site，goto 被 terminator 覆盖，不凭 “Nop” 静默删除。有真实 NOP 才标 NoEffect；unsupported 无法 Java 表达仍可已知格式 raw fallback。

### 5.2 CFG 与 effect

`FlowBody` 拥有 blocks 与有序 ops；block terminator 独立。normal edges 是 Goto/Branch/Switch/Return；exceptional edges 是 `(throw_site, handler_order, handler_target, catch_type)`，不加入普通支配算法的 normal successor 列表。SSA 分别消费 normal/exception graph。

受保护 may-throw op 必须分割 site：其 block 最后一个 op 是该可能抛出操作，正常结果仅在 normal successor 可见。handler chain 保留原 try 表区间、typed catch 顺序、catch-all 最后一项；不能按类型/目标排序消除优先级。exceptional edge 接受错误前 state，默认 result 不 commit，若特定 opcode 有特殊规则则显式建 transfer，不用一条规则猜所有指令。

`Effect` 表由 opcode 固定元数据驱动：读/写 heap、call、may_throw、monitor、class_init。未知外部 call = 全部保守；字段/数组读即使无 side effect write，也可能 throw/volatile/init，不删。integer div/rem 可 throw，float div/rem 不因此 throw。FP arithmetic 不做 reassociation；未使用计算默认仍输出临时赋值。纯 local copy/常量可以用于寄存器搬运，不让它们引入 checked cast。

### 5.3 SSA 方案

初版用显式 block input 环境 + finite worklist，不要求立即移植复杂 heritage：

- 按方法 descriptor 分配参数 word slots，long/double 占两个物理 words；this 为受约束 Ref 或 UninitThis。环境不是 `reg -> JavaType`，而是 `reg -> ValueId`。
- 每条定义分配 ValueId；wide 定义占两 words 并覆盖以前重叠 slot，读取必须完整 pair；高 word 不能独立当值。未定义/部分重叠使用是 Malformed/ProfileInvalid。
- 给 join 每个 live-in register 预分配稳定 Phi，循环不在每轮创建新 Phi。worklist 传播 Environment 中 finite ValueId/Phi；同一 incoming 则可别名缩减但不重新编号。
- 每个 edge 的环境明确：normal 使用 commit 后环境；throw edge 使用 throw 前环境（以及 opcode-specific 规则）。handler 的 `move-exception` 绑定新 ExceptionValue，不把异常对象等同于任何 incoming 普通寄存器。
- verify：所有非 Phi use 有可达且支配的定义；Phi 每条 eligible predecessor edge 恰有一个值；所有 register/use word kind 合法；没有 Unknown definition 被转换为 Java success。

复杂异常 SSA 是阶段风险；可以先支持无 handler，然后单受保护 invoke，再扩大。阶段支持矩阵必须解释降级理由，不能声称 general SSA 已完成却忽略 exception environment。

## 6. 类型与调用：最容易被“压缩”误删的核心

### 6.1 有限类型域

`ValueType` 初版：Bottom、IntegralEvidence（boolean/byte/short/char/int 候选位集与约束）、Long、Float、Double、Null、Ref(TypeId)、Uninit(AllocationId)、UninitThis、Conflict。原始常量保留 u32/u64 bits，不因第一 use 先解释成整型；每 use 可有 `UseConversion`。同一 DEX const 若分别进入 int/float use，P8 可以为两个纯 literal use 生成不同合法表达，但非 literal 的 numeric reinterpret 不得当数值 cast。

P5 以 opcode、参数/返回、field descriptor、invoke proto 给约束；入队采用当前算法的可用工作队列，依赖就绪即可处理；finite集单调收敛，不额外按`(ValueId, ConstraintKind)`排序追求相同文本。reference join 用给定 hierarchy 的 LUB；没有 hierarchy 证据可用 Object 表示 join，但在 narrower use 若会新增 runtime check 必须通过 “原路径已有等价 check” 或 known assignability 证据，否则局部 Unsupported。不能随便新增 `(T)` 通过 javac 而改变 ClassCastException 行为。

boolean 与 int 不能用 `p ^ 1` 当 Java boolean；boolean literal 只发 true/false。zero-equality 依据被比较值是 int/boolean/ref 分别写 `== 0`/`!x`/`== null`。NaN 比较保留 cmpl/cmpg bias：`cmpg` 的 NaN 结果是 +1，`cmpl` 是 -1，不能把所有 compare+branch 简化成 `<`。literal/FP bitcast 使用 Java 标准库时记录受限 intrinsic 语义与 class-init 风险；不能静默改 bits。

### 6.2 调用证据

**P4 → P5 → P8 交接**：P4 只确定原始调用身份、候选声明完整性、访问/dispatch/throws/参数 descriptor 与证据义务，不要求尚未求出的 source receiver/argument type。P5 消费这些约束生成 ValueType/UseConversion；P8 使用最终 Java 静态类型核查 overload/dispatch 选择。`CallEvidence` 在 P4 可含未完成的 binding obligation，只有 P8 全部 discharge 才构造 VerifiedJavaMethod。P4 的 `verify_declaration_evidence` 不等同于最终 Java 绑定验证；这避免求类型之前又要求准确源类型的循环依赖。

DEX proto 不代表 Java 能按相同 name 精确绑定。P4 保留：owner、name、proto、invoke static/direct/virtual/interface/super/polymorphic/custom、receiver、param word count、access、throws、可能的 overload/bridge。

- 内部声明用 ProgramIndex 中 declared group 与 hierarchy；外部由 DeclarationCatalog 提供候选完整性。catalog 不完整时保持 Unknown，默认需要绑定证据才 Java 成功。
- argument 顺序与 receiver 在 FlowBody 中冻结；显式临时变量让 P8 不重排或重复求值。
- 强制 argument 的 descriptor 静态类型可能消除 overload 歧义；cast 只在已证明可赋值或对应字节码 check-cast 时允许，原 check-cast 即使 unused 仍保留异常。
- 静态 hiding、invoke-super、private/direct、返回类型-only DEX overload、非法 Java identifier/descriptor 必须单独验收。不存在一种“全部给参数加 cast”解决所有分派问题。
- 外部 catalog 来自受信声明快照或同请求依赖，记录来源/完整性/环境标识；不会从网络猜平台事实。最早 fixture 全部使用自制依赖；标准 Java scalar signatures 后续建审阅过的小表。
- Java checked throws：从表达式/handler/原 Throws metadata 求剩余 throws；若新增声明与 override 冲突，或者不能保证调用异常被正确捕获，方法 Unsupported。不偷偷 catch 包装、sneaky throw、吞异常或扩张 public contract。

### 6.3 new/init 与 constructor

`new-instance` 创建 Uninit(AllocationId)，配对 invoke-direct `<init>` 后才成为 Ref。首个 profile 只接受同 block、唯一支配 allocation、无 effect/may-throw intervening op 的相邻 init，pure alias moves 可被追踪；args 已在 allocation 前准备，或是纯 literal。不能让 Java 的 argument 求值跨过 DEX 的 allocation/OOME/class-init site。

真正 `<init>` 的 this 在 super()/this() 返回前是 UninitThis。首版 Java 源码目标 `--release 8`（可用 JDK 21 工具运行）只接受可表达且不观察未初始化 this 的 prologue、单次合法构造链调用、之后 final field writes 满足 Java 约束。复杂 pre-super 字段写、锁/控制流、helper 合成先 Unsupported。不能删除 `super()` 来跳过原始父类行为。父类构造/字段声明不完整时不猜。

## 7. 源变量朴素分配：省掉合并，不省掉边语义

### 7.1 LocalId 方案

所有非 literal、非参数 ValueId 分配独立 LocalId；Phi result 也是独立 local。不做 interference coloring、跨定义 reuse、名字角色推断。声明统一在方法主体可见位置，Java type 来自 P5。debug 名只用于安全命名 hint，重复/关键字/非法字符按本次请求唯一suffix分配；不承诺跨运行同名。

初始默认赋值仅为满足 javac DA：**前提是 `DefinitionCertificate` 证明所有原输入 use 和新增 copy 来源在每条相应路径已定义**，不可能观察默认值。locals 的默认写不代替证书；uninitialized this/allocation 不按 null 初始化伪装正常 Ref。未通过证书则失败。

### 7.2 正常 Phi 与平行拷贝

`CopyPlan.normal[EdgeId]` 是 `(dst LocalId, src LocalId|Literal)` 的同时赋值。多 successor predecessor 必须在选中的 edge 上执行，绝不一律塞 block 尾。结构计划的 Transfer 节点消费 EdgeId，把 copy 放在该 branch/break/continue/goto 等价转移之前。

sequentialize：

```text
去除已证明类型一致的 self-copy
while assignments 非空：
  找 dst 不作为任何剩余 source 的赋值，输出并移除
  若不存在：
    选任一剩余周期的 source s（无需稳定tie-break）
    t = s                  // exact local type，pure copy
    将所有 source==s 改成 t
```

`a <- b, b <- a` 必须 `t=b; b=a; a=t`。loop latch 的多个 Phi 同样以 entry/latch **EdgeId** 分开；存在 shadow/type conversion 不能让 checked cast 进入 copy plan。

### 7.3 异常入口不能像正常边插代码

**ShadowSlot eligibility**：只有每一条能实际选中该 handler 的 throw edge，其 Phi incoming source 都已定义、已完成初始化且存在 exact/no-runtime-check 的 slot type，才建立 shadow。否则即使 handler 体不读该值，普通 SSA join 中的未初始化状态也不得被默认 null/0 冒充；该 Java profile 失败或排除无用 Phi（需独立 retained-use 证据）。宽值与 reference joins 分开，不在 snapshot 里调用 cast/boxing/helper。P8 把 throw-site op 的原有输入临时量先按原序求好，再插纯 snapshot，紧接该 op；若无法精确隔离其失败前状态，则降级。

exception edge 不是程序能执行一个普通 block 的位置。对 handler join 的 retained Phi，为每 `(handler-entry, Phi result)` 分配 ShadowLocal；在每 throw site **之前**，按该 exception edge 的 incoming environment 进行纯 local/literal snapshot。catch ingress 复制 ShadowLocal → handler Phi Local，然后执行 move-exception 绑定。

多个有序 handler 可以需要多组 shadow；在同 site 写所有潜在 catch 的影子槽，写入都是 no-throw/no-allocation、不读未定义值、不改 heap。若源类型转换可能抛出、不同 handler aliases 无法验证、保护区/cleanup 中原 throw-site 无法对应 Java 结构位置，则 Unsupported。shadow 多会增加代码量，这正是保守取舍。

这一方案必须测试 `v = invoke()` 失败时 catch 读取**旧 v**，以及 handler 内再抛异常、共用 handler body、循环内 try、多个 catch-type order。初期仅单 interval/简单独立 handler，未知复杂 join 不强行降级算法证明义务。

## 8. 结构化：自然结构 + 无异常 normal dispatcher

### 8.1 P7 计划与覆盖证书

normal region：使用SCC、算法需要的RPO/支配/postdom识别Sequence、Diamond、NaturalLoop、Switch；不加stable BlockId排序/tie-break以保证相同结构或文本。只读 CFG，私有 region arena collapse；Node 覆盖的 BlockId 集不能重叠或遗漏。initial schemas 不复制 effect blocks，不做 tail duplication/NoMoreGotos 复杂标签恢复。

P8 必须核对 `StructureCertificate`：每 executable op site 恰被处理；每 normal edge 映射为一个 Transfer 或合法 fallthrough；exception chain 与 monitor plan 单独核对；允许同源 site 多个纯 spelling span，但语义 effect 不能重复。visited/active flag 不是证明。

### 8.2 normal dispatcher fallback

Java 无 goto。对**无 exception handler、无 monitor scope、无未处理 uninitialized object**的 normal graph，P8 可以生成具体 Java AST 状态机：

```java
int pc = ENTRY;
while (true) {
    switch (pc) {
        case B0: {
            // 按原顺序执行 ops
            if (cond) { /* edge copies */ pc = B1; }
            else      { /* edge copies */ pc = B2; }
            continue;
        }
        case B1: { /* ops */ return result; }
        // 每 reachable block 一个 case
        default: throw new AssertionError("invalid internal state");
    }
}
```

pc 的值域由 certificate 保证 default 不可达；新增 primitive local/assign 无分配和无 heap 观察。dispatcher eligibility 另要求所有 executable ValueDef、return、condition 和边 copy 的 Java type/用法已通过 P5/P6；不以状态机替代类型证据。每轮恰选一个 case，case entry 环境等于相应原 block entry，ops 与 transfer 保持顺序，pc 更新映射同一 EdgeId；这是要由测试与结构证书核对的逐步模拟关系，不是单凭有 switch 就声明等价。所有条件仅求值一次；Switch 的目标和 fallthrough 转成 EdgeId transfer。任何跨 try/monitor 调度、隐式捕获 Exception 的运行时 interpreter、Object[] 装箱寄存器都不属于这个 fallback。

先实现 dispatcher 能让 if/loop/cyclic CFG 的端到端语义验证早点出现；自然结构是更好 readability 的 adapter，不需双方文本一致，只要求行为和证书一致。unsupported Java 本身仍 raw smali，不能以 dispatcher 名义漏 handler。

### 8.3 异常与 finally 的保守 profile

第一档：单连续 try interval、handler 独立入口、无 normal 跳入 handler、handler 的正常 join 明确、捕获顺序可 Java 表达。保护 interval 按 P2 op 边界确定，Java try 只包含受保护的原效果操作，不能扩大到 finally/外部 call。

- DEX catch-all 先写 `catch (Throwable e)` 并保留 handler 内容，不自动输出 finally。
- javac/d8 的重复 cleanup 可以保留重复，前提是相应 try/catch 控制和 edge-copy 都能完整表达；重复 effect 是原程序不同路径的重复代码，不是把一个路径执行两次。
- 第二档才处理嵌套/相邻保护区、共享 handler；必须具有 ownership 证书，catch 被前项遮盖/多 catch-all/handler body normal reentry 无法 Java 表达时 fallback。
- finally 美化需要全路径 cleanup 等价、throw/return 覆盖和选值时机证据 [DX-07]，不进入必需主路。

**不把有异常图交给 normal dispatcher**。保护结构无法证明时降级 smali，避免低代码量实现中最危险的 catch/cleanup 静默误恢复。

### 8.4 monitor

声明 ACC_SYNCHRONIZED/DECLARED_SYNCHRONIZED 保留 raw metadata，Java method modifier 由精确 flag profile 推导，不照搬 DEX 标志文字。

显式 monitor-enter/exit 只有证明：同 lock identity、区域 single-entry、所有 normal/exception exits 精确 release 一次、null lock 抛出时机、重入/嵌套计数及 catch-all 清理一致，才转 `synchronized(lock)`。lock 在进入前求一次值；不能换成看似等值对象。缺 release、锁错配、handler 保护越界或无法证明只读 scope 都 raw fallback。不要输出空 try 然后锁外执行真正调用。

## 9. Java AST、类级装配与 raw source 策略

P8 的 Java AST 采用有限节点：Block、LocalDecl/Assign、ExprStmt（仅合法 call/new/update）、If、While、Switch、Break/Continue（具名 target）、Return、Throw、Try/Catch、Synchronized、Dispatcher。表达式保留 StaticType/Origin/ExpectedType。未使用 field read 用有类型临时赋值，不写非法的 `obj.field;`。

class shell：支持普通 top-level class/interface/abstract/native，然后 field、constructor、static initializer、nested、自制 enum/annotation profile 分任务逐步扩展。synthetic/bridge 是 metadata 不发 Java keyword；varargs 用最后参数 `T...` 且核对 descriptor array。inner class fragment 不能顶层发 `public static class` 并称完整 compilation unit。

- raw generic policy：descriptor/static source declaration 可使用 raw List 等，保留 Signature metadata 的原始来源；implements/override 的 erased contract、bridge/return-only collision 必须合法。对无法 raw 表达的 public identity 不自动全局重命名外部成员。
- final/static field：encoded_values 与 `<clinit>`/`<init>` 顺序联合验证。不能默认给 final 字段再写；复杂分支无法 Java DA/单赋值证明则 class shell 或相关方法降级。
- enum 必须用 Java enum 语义处理 name/ordinal/constant constructors/values/valueOf 和合法不可见 super 调用，不能写 `extends Enum` 普通 class。没有完整证据时 shell smali，独立可表达方法仍保留。
- nested ownership 与匿名类恢复是后续 capability，不需要源级匿名类美化才成功：可合法命名 nested class 优先。捕获 outer this、super 构造参数、私有访问与 metadata 都要验证，不能单靠 `$` 名字猜。
- annotations：system 注解用于 metadata，不直接写不存在的 Java annotation；用户 annotation 合法元素类型/retention/value 字面量另验。无法 source 表达的注解以 raw metadata sidecar 保留，报告 JavaUnit 的 metadata policy；不能改变运行时可见 annotation 同时宣称相同完整类语义。

P9 编译单元、Mixed 文本和 smali 输出各有明确 label。OriginSpan 使用最终 UTF-8 half-open byte range，包含 MethodKey/FieldKey、原 code-unit origins、合成原因；拼接后 offset 一次统一重定位 [RG-04]，字符串 literal 的 Unicode escaping不能破坏 spans。

## 10. 调度、registry、失效与预算

### 10.1 最小 Kuna 骨架

`pipeline/schedule.rs` 有具体 typed method transitions；`pipeline/pass.rs` 仅在同一 artifact 的 repeat group 使用：

```rust
trait Pass<A> {
    fn id(&self) -> PassId;
    fn run(&mut self, a: &mut A, ctl: &mut RunControl) -> Result<Change, PhaseError>;
}
enum Change { Unchanged, Changed }
```

`PassSpec` 包含 id、phase、required/optional、input/output kind、invalidates、limit category；初版通过 Rust 定义同时提供 schedule 和 dump spec，`verify_registry` 检查重复 ID、必需 pass 不被禁用和读写声明。不做 “外部 TOML 声称一个顺序，Rust 暗中跑另一个”。compiler 不能自动证明 metadata 与算法行为一致，仍需 contract tests。

默认 `RecoveryPolicy`：`NormalStructure::StructuredThenDispatch`；`RecoveryPreset::Core`（readability默认关闭）；`inline_pure=false`；泛型只 raw；未知/复杂 Java → Smali。必需 correct lowering 无用户开关。用户确认后续美化可独立开关扩展，记录最早消费阶段；具体候选/拒绝条件见 [RECOVERY_EXTENSIONS.md](RECOVERY_EXTENSIONS.md)。不为未来可选pass先复制BDD/完整ValueRecovery到核心。强类型 assertions/全局 restart/plugin 延后直到有实际 producer/consumer。

### 10.2 明确失效范围

| 变化 | 保留 | 重算/失效 |
|---|---|---|
| literal formatting/indent/输出名字 | 全部 verified artifact | P9 text/spans（名字若改变 Java binding 则不能仅 P9） |
| normal structured vs dispatcher policy | P1–P6 | P7/P8/P9；只选 plan 不改 CFG |
| 外部 declaration/hierarchy catalog | raw input/P2 原语义 | P4–P9；P3 若异常边/throws 输入被改变也重建，初版用 request 整方法重算最安全 |
| CFG topology/exception chain | 仅 raw/catalog | P2 下游全部作废；不复用旧 ValueId/CopyPlan |
| generic/nested source policy | raw/P2–P3 可复用的值身份 | P4 后的 binding/types/storage/Java；初版同样整方法重算 |

本表是设计规则，不是初版 incremental cache 承诺。初版新请求整方法重跑，保留不可变 ProgramIndex；请求内`EnvironmentId`/profile标识可检测误用artifact；不同请求不要求同数字/跨运行canonicalfingerprint，不为无缓存的初版额外排序事实表。局部 Repeat 只修改指定 finite facts。每回合先 charge 再执行，Changed 必须对应可测 artifact revision/fact delta；没有 delta 的 Changed 视内部错误。

### 10.3 资源限制（工程初值，不是格式或性能结论）

| Limits 字段 | 默认初值 | 约束时机 |
|---|---:|---|
| `max_image_bytes` | 64 MiB / DEX | open 前；超限 Resource，不误报 malformed |
| `max_request_input_bytes` | 256 MiB 合计 | 累加 checked_add 后接收多个 DEX |
| `max_metadata_items` | 1,000,000 | 解析 ID/annotation/class_data 前；同时受剩余输入最小字节数限制 |
| `max_string_units` | 1,000,000 / string | MUTF-8 allocation 前，另有总 decoded units 预算 |
| `max_method_code_units` | 131,072 | method code_item length，fallback 有独立限额 |
| `max_blocks` / `max_edges` | 16,384 / 65,536 | CFG 分割/edge 插入前 |
| `max_values` / `max_constraints` | 262,144 / 1,000,000 | Phi/ValueId/constraint 创建前 |
| `max_work_steps` | 5,000,000 / method | decode/graph/worklist/copy 每个循环收费，不只 pass 入口 |
| `max_copy_assignments` | 262,144 | normal+shadow+cycle temps 合计，先估算再预留 |
| `max_nesting` | 128 | metadata/AST/handler 递归改成显式栈或深度 guard |
| `max_output_bytes` | 8 MiB / method，64 MiB / report | 有界 String writer，含诊断和 span |
| `fallback_code_units` / `fallback_output_bytes` | 131,072 / 8 MiB | 独立只读 smali 路径，不复用已耗尽 work budget |

语法上合法但超限是 Resource；不使用饱和运算 silently truncate。所有 `count * size + offset` checked arithmetic，分配前验证 remaining input/预算，use `try_reserve` 映射资源错误。寄存器数遵循 DEX 字段上限但同时计入 method value/storage budget。每类累积输出和 parallel workers（以后）还有总请求预算；诊断截断必须有显式 marker，不能截断 Java/smali 并标完整。

TraceEvent 至少有 method、phase/pass、start/end、count/changed、budget spent、fallback reason、artifact revision。独立任务按完成/到达顺序记录，不按method输入序重排；无需默认 dump 所有 IR，占用受限。首版只有计数和可选 stage snapshot，不引入独立 console。

## 11. 架构决策与拒绝的替代方案

| ADR | 选择 | 替代与拒绝理由 | 验证 / 任务 |
|---|---|---|---|
| A01 | 原生 checked DEX reader，参考 RG 实现局部 | 直接先喂 rusty-dex 再 guard：可能先分配；整体复制 RG：UTF-16/计数策略不满足契约 | T02–T04 输入安全 |
| A02 | explicit Op CFG → SSA/type/storage → region/Java | 原样移植 dexdec ValueRecovery：依赖 structured IR、大幅增加互相调度 | T06–T10；不宣称普遍更快 |
| A03 | 一 definition 一 local + copy证书 | 全图 coalescing/lexical naming：不属于核心；直接物理寄存器 locals：重用类型/异常旧值风险 | T07、T08、T17 |
| A04 | 无异常 normal dispatcher fallback | Java goto；完全复制 Kuna collapse：目标语言不同；把异常也塞 dispatcher：容易扩大保护语义 | T09、T13–T19 |
| A05 | raw method → 标准 smali 或 Unavailable | 方法抛异常 stub、半个 Java 输出 | T03–T04；mixed 指标 |
| A06 | P4 实际绑定/throws事实，raw generics | 将 P4 判为空：忽略 invocation/source gap；全 source generic solver：首版不必要 | T11、T12、T21 |
| A07 | 不动点只 finite facts，完整 schedule可观测 | 盲目整 band Repeat、无实际消费者的 restart/assertion system | T01、T10、T25 |
| A08 | 先贯通能力切片，再完善阶段 | phase-by-phase 全部完成才验证：末端才发现不可表达的契约 | IMPLEMENTATION_PLAN.md |
| A09 | 美化是可选恢复层，可以后扩展，Core独立 | 一次复制全部恢复算法或将美化放printer做语义改动；可选不意味着允许改变语义 | T24、RECOVERY_EXTENSIONS.md |
| A10 | 唯一 Diana 产品，root CLI/查询 + 内部 diana-decomp；MCP 延期 | 新库另建 APK/CLI、全量 parser 拖慢轻量查询，或未经授权重写 MCP 协议 | PROJECT.md、H01–H10 |
| A11 | 不保证结果/文本跨运行稳定顺序，按完成输出 | 仅为展示序列增加sort/tie-break/按序等待/重排缓冲；这不允许改变程序语义顺序 | PROJECT §6、H03、T25 |

## 12. 未验证风险与退出规则

R1：exception edge SSA 与 shadow copies 是关键算法风险。单 site/单 handler fixture 无法推广到共享 handler，必须分档验收；困难不降证明要求，只 fallback。

R2：raw type policy 不能解决全部 Java overload、bridge、private access、return-only collision；P4 外部 facts 不足会降覆盖率，需明确原因统计后补 catalog，不假设全平台。

R3：constructor/new timing 和 final/class initializer 是 Java 表达难点。先窄支持但不拒绝其他方法；如既定 `--release 8` 源码目标不能合法表达，smali 是正确结果，不自动升级目标语言规则。

R4：normal dispatcher 的默认初始化只在证书下合法；证书有误可能把未定义读变成 0。必须 fuzz/mutation test 保证此类输入拒绝。

R5：natural protected region 的 schema 覆盖有限；重复 cleanup 不一定可直接 Java 表达，任何扩大 catch 或 monitor scope 都需要单独反例测试。

R6：生产 validator 不是 javac/ART，运行测试也不是任意输入证明；缺失真实依赖/授权 ART 环境时，分别标 not-run，不把参考文本等价当完成证据。

R7：新 reader 和内部类/annotation 支持从零会有工作量；已复制代码须许可登记。文档不能用推测 LOC 替代任务验收。

R8：P3/类型域可能在 Java 无法表示的 Phi join、异常帧输入、常量多种 bit interpretation 上卡住。来源表必须区分 pending/undefined/conflict，类型证据不充分允许局部失败，不放宽为 Object/null。T07/T10/T16 的定向反例是扩档前闸门。

每项风险的最小实验、任务和停止条件在 [VERIFICATION.md](VERIFICATION.md)。没有足够证据时默认保留 raw、发明更少；若影响总体输出契约或许可选择，询问用户，不擅自扩大范围。
