# Module 规格与内部 Interface

本文是内部 `diana-decomp` 的 [DESIGN.md](DESIGN.md) 实施细化；root产品/CLI整合见[PROJECT.md](PROJECT.md)。B01–B04的E2E准备先于引擎实现，测试资产/expected在root `tests/e2e/`共享，来源/工具见[REPRODUCIBILITY.md](REPRODUCIBILITY.md)，不依赖作者机器路径。MCP暂不设计。所有路径是**未来目标路径**，并非仓库已存在的 Module。所有类型草案均需在 T01/T05 对齐，未提供完整可编译程序。Module 的 interface 不仅是 Rust signature，也包含本页的不变量、失败模式和工作预算。

## 1. 目标目录与落点规则

整个 Diana 产品沿用 root `src/cli/`、`src/analysis/` 等职责；具体整合/品牌/输入/查询任务见 PROJECT_TASKS 的 H01–H10。以下只列内部库，不是把整项目都放进一个 decompiler crate。现有 MCP 代码保持延期，不在本页创建或定义新的协议模块。

```text
crates/diana-decomp/
  Cargo.toml                 # 将来 T01 创建；不依赖 dexdec/radguard/kuna
  src/
    lib.rs                   # 只 re-export Engine/Options/Report/诊断/键
    engine.rs                # request/class driver、分类结果，不含分析算法
    substrate/
      ids.rs                 # Dex/Method/Insn/Block/Edge/Value/Local ID
      origin.rs              # code-unit 来源、synthetic reason、UTF-8 spans
      types.rs               # descriptor/primitive/UTF-16 表示
      cfg.rs                 # blocks/ops/normal+exception edges，不含 passes
      java.rs                # 有类型 AST、私有 Verified 构造器
    pipeline/
      schedule.rs            # 实际 typed transitions，一处列顺序
      pass.rs                # 同一artifact的Pass/Change，仅实际Repeat时创建
      registry.rs            # 同定义的 PassSpec/catalog，contract tests
      control.rs             # work/output/arena budgets、取消
      diagnostics.rs         # 分类错误、原因码与 bounded detail
      trace.rs               # method/phase/pass event 和统计
    p0_knowledge/
      mod.rs                 # RequestFacts 冻结，EnvironmentId
      declarations.rs        # DeclarationCatalog、complete/unknown 证据
      policy.rs              # required/optional profile 与限额
    p1_input/
      mod.rs                 # ProgramIndex::open / class/method views
      cursor.rs              # checked_range、u16/u32、LEB
      header.rs              # 版本、endian、ID/map section 范围
      catalog.rs             # descriptors/field/method/class 声明索引
      code.rs                # code_item、tries/handler、raw ranges
      formats.rs             # 每 opcode 的宽度/操作数格式/版本表
      payload.rs             # switch/array payload 结构，不做值优化
      strings.rs             # MUTF-8 → Vec<u16>，不损失 surrogate
      values.rs              # encoded values/annotation 有界读取
      debug.rs               # debug program：有效性与 raw 原始记录
    p2_flow/
      mod.rs                 # decode_and_bind -> FlowBody
      decode.rs              # 格式表→Op，已知不支持→分类拒绝
      effects.rs             # opcode effect classification
      build.rs               # block split、终结、normal edge
      results.rs             # move-result/exception adjacency 绑定
      protection.rs          # throw sites、有序 exception edges
      verify.rs              # instruction/edge 原始覆盖
    p3_ssa/
      mod.rs                 # build_ssa -> SsaBody
      inputs.rs              # descriptor parameter words/this
      liveness.rs            # 稀疏 live-in，normal+exception 各自 transfer
      build.rs               # 稳定 Phi 预分配和环境传播
      verify.rs              # 定义/支配/edge inputs/word pair
    p4_calls/
      mod.rs                 # resolve_calls -> ResolvedBody
      members.rs             # owner/proto/access/kind 完整性
      allocation.rs          # Uninit/init/constructor prologue profile
      contracts.rs           # exact Java binding、override、throws
    p5_types/
      mod.rs                 # solve_types -> TypedBody
      constraints.rs         # opcode/use/field/proto 约束
      lattice.rs             # finite primitive/ref/null/uninit/conflict
      solve.rs               # 单调 worklist，类型 narrowing/widening说明
      conversions.rs         # no-extra-check use conversion 证据
    p6_variables/
      mod.rs                 # allocate_and_copy -> StorageBody
      locals.rs              # 独立 ValueId->LocalId、参数/this绑定
      copies.rs              # normal edge copy+parallel sequentialize
      exceptional.rs         # throw-site snapshots、handler ingress
      definitions.rs         # 所有读必须已定义的证书
    p7_regions/
      mod.rs                 # identify_regions -> RegionPlan
      normal.rs              # 私有 region arena、SCC/SESE/shape plan
      protection.rs          # try/handler lexical ownership certificate
      monitors.rs            # lock identity、release路径、scope certificate
      verify.rs              # coverage、transfer、scope完备
    p8_java/
      mod.rs                 # lower_method、assemble_class、验证
      expressions.rs         # Java 静态类型、literal/use conversions
      dispatcher.rs          # handler/monitor-free normal fallback
      structured.rs          # 已认证 if/while/switch/try 计划
      declarations.rs        # 方法/字段/class shell/constructor/throws
      initializers.rs        # encoded static + <clinit>/<init> final
      ownership.rs           # 命名 nested class 与原始身份
      enums.rs               # 受限 enum 结构，非 extends Enum
      verify.rs              # method/unit合法性、语义覆盖证书
      recovery.rs            # 后续可选纯表达式/loop形式，不做必需 lowering
    p9_emit/
      mod.rs                 # RenderedReport + renderer routing
      java.rs                # 纯 AST printer
      smali.rs               # 只从 ValidatedRawMethod 生成标准 smali
      literals.rs            # Java/smali 各自 escape 规则
      spans.rs               # UTF-8 offset、最终拼接重定位
  tests/
    contracts.rs             # register 各子模块的执行测试
    input/                   # 自制小 DEX 与畸形边界
    pipeline/                # artifact/pass invariant mutation tests
    behavior/                # Java->DEX->Diana->Java 的 driver assertions
    support/                 # 薄公开interface adapter，读取root tests/e2e资产
    # 不复制第二份expected/source/tools；共享资产在root tests/e2e/
  examples/
    inspect_class.rs         # 将来显式开发示例，不接生产 CLI
```

