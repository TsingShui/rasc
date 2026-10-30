# Diana 设计：产品合同、接口与数据结构

状态：只有Rust包骨架，Engine、可运行fixture与runner尚未实现。算法已选定，见
[ALGORITHMS.md](ALGORITHMS.md)；开发顺序、B/T/H任务及验证统一见[TASKS.md](TASKS.md)。
[e2e_cases.json](e2e_cases.json)的70个固定案例保持独立，不从实现结果刷新。

## 1. 产品边界与工程范围

- Diana是native Rust APK/DEX分析产品。应用持文件/私有snapshot/ZIP、查询、输出；
  diana-decomp借原DEX字节与声明facts，负责分析、Java/Smali与分类结果，不持文件权限。
- 活动workspace只有`apps/diana`和`crates/diana-decomp`，Rust>=1.93、edition2024、
  unsafe_code=forbid、publish=false、0.0.0骨架标记。根Cargo.lock只覆盖Diana。
- `ref-fork/rasc/`独立workspace/lock，仅供阅读及审阅后的聚焦复用，不自动build/test/
  lint/fmt/release，不依赖reference crates。历史固定路径在本地加ref-fork/rasc前缀。
- `apps/diana-gui/`、`apps/diana-web-ui/`仅README占位，不加框架/manifests/deps/CI；
  MCP延期。不提交私有APK、真实语料身份或许可证bundle，不擅自commit/push/发版。
- 正确语义 > 明确失败 > 可实施验证 > 覆盖率 > 可读性。默认美化off；没有恢复/重试平台。
  不把未知opcode当NOP、不造throw桩成功，不以javac成功代替行为证据。
- 独立结果/源码/合成名字无跨运行稳定序要求，不排序/按输入等候/建重排buffer；查询返回
  全部匹配，无分页/offset/limit。原指令、求值/参数/effect、catch、锁和初始化语义序不变。
- 非核心代码测试须用户单独授权；不新增CLI黑盒/help/参数/列名/getter/转发测试。核心
  复杂语义、输入/身份/提取与资源文件安全仍走公开库/module interface。

实际路径：应用`apps/diana/src/`，引擎`crates/diana-decomp/src/`，本文档及固定case在
`docs/diana/`；未来自制资产`tests/e2e/`尚不存在；cache统一在忽略的`.cache/`。
本轮不维护HTML看板、生成器或额外流程/复盘文档。


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
pub struct RunContext<'c> { /* private Limits + cumulative ledger + cancellation callback */ }
// RunContext::new(limits: Limits, is_cancelled: &'c dyn Fn() -> bool) -> Self
pub struct Engine<'a> { /* 不可变 ProgramIndex<'a> + 请求配置；字节由 root scope 持有 */ }
pub struct Options { pub recovery: RecoveryPolicy } // Limits由RunContext唯一持有
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

内部库采用以下interface，不再将取消留到方法分析才接线：

```rust
Engine::open(images: Vec<DexImage<'a>>, options: Options,
             catalog: DeclarationCatalog, control: &mut RunContext<'_>)
    -> Result<Engine<'a>, InputError>;
Engine::decompile(&self, request: Request, control: &mut RunContext<'_>)
    -> Result<Report, RequestError>;
render(report: &Report, control: &mut RunContext<'_>)
    -> Result<RenderedReport, RequestError>;
```

应用每个请求创建一个RunContext并顺序传过读取/索引、分析和打印，不能重建context重试来绕限。
Engine不保存callback或可变control，不需要全局状态/调度器；open与render同样检查取消并返回
Cancelled/BudgetExceeded，不能误报Malformed。Limits不在Options/Engine重复保存。Report拥有
结果必需的声明、literal UTF16、原始身份与证据摘要，不借用临时SSA/Env；render不要求保留
整图，也不因输出失败重新分析或触发另一套恢复路线。

单 DEX 用长度为 1 的 images；DexId在请求内分配并以ImageOrigin映射回root的physical/logical身份，不要求不同到达顺序得到相同数字，也不为文件名/输入顺序额外排序。catalog 允许 empty；类型/调用证据不足时局部降级，不猜测。库无磁盘、进程或网络访问，root request scope 持字节 owner 至 Engine 使用结束；普通 DEX header_offset=0；T04实现原生041读取，H04只做应用借用/原身份适配，T25/H04分别提供D065两侧证据。

`VerifiedJavaMethod/Unit` 字段私有，仅 P8 验证器能构造。证据记录依赖的 `EnvironmentId` 与支持 profile；**生产引擎不调用 javac**。外部环境满足声明事实时才声称合法 Java；测试必须使用真实配套 classpath。不能把 Unknown 外部方法完整声明集当 Complete。

`VerifiedJavaMethod`内部采用`MemberEmission::Explicit | Generated(owner_node, rule)`：
Generated用于已证明由enum/override/inner语法生成的有code成员，不计DeclarationOnly。
unit printer只打印owner语法，不重复发bridge/values等冲突声明；原MethodKey映射到owner spans。
单独请求无法独立表达的Generated方法时返回原方法Smali，不造wrapper，详见ALGORITHMS §12。

`JavaUnit` 要求合法 class shell、所有成员为 Java（含已验证Generated）或 DeclarationOnly、成员绑定/初始化/构造约束全通过。只要某方法 smali/unavailable 或 class shell 无法表达，结果为 Mixed；renderer 顶部必须标明非完整 Java，不能用 `.java` 后缀暗示可编译。方法级成功允许存在于 Mixed，但编译测试必须用可表示的上下文，不注入改变行为的 stub。

### 2.3 四类失败与取消

