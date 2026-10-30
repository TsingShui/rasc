# E2E 先于引擎开发：独立准备阶段与冻结规则

用户要求测试案例先于开发，并用于共同开发，不依赖作者机器。本页替代旧计划里“等T05开发时才选择fixture、填写expected和工具版本”的顺序。

**现在已有**：[e2e_cases.json](e2e_cases.json) 的70个具体案例设计及独立预期、[E2E_CASES.md](E2E_CASES.md)的oracle/比较义务、[REPRODUCIBILITY.md](REPRODUCIBILITY.md)的公开来源与工具合同。

**还没有**：可运行的Java/smali源fixture、DEX/ZIP输入、工具完整checksum锁、参考Driver成绩、runner代码、新Diana引擎。不能把案例设计称为全部E2E已绿色。本页B01–B04在引擎实现T01与产品功能改造H01之前完成。用户后续明确要求先建项目结构，已允许纯 Cargo/包骨架/构建发布隔离先行，见 [STRUCTURE.md](STRUCTURE.md)；这不算 T01/H01，不实现 Engine/CLI 命令，测试准备与固定期望规则不变。最新 ref-fork 隔离使主 workspace/CI 只含 Diana；旧 tests 保留但不构建/执行，也不能拿它们的通过数冒充准备或新引擎验收。

批次开发流程、记录模板和停止条件见 [DEVELOPMENT.md](DEVELOPMENT.md)。按批执行 B02–B04 的建议仍待明确确认；**当前本页全量前置继续有效**，准备工具的单案例试点不算 B04 完成，不授权提前写生产引擎。

## 1. 测试接口先确认，案例不绑内部phase

沿用户已约定范围：不新增CLI黑盒、help/参数/路由/列名、简单getter/转发测试。

- 语义主seam：`diana_decomp::Engine::open/decompile`公开库interface → Java/smali/report；期望按原始method identity和行为，不断言私有CFG、Phi结构、内部调用次数、变量名/排版。
- 复杂领域seam：公开原始输入/definition/reference/extract interfaces；测试跨DEX/041身份、raw引用site完整性、CRC/no-clobber/alias、预算/取消，不启动Diana CLI进程。
- 运行观察：JVM Driver的返回/bits/u16/heap、调用/异常/cleanup/lock/init事件。**分析结果序无要求，程序事件顺序必须精确。**

`e2e_cases.json`是一份行为合同，`inputRecipe`是自制源的具体规格草案而非已经可编译文件。B02将它落实source并证实输入结构；如编译器消去了关键指令，B03可改fixture生成recipe以构造**同一语义义务**，必须独立审阅并记录。不能等Diana失败再删反例或把case简化为别的问题。

## 2. 依赖：先test准备，再引擎/产品实现

```text
prep-deps B01: -
prep-deps B02: B01
prep-deps B03: B02
prep-deps B04: B03
```

更新后的联合图：B04是T01/H01的明确前置。准备器/fixture/reference-driver不依赖Diana实现；未来runner对Engine的thin adapter定义和模拟故障输入可以预先完成，不为此做假的生产引擎。T05只接入准备好的suite到真实publicinterface，不重设计案例/oracle。

只开发测试工具/自制源不是引擎功能开发，因此不发生“要引擎存在才能准备所有输入”的循环。B04的`verify-harness`验证comparator会拒绝错误结果、缺产物、截断/超时；它不要求Diana现阶段执行70个case全部通过。引擎缺失为not-implemented、E2E未通过，不能skip变绿。

## B01 — 案例与期望规范审阅、冻结v1

**依赖**：无。**Phase**：测试准备，非计算phase。**范围**：70个D001–D070的核心行为规格与E01–E08可选扩展边界。

**文件**：`docs/diana/e2e_cases.json`、E2E_CASES/VERIFICATION/CLI/REPRODUCIBILITY；未来`tests/e2e/cases.json`和manifest schema，全部相对checkout。

**输入→输出 / Interface**：行为recipe+固定inputs→独立expected+mutation-killer+oracle类型，state从`designed-not-built`成为`spec-approved`；记录review人/规范版本，不是假引擎结果。

**步骤**：
1. 对每case检查原始Java/DEX语义、inputs、返回/exception/effect序，workedexample与JVM reference driver分别支持结论；修规范错误必须在引擎实现前。
2. 确認seam/validationScope，method-in-known-context与full-unit分开；内部op解释器不充当异常/锁oracle。
3. 已知合法但Java不可表达的例子明确Smali/Mixed；损坏输入明确Malformed/Unavailable，不能把全部negative统一assert any-error。
4. 核对“必需Java能力”的case不会因缺实现切为expectedSmali；可选美化另行指定，不对core期望造成影响。
5. 锁suite成员/inputs/expected/schema与可分发来源，记录spec SHA-256；不能含真实私有APK或路径。