不要第一次提交就创建全部空文件。只创建当前任务真正编译和测试的 Module；接着能力扩大再新增具体文件。未启用代码不得先迁入 40k 行等待使用。每个 phase 的 `mod.rs` 只有 interface/私有子模块声明，不复制算法来装饰阶段。

## 2. Product schema 与消费关系

本页“固定/稳定”只指同一次分析内ValueId/PhiId/引用的生命周期，不是跨运行数字、命名、结果排列或文本保证。独立任务完成顺序自由；不增加输出排序/按序等待。copy语义顺序、handler链、原始指令和分析依赖仍是正确性义务。

### 2.1 数据共享、移动与阶段私有性

`ProgramIndex<'a>` 借用 root request scope 已持有的 `DexImage<'a>`，不把 snapshot/stored-entry 全量复制进 Arc；metadata typed views 校验后读取，StringId/TypeId/MethodKey 使用原 DEX index。Method product 按值移动而不是复制全图。中间产物私有 fields 由 owning phase 构造，下一阶段只借 read view 或通过 typed transition 取得 ownership；实际 Rust 的原始视图借用沿 MethodInput<'a> → FlowBody<'a> 等传播，下方示意省略生命周期参数。

```rust
struct RawMethodView { /* 安全 ranges + key；不够生成完整 Smali */ }
struct ValidatedRawMethod { /* P1 格式/metadata/边界检查后的只读 raw */ }
struct MethodInput { key: MethodKey, raw: ValidatedRawMethod, decl: MethodDecl }
struct FlowBody { raw: RawMethodRef, cfg: FlowGraph, origins: OriginTable }
struct SsaBody { flow: FlowBody, values: ValueArena, phis: PhiArena, edges: EdgeEnvironments }
struct ResolvedBody { ssa: SsaBody, bindings: BindingTable, allocations: AllocationTable }
struct TypedBody { resolved: ResolvedBody, value_types: TypeTable, uses: ConversionTable }
struct StorageBody { typed: TypedBody, locals: LocalTable, copies: CopyPlan, defs: DefinitionCertificate }
struct RegionPlan { regions: RegionArena, protection: ProtectionPlan, coverage: RegionCoverage }
struct PlannedBody { storage: StorageBody, plan: RegionPlan }
```