| 分类 | 例子 | 处理 |
|---|---|---|
| `Malformed` | 越界 ID、截断 opcode、错误分支边界、无效 LEB、使用未定义寄存器 | 全局 header/catalog 无法定位则 RequestError；局部 method Invalid → Unavailable，可附明确 raw excerpt。不是普通 Unsupported，不输出声称有效的 smali |
| `Unsupported` | 已知合法 invoke-custom、无法合法化的 constructor、无法证明的 monitor/异常结构、缺外部声明证据 | 从完整校验的 raw method 重新生成 smali；raw 不足则 Unavailable |
| `BudgetExceeded` | 方法 blocks/worklist/copies/输出超过限额 | 停止该方法分析；在独立小额 fallback 预算内 smali，否则 Unavailable；不得重试增加限制 |
| `InternalInvariant` | verifier 前后身份不一致、证书漏 edge、算法 impossible 状态 | 诊断显式 BUG、dev test 必败；请求报告可保留方法级 raw fallback，但状态仍为内部错误，不能算正常 Unsupported 或 Java 成功 |
| `Cancelled` | caller 的取消 token | 停止请求，RequestError::Cancelled；不为取消逐个生成 smali |

任何分析失败后的 Java AST/文本全部丢弃，fallback 永远只读原始字节。一个方法失败不修改其他方法配置。Unknown opcode 若无法可靠识别格式，只能 raw excerpt（例如 offset+hex），不能发明非标准 `.word` 并称 smali。

## 3. Module与数据

输入、方法分析、打印三个seam；不暴露私有CFG/Phi/AST。仅创建当前实际消费的文件和容器。

### 3.1 实际Module与文件

阶段是所有权/职责划分，不要求一职责拆一个文件。先在当前phase的mod.rs实现具体
函数，文件变大或出现真实独立消费者后再拆。不存在空pass/通用Context占位。

| Module | 初次实际文件 / 主要函数 | 所有者 |
|---|---|---|
| Engine/input | engine.rs、p1_input/{mod,cursor,header,code}.rs；open/index/method_input | T01/T02/T04 |
| Raw format | p1_input/formats.rs、p2_flow/decode.rs；decode/validate_raw | T03/T04 |
| Flow | p2_flow/{mod,build,effects,results,protection}.rs；decode_and_bind | T06/T16 |
| SSA | p3_ssa/{mod,inputs,liveness,build,verify}.rs；build_ssa | T06/T07/T16 |
| Calls | p4_calls/{mod,members,allocation,contracts}.rs；resolve_calls | T11/T12 |
| Types | p5_types/{mod,constraints,lattice,solve,conversions}.rs；solve_types | T06/T10 |
| Storage | p6_variables/{mod,locals,copies,exceptional,definitions}.rs；allocate_and_copy | T08/T17 |
| Regions | p7_regions/{mod,normal,protection,monitors,verify}.rs；identify_regions | T09/T13/T18/T19 |
| Java | p8_java/{mod,expressions,dispatcher,structured,declarations,initializers,ownership,enums,verify}.rs；lower_method/assemble_class | 各核心能力卡 |
| Emit | p9_emit/{mod,java,smali,literals,spans}.rs；render | T03/T06 |
| Control | pipeline/{schedule,control,diagnostics}.rs；直接typed调用和配额检查 | T01 |
| Facts | p0_knowledge/{mod,declarations,policy}.rs；不可变catalog/源码策略 | T01/T11 |
| Data | substrate/{ids,origin,types,cfg,java}.rs；仅当前使用的容器/ID | 随实际消费者创建 |

不建registry.rs、Pass trait、调度macro、build.rs生成器或restart framework。可选trace
只用有限PhaseId和有界event，不为它建立测试/平台。tests/support只接既有公开interface
与根tests/e2e资产；核心测试按行为分文件，不按私有phase调用次数构造golden。

### 3.2 Public interface与生命周期

应用每请求创建一个context并用于完整链，不另建context重试；Engine不保存可变control。
open/index/render中的循环同样可取消。InputError/RequestError携带统一FailureKind，
取消/资源不能包装成Malformed。callback是协作式检查，不承诺抢占外部codec或硬超时。

DexId按本次images枚举分配，映射到原ImageOrigin；原始身份是origin+原表index/proto，
不是输出数组位置。同一physical slice的041 views输入admission计一次；各logical
catalog独立。caller提供的相同origin必须对应相同原bytes/header，冲突请求拒绝。

Report是owned结果：Java AST中literal UTF16、所用声明和原身份复制/驻留到ResultFacts，
计入同请求配额；不借Env/Phi arena，不携带整套分析图。render读ResultFacts即可，不需
活着的Engine。RenderedReport拥有UTF8 text/spans；JSON/文件/CLI由应用负责。

供engine与实际轻量scanner共用的输入辅助为
`raw::instruction_layout(code_bytes: &[u8], pc_cu: u32, version) -> Result<InsnLayout, RawDecodeError>`：
`InsnLayout { width_cu: u32, refs: [Option<IndexedRef>; 2] }`，ref含kind/index/operand_slot。
它只核format、padding与完整word/payload范围，不冒称全method/index/branch验证；caller
循环收费并核各自catalog。实现与内部decoder共用同一OpcodeSpec，不公开SSA/私有phase。

### 3.3 内部产物：按值移动，不复制历史图

```text
RawMethodView { key, checked code/try/debug ranges }
ValidatedRawMethod { raw_view, raw_insns, payloads, checked metadata }
MethodInput { key, raw, decl }
FlowBody { raw, cfg, origins }
SsaBody { flow, values, phis, bound_uses }
ResolvedBody { ssa, bindings, allocations }
TypedBody { resolved, value_types, use_conversions }
StorageBody { typed, locals, copies, definition_evidence }
PlannedBody { storage, regions, protection, monitors }
```

raw借原scope；各stage消费上个owned product。临时live/Env/solver worklist在最后消费点
释放，不必永久挂在wrapper里。证据仍引用的表不得提前drop；P8完成验证后复制结果必需
facts到owned输出，整套PlannedBody可释放。禁止全图Arc<Mutex<_>>或第二套可变MethodArena。

#### 3.3.1 ID、slot与原始来源

