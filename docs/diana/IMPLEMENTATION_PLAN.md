# Diana 实施路线：按能力切片贯通，不按 Phase 闭门完成

状态：功能实施计划；纯包结构已按用户新要求先搭建，见 [STRUCTURE.md](STRUCTURE.md)，不算 H01/T01 或 E2E 准备完成。本次**不执行以下功能 Rust/fixture 任务**。本页T01–T25是内部`diana-decomp`引擎路线，整个Diana产品的CLI/查询/输入整合见[PROJECT.md](PROJECT.md)、[PROJECT_TASKS.md](PROJECT_TASKS.md)（H01–H10）。MCP暂不设计，也不是CLI验收前置。目标架构见 [DESIGN.md](DESIGN.md)，文件与接口见 [MODULES.md](MODULES.md)，具体执行卡见 [TASKS.md](TASKS.md)。现有实验分支不是兼容或迁移目标。

开发流程与批次记录统一见 [DEVELOPMENT.md](DEVELOPMENT.md)。其中按批认证后进入对应实现的建议**尚未生效**；本页全量 B04 前置、T/H 依赖不因记录该建议而改变。确认后再统一同步，不能从候选批次表推导跳过依赖。

## 1. 回答“一个 phase 一个 phase 做最好吗？”

**架构按 phase 管理，开发按端到端能力切片推进。** 完成某 phase 的全部算法才进入下一个，往往会在几个月后才发现 Java 无法表达上游 product、异常边状态不对或测试 oracle 不成立。反过来，直接在 printer 打补丁也会失去 Kuna 的职责分离。

每个增量沿既定接口穿过多个 phase，只有当前切片必需的实现接线。未支持的 opcode/声明结构明确降级，不能留下空 pass 却宣称完成 P5。一个阶段的完成度由支持 profile 与证据决定，不由目录里的文件数决定。

例如：

```text
常量 return：P1 → P2 → 最小P3/P5 → P6 → 最小P7/P8 → P9
if/Phi：     扩展P2/P3/P6/P8，P4可对无call方法返回空但有效的BindingTable
调用：       扩展P4/P5/P8，不改无call方法的已验证结果
try/catch：  扩展P2异常图/P3异常环境/P6snapshot/P7ownership/P8try
monitor：    在try/catch能力已验证后加入lock/release证书
```

测试一直穿过 public method seam；phase 内部 mutation tests 补充定位。未支持方法通过完整 raw 校验后 smali fallback，不能因此要求一次补完所有 Java 模式。

## 2. 先完成测试准备，再启动生产实现

**用户最新约定：E2E案例先于开发。** 已设计70个D001–D070，见[E2E_CASES.md](E2E_CASES.md)/[e2e_cases.json](e2e_cases.json)；[TEST_PREPARATION.md](TEST_PREPARATION.md)的B01→B02→B03→B04先审阅冻结行为、生成自制源/inputs、锁公开工具/依赖、独立基准与harness证伪。用户最新授权先做纯 Cargo/包骨架/构建发布隔离；B04通过前仍不启动T01/H01引擎或产品功能改造。团队获取/相对路径/工具锁见[REPRODUCIBILITY.md](REPRODUCIBILITY.md)，不依赖作者机器。

测试准备通过后，引擎线先执行 **T01 → T02 → T03 → T04 → T05 → T06**。产品线先H01在应用根 `apps/diana/` 实现唯一产品 `diana` 与内部 `diana-decomp` interface，旧 rasc 独立归档为 ref-fork，不参加主 workspace/CI，再H02/H03与引擎线交错；H04/T04→H05/T06尽早接线CLI并通过公开反编译interface跑实际语义回环，不新增CLI黑盒，不等所有phase写完。这不是 six full phases，而是最小引擎骨架与直线行为闭环：

1. T01：只建 crate、输出状态、诊断、预算、真实 schedule 与 contract 注册；不一次建全部空目录。
2. T02：安全 header/catalog/code-item reader，能定位一个自制类与 exact proto 方法；错误分类可测。
3. T03：标准格式 decoder/raw smali 与 Engine request/report，Unsupported 不丢 bytes。
4. T04：UTF-16、payload、debug/encoded metadata 与可靠 literal，避免基础字符串先被改坏。
5. T05：把B01–B04已冻结的case/fixture/harness接到真实公开库，完善Op/Java schema；不再设计expected或等到这一步填工具版本。
6. T06：首个成功 Java 方法 `static int answer(){return 7;}`，以及参数 arithmetic/return；每个 pass product 真正接线，超限和unsupported具有正确状态。