采用上述按值 ownership wrappers；immutable raw 是 scope 内只读借用，root 独占实际字节持有权。不要在首批实现中再引入 private MethodArena + typed view 的第二种产品体系，避免 `Context` 同时持有可变 cfg/旧 phis/新 types。需要改变此决策时先更新架构合同与已接线测试，而不是让 Agent 自行二选一。P7 transition 返回 PlannedBody，不能只返回 RegionPlan 丢失 StorageBody；P8 消费 PlannedBody。

`Arc` 如有需要只用于 immutable RequestFacts，原始字节持有权留在 root request scope；不使用全图 Arc<Mutex<_>>。不公开 Vec 内部 index 让 caller 依赖拓扑。Arena 插入先 charge，不重用移除 ID；存 tombstone 或 mapping table。超过 ID u32 domain 是 BudgetExceeded，不 wrap。

### 2.2 FlowGraph 核心

```text
FlowGraph:
  entry: BlockId
  blocks: Arena<Block> { ops: Vec<OpId>, terminator: Terminator }
  ops: Arena<Op> { raw_opcode, inputs: Vec<RegUse>, defs: Vec<RegDef>, effect, origin }
  normal_edges: Arena<Edge> { src, dst, kind, origin }
  exception_edges: Arena<ExceptionEdge> { site, dst, chain_id, rank, type }
Terminator = Jump(EdgeId)
           | Branch(condition-op, true_edge, false_edge)
           | Switch(selector, keys_to_edges, default_edge)
           | Return(optional use) | Throw(use)
           | EndInvalid
```

Throw 的 handler edge 独立，不重复制造 normal branch。switch 有多个 key 指向同 block 时保留 key→Edge mapping；跳入 payload/truncated middle instruction 是 Malformed。no-fallthrough return 后的 raw unreachable instructions 仍保存、结构校验；Java 只生成可达行为，不能靠删除未知 unreachable opcode 掩盖 raw validation失败。合法不可达 unsupported instruction 可以使整方法 Unsupported（第一版的保守策略）。

`Op` 不保存无限递归 Wrapped 计算树，inputs 是原寄存器或后来 ValueId；literal 存 bits/StringId，不先转字符串。复杂 expression nesting 只由 P8 有界生成。

### 2.3 SSA 产品

```text
ValueDef = Parameter(word_index, descriptor_type)
         | This | OpResult(OpId, lane_kind)
         | Phi(BlockId, register_start, Vec<(EdgeId, ValueId)>)
         | ExceptionAt(handler_entry)
EdgeEnvironments:
  normal: EdgeId -> RegEnvironment
  exceptional: ExceptionEdgeId -> RegEnvironment
RegEnvironment[word] = Undefined | Narrow(ValueId) | WideLow(ValueId) | WideHigh(ValueId)
```

参数 descriptor 才是 input truth：`ins_size` 必须和实例 this+proto words 一致。宽参数按低/高 words，不能按“一个 parameter 一个寄存器”。所有 read 不按名称查定义。

SSA 实施步骤：