```text
DexId/ClassKey/MethodKey/FieldKey       请求内原声明handle
InsnId/BlockId/EdgeId/ExceptionEdgeId   同方法arena handle，不重用
ValueId/PhiId/LocalId                  定义/Phi/源码storage，不等于物理reg
Slot { start: u16, width: One|Two }
Site { method: MethodKey, code_unit: u32 }
Origin { sites: Vec<Site>, synthetic_reason: Option<Reason> }
```

所有arena增长先计费；u32 ID耗尽为BudgetExceeded，不wrap。UTF16/input byte/source UTF8
坐标不得混用。debug names只作提示，不决定成员身份、slot或type。

#### 3.3.2 FlowGraph

```text
Block { ops: Vec<OpId>, terminator: Terminator }
Op { raw_opcode, inputs: Vec<Slot>, normal_defs: Vec<Slot>, effect, origin }
Terminator = Jump(EdgeId) | Branch(Condition, true_edge, false_edge)
           | Switch(ValueUse, Vec<(i32, EdgeId)>, default_edge)
           | Return(optional use) | Throw(use)
Edge { src, dst, kind }
ExceptionEdge { throw_site, handler_target, chain_id, rank, catch_type }
```

P2是唯一拓扑编辑阶段；受保护may-throw最后，normal/exception edges分开。producer与
move-result共享语义Op但保留原两个site；payload不作Op。entry还有EntrySeed incoming，
不是真实可跳转代码。未校验EndInvalid不进入SsaBody。

#### 3.3.3 SSA固定入口Phi

```text
ValueDef = Parameter(slot, descriptor) | This
         | OpResult(op, slot) | PhiResult(phi) | ExceptionAt(handler)
Sym = Def(ValueId) | Phi(PhiId) | Bad(Undefined|PartialWide)
Phi { block, slot, result: ValueId, inputs: Vec<(IncomingEdge, Sym)> }
IncomingEdge = EntrySeed | Normal(EdgeId) | Exceptional(ExceptionEdgeId)
Env = sparse map<Slot, Sym>             // builder scratch，不进最终SsaBody
```

采用ALGORITHMS §3：live-in只增；所有block live-in都建Phi；每block一次绑定、每incoming
一次连接；retained/bad/grounded各一次单调标记。构造中的尚未填入用Option表示，不是
语义Undefined。首版不做trivial-Phi或别名缩减，也不传播ValueId直到“看起来不变”。

验证器核每个Op input是本block入口Phi或已执行normal Def；Phi input等于对应前驱出口/
throw前slot绑定。缺定义/partial wide与类型不可表达分开。普通dominator不替代异常检查。

#### 3.3.4 类型、binding与allocation

```text
ValueFacts { width, finite candidates, producer requirement, required_uses }
UseConversion { use_site, target_source_type, kind, supporting_fact }
CallEvidence { original_ref, invoke_kind, candidate_group, completeness,
               parameter_words, access/throws/init facts, binding_obligations }
Allocation { id, new_site, init_site, aliases, accepted_constructor_profile }
```

类型solver只改有限facts；最终use转换全有定义后交P6。纯数值literal可逐use拼写，
非literal不能在int/float/ref之间猜cast。P4交给P5的是约束，P8才核最终Java overload/
dispatch/throws；不得循环要求P4先知道source static types。

### 3.4 DeclarationCatalog与最小调用绑定

具体不可变数据表：TypeDecl、MethodDecl、FieldDecl、HierarchyEdge。FieldDecl另记录
`JavaConstantKind=Constant|NonConstant|Unknown`，用于防止sget被javac常量内联；raw encoded
值不自动证明外部source常量属性。每个owner/candidate
组有Complete/Partial/Unknown及来源。empty合法，缺证据仅影响相应Java能力；不读文件、
查网络、spawn或抓全平台数据库。EnvironmentId只标本请求facts快照，不做跨请求内容缓存。

```text
lookup_type(TypeRef) -> Fact<TypeDecl>
lookup_field(FieldRef) -> Fact<FieldDecl>
lookup_method(MethodRef) -> Fact<MethodDecl>
overloads(owner, name, invoke_kind) -> {candidates, completeness}
subtype(a,b) -> ProvenYes|ProvenNo|Unknown
```

subtype遍历已知parent/interface图，用visited+显式队列，循环/深度/候选数计费；只有
穷尽完整祖先且未命中才ProvenNo，缺节点为Unknown。overloads包含继承/hiding与access
所需事实；candidate遍历不能以第一个到达项代替Java选择。

P8采用精确formal静态类型的fixed-arity调用，候选applicability只允许已知的reference/
primitive widening，不引入boxing/varargs展开。适用集合中要求唯一最specific erased
目标并与原invoke解析一致。generic inference、return-only collision、未知父契约等
不在本profile中猜测。完整步骤见ALGORITHMS §9。

### 3.5 Storage与词法计划

```text
CopyPlan {
  entry: ParallelCopySet,
  normal: EdgeId -> ParallelCopySet,
  before_throw: InsnId -> Vec<ShadowSnapshot>,
  ingress: HandlerEntry -> Vec<LocalCopy>,
  temps: Vec<LocalDecl>
}
ShadowSnapshot { handler, phi, dst_shadow, src_local_or_literal, exact_type }
TransferPlan { original_edge, copies, target, abrupt_kind }
RegionPlan { owned_blocks, owned_edges, nodes, entry, follow }
ProtectionPlan { owner_try, ordered_catches, site_to_catch_stack }
MonitorPlan { enter_site, lock_identity, body, normal_exits, release_handler }
```

copy先选edge再执行；exception snapshot紧挨原throw前；catch参数绑定ExceptionAt后再
复制shadow。独立重放原uses/entry copies/transfer核默认local不可观察，不信布尔证书。
region不改CFG，不为自然源码复制effect。try先inner后outer归约，验证实际catch-stack；
monitor验证有限lock stack与所有退出后才替换synchronized。具体匹配规则归ALGORITHMS，
不能让printer补不满足的结构或扩大保护域。