**不变量 / 限额**：案例依观察行为，不绑phase命名或打印形式；inputs/expected独立设计，不用被测输出生成golden。70case不等于所有合法DEX穷举，后续新增反例可追加但不可删旧case凑通过。

**测试**：规范审阅不是CLI测试；对具有反例的oracle推导确认counterexample确实有不同返回/异常/序列。不为JSONgetter写测试。

**验收**：70个case都有具体inputs/expected/oracle/mutation；范围/来源/path审阅通过，规范标记v1approved。**完成**：Engine开发者不再需要决定测试该返回什么。

**不扩展**：CLI黑盒、私有phase测试、输出文本golden、真实语料下载、实现引擎。

## B02 — 源fixture、工具内容锁与可移植准备器

**依赖**：B01。**Phase**：测试工具，非生产phase。

**文件**：`tests/e2e/sources/`、`expected/`、`toolchain.lock.json`、`manifest.json`、`cases.json`；`tools/e2e.py`（开发工具选项，使用Python3.11+标准库优先），与必要tests-onlyJava/smali/Driver。所有路径相对repository，生成物只在`.cache/e2e/`。

**输入→输出 / Interface**：REPRODUCIBILITY固定artifact坐标 → 完整hash/依赖锁；D001–D070recipe → 可执行自制源+literalexpected，提供`fetch-tools/prepare/verify-inputs`命令；不需要个人SDK、android.jar、参考引擎cache。

**步骤**：
1. 将70个recipe按suite分成独立源文件/driver，依赖全部自制源码签入。保留single-targetmethod的已知context，不插改变语义的stub。
2. 记录JDK各支持平台JDK资产/官方hash，R8/smali/dexlib2及所有transitiveJAR完整artifacthash；下载与运行主类按锁，不盲用`java -jar smali.jar`。
3. 确保Java21工具 `--release 8`、R8 min-api/DEXversion检查与smali035生成路径一致；需要新版case独立profile，不能猜version。
4. prepare只运行锁定的工具、selfmadesource，生成DEX/ZIP/invalidmutation/041recipe以及origin/identity证书。取原返回/轨迹expected来自规范和source reference，不来自Diana输出。
5. 准备器支持不同cwd、Windowsseparator/classpath/exe suffix、路径含空格；系统temp API，不写死home或OS临时路径。只有fetch可联网。
6. CRC坏/entry重复/无定义引用/孤立surrogate/循环Phi等input需独立raw检查；指定bytepatchfield/opcode锚点/hash不是未检查的裸offset常量。

**不变量 / 限额**：网络/子进程只开发工具，生产crate无依赖；未锁artifact/placeholderchecksum阻止准备；toolcache缺失错误，不能用作者预装SDK代替。

**测试**：重建后source-driver与70spec expected一致；固定错误输入、hash/size/超时/跨平台path错误明确prep失败，comparator证伪交给B04。不扩大成简单CLI测试。

**验收**：干净cache可以显式fetch并生成全部核心fixture，所有输入/expected/deps路径可分发，tool/fixturehash锁完整；未生成或缺oracle的case列blocked而非移除。**完成**：不需要引擎开发阶段临时设计fixture/source依赖。

**不扩展**：生产Rust/API实现、自动外部平台catalog、下载真实corpus、私有testpaths。

## B03 — 独立输入/基准验证与冻结artifact v1

**依赖**：B02。**Phase**：输入/基准认证。

**文件**：`tests/e2e/manifest.json`、独立dexlib2projection/helper、source reference Driver，`.cache/e2e/preparation-report.json`生成report（不带机器绝对路径进入golden）。

**输入→输出 / Interface**：fixtureartifact → checksum、rawstructuredprojection、JVMdriver参考轨迹与规范一致性报告；manifest state=`inputs-certified`。

**步骤**：
1. 独立decoder/dexlib2检查method/ref/word/branch/payload/handler/stringunits。重点case确认原生成DEX真的含被测义务：Phi循环/正确invokekind、unusedcast、try外cleanup、monitor/桥接/annotation；不是只有source文字看着像。
2. source-origin核心case在隔离JVM执行真实referenceDriver，核对规范独立expected。FP bits、UTF16、lock、init freshprocess规则严格。
3. 人工smali若无Java源JVMoracle：pure scalar可用测试专用独立解释器（须与source-origincase交叉校验）；exception/monitor不得用忽略effect解释器，或保留raw-integrity/declined明确状态。
4. D065的041builder需公开格式约束与独立projection双检，不从开发者个人设备拿样本。无法证合法构造则prep blocked，不能换普通DEX假装覆盖041。
5. 固定invalidmutations事实：格式越界/unknownwidth/undefinedwide/CRC分别证明，不通过assemblerexit0推断所有合法性。
6. freshcheckout/freshcache复建reference成果，记录可支持的平台和工具版本；平台没跑不能宣称已通过。