1. 扫每 Op 的 read/write word sets 和 may-throw-before-write 规则；形成 block gen/kill 与 edge transfer。
2. 求 live-in，仅让可能实际读取的 register进入 Phi 需求；handler 的 exception对象定义与 incoming普通寄存器分离。
3. 为每需 merge 的 `(block, register-start)` 建唯一 PhiId/ValueId，类型未知但身份固定；在遇到回边前已能引用它。
4. 遍历 worklist，block 输入为 predecessor env join，更新稳定 Phi.inputs；op defs 也是预分配固定 ID，不每轮创建。
5. 循环 entry 的 external predecessor必须存在；如果一个输入是 Undefined且有 reachable实际 use，输出 Malformed/ProfileInvalid，不把 Undefined消成 null。phi input 临时未就绪与最终 Undefined不同状态。
6. 最后绑定每 OpUse 到 ValueId；对普通 use做dominance，对Phi用edge availability；移除无use Phi可选但需身份/证书更新。
7. P3 的环境传播是局部 fixed point：ValueId/PhiId 先固定，环境表只收集有限 predecessor facts；每轮只补已有身份的edge/inputs，不创建新身份。超额 Resource，不fallback增加 rounds；若环境没有变化则停止，不能只依赖“轮数应当很少”。
8. P5 的 type worklist 是另一独立 fixed point；P3/P5 都用 RunControl 的有限工作队列执行，不把整个 SSA→calls→types→variables pipeline 互相重跑。

**支配计算有两个视图**：ordinary regions用 normal edges；SSA validator需要相应定义可达性与 exception-edge transfer，不能直接拿 normal dominator判handler所有use。对handler跨普通区域使用变量按flow/edge availability检验。

### 2.4 CopyPlan / Definitions

```text
CopyPlan:
  normal: EdgeId -> ParallelCopySet
  before_throw: InsnId -> Vec<ShadowSnapshot>
  ingress: HandlerEntry -> Vec<LocalCopy>
  temps: Vec<LocalDecl>
ShadowSnapshot: { shadow: LocalId, source: LocalId|Literal, handler_chain, phi_result }
ParallelCopySet: Vec<{ dst: LocalId, src: LocalId|Literal, type_proof }>
DefinitionCertificate:
  use_sites: origin -> defined sources
  edge_sources: edge -> all_defined
  snapshots: site -> no_throw_no_allocation_exact_type
  default_initializers_unobservable: set<LocalId>
```

证书由独立 validator重新计算，不接受算法自报 bool。`type_proof` 是引用P5fact的ID，validator核查source/destination转换不新增checked cast/boxing/call。默认 local initialization不覆盖Undefined值；only synthetic pc/temp可以由compiler赋具体初值。P8受保护结构必须为`before_throw`找到**相同异常保护域内的操作前位置**；不能放到外层try外并把throw路径live-in证书当满足。

## 3. 外部声明与 EnvironmentId

`DeclarationCatalog` 是不可变具体数据表，不是任意运行时回调：TypeDecl、MethodDecl、FieldDecl、HierarchyEdge。每个owner/candidate-group有`Completeness::Complete|Partial|Unknown`与来源。调用方可提供 empty，或同请求 images+自制fixture依赖；初版不读platform文件、不spawn、不查网络。

Interface：

```text
lookup_type(TypeRef) -> Fact<TypeDecl>
lookup_field(FieldRef) -> Fact<FieldDecl>
lookup_method(MethodRef) -> Fact<MethodDecl>
overloads(owner, name, invoke_kind) -> CandidateSet { candidates, completeness }
subtype(a,b) -> ProvenYes | ProvenNo | Unknown
```

输入声明定位使用 `ClassKey{dex,class_def_idx,descriptor}`、`MethodSelector{name,proto}`，解析后得到 `MethodKey{dex,method_idx}` / `FieldKey{dex,field_idx}`。catalog 查询使用跨 DEX/外部统一的 `TypeRef{descriptor}`、`MethodRef{owner,name,proto}`、`FieldRef{owner,name,type}`，不是本地 index。外部没有输入method_idx也不能编造index。报告同时保存 original MethodRef。`EnvironmentId` 初版是请求内catalog/options快照身份，用于防止产物跨scope误用；不要求跨运行canonicalfingerprint，不排序image/class/candidate来取得同一标识。候选lookup必须按原始member identity核验，不按最先到达挑重载。后续若真的引入跨请求缓存，其内容版本/失效正确性是独立任务，不因此恢复全局稳定输出要求。