### 3.6 Java结果、Generated与打印

```text
VerifiedJavaMethod { private method_decl, body_or_generated, ResultFacts, evidence }
JavaMemberKind = Method | Constructor | StaticInitializer
MemberEmission = Explicit(JavaBody) | Generated(owner_node, canonical_rule)
VerifiedJavaUnit { private class_decl, explicit_members, generated_member_evidence }
Expr { source_type, origin, kind }
Stmt = Decl|Assign|ExprStmt|If|While|Switch|Break|Continue|Return|Throw|Try|Synchronized
```

`Generated`仍有原可执行方法，只是由enum/override/inner语法生成，不是DeclarationOnly。
P8须匹配ALGORITHMS §12的canonical行为；不见synthetic就删。单独请求不能独立表达的
Generated方法时返回原Smali。JavaUnit需所有成员或其生成规则及整个shell通过验证；
Mixed可以展示但不冒充完整可编译unit。private Verified构造器只供owning validator。

StaticInitializer只发static block，不发名为<clinit>的方法；return-void/必定abrupt的
Java合法化按ALGORITHMS §12.2处理，不能让printer临时猜控制流。

P9是纯printer，不求类型/结构；输入已封口但仍逐段检查预算/取消。Java表达式使用
precedence表：primary/postfix→unary/cast→mul→add→shift→relational→equality→bitwise
and/xor/or→logical and/or→conditional→assignment。低于父优先级必须括号；同优先级的
右operand对sub/div/rem/shift及非结合运算保留括号，FP不重结合。简单实现可多加括号。
statement-expression仅call/new/update/assignment；裸array/field/arithmetic读发typed赋值。

OriginSpan为最终UTF8 half-open区间；每次写入前记byte cursor，完成node后记录origin。
拼class/unit前缀时统一重定位；Generated映射到其owner语法，原始key不丢。local名字
用本请求唯一v前缀即可，不做命名恢复或文本稳定排序。

### 3.7 RunContext与预算实现

Limits仅归RunContext。公开方法顺序调用；内部RunControl借`&mut RunContext`并加当前
method子配额。无需RefCell/Mutex、全局context或self-referential结构。

```text
Quota { limit, used, held }
claim(n):        checked used+held+n<=limit; held+=n
spend(n):        checked used+held+n<=limit; used+=n
spend_held(n):   held-=n; used+=n          # 仅对应已领取的保留量
release_unspent: held-=remaining         # 不是退款已消费的work/分配
```

request与method同一操作分别核本层上限，不相加当成两份内存。method入口先claim raw
work/分配保留量；normal不能消费held。MethodBudget drop归还未消费held，不回退used。
无法预留就不启动normal；可定位方法报资源Unavailable，连报告也装不下则RequestError。
Cancelled在检查点立即向请求传播，不逐方法生成raw。

分配以容量增量计费：Vec按delta_capacity*sizeof(T)，字符串按encoded字节，HashMap/
集合加保守bucket/control开销；checked后才try_reserve。输入admission与派生分配分开，
同一caller-owned slice借用不重复计驻留内存。此账本是累计逻辑上界，不伪称精确RSS。

每个decode、graph、slot集合/约束变化、copy、subtype与写出循环调用charge/check_cancel；
长字符串按固定小块处理，trace/diagnostic不得绕过限额。实际默认数字以DESIGN §10.3
为唯一表，不再生成第二份registry。render只发完整结果，错误不重新分析/重试。

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

初版的固定点仅为P3 live-in需求与P5有限类型域；P3采用固定入口Phi、block一次绑定、incoming一次连接及有限bad/grounded传播，不反复猜ValueId/别名。它们只收集有限facts，不在循环创建新身份。P2 的 topo normalization 先收集编辑，再原子应用一次。可选纯表达式内联以后加入 P8 的有限 rewrite group，不改变 SSA/CFG。**不把 P3–P6 整段塞入互不动点**，也不强制所有 phase 必须各有相同数量 pass。

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

采用ALGORITHMS §3的**固定入口Phi建造**，不再保留两个待选SSA策略：

- `Slot={start,width}`；宽值作为一个slot读取，所有source先读、normal writes后写，写入使重叠旧slot失效。
- 先求normal/exception live-in需求，再为每个block每个live-in slot分配Phi（单predecessor也保留）。虚拟EntrySeed提供参数/this，因此入口回边也有正确首次值。
- 每个block一次扫描，从入口Phi和本block先前Def绑定use；每条normal/exception incoming一次连接。exception使用对应site的旧环境，不使用成功result。
- 从真实uses保留Phi闭包；Bad沿Phi users单调传播，grounded从参数/OpResult/ExceptionAt单调传播。无用Phi剔除；实际缺定义、partial wide或无根定义环明确拒绝。
- 首版**不消除trivial Phi、不做别名缩减**。所有状态/遍历有有限度量和计费；构造Env工作表封口后drop，不把R×E矩阵一路带到P8。
- verifier从raw读写顺序与edge transfer重放本地最新定义，核Phi来源、word宽度与异常旧状态，不信builder bool或normal dominator。

复杂异常 SSA 是阶段风险；可以先支持无 handler，然后单受保护 invoke，再扩大。阶段支持矩阵必须解释降级理由，不能声称 general SSA 已完成却忽略 exception environment。

## 6. CLI合同

十一命令、字段、选择器与退出码只在本节维护，任务卡不复制另一份规范。

### 6.1 顶层帮助与命令表

目标帮助的命令部分：