**第一批终点不是“所有 phase 架构完成”**，而是可运行的 request → raw validation → analysis → Java → javac → 行为 driver 闭环，以及保留原指令的 smali拒绝路径。估计人日不可靠，按任务验收与风险停止点控制进度。

## 3. 能力增量与验收门

| 增量 | 任务 | 交付行为 | 硬验收 / 不进入下一增量的条件 |
|---|---|---|---|
| I0 安全输入与可见失败 | T01–T05 | exact class/method定位；完整已知格式smali；header/method错误明确；工具链闭环可用 | Missing fixtures/tools不能算pass；unknown opcode不当NOP；预算前置；raw/fallback覆盖证书成立 |
| I1 显式scalar locals | T06–T10 | constant/参数算术/branch/Phi/循环普通图，dispatcher保留可观测行为；primitive/null/literal准确 | CFG/SSA/storage证书、平行copy与default初值不可观察；FP/boolean/undefined读反例拒绝；不能靠忽略javac errors通过 |
| I2 调用与对象 | T11–T12、T15 | exact call kind/绑定，邻接new/init、受限constructor、fields/arrays，副作用顺序 | 自制complete依赖catalog；缺catalog明确降级；new/init时机、unusedcheck-cast、volatile/initthrow不丢 |
| I3 普通自然结构 | T13–T14 | reducible if/while/switch产生可读AST；复杂普通图dispatcher | 两adapter行为一致、coverage/transfer证书一致；不是要求text相同；nohandler/noMonitor fallback范围明确 |
| I4 异常正确性 | T16–T18 | ordered exception edges、old state snapshots、单intervaltry/catch及真实cleanup路径 | catch收到throw前值；handler优先级、不扩大保护域；unexpectedexception不吞；复杂结构smali |
| I5 同步与初始化 | T19–T20 | declared/显式synchronized、staticencoded值、<clinit>、final字段受限 | all exits精确release、null锁、final单赋值、init顺序证据成立；不转空try或删super |
| I6 源码声明完整性 | T21–T23 | rawgeneric/bridge精确契约、命名nested、enum/annotation受限，合法完整unit或Mixed | enum不是extendsEnum；runtimeannotation不静默丢；nestedfragment不冒充unit；missingdependencies不吞掉errors |
| I7 可选美化与加固 | T24（可选）、T25（必需） | 纯表达式/循环形式有限优化；全支持profile的fuzz/预算/无序结果完整性与gate文档 | 每个option有on/off证据，failure状态不回归；T24未启用不阻塞coreRC；T25所有必需能力必须有negativecase |

任务不是功能全景“一次完成”，每张卡只选可直接实现的第一档 profile；卡中的延期部分明确不计入完成。需要扩大时新增独立任务，不用修改golden吞回归。

## 4. 任务依赖（可用作机械检查输入）

以下是引擎依赖清单，与TASKS一致；B01–B04依赖由TEST_PREPARATION给出，H01–H10由PROJECT_TASKS给出。联合图必须无环，B04是T01/H01共同前置。后文“可先做”不改变依赖。

```text
deps T01: B04
deps T02: T01
deps T03: T02
deps T04: T03
deps T05: T04
deps T06: T05
deps T07: T06
deps T08: T07
deps T09: T08
deps T10: T09
deps T11: T10
deps T12: T11
deps T13: T10
deps T14: T13
deps T15: T12
deps T16: T15 T07
deps T17: T16 T08
deps T18: T17 T13
deps T19: T18 T12
deps T20: T19 T15
deps T21: T20 T11
deps T22: T21 T12
deps T23: T22 T20
deps T24: T23 T14
deps T25: T23 T14
```

可行拓扑顺序示例：B01→B02→B03→B04→T01…T12、T13、T14、T15…T23、T25、T24。T24独立可选；T25不依赖T24，因为correctnessgate不能依赖可选readability。I1中T06只实现scalar最小类型；T10扩充primitive/FP/null兼容，不在T06提前宣称全opcode支持。