P4最小可接受call证据：原始owner/name/proto/kind、参数word count、候选声明/visibility/throws完整性；它生成receiver/arguments的descriptor约束与overload/dispatch待验证义务，不提前假设P5的最终source type。P5产生类型和use conversions；P8核对receiver可赋值/nullable行为、目标选择唯一或动态dispatch contract、Java name/visibility与checkedthrows。catalog不完整且不能证明绑定则Unsupported::BindingEvidenceMissing。后续完善平台catalog是独立能力，不是P2未解码所有invoke。

## 4. Java AST 与验证器产品

`JavaMethodDecl`包含owner/source context、参数、static/instance、return、throws、body；`JavaClassDecl`包含合法simple name、package、classkind、fields、metadata policy和nested declarations。

`Expr`每个节点有static JavaType/Origin；`UseConversion`只允许：纯literal重新拼写、已证明widen、原check-cast、可证明无新runtime check的source cast、准确primitive narrowing/bit操作。不会把全部Ref→Object后任意cast回来作为通用合法化。

P8输入verification obligations：

- P6use可用、localtype一致、parallelcopy已经顺序化。
- normal/exception transfers都被计划消费，没有未知edge、normal跳进catch、跨monitor未释放。
- 每块ops顺序保留；转换syntax消耗的monitor/branch/resultinstruction都有explicit lowering理由。
- 每effect obligation恰对应一种sourceexecution语义，而非“文本正好出现一次”。loop重复运行、方法synchronized语法消耗多个monitor相关site不会按span数判断执行次数。
- constructor单合法super/this call、Uninit不能escape；final单赋值、checkedthrows与override、annotationclassmetadata policy一致。
- Java statement-expression legality、return必达规则、break/continue target存在、switchlabel重复/fallthrough、局部decl作用域和DA。

`MethodEvidence`记录支持profile、EnvironmentId、cfg/edge/hash签章及结构/定义证书摘要。fingerprint只用于cache/trace身份，不证明语义。Verified包装只表示通过当前validator，并有测试覆盖其义务。

`VerifiedJavaUnit`与单method不同：上下文缺失、静态nested片段、runtimeannotation丢失、任何成员fallback都会阻止整unit成功；Mixed允许display不允许假装recompile。T22/T23完善classkind，前期shell不支持时仍能有独立method结果。

## 5. registry / trace / repeat 的具体实现

T01仅写实际schema，不生成空actiontree。`PhaseId`枚举与`PassId`常量；`schedule()`显式调用具体transition，配套`specs()`使用同一声明macro/constructor产生元数据（优先small macro，不TOML）。tests核对实际trace顺序等于spec：普通失败停止后suffix不应执行；fallback为独立路径不重复收取normalwork。

`RunControl`的interface：

```text
charge(WorkKind, amount) -> Result<(), BudgetExceeded>
reserve(ArenaKind, count) -> Result<Reservation, BudgetExceeded>
check_cancel() -> Result<(), Cancelled>
emit(TraceEvent)                 # bounded sink，禁用时只保留累计计数
begin_pass(PassSpec) / end_pass(delta, counts)
```

`Reservation`不代表实际Rustallocator成功，allocation仍try_reserve；reserve并charge后不rollback预算以避免循环反复“申请/取消”绕限。trace字符串不拼整个DEX descriptor树。`Elapsed`及跨独立任务的trace/diagnostic/source展示顺序都不保证跨运行一致；单方法pass仍满足实际数据依赖和程序语义。测试按原identity/义务核对，不写全报告byte-identical golden；wall-clock不纳入golden。

P5：`run_worklist`输入finiteconstraint图；每fact仅单调收窄候选域或按指定有限join增长，不能在同变量上来回“猜Ref/猜Int”。需要两个方向的solver就拆事实种类而不是让enum不受控互换。