```text
Commands:
  get-class   Fetch one class as Java
  entries     List APK/ZIP entries (columns: name, compressed_size, uncompressed_size, compression)
  extract     Extract original entry bytes to -o
  strings     List DEX string literals (columns: at, value, entry)
  xref        Search code references across every DEX (columns: entry, context, match)
  manifest    Extract decoded AndroidManifest.xml
  classes     List classes across every DEX (columns: at)
  get-method  Fetch one method as Java
  fields      List fields across every DEX (columns: at)
  methods     List defined methods (columns: at)
  help        Print this message or the help of the given subcommand(s)
```

下表是命令合同；usage是参数形式，不代表实现已经存在。

| Command | Summary | Columns | Usage |
|---|---|---|---|
| get-class | Fetch one class as Java | — | diana get-class <input> <class> |
| entries | List APK/ZIP entries | name, compressed_size, uncompressed_size, compression | diana entries <input> |
| extract | Extract original entry bytes to -o | — | diana extract <input> <entry-name> -o <file> |
| strings | List DEX string literals | at, value, entry | diana strings <input> |
| xref | Search code references across every DEX | entry, context, match | diana xref <input> <kind> <target> |
| manifest | Extract decoded AndroidManifest.xml | — | diana manifest <input> |
| classes | List classes across every DEX | at | diana classes <input> |
| get-method | Fetch one method as Java | — | diana get-method <input> <method> |
| fields | List fields across every DEX | at | diana fields <input> |
| methods | List defined methods | at | diana methods <input> |
| help | Print command help | — | diana help [command] |

核心形式统一为 **`diana <command> <input> [selector] [options]`**。`help`、`--help`、`--version` 不要求 input。input 是一个 APK、含 root DEX 的 ZIP 或裸 DEX；entry 命令与 manifest 的更窄输入范围见下文。不自动调用 d8，不直接处理普通 JVM `.class` / JAR 字节码。

### 6.2 通用输出与选项

- 默认表格为 **UTF-8 TSV**，首行是指定列名，恰按上表列顺序；不额外加序号、索引表、颜色或调试列。记录顺序不保证，按完成/到达输出。
- `--format text|json`，默认 text；表格 JSON 为对象 `{ "items": [...] }`，字段与列名相同。Java/manifest 的 JSON 是明确 domain report，见 §6.5/§6.7；不是 MCP envelope，不设计协议工具。
- `-o, --output <file>`：文本/JSON命令可指定文件，成功时文件与 stdout 的同次 payload 字节相同；`extract` 是例外，只向文件写二进制，不向 stdout 输出副本。
- `--filter <literal>`：仅适用于 `classes` / `fields` / `methods` / `strings`；大小写不敏感的字面子串，作用于完整 `at` 表达中语义名称/描述符（不匹配 opaque e/l/c/m/f/s 索引前缀），strings 则作用于 value。不是正则，不截取记录。
- `--class <class>`：fields/methods 的定义类选择；xref 的引用方类选择。采用 §6.3 精确 class selector，不默认模糊匹配，不代替 xref target。
- `--jobs <N>`：对独立 DEX 扫描的并发资源配置，N 必须大于0；不承诺稳定输出，也不要求 engine 每个方法另起线程池。无相关并行工作的命令不暴露无效选项。
- `--debug`：诊断/trace 去 stderr；不污染 TSV、JSON、Java、XML 或提取文件。没有“按钮改变但实现未消费”的选项。
- **没有 `--offset` / `--limit` / cursor / preview / stable-output**。成功查询返回全部匹配项。安全 input/work/output 预算保留：超预算错误，不以只返回前N条冒充成功。内部 batch/backpressure 不是分页。

TSV 的任意字符串字段按显示转义：`\\`、`\t`、`\r`、`\n`、`\0`；孤立 UTF-16 surrogate 用 `\uXXXX`，不得改成 replacement character 或让一条记录占多行。列中包含 delimiter 的值不靠有损切割。JSON serializer 同样保留必要的 Unicode escape；控制/孤立 surrogate的具体表示须 F03/Q09 验收。`at` 自身可逐字回传 CLI，包含空格或 `$` 等时需 shell 引号。

流式 text 查询可以在失败前已写部分记录，必须非零退出并在stderr明确错误；这种部分输出不是完整成功。JSON报告、Java获取、manifest和extract文件按完整产物构建并原子提交，失败不留假完整文件。大表格JSON受完整结果预算；用户可改用流式text与外部shell过滤，不通过新增分页解决。

### 6.3 `at`：原始身份定位符，不是结果行号

列表必须具备可回传的精确定位，不依赖结果顺序。**索引均为原输入**，不是排序后的行号、合成 LocalId 或 Java 名称。

#### 3.1 规范格式

```text
class-at  = e<physical-entry>:l<logical-member>:c<class-def>::<class-descriptor>
method-at = e<physical-entry>:l<logical-member>:c<class-def>:m<method-id>::<owner>-><name><proto>
field-at  = e<physical-entry>:l<logical-member>:c<class-def>:f<field-id>::<owner>-><name>:<type>
string-at = e<physical-entry>:l<logical-member>:s<string-id>
```

数字为非负十进制，`e`是物理 ZIP central-directory record index，`l`是该entry内部 logical DEX序号（普通DEX为0），`c`为class_def index，`m/f/s`为对应DEX原id表index。裸DEX视为单entry `e0:l0`。规范class descriptor如`Lcom/example/App;`；method proto必须包含参数与返回type，如`(ILjava/lang/String;)Z`。

例子（示例索引，不是真实 corpus）：

```text
e0:l0:c7::Lcom/example/App;
e0:l0:c7:m23::Lcom/example/App;->check(I)Z
e0:l0:c7:f9::Lcom/example/App;->count:I
e0:l0:s15
```

规范格式中的 descriptor/name/proto 与索引双向核对：修改输入后原定位符可能失效，返回错误，不能索引指向别的对象时忽略名称检查。不同输入/DEX相同名字不合并。所谓“不重不漏”指每个原身份记录一次，不是按文本name去重。

#### 3.2 可读短选择器