P2普通call读proto、P3参数输入、P5最小scalar事实在T06起已存在；P4/P7对当前无call/naturalregion需求返回有效空binding/单block或DispatchPlan（T06只singleblock，DispatchPlan到T09才创建），不用未实现pass假冒成功。T05不是重新设计测试：B01–B04必须已经交付可复验source/inputs/expected/harness，T05只将其接入真实publicinterface使数据可观察。

## 5. 集成纪律与 Agent 执行策略

- 先完成B01–B04规范/fixture/toolchain/reference/harness准备再生产开发。每个focused实现引用固定D案例组逐步转绿，不根据actual改expected、删/skip required成员或把本应Java的案例改Smali成功；真正oracle错误须独立证据/单独审阅修正。help/参数/路由/列名/简单getter不新测，H01/H07仅审阅/构建；不冻结私有phase形状，旧测试整体保存在 ref-fork，不删除但也不运行来验收 Diana。
- 阶段privateconstructor不让下游绕validator创建Verified；fixture可制造invalidartifact检验拒绝，helper只在tests。
- 首轮一个writer/工作区。若用户之后允许并行：只有不修改相同schema/registry/support文件的叶任务可并行；独立worktree，先冻结共享interface再fanout。规模大不自动授权子Agent。
- 不要求查询、独立方法/类的输出次序或生成文本跨运行一致，也不为此排序/按输入ordinal等待。验收按原identity核对完整记录、实际spans与行为driver，不能用宽松normalization把错类型/异常/副作用当相同；纯性能task也不额外要求byte-identical。
- 多个参考algorithm只是候选。每次copy前查license/NOTICE与具体helper依赖，记录sourcefingerprint，不把整个模块迁入再disable。
- 任何任务无法通过自己的核心反例，则保持该capabilityUnsupported，补风险说明；不能以smali fallback的presence作为该Java能力完成。
- 若发现核心interface必须改变：停止受影响card，写architecturedecision与新contract，再重跑已有cards。不要只用打印器补洞。

## 6. 后续可选扩展与明确延期

用户确认美化可以可选、以后扩展：保留独立开关、前置事实和验证合同，但不提前复制整套实现或让核心依赖它。详细 slot、E01–E08候选与on/off验收见 [RECOVERY_EXTENSIONS.md](RECOVERY_EXTENSIONS.md)。这些是明确延期，不是隐含CoreRC承诺：

- 泛型原源码重建、lambda/anonymous源级美化、string-builder拼接、string/enum-switch糖化、loop-invariant motion、全图GVN、广泛sourcecoalescing。
- 无限制protectedgraph、不可归约跨handler/monitor dispatcher、任意constructorprologue合法化、native/reflectiondynamiccall确切target恢复。
- 全平台API符号库、自动从网络抓声明、完整APK资源CLI、agent持久HARDassertion和通用restartframework。
- 多线程性能优化、增量cache、TOML/codegenregistry。只有profile语义与身份/trace可验收后再量化需求；cache须验证invalidationscope，parallel按完成输出并验证无重无漏/有界资源，不做stablemerge。文档任务编号的排列仅用于阅读，不要求实现采用同一输出顺序。
- invoke-polymorphic/custom及新版DEX支持：T25先登记为knownUnsupported并保证rawfallback；真正Javasemantics在独立扩展卡中实现，不能只打开opcodegate。

## 7. 量测与项目完成定义

不要承诺实现必然比dexdec少多少行。每个增量记录：生产/测试LOC（精确module测量，不用第一cfg(test)截断）、模块interface大小、pass/worklist/copy计数、Java/DeclarationOnly/Smali/Unavailable/Error各自分母、VMdriver结果、stage预算、release耗时和峰值内存（性能任务才跑代表输入）。

EngineCoreRC（原CoreRC）= T01–T23与T25合同达成，supportedprofile与knownunsupported表一致、fixture与negativecase全部gate；T24readability可晚于EngineCoreRC。CLIProductRC还要求H01–H10的查询、输入、041、身份、指定十一命令/列、提取、CLI结果/agent文档及发行配置证据；MCP明确延期，不在该分母。这不等于“所有DEX均Java成功”。本次文档完成只代表计划就绪，不代表任一RC已完成。

最小验收命令、工具链合同与行为差分限制见 [VERIFICATION.md](VERIFICATION.md)；实际未来card命令见 [TASKS.md](TASKS.md)。