T24可选rewrite：有明确localexpressionproducer/consumer才构造Repeat group。requiredlegalization不能被用户禁用；Core/Readable都必须同profile语义验收。默认`RecoveryPreset::Core`关闭readability，不造327个潜在option。后续美化的候选副本、Applied/Declined/BUG、独立预算和前置facts合同见 [RECOVERY_EXTENSIONS.md](RECOVERY_EXTENSIONS.md)。

初版不提供全局persistedHARDtypeassertion；operator重命名hint不能覆盖DEXprimitive事实。添加知识干预之前必须具备conflictdiagnostic、最早consumer、重跑scope、有限restart次数及环境fingerprint测试（T25后可做，但不是本轮必需实现）。

## 6. Reader 与 fallback 的资源决策

- 两级raw校验：T02产出`RawMethodView`（只证明range安全），T03/T04通过P1 `validate_raw`产出`ValidatedRawMethod`（opcode格式/register/index/branch/payload/metadata结构完整）。只有后者可输入standard smali；fullARTverifier规则在本设计支持profile逐步加入，后阶段确定的非法输入依然返回Malformed而非Smali成功。
- header/file_size/endian/map、每IDtable range采用checked_range(offset,count,stride)。`offset=0`只在spec允许absence时合法，不能默认空结构。
- ID/classdata delta用checked_add，table index在解引用前验证；ULEB32最多5byte且高位合法，SLEB32signextension校验。declared count还须`count <= remaining/min_record_size`及总metadata预算；即使count没超file.len也可能allocate大量对象，不够。
- typedcatch的sleb.abs不在i32::MIN上overflow；没有先验证encodedbytes再`with_capacity(typed_count)`的路径 [RG-06]。
- code_size bytes=count*2；try padding/alignment、handler offset以handlerlist起点计、switchoffset以original switch PC计；payload非可执行目标。
- string decode返回`DexString(Vec<u16>)`；diagnostics可以lossy展示但literal绝不能。JavaUnicodeescape有词法前处理效应，NUL、newline、反斜线-u必须由literal测试锁定（T04/T10）；不要简单把每个u16无条件发`\uXXXX`。
- formats覆盖version035的已知opcode和payload，knownversion038/039 extension在T25独立验收。未知version是UnsupportedContainer；未知opcode无可靠width则method Unavailable并hex excerpt，不发不标准smali。
- smali输出按标准mnemonic/operand、标签、registerword、catch directive；已识别但未实现Javalowering可生成smali。raw bytes保留不等于smali无需实现，不等于任何损坏代码能正确重建。
- 未解debug或annotation可在明确字段标raw metadata excerpt，不把它当完整standard smali。T04完成前只有**原输入确实没有这些metadata且所有instruction已完整校验**的case可返回Smali；存在尚未支持的metadata时返回Unavailable+RawExcerpt，不能发半个Smali冒充完整产物。T04之后Smali状态必须含其声明范围内的全部metadata，V6全case验收。

## 7. Module 完成检查清单

每个任务接入前：

1. 明确生产者、消费者、privateconstructors；publicinterface是否比implementation还复杂？能否浓缩到一个typedproduct？
2. 至少一个输入→报告的自制例子，不仅内部unittest；至少一个会拒绝/降级的边界反例。
3. raw→op/edge→local→AST→span来源连续；不能用字符串regex补后端错误。
4. 每个循环/递归/分配是否有charge/checkedsize？cancel/maxoutput是否按合同分类，而非依靠相同输出顺序判断？
5. 失败后fallback是否重用未改变raw而非半个AST？同方法其他重试会否绕预算？
6. profile/catalogcomplete状态明确；default不宣称支持尚未接线算法。
7. 任何从参考复制的函数/helper是否有来源登记和对应测试；没有大量undeclaredModule残留。

下一步：[端到端实施路线](IMPLEMENTATION_PLAN.md)、[任务卡](TASKS.md)、[验证矩阵](VERIFICATION.md)。