- get-class / `--class`接受 class-at、`Lowner;` 或 dotted `com.example.App`；点仅按明确class名称归一化，不做模糊搜索。
- get-method接受 method-at 或完整 `Lowner;->name(args)return`；可读owner也可dotted，但proto中的描述符不能随便点斜杠替换。**没有裸方法名或单独 `--method` / `--proto` 组合**，一个完整selector已表达身份。
- xref field/method target接受原始完整引用 descriptor；也可接受规范field-at/method-at，但先校验定位并解析为reference，再跨全部DEX查引用。目标选择definition不自动限制引用方entry。
- 短名称若有多定义，返回 `AMBIGUOUS_SELECTION`（exit4）并提供规范at定位信息；不按最先完成的worker任意挑一个。exact at可以区分同DEX同descriptor的重复class-def。
- 查询空集合正常 exit0；唯一对象获取/提取缺失exit3。非法at或proto语法是用法错误exit2；范围失效/名称索引不一致是输入/selector错误exit1，不发错对象。

### 6.4 列表命令

#### entries

`diana entries <apk-or-zip> [--format text|json] [-o file]`。

列恰为 `name, compressed_size, uncompressed_size, compression`，sizes是非负字节数，compression用数值ZIP method（例如0 stored、8 deflate），不是随语言变化的标签。不解压所有entries来列目录；不按name去重或排序。所有physicalrecords都列出，未知压缩方法也可作为metadata列出，不意味着extract支持。

指定列不含额外entry_index。重复name的精确提取通过 `extract --entry-index`，详情与歧义诊断可带index；不擅自在entries默认列里加第五列。裸DEX不当ZIP目录，明确输入类型错误。

#### classes / fields / methods

```sh
diana classes sample.apk --filter 'example'
diana fields sample.apk --class 'com.example.App'
diana methods sample.apk --class 'com.example.App' --filter 'check'
```

每命令只有 `at` 一列。classes枚举class_defs定义；fields枚举各class_data中实际定义的static/instance fields；methods枚举direct/virtual **defined methods**，含abstract/native/constructor/clinit声明，不把所有method_ids里的外部reference当defined method。

使用原始DEXname/proto/index，包括synthetic/bridge flag涉及的原方法（flag如需查看属于JSON后续明确扩展，不擅自增列）；不为只列符号运行Diana SSA/Java后端。duplicate定义各有独立at，CLI只列其原identity，非法结构仍错误。

#### strings

`diana strings <input> [--filter literal] [--format text|json] [-o file]`。

列为 `at, value, entry`。这里默认采用原查询的**string_ids值枚举**（既包括代码literal，也包括名称/描述符/注解值），不声称仅扫描`const-string`出现的字符串。代码实际使用位置用`xref ... string ...`查询，两者不混淆。保留完整UTF-16值并按§6.2转义。

entry列是物理entry原name；041 logical/member身份在at里，不拼造另一个真实ZIPname。裸DEX的entry使用caller提供输入的basename作为展示标签；不用于身份判断。

### 6.5 get-class / get-method：默认必须是 Java

```sh
diana get-class sample.apk 'com.example.App' -o App.java
diana get-class sample.apk 'e0:l0:c7::Lcom/example/App;'
diana get-method sample.apk 'Lcom/example/App;->check(I)Z'
diana get-method sample.dex 'e0:l0:c7:m23::Lcom/example/App;->check(I)Z'
```

- 默认text：get-class只在完整`VerifiedJavaUnit`时成功输出Java；get-method输出完整方法声明/body，abstract/native输出合法DeclarationOnly。方法源码需要owner上下文，不声称它本身是一整个编译单元。
- 默认Java不可恢复时exit1，stdout不发smali、Mixed或半个Java；stderr报告分类原因/原identity。失败不要写 `.java` 文件，也不通过抛异常stub补齐。
- **引擎仍保留逐方法raw fallback**，只是CLI不把它当“Fetch as Java”的默认成功。`--format json`显式获取完整domain report，允许`java_unit/mixed`以及方法`java/declaration_only/smali/unavailable`，每项原identity与diagnostics/spans明确。完整Mixed报告本身可exit0；requesterror、BUG、Resource、Cancel仍错误，不因有raw片段变成功。
- JSON保持`schema_version:1`作为**本地CLIreport格式**（不是MCPversion），含command/selection/status/source/methods/diagnostics/spans；不serialize内部AST/arena。source_range/spans相对于对应source字段UTF8bytes，字段scope明确。top-levelreport与method-onlyprojection在接口规格/源码审阅中明确，不为这种基础包装另建测试；复杂语义/raw/identity验证走公开库interface。
- 取消此前 `getclass --members` 与 `--require-java` 提议：fields/methods是独立符号清单，get-默认就要求Java。不要同时输出索引表前缀污染source/span坐标。没有旧命令alias或自动转旧enginefallback。

Java编译成功不等于语义等价；完整classpath和语义fixture仍按TASKS §4验收。JSON显式选择并不允许吞掉malformed/内部错误、假smali或语义不忠实Java。

### 6.6 xref

```sh
diana xref sample.apk string 'Authorization'
diana xref sample.apk type 'Ljava/lang/String;'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z'
diana xref sample.apk field 'Lcom/example/App;->count:I'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z' --class 'com.example.Caller'
```

kind明确为`string|type|method|field`；target是字面精确值/descriptor（string为完整UTF16字符串相等，非正则；type接受class dotted归一化或标准primitive/arraydescriptor）。首版不把名相同的所有overload混为methodreference。以后substring/fuzzy搜索必须单独命名/验收，不从当前裸value猜。

默认跨输入的每个rootDEX和041logicalmember扫描代码引用，使用原始opcode/refindex；不先反编译源码再搜索。`--class`只约束引用方定义，可读短selector歧义规则同§6.3。

列恰为 `entry, context, match`：