**不变量 / 限额**：referenceinput与expected冻结在引擎实现之前；任何修改需casev2reason/独立oracle/审阅，不用自动“refreshgolden”。生成字节hash不同时先解释compilerrecipe差异，不修改预期语义。

**测试**：`verify-inputs`覆盖70cases全部required资产；JVM/handcraftedraw/oracle缺失分开记，不把ARTnot-run藏成pass。异常/锁的ART增强不作为本轮已完成主张。

**验收**：所有requiredcase的source、input结构与独立expected可复验，hash/范围证书完整；没有unknownlocaldeps；report分母固定70。**完成**：开发过程中无需反复改案例输入来适配错误实现。

**不扩展**：使用Diana/dexdec/radguard生成真值、私有cache或自动删失败case、设计CLI测试。

## B04 — E2E comparator、runner adapter与故障证伪

**依赖**：B03。**Phase**：测试工具/对接合同，非引擎实现。

**文件**：`tests/e2e/runner/`、expected/cases/manifest，未来thin public-library adapter源码；`tools/e2e.py verify-harness`，引擎实际接入在T05。

**输入→输出 / Interface**：固定case/input/期望 → 定义run-resultschema/按原identity及行为比较；没有Diana时能验证preparation与comparator，但不能报告engineE2E通过。

**步骤**：
1. 每case输入公开Engine request/RunContext/catalog来源，结果project到method语言/原identity/source/诊断；adapter薄且不窥privatephase，不实现fake引擎来替代缺实现。
2. Java结果编译在同自制依赖/knowncontext，Driver以returnbits/u16/events/exception/heap比较；有缺class/error/超时/无输出立即fail，不能grep掉javacerrors。
3. rawsmaliadapter通过原classshell+原必要方法组装再独立projectioncompare，漏branch/payload/handler/UTF16/ref必须失败。
4. Inject result mutations仅用于测试验证器：wrongreturn、missingevent、swappedcatch、float-0changed、lossysurrogate、truncatedraw、missingartifact、timeout/0cases，verify-harness须逐项拒绝；正确referenceinput可接受。
5. 固定expected_java与expected_raw分类，不能按引擎“当前支持capabilities”动态过滤requiredmembers。实现里程碑显式选固定ID组，整core-suite始终报告全部required未通过分母。
6. 接入失败报告保留原case输入/版本/actual/expected/artifacts到checkout cache，用相对路径，不写机器路径进golden。

**不变量 / 限额**：程序事件序列严格，独立report顺序无要求；source/localname没有golden；comparator无法自动改expected。不要求CLI黑盒，toolinvocation不等于被测CLI。

**测试**：上述复合错误结果证明harness能反对错误，非平凡“assertcaseid==caseid”；资源和运行超时有界，referenceJava编译失败不能当引擎“不支持”吸收。

**验收**：verify-harness所有negative被拒绝，预期正确reference被接受；suite锁与路径可复制，未实现engine报告not-implemented。**完成**：B01–B04准备验收明确通过后才开始T01/H01，T05只接到真实库。

**不扩展**：生产stub、基础JSON/CLI参数测试、自动testgolden更新、为了版本支持移除case。

## 3. 测试冻结、修正与共同开发纪律

冻结的是**问题、input identity、行为期望、oracle类型、required suite成员**，不是私有phase接口或输出变量名。bugfix/refactor可以改thinadapter、tool路径实现、日志格式，不能改变一条case本应返回的值、异常类型、event顺序或完整性要求。

允许测试变更但必须有理由，不许“永远不能修测试”导致错误oracle固化：

| 变更 | 必须提供的独立证据 | 处理 |
|---|---|---|
| 新真实反例 | 自制输入+workedexpected | 新增case ID，不删除旧case |
| 规范/oracle确有错误 | 原源/JVM/DEX规范或独立decoder复验，非引擎输出 | 独立review、case revision、保留旧理由/新expected；不在实现PR偷refresh |
| 工具版本/fixture重建 | 新artifacthash/recipe差异、input语义projection与reference结果 | 独立工具锁变更；不得隐藏回归/删关键opcode |
| publicinterface实现变更 | 问题与expected不变的thinadapter更新 | 只改adapter；不批量改cases跟随phase私有结构 |
| 尚未实现Java功能 | 没有允许改expected的证据 | case仍fail/not-implemented，不能换为Smali成功或exclude |

core Java、negative raw、invalid input、安全/领域suite分别报告。实现期间每张任务卡引用固定case组逐步转绿；全core-suite红很正常，但不能把红计为skip通过。**准备完成≠Diana通过；当前案例已设计≠inputs-certified**。