- entry：引用方物理entry名，显示标签不决定identity。
- context：`<referencing-method-at>@cu:<offset>`，offset是该方法原始code units，显示为十六进制如`@cu:0x0012`；不是输出行号。
- match：实际被引用的字符串值、TypeRef、MethodRef或FieldRef，按§6.2转义；不得只回显用户target但忽略真实引用。

**每个实际引用site一行**；相同方法多个site各列，不能像旧findrefs方法级结果那样无声合并。查询相同site可能重复引用参数时按已定义opcodeoperand-site身份表达，不以名称去重。这是有意精化，H09/Q09验收。空结果header/emptyitems exit0；成功全部匹配，不分页、不保证顺序。

### 6.7 extract / manifest

#### extract：原始 entry payload，不是重建 DEX

```sh
diana extract sample.apk 'classes.dex' -o original.dex
diana extract sample.apk 'classes.dex' --entry-index 4 -o original.dex
```

`extract`只处理physicalAPK/ZIPentry。原始字节指**解压后的完整entry payload**：method0直接原payload、method8原deflate解压；不是ZIPlocalheader或压缩流，不改DEXheader/checksum、不切041logicalview、不做Java/smali转换。文件内容须与原entry解压bytes逐字相等。

`<entry-name>`精确匹配，重复name未指定`--entry-index`则exit4并列出候选physicalindex；有index时须核name与index一致。只提取一个entry，`-o`必需且是一个文件路径，不提供批量目录解包。这样ZIP中的`../`等name不成为输出路径，避免ZipSlip。

默认不覆盖已存在输出；`--force`显式允许替换普通文件，拒绝输出目录/符号链接/输入文件自身或hardlink别名。先创建目标目录内private临时文件，有界解压/完整性校验后原子提交；取消/解压错误/预算不足清理临时文件，不输出半成品。no-force发布不得存在check-then-rename覆盖竞态；force也要按平台语义验收。不向stdout打印二进制或状态行，错误/`--debug`仅stderr。

compression不支持、加密、坏range/size/CRC必须明确错误，不能列目录成功就假定提取可靠。CRC由H10按TASKS §6遍历实际payload明确核验，不假定codec默认提供；尚未实现。预算受原input/inflate限额，不是分页。

#### manifest

`diana manifest <apk-or-zip> [-o file] [--format text|json]`。

默认输出完整decoded `AndroidManifest.xml`；原binaryAXML bytes用extract获得。沿用原项目最后一个同名physicalentry的选择规则（依据目录顺序而非worker完成顺序），manifest选择不会因无序显示改变。缺失exit3，坏AXML/超预算exit1；裸DEX或普通JAR没有manifest则输入错误。

text为XML；json为`{ "xml": "..." }`，不附假Java状态。默认无自动外部工具；调用patchedvendoredAXML前检查其PATCHES及安全范围，修改须独立测试。

### 6.8 帮助、错误和退出码

- `diana help`、`diana --help`、`diana help get-method` / `diana get-method --help`成功exit0。
- exit0：完整查询/提取/manifest、符合默认Java要求的get、显式JSON完整报告（含标记清楚的Mixed）。
- exit1：I/O、Malformed、UnsupportedJava默认get、预算、内部BUG、取消、失效选择器等请求失败。
- exit2：未知命令/flag、非法selector/proto、缺input、extract缺`-o`、不适用选项组合。
- exit3：唯一对象/entry/manifest查无结果。列表/xref空匹配是正常exit0。
- exit4：唯一选择有歧义，如重复class/method/entry名，拒绝任意取第一项。

BrokenPipe按原Unixfilter行为正常结束，不打印panic。项目名称统一Diana；不提供`getclass`、`findrefs`、`skill`、`--members`、`--require-java`、`--class-id`、`--offset`、`--limit`等旧提议接口；canonical at已解决精确identity。

`mcp`延期，旧实现只在ref-fork，不恢复活动接口或设计新协议；旧agent skill安装逻辑也不自动执行迁移/删除用户目录。agent使用说明作为普通文档/发布资产，不添加第十二个公开`skill`命令。

### 6.9 实施与验证落点

- H01冻结本页命令/flag/selector/error规则，update futureCLIargs/README，但当前生产源码未改。
- H03复用快查询并迁移指定TSV/JSONcolumns；H04 canonical at与041原始identity；H05两个get命令与默认Java/显式JSON规则。
- H09补fields/methods **definitions** 和xref **sites**；H10完成原entryextract/no-overwrite/CRC/atomicfile安全；H07统一最终help与agent说明。
- 按用户约定不新增CLI黑盒测试。Q01/Q07是基础接口人工审阅清单，不是测试族；Q05/引擎F测试走公开反编译库语义回环，Q02–Q06/Q09/Q10仅验证复杂身份、内容、安全与资源义务。具体Agent卡见[TASKS.md](TASKS.md#6-应用任务-h01h10)。

命令/flags/help/列名/默认包装/旧alias移除通过源码与本规范核对和构建确认，不为它们写CLI黑盒或golden。自动化测试只投入反编译行为、raw/smali完整性、复杂跨DEX/041身份、xrefsite/UTF16值、原entry提取CRC/竞态/alias/预算安全，跨公开库/module接口而非CLI子进程。不要求排序或跨运行同文本，不设计MCP；本次不删除现有测试。


## 7. 应用限额与生命周期

限额分层、显式组合：

| 层 | 本轮合同 |
|---|---|
| 文件/ZIP | 以现有安全读取默认 2GiB input、100k directory entries、64MiB directory、256MiB inflated entry 为工程基线；移到 CLI 若改变旧限制需说明并测量 |
| CLI 请求/解压 | 持有字节、解压 reservation、queued query rows、声明依赖与输出 scratch 合计有界；只保留同进程/同请求必要的字节复用，不新增常驻服务 |
| Engine | DESIGN 内 64MiB/DEX、256MiB/request 和 method work/graph/output 初值保持独立；超过引擎限额的输入仍可轻量查询，反编译明确 Resource |
| CLI 输出 | 现有查询可流式；单反编译报告完整渲染后提交。text/JSON 分别计最终字节，JSON escape、diagnostics、spans 要算；不截断后标完整 |

这些是不同预算，不将当前会话的 16MiB MCP envelope 限额硬套为新 CLI 产品合同。JSON 报告默认先采用 engine report 上限的 64MiB **最终序列化**上限作为待测工程初值，按剩余请求预算收紧；最终值在产品任务测量确定，不声称硬 RSS 上限。

首版可单worker；后续只为admission/背压保留有界队列，不为展示顺序等待。native inflate不可抢占，取消为协作式。输入/扫描/发布算法见ALGORITHMS §14。

## 8. 验证与计数边界

Java AST封口不是任意输入等价证明。值/edge/copy/绑定/保护域各自验证，公开行为另比独立expected；V0–V7与Driver见TASKS §4。method-in-known-context不计full-unit，JVM-only与ART-not-run分开。

Generated是有code成员的已验证owner生成规则，不是DeclarationOnly；单独不可表达的get-method给原Smali。完整class只有shell、全部成员/生成规则与初始化通过才为JavaUnit。EngineCoreRC与产品全部70的归属见TASKS。

## 9. 实施取舍

不整体搬高级ValueRecovery/GVN/LICM、全平台catalog、SMT/证明器、自动重试或全局最优结构化。先固定入口Phi与独立local，再受限dispatcher/自然结构，最后异常、锁与完整class。算法被独立反例推翻则修方案，不自行换路线或改expected。资料依据只在SOURCE_MAP维护。

## 10. 资源、策略与失效

### 10.1 最小 Kuna 骨架

`pipeline/schedule.rs`直接按§4调用具体typed transitions；Rust类型限制产物先后。
首版不建Pass trait、PassSpec registry、调度macro、TOML或restart tree；局部liveness/
type worklist就是其owning Module内的普通循环。PhaseId只作有限诊断标签，trace可选且有界。
T24真正存在第二个可选recovery实现时，才添加必要选项，不为未来pass提前搭基础设施。

默认 `RecoveryPolicy`：`NormalStructure::StructuredThenDispatch`；`RecoveryPreset::Core`（readability默认关闭）；`inline_pure=false`；泛型只 raw；未知/复杂 Java → Smali。必需 correct lowering 无用户开关。用户确认后续美化可独立开关扩展，记录最早消费阶段；具体候选/拒绝条件见 [ALGORITHMS.md](ALGORITHMS.md#15-可选美化)。不为未来可选pass先复制BDD/完整ValueRecovery到核心。强类型 assertions/全局 restart/plugin 延后直到有实际 producer/consumer。

### 10.2 明确失效范围

| 变化 | 保留 | 重算/失效 |
|---|---|---|
| literal formatting/indent/输出名字 | 全部 verified artifact | P9 text/spans（名字若改变 Java binding 则不能仅 P9） |
| normal structured vs dispatcher policy | P1–P6 | P7/P8/P9；只选 plan 不改 CFG |
| 外部 declaration/hierarchy catalog | raw input/P2 原语义 | P4–P9；P3 若异常边/throws 输入被改变也重建，初版用 request 整方法重算最安全 |
| CFG topology/exception chain | 仅 raw/catalog | P2 下游全部作废；不复用旧 ValueId/CopyPlan |
| generic/nested source policy | raw/P2–P3 可复用的值身份 | P4 后的 binding/types/storage/Java；初版同样整方法重算 |

本表是设计规则，不是初版 incremental cache 承诺。初版新请求整方法重跑，保留不可变 ProgramIndex；请求内`EnvironmentId`/profile标识可检测误用artifact；不同请求不要求同数字/跨运行canonicalfingerprint，不为无缓存的初版额外排序事实表。局部worklist只修改指定finite facts，实际变化才enqueue，先charge再执行。没有通用Repeat/Changed调度协议，也不为可观测性创建一套机械测试。

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
| `max_phi_inputs` / `max_ast_nodes` | 1,000,000 / 262,144 | incoming连接、AST候选创建前 |
| `max_method_allocation_bytes` | 128 MiB 累计 | normal/raw合计；arena/Env/候选/文本容量增长前 |
| `max_request_allocation_bytes` | 256 MiB 累计 | index、所有method结果与render派生分配合计，不含caller-owned输入 |
| `max_request_work_steps` | 50,000,000 | open/所有method/render共同消费 |
| `max_work_steps` | 5,000,000 / method | decode/graph/worklist/copy 每个循环收费，不只 pass 入口 |
| `max_copy_assignments` | 262,144 | normal+shadow+cycle temps 合计，先估算再预留 |
| `max_nesting` | 128 | metadata/AST/handler 递归改成显式栈或深度 guard |
| `max_output_bytes` | 8 MiB / method，64 MiB / report | 有界 String writer，含诊断和 span |
| `fallback_code_units` / `fallback_output_bytes` | 131,072 / 8 MiB | 仅一个只读raw出口，仍受request总界限 |
| `fallback_work_steps` / `fallback_allocation_bytes` | 262,144 / 16 MiB | 每method入口从父配额预留，normal不可消费 |

语法上合法但超限是 Resource；不使用饱和运算 silently truncate。所有 `count * size + offset` checked arithmetic，分配前验证 remaining input/预算，use `try_reserve` 映射资源错误。寄存器数遵循 DEX 字段上限但同时计入 method value/storage budget。所有容量增长以实际元素容量乘元素尺寸/保守容器开销checked计费；Env、Phi inputs、UTF16、spans、diagnostics不能漏计。配额累计不退款，未消费的预留与已消费额度分开；fallback不是新发一份request限额。诊断截断必须有显式 marker，不能截断 Java/smali 并标完整。

trace仅作可选有界定位，不设registry/调用次数测试，不引入独立console。
