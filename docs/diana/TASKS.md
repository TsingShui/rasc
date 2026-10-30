# Diana 任务与验收

**状态：仅包骨架，39个B/T/H任务尚未验收；70案例仍为designed-not-built。**
本页统一流程、准备、任务卡与验证；接口/数据只看[DESIGN.md](DESIGN.md)，算法只看
[ALGORITHMS.md](ALGORITHMS.md)，来源只看[SOURCE_MAP.md](SOURCE_MAP.md)。
固定[e2e_cases.json](e2e_cases.json)不改，不另维护HTML/看板或案例expected副本。

## 1. 工作规则

完整B01→B02→B03→B04先于生产功能。小批开发不是准备豁免；B04工具自测仍需单独
授权，不能改名/手工试跑后冒称同等保证。T/H依赖表示具体producer产物就绪，不是
要求未来集成测试先完成；partial不得标整卡done，空文件/空pass不算产物。

每批一个公开可观察目标：确认固定case与输入认证→真实失败→owning Module最小改动
→正例/反例/资源与已验收回归→记录并结束。实际开工才建`docs/diana/batches/<id>.md`，
只记目标/不做、固定D ID与版本、producer/算法节号、修改函数、预算/取消点、真实
命令/执行数/平台/产物相对路径及剩余义务，不复制expected或新建进度数据库。
无用户授权不启子Agent，不一批混多种能力，不预铺全部phase/trait/registry/AST。

发现设计反例、必须改public interface/固定expected/有效依赖、无法认证oracle/工具，
或必须写未授权辅助测试时停止受影响部分。blocked/负结果不是通过；真正oracle错误
需独立证据、单独review/revision，不能从actual刷新。核心测试跨公开库/module，
不新增CLI黑盒、机械getter、私有IR/排版/调用次数golden。可选美化默认off。

## 2. 顺序与完成定义

首条链D009（配D055/D059拒绝）→D001固定Phi/edge copy→D002/D003 dispatcher循环；
调用/类型就绪后D029/D030 shadow/try，再扩其他固定case。各卡named输入不能跳过。

| 增量 | 任务 | 交付行为 | 硬验收 / 不进入下一增量的条件 |
|---|---|---|---|
| I0 安全输入 | T01–T05 | 原scope/精确proto；035及T04原生041；格式/metadata/raw通路；同一control接线 | 范围先核再分配，未知width不猜；完整raw状态不遗漏metadata；工具/fixture未准备不算通过 |
| I1 scalar与循环 | T06–T10 | 参数/算术/return、D001合流、D002/3循环copy与dispatcher、primitive/null/FP | 固定SSA、parallel copy、DA初值不可观察；cmp NaN bias和word宽度正确 |
| I2 调用与对象 | T11–T12、T15 | descriptor绑定、receiver/args顺序、邻接new/init、field/array/cast效果 | 完整catalog/checked throws、no-extra-check、allocation/init与volatile/class-init不移动 |
| I3 普通自然结构 | T13–T14 | 有界Sequence/Diamond/Loop/Switch；不匹配仍用合法dispatcher | 同case行为一致，不比文本；候选前收费，不复制effect或重排copy |
| I4 异常 | T16–T18 | throw-before SSA、shadow、ordered catches与一层nested | D029–D035语义；逐site catch-stack不扩大try，handler自身只受outer保护 |
| I5 锁与初始化 | T19–T20 | alias根/lock-stack、synchronized、static/constructor/final | 所有exit释放精确；static final不引入新的常量内联；final正常路径恰一次 |
| I6 完整源码声明 | T21–T23 | bridge、named inner、enum/annotation；Explicit/Generated与Mixed分类 | Generated规则逐项匹配；full-unit直接编译，不splice修补，不把generated算无code |
| I7 核心汇总 | T25（必需）、T24（可选） | EngineCoreRC的固定核心案例与边界证据；可选pure recovery默认off | T25不等T24；资源/取消从I0贯穿，不到最后才补；整体产品由H08汇总全部70 |

第一条Java链只需当前真的会消费的Module，不先搭全部phase目录或AST大全。T06的
answer7只是说明，实际固定案例为D009；无call/P7单块返回的是有效空事实，不是假pass。

- EngineCoreRC：T01–T23/T25的D001–D064及D065 engine义务（65个case身份）全过。
- ProductRC：复用核心证据，补D065应用映射与D066–D070，汇总完整70 required。
- D065仍是同一case/expected的两个seam证据；缺应用侧不能称整个D065/产品通过。
- required Java不因降级变绿；未实现/缺工具/未执行平台仍未通过。T24不阻塞两个RC。
- 根Cargo gate只跑Diana，零测试/骨架构建不算语义验收；性能只测release授权输入。

## 3. 准备任务 B01–B04


根 [AGENTS.md](../../AGENTS.md) 的最新规则禁止为任何非核心代码写测试，除非用户
明确授权。本页 B02 中针对准备器本身的hash/path/timeout故障测试，以及 B04 对
comparator/runner 的故障注入自测，不能凭既有计划文字取得授权；不得换名为“核心
E2E准备”来绕过。自制源/输入的独立语义认证仍是核心案例的准备工作，两者要分清。

**B04原通过条件和全量前置仍保留，但其工具自测实现当前受授权阻塞。** 需用户
明确授权限定的B04工作，或另行批准准备门槛调整；不能擅自省略后宣称B04通过，
也不能以一次人工试跑等同原故障证伪保证；以本节授权边界为准。

## B01 — 案例与期望规范审阅、冻结v1

**依赖**：无。**Phase**：测试准备，非计算phase。**范围**：70个D001–D070的核心行为规格与E01–E08可选扩展边界。

**文件**：`docs/diana/e2e_cases.json`、e2e_cases.json/TASKS §4/CLI/SOURCE_MAP §10；未来`tests/e2e/cases.json`和manifest schema，全部相对checkout。

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

**输入→输出 / Interface**：SOURCE_MAP §10固定artifact坐标 → 完整hash/依赖锁；D001–D070recipe → 可执行自制源+literalexpected，提供`fetch-tools/prepare/verify-inputs`命令；不需要个人SDK、android.jar、参考引擎cache。

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

### 冻结与修订规则

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

## 4. 验证规则

所有公共调用共用一个RunContext：open→decompile→render，Options不重复存Limits；
Report只持owned结果facts。source/expected独立于Diana，F/Q只是历史能力标签，实际
执行集合永远是JSON的70个D案例。fixture、tool/hash、runner目前都尚未建立。

### 4.1 验证层

| 层 / ID | 核查什么 | 证据 | 不能证明 |
|---|---|---|---|
| V0 InputSafety | offset/count/LEB/alignment/ID/word/payload/UTF-16/handler/depth | 小型自制 binary、定向畸形 mutation、预算/取消、无 panic/无无界分配测试 | 任意输入安全、所有 DEX verifier 规则已完备 |
| V1 FlowSSA | raw instruction 消费、normal/exception edge、参数 words、定义/支配/Phi/旧状态 | public seam profile case + 独立 artifact verifier mutation；edge/env 预期记录 | 仅凭图无环或 SSA 名称不能证明所有语义 |
| V2 Storage | parallel copy、critical edge、loop latch、throw 前 snapshot、默认值不可观察 | CopyPlan 重执行测试、定义可用性、exact type/no-extra-check、negative cycle/order cases | one-local-per-value 并不自动给出合法 Phi 消除 |
| V3 Structure | op/edge coverage、transfer target、try/handler order、monitor release | Structure/Protection/Monitor certificates 独立验证；对应行为 fixture | label/goto/state 更少不证明更正确；visited 集不够 |
| V4 JavaLegality | 方法/class context、成员绑定、primitive/ref/null、checked throws、constructor/final/nested/enum/annotation | P8 validator + **完整 classpath 下真实 javac exit=0** | Java 可编译不证明等价；忽略缺依赖的错误不是通过 |
| V5 Behavior | 返回、heap/call 日志、异常及 cleanup、循环、锁、初始化、字符串/FP bits | 自制基准源码与恢复 Java 的隔离运行差分；必要时授权 ART 环境 | JVM≠ART 的实现相关行为、没有覆盖的路径、所有输入 |
| V6 RawFallback | 标准 smali 完整性与原始指令/metadata 语义 | 自制合法 fixture → smali → 受控 assembler → DEX 语义字段比较 | raw excerpt 不是 smali，offset byte-identical 不是必要条件 |
| V7 System | 状态分母、预算/trace/catalog一致、实际source spans、无序输出完整性、许可、性能 | 无empty-run报告、按原identity核对记录/证据、多种到达次序、文档/schema检查 | 不要求跨运行文本/记录排列一致；release成功或多线程不证明质量 |

每个能力至少一正例、一个能破坏其核心不变量的反例、一个 limit/cancel 例；反例必须验证正确分类，而不只是“返回任何 error”。Default initializer 将未定义读取变为 0/null 的情况必须被 V1/V2 拒绝。

### 4.2 状态与分母

### 2.1 报告必须逐方法计数

EngineCoreRC实际验D001–D064及D065的engine义务；ProductRC复用这些证据，补D065
应用映射与D066–D070，汇总完整70 required。不得把应用未执行记engine pass，或把65
当作产品总分母。D065身份/expected不拆改，仅分别保存两个seam的证据。

统计以下互斥状态，并同时输出 denominator：

- `Java` executable methods（有 code，经过 P8 验证）及其 behavior-tested 子集。
- `DeclarationOnly`（abstract/native，无 body）——不能抬高 executable Java 恢复率。
- 已验证的`Generated` bridge/enum/inner成员归Java executable，并记录owner生成规则；
  不重复打印冲突声明、不计DeclarationOnly。full-unit必须由该owner语法生成预期行为；
  单独不能表达的get-method仍给原Smali，不为它造wrapper。
- `Smali`（已校验 raw + 全部指令/规定 metadata）；与 `Unavailable` 分开。
- `Unavailable`（损坏/未知宽度/无法可靠 fallback/资源失败），诊断原因为 Malformed、Unsupported、BudgetExceeded、InternalInvariant。
- `JavaUnit` 与 `Mixed` class 数；单 inner 片段、缺 shell metadata、任意 fallback 不算完整 JavaUnit。
- RequestError 与 Cancelled 单列；BUG 必須为红色 gate 信号，即使能输出 raw。
- missing catalog、known unsupported instruction、exception profile、monitor profile、constructor profile 各原因数。

例如：`Java=12/20 executable, DeclarationOnly=3, Smali=5, Unavailable=3; JavaUnit=1/4; BehaviorVerified=10/12 in env X`，而不是“17/20 能输出就是 85% 成功”。无法真实编译的 Java 片段即便 P8 成功，也不能计为 V4 已通过。

### 2.2 冻结基线纪律

每次baseline包含：引擎commit、options/profile/schema、输入checksum、该请求catalog标识、toolchainversions、运行命令、按原identity关联的方法状态/trace、已测试输入集。不冻结查询/跨方法结果序列，不要求跨运行局部名或生成源码相同；text样本仅作诊断，source spans对本次真实输出逐项校验。复杂查询内容通过公开module核对原identity多重集合，不提供分页或数量截取参数；旧flag移除只做源码/文档核对，不新增CLI测试。安全预算不足通过领域interface验证明确失败，不前N条成功。行为变化仍须独立审阅。

参考 dexdec、radguard、DAD 输出只作 triage：相同可能同错，不同可能都是等价形态。不能冻结宽松 normalizer 后宣称真值；comparison 规则变更与引擎变更必须分别记录。本设计不复制 RG pickle/DEX fixtures，不运行未授权缓存，不让 fixture 名揭示真实 corpus 身份。

### 4.3 行为Driver

### 4.1 Driver 输出结构

本项目不保证分析结果稳定顺序，但**被分析程序的执行轨迹不是展示顺序**。driver中的event seq、调用参数求值、异常/catch、cleanup、锁和初始化顺序必须逐项保留，不能按事件kind排序或当多重集合比较。独立方法/查询报告才按原identity关联且忽略展示次序。

比较规范字段的JSON结构与语义event序列，不比较打印Java文本或JSON对象键排列。每 case 独立执行原始自制源码与恢复源码的相同public入口，依赖Driver/Logger与catalog版本一致。

```json
{
  "case": "D029",
  "result": {"kind": "int", "value": 7},
  "events": [
    {"seq": 0, "kind": "call", "label": "throwing-producer"},
    {"seq": 1, "kind": "catch", "type": "fixtures.ExpectedError", "old": 7}
  ],
  "exception": null,
  "heap": {"counter": 1},
  "completed": true
}
```

类型编码：int/long十进制或固定bits；float/double用rawbits、NaN策略显式（canonicalNaN是否足够取决case）；strings用u16codeunits数组；null与空String不同；对象仅自制identitytoken/可见字段，不用地址或默认toString。

日志只存在于自制依赖中，记录call/field accessor/cleanup/lock等观察，不给恢复程序加入改变原行为的stub。额外Driver日志本身可能有effect，fixture要避免因日志改变算法可表达profile；source与恢复都使用相同依赖，但这仍只是覆盖给定fixture的证据。

异常比较：类型、可见cause/自制消息（不含runtime地址）、发生时的eventprefix、catch选择与cleanup行为。不要简单比较top-level成功/失败；一个错误catch可能同样返回-1。stacktrace行号不作为成功要求，错误类型/handler不能忽略。

### 4.2 双向测试与原始 DEX 的证据局限

基础闭环：selfmade Java → javac/d8生成DEX → Diana → Java → javac → JVMdriver。原基准为生成DEX的自制源/JVM，依赖d8编译语义符合目标。这能检验恢复逻辑，但**不是直接在ART执行原DEX与恢复DEX的比对**。

加强层：在用户明确提供授权 Android 测试环境后，原始DEX与恢复Java经d8的DEX分别执行同Driver，比较事件。生产依然不依赖外部runtime。没有该环境时V5标`JVM-only, ART-not-run`，例如volatile/monitor/FP/constructorallocation的实现相关部分列风险，不宣称全部ART等价。

对于人工smali构造的不可归约CFG或Java无法直接产生的rawcase：

- normal op-graph/copy测试可以使用**tests-only reference interpreter**（只支持自制pure scalar ops），并对实际Java执行比较；interpreter没有生产路径，不成为DEX运行时。
- exceptional/monitor不使用“简化 interpreter 忽略异常/锁”当oracle。人工protectedgraph若无授权ART环境，仅做V0/V1/V2/V3/V6证据，并保持Javaprofile实验性/未完成相应V5合同。
- 不能用dexdec/radguard给出的文本作为自制case真值。基准是case设计中具体返回/轨迹与独立语义推导，参考引擎只是补充debug。

`behavior_roundtrip`要区分`source-origin JVM case`、`handwritten smali + scalar reference interpreter case`与`ART case`；后者没有适用执行oracle就not-run/failrequired，不补一个“更简单但不是相同CFG”的源码代替。scalar interpreter只有T05 tests-only受限pureops、没有heap/exception/monitor，也须正例对JVM交叉校验；不把它算完整DEX VM。

### 4.3 超时、终止与并发

每子进程启动有deadline，超时kill并记录非通过；累计输出上限避免日志爆炸。无界循环fixture仅能证明某有限前缀，不得timeout后当expectedcomplete；循环测试使用有界计数并分别0/1/多次退出。

monitor测试首要：同线程holdsLock、nullenter异常、所有退出后的lock释放、重入/嵌套sameobject。可加有deadline的双线程交接检查，但stress成功不是完整内存模型证明。锁release必须由CFG证书补充；“body还能运行”不是保持同步语义证据。

### 4.4 能力族索引（执行真值仍为D001–D070）

**执行规范是[e2e_cases.json](e2e_cases.json)中D001–D070，完整具体inputs/expected/oracle先由B01–B04审阅、准备、认证。** 下列F00–F25/U/M只是能力族和反例命名映射，不是让实现Agent自行补expected或按当前能力删case；映射见[e2e_cases.json](e2e_cases.json)。

所有F/U/M均自制。binary方法满足编译器或smaliassembler规则；专门invalid case用于拒绝，没有“畸形也能生成Java”的要求。Native/abstract不可执行的声明放独立count。

| Fixture | 核心输入/路径 | 期望成功/反例 | 归属任务 / 验证 |
|---|---|---|---|
| F00 ResourceControl | 公开请求的预算隔离/失败释放/取消停止 | 真实边界与超额、fallback不绕预算；基础enum/包装/私有调用序不测试 | T01/T05；V7 |
| F01 TinyDex | 自制smallheader/catalog/exactproto/abstract | exactMethodKey、rawranges；M01/M02越界/LEB/countfail | T02；V0 |
| F02 RawMethods | move/return/wide/range/branch格式 | standard完整rawsmali，U01Java不支持但raw保留，M03unknownwidth不能假smali | T03/T05；V0/V6 |
| F03 Utf16Payload | NUL/quote/backslash-u/isolatedsurrogate、switch/fill、debug/encoded | u16/bits/payloadtarget保留，M04截断/overflow/metadata拒绝 | T04/T05；V0/V6 |
| F04 HarnessSelfTest | 工具缺失、wrongchecksum、0selected、syntax/behaviorfailure | harness自身必须红，不计Java成果 | T05；V7 |
| F05 ScalarReturn | answer7、param41+1、intwrapMAX+1 | 目标方法正确返回/locals；早期可 method-in-known-context，class 如实 Mixed；M05undefined/badreturn/ins/cancel拒绝 | T06；V0/V1/V2/V4/V5 |
| F06 DiamondPhi | choose两branch、nested、同target不同edge | 同condition一次eval，M06missingPhi/edge/noDominance，M07wrongbranchcopy拒绝 | T07/T08；V1/V2/V4/V5 |
| F07 CopyCycleLoop | a/b交换、3cycle、0/1/Nlatch、criticaledge | 同时赋值、循环Phi；M07naiveseq/temp/默认值观察拒绝 | T08/T09；V1/V2/V5 |
| F08 DispatchGraph | 普通two-entrycycle、多exit/throwterminal | dispatcher返回/日志一致，M08lost/dup/default/eligibility失败 | T09；V3/V4/V5（有smalioracle限制） |
| F09 PrimitiveBits | boolean/int/ref0、wide、shift、FPNaN/-0/cmpbias/conversion | 无library-call核心bits/异常一致；无P4证据的bitcast子例本卡Unsupported，M09partialwide/unresolvedtype拒绝 | T10；V1/V4/V5 |
| F10 CallKinds | static/private/super/virtual/interface、overload/null、effectargs | samecallee/argorder/callresult；M10badresult/kind/proto/visibility | T11；V1/V4/V5 |
| F11 MissingCatalog | F10合法raw但empty/partial外部facts | Smali+BindingEvidenceMissing；completecatalog时Java，不能填public假事实 | T11；V6/V7 |
| F12 AdjacentConstruction | argsready、new/init/purealias、super/thischain | 构造轨迹相同；U02complexpre-super/maythrow/lock拒绝 | T12；V1/V4/V5/V6 |
| F13 NaturalControl | ordinaryif/while/nestedloop/break/continue | natural/dispatch两adapter行为一致，M11dupRegion/lostEdge | T13；V3/V4/V5 |
| F14 IntegerSwitch | packed/sparse、negative/extreme/shared/fall/default | selector一次、copyedgeexact；M12payload/key/bounds | T14；V0/V3/V5 |
| F15 HeapEffects | staticinit/field/array/null/bounds/volatile/unusedcast | effect/异常prefix完整；M13fieldkind/payload/missingcatalog | T15；V0/V1/V4/V5 |
| F16 ThrowState | old=7；result-producingcall抛；catch读old | catch7不是正常newresult；M14normalstate/order/moveexception坏 | T16–T18；V1/V2/V3/V5 |
| F17 HandlerPhi | 2sitesoldvalues、loopprotected、sharedtarget | snapshots在throw前，M15postcall/Undefined/cast/alias不允许 | T17/T18；V2/V3/V5 |
| F18 ProtectedControl | subtype顺序、catchall/rethrow、cleanupreturn/throw、cleanup自身抛、一层nested | samehandler/domain/trace，U03复杂reentry/ownership降级；M16expandedtry等失败 | T18；V3/V4/V5/V6 |
| F19 MonitorScope | holdsLock/return/throw/null、nested/reentrant/static/instance | allreleases、sameidentity；U04mismatch/missing/multientry降级 | T19；V3/V5/V6 |
| F20 Initialization | encodednull/false/bits、nonconstantstatic、thisdelegate、blankfinal、initexception | newprocess初始化顺序，U05unrepresentablefinal→Mixed | T20；V4/V5/V7 |
| F21 ErasedContracts | selfmadegeneric/rawinterface/bridge/covariant/nullOverload | raw警告可接受、memberbinding不改；U06return-only/unknown/bridgeeffect拒绝 | T21；V4/V5/V6 |
| F22 NamedNested | static/instanceoutercapture、privateaccess、name冲突/Unicode | unit合法、spans映射；U07cycle/missing/ambiguousownership | T22；V0/V4/V5/V7 |
| F23 EnumAnnotation | canonicalenumname/ordinal/valuesclone/valueOf、runtimeannotation | 无extendsEnum、reflectmetadata一致；U08custombody/annotation不可表达Mixed | T23；V4/V5/V6 |
| F24 PureRecovery | on/offsingle-usepurecopy；call/read/div/locknegative | samebehavior，option真实consume；effect不能移动 | T24可选；V3/V5/V7 |
| F25 HardeningMatrix | 所有M/U、smallbudget、cancel、原始语义/取消/预算/spans、known038/039rawextension | 基线支持/状态一致、无panic/0tests、BUGgate红 | T25；V0–V7 |

F07/F08手写smali部分必须在manifest指出referenceinterpreter/ART/JVM实际证明范围；F17sharedhandler若未实现ownershipprofile则作为U03式降级case，不以它不Java而否定第一档T18，亦不列为其positiveJava支持。

### 4.5 固定工具与内容锁

候选组合先于引擎实现验证，用于自制fixture生成/基准执行。生产二进制不依赖Java/Maven/Python。

| 工具 | 固定版本与公开来源 | 使用与验证 |
|---|---|---|
| Rust | 根manifest最低`1.93`；runner实现时固定patch版 | 普通native构建；锁文件记录`rustc -Vv`，不手工改现有工程toolchain |
| Python | harness准备器最低3.11（若采用Python） | 只作开发工具；不得进入生产runtime，脚本应跨平台 |
| JDK | Eclipse Temurin **21.0.8+9**，源码目标`--release 8` | [官方release](https://github.com/adoptium/temurin21-binaries/releases/tag/jdk-21.0.8%2B9)，选择对应OS/arch的JDK资产，SHA-256用资产官方记录核；不是只下载JRE |
| R8/D8 | `com.android.tools:r8:8.3.37` | [Google Maven POM](https://dl.google.com/dl/android/maven2/com/android/tools/r8/8.3.37/r8-8.3.37.pom)，主类`com.android.tools.r8.D8`；从固定JAR构造参数数组，不依赖AndroidSDK包装脚本 |
| smali / dexlib2 | `org.smali:smali:2.5.2`、`org.smali:dexlib2:2.5.2` | [Maven Central POM](https://repo.maven.apache.org/maven2/org/smali/smali/2.5.2/smali-2.5.2.pom)，主类`org.jf.smali.Main`；POM有transitive依赖，**不是假设任意smali.jar可`java -jar`执行** |
| 测试依赖classpath | 自制Java源码或JDK既定声明 | 全部source签入，classpath从recipe得到；核心suite不依赖Android platform jar |
| ART增强验证 | 暂不规定设备/镜像 | 与JVM-only报告分开；没有已授权可复现环境不将ART算通过，不成为核心prep隐含本地SDK依赖 |

这些公开入口本轮作存在性核查；实际组合能否完整生成全部fixture须B02/B03验证，不能仅POM有HTTP200就标工具可用。B02产出`tests/e2e/toolchain.lock.json`：每个必需artifact的完整URL、version、SHA-256、size、依赖及mainclass；JDK每目标平台需asset/hash。**任何未填checksum/占位值、混入未锁transitive版本均阻止prep通过**。校验锁由官方checksum/可信获取的实测digest审阅；不能伪造SHA。

不要将R8/smali的JVM依赖搬到Rust生产或build.rs。可用受控Maven步骤产生classpath，但必须锁住传递依赖与artifacthash后离线运行；锁不能只写两个顶层POM。核心suite的checked-in artifact manifest与source recipe可供其他开发者复验，无需外部参考仓库。

### 4.6 资产目录与生成纪律

固定行为规格只保存在`docs/diana/e2e_cases.json`，本节维护准备/资产合同。实际B02产出：

```text
tests/e2e/
  sources/<case-id>/        # 自制Java/smali/driver/依赖；无机器路径
  expected/<case-id>.json   # 独立返回/轨迹/状态，不由引擎输出生成
  inputs/                  # 小型自制DEX/ZIP/AXML/badbytes；或审阅过的生成recipe
  manifest.json            # 来源、recipe/toolchain lock、artifacthash、suite固定成员
  toolchain.lock.json      # 完整artifact/hash/传递依赖/主类
  cases.json               # 从审阅过设计清单生成，签入执行版本
  runner/                  # 公开interface的薄adapter；无私有phase依赖
```

执行workspace在`.cache/e2e/`，其他目录的source/expected不可自动write-back。若签入binary fixture，需局部白名单.gitignore而非取消全局DEX/APK忽略；来源/hash/小size审阅。可以只签recipe，但prep须验证每个开发者生成结果受同hash/语义字段锁约束，不假定作者cache存在。

inputs编译/拼接可改变符号池offset时，recipe的语义检查要逐field比较；artifacthash变更需要独立fixture rebuild记录，不能当“反编译失败，重新生成一个更简单DEX”。返回/事件期望永远源于规范 worked example或独立基准，不copy Diana/dexdec/radguard打印文本。

### 4.7 执行与失败判定

未来开发工具interface为`python3 tools/e2e.py fetch-tools|prepare|verify-inputs|verify-harness`；
脚本目前不存在。只有fetch可联网，其他只读锁定工具和签入源，不扫描home/SDK/cache。
子进程用参数数组/锁classpath，Windows用join_paths，不能假定裸smali.jar能java-jar。
每case隔离临时目录/原始与恢复classfiles；clinit每输入新JVM，进程/输出均有上限。
manifest记录原身份、source/license、recipe、真实SHA-256/tool锁、validationScope、
request/catalog、独立expected、checks与Driver inputs；占位hash阻止准备通过。

方法级只在原自制合法context精确替换目标完整声明/body，不改目标/依赖或造stub；
只计目标方法，不计JavaUnit。full-unit直接编译完整输出，不能splice修补。D8输入为
显式classfile列表/self-made JAR，不把目录当classfile；--lib显式记录，不能忽略warning。
raw通过原shell组装并独立projection比对opcode/word/ref/target/payload/handler/UTF16/
metadata；不要求offset/debug字节相同，不用恢复端decoder生成两端truth。
干净checkout/新cache显式fetch→prepare→verify-inputs→verify-harness；未跑平台如实
not-verified，没有依赖源/许可/锁的case仍blocked，不从required移除。

G0（仅活动workspace）：
```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo test --workspace --all-features --locked
```
真实测试存在后按固定D ID/卡前缀运行公开contracts；roundtrip/assembly显式opt-in。
缺工具/fixture/classpath、非零exit、超时、0-selected、无产物/截断都失败，不能skip=pass。
全suite成员固定，不按当前engine-supported过滤；单批可选固定ID，全70总账仍保留。
不运行ref-fork gate。workspace现在零测试不是通过，未来命令不是本轮已运行成绩。

## 5. 引擎任务 T01–T25

## T01 — 引擎合同与直接 typed 调度

**依赖**：B04。**Phase**：P0/Pipeline/P9。**源码参考**：KU-01/02/04/05，RG-04。

**目标文件**：现有骨架 `crates/diana-decomp/Cargo.toml`（其余路径按本页约定相对该crate）、`src/{lib,engine}.rs`、`src/pipeline/{schedule,control,diagnostics}.rs`、`src/p0_knowledge/{mod,policy}.rs`、`src/substrate/{ids,origin}.rs`、`tests/contracts.rs`。package=`diana-decomp`，Rust导入=`diana_decomp`，与应用包 `apps/diana/` 分开；已加入 workspace，不能将骨架构建当本卡完成。旧依赖已从主 workspace/lock 去除、保留在 ref-fork；H08 核对不再引入，版本按正式流程，不手工推动release。

**输入→输出 / Interface**：实现DESIGN §2和DESIGN §2的`DexImage<'a>/Engine<'a>/RunContext/Options/Request/Report/MethodOutput/ClassOutput`、分类诊断；未接线Java路径只能Unsupported/Unavailable，不能占位成功。Limits唯一放RunContext；open/decompile/render共用`&mut RunContext`，实现累计used/held配额与method raw预留。schedule直接调用具体typed函数，PhaseId仅为诊断标签；不实现PassSpec/registry/trait/macro。

**步骤**：
1. 创建最小crate与测试注册；enum和private Verified构造器只给owning validator访问。
2. 基础enum、interface、转发与错误包装通过编译/源码审阅确认，不写“枚举值等于自身”或机械getter测试。
3. 按DESIGN §3.7实现request/method配额；method入口预留raw work/分配，normal不可用；used不退款，未消费held才归还。open/index/render同样charge/取消，Report脱离临时IR；trace仅可选有界。
4. 不为私有Pass调用顺序写测试，也不为未实现phase创建空算法；通过真实公开请求的资源耗尽/取消行为验证调度是否停止，trace仅辅助定位。
5. 固定diagnostic字段：code、severity、method/site、phase/pass、kind、boundeddetail，WallTime不得进入文本golden。

**不变量 / 限额**：所有状态可穷举；必需typed transition不可被recovery policy禁用；无registry/PassId重复测试；work/allocation charge不得溢出或退款重试绕限。Cancelled停止请求，不触发fallback。

**测试**：F00聚焦复杂资源控制：公开请求恰在预算边界与超额、资源错误后的fallback预算隔离、长工作取消后停止、重复申请/失败不会绕限制或泄漏。基本状态enum/包装/私有Pass调用顺序不测试；真实Malformed/Unsupported在后续输入/语义fixture验证。T01未有可运行真实请求时这部分作为后续接入要求，不造无效测试凑数。

**验收**：G0和源码/interface审阅；有真实资源/取消请求时运行非零`t01_`领域测试，接口尚未接线时明确待T05接入，不写空green。**完成条件**：crate编译、资源控制实现有明确验证落点；不要求基础enum/路由/转发测试，不能把未接线功能当成功。

**不扩展**：Pass trait/registry/schema生成器、TOML/build.rs、ActionDatabase、restart、并行cache、独立console。

## T02 — 安全 DEX catalog 与方法原始范围

**依赖**：T01。**Phase**：P1/P0。**参考**：RG-06、DX-11/12。

**文件**：`src/p1_input/{mod,cursor,header,catalog,code}.rs`、`src/p0_knowledge/declarations.rs`、`src/substrate/types.rs`、`tests/input/`。

**输入→输出**：`DexImage<'a> -> ProgramIndex<'a>`，bytes由caller scope持有；普通DEX本卡header_offset=0，041由H04补齐；按descriptor+class_def_idx定位class、按`name+proto`定位MethodKey；本卡产出`RawMethodView`只保证code/triesranges安全，T03/T04通过完整P1 `validate_raw`后才构造携带`ValidatedRawMethod`的`MethodInput`，不跨seam暴露uncheckedoffset。metadata暂时仅ASCIIdescriptor/name，非ASCII走已知Unsupported，原range保留待T04。

**步骤**：
1. implement checked_range(offset,count,stride)及u16/u32/ULEB32/SLEB32；强制byte/code_unit类型区别。
2. header035、little-endian、file_size、headersize、map及IDtables各自bounds/alignment；超限Resource，截断Malformed；unknowncontainer/version Unsupported。
3. Type/Proto/Field/Method/classdata索引检查，delta累计checked_add；duplicateclass或ambiguousname按key/proto处理。
4. code_item完整range：insns_size*2、oddpadding、trytable、handlerlistbase及target范围；在分配前用remaining/min_record与全局预算核count。
5. 支持zero code_off的abstract/native声明；proto words与ins_size一致性等code存在时检查；未来rawfallback重新读取同validatedrange。

**不变量 / 限额**：分配前验证长度；5byteLEB高位overflow拒绝；sleb::MIN不直接abs分配；decodedmetadata总预算，不仅每table上限。可定位的localmethodmalformed不让class其他method失去raw身份。

**测试**：F01 `TinyDex`：空class、abstractmethod、exactproto overload；M01 header截断/offset*stride overflow/map越界；M02 methodcodedeltaoverflow、triespadding/handler_off错；Resource小limit。Rust自行造微型DEX，记录builder来源，不能引用未知真实sample。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t02_`。**完成**：正确MethodKey/rawrange，所有M01/M02在分配前明确拒绝，无panic。

**不扩展**：APK/JAR载入、全平台符号解析、源码支持判定、先调用第三方parser再guard。

## T03 — 指令格式表与独立 raw smali 通路

**依赖**：T02。**Phase**：P1/P2(rawdecode)/P9。**参考**：DX-11，RG-02/05。

**文件**：`src/p1_input/formats.rs`、`src/p2_flow/decode.rs`、`src/p9_emit/{mod,smali,spans}.rs`、`src/engine.rs`、smali/input tests。

**输入→输出**：`RawMethodView -> validate_raw(Vec<RawInsn>) -> ValidatedRawMethod`（offset、width、operands、originalwords）→ standard-smali MethodOutput；knownUnsupportedJava与unknownformat分开，unknownformat为Unavailable+hexexcerpt。首版format表覆盖035全部标准opcode格式，Javalowering可只支持极少op。

**步骤**：
1. 单表定义opcode/mnemonic/format/width/version；decoder/formatsvalidator/smaliprinter共用，避免长度表和打印表漂移。
2. 第一遍标instructionstart/payloadstart，检查operandregister/index与widewords；第二遍检查branches相对PC/指令边界；填充payload由T04完善，目前合法格式记录，Java拒绝且不称完整metadata-smali。
3. emitted labels用code_unit稳定命名；invoke/range registerword count保留，不按sourceparameters错误拆words。
4. Engine Unsupported路径丢弃分析partialsource，只调用rawadapter；smali text输出所有instruction，goto不能Nop删除，header/register/return类型完整。
5. add byte spans对应memberdecl/reference与InsnOrigin；T04前只有原输入无待支持metadata、指令完整的case可标Smali。存在尚未支持metadata则Unavailable+RawExcerpt，不发“metadata不完整”的Smali成功。

**不变量 / 限额**：每rawinstruction有且只有一个standard表达或明确unsupportedraw状态；非法unknownop不猜1word继续；fallback输出检查reserve，每method独立预算。

**测试**：F02 `RawMethods`：无debug/annotation的return、moves、wide/rangeinvoke、分支；U01 knownJavaUnsupported仍保留完整knownraw；M03截断operand、branchintoinstruction、invalidregister/index、unknownopcode。对原words/offset逐条比较；尚未支持metadata的原输入明确Unavailable。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t03_`；T05后补S(F02)。**完成**：rawfixtures所有known指令无遗漏，M03无panic，不能fake-smaliunknownword。

**不扩展**：字节码解释器、把unsupported重试成Java、生成抛异常stub、生产spawnassembler。

## T04 — 无损 UTF-16、payload 与 raw metadata 完整性

**依赖**：T03。**Phase**：P1/P9。**参考**：RG-06/07/09，DX-08/11。

**文件**：`src/p1_input/{strings,payload,values,debug}.rs`、`src/p9_emit/{smali,literals}.rs`、`src/substrate/types.rs`、input/literal fixtures。

**输入→输出**：MUTF-8→`DexString(Vec<u16>)`；validswitch/arraypayload/encodedvalue/debugraw→完整Method/Declaration raw记录。另实现原生041的0x78 header/物理offset规则，直接接受原container+header_offset；D065的engine读取由本卡拥有，应用适配留H04。diagnosticlossystring不得成为literaltruth。

**步骤**：
1. decode MUTF-8 NUL、surrogate、invalidencoding、声明utf16_len；remainingbytes与stringunits先核，绝不replacement改变literal。
2. switchpayload对齐/target基于switch-PC、检查原格式要求的keys递增/唯一与数量（不为稳定输出重排原输入）；fill-array宽度countchecked且保留bytesbits；payload不是executableblock。
3. debugprogram逐opcode有限解码，地址增量/line/signoverflow检查；若无法可靠decode该metadata标Malformed/Unsupported，不能打印完整smali承诺。
4. annotation/encodedarray有界栈，非法valuewidth/tag与index拒绝；systemmetadata保留原身份，不写Java关键字。
5. Java/smali literal按ALGORITHMS §4处理NUL/newline/backslash-u/surrogate。041按§1.1遍历logical header链，file_size仅作步长，所有section偏移仍是物理坐标，可指后续共享数据但不指前面的数据；不overlay/rechecksum。

**不变量 / 限额**：保留每u16、float/doublebits；allpayloadbytecoverage，nestedcountdepthcharge；no`Vec::with_capacity(input_count)`先于校验。

**测试**：F03 `Utf16Payload`：unpairedhigh/low、NUL、`\\u000a`文字、NaNbitpattern、sparse/packedswitch、fillarray；M04负relativepayload、overlap、oversizedarray/annotation、baddebugtarget/unterminatedMUTF8。futureS(F03)校验smali→DEXliteral/payload等价。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t04_`；T05后S(F02,F03)。**完成**：rawroundtripcomparison可逐field说明精确保留或明确unsupported，无静默lossy值。

**不扩展**：源级annotation/generic美化、stringbuilder拼接、全部新版DEXmetadata。

## T05 — 接入已冻结 E2E 与最小 typed product/AST

**依赖**：T04。**Phase**：Pipeline/Substrate/P9、测试体系。**参考**：KU-08、RG-08、DX-13。

**文件**：`src/substrate/{cfg,java}.rs`、`tests/contracts.rs`、薄`tests/support/e2e_adapter.rs`、`examples/inspect_class.rs`；读取B01–B04先准备的root `tests/e2e/` cases/manifest/expected/锁定harness，不复制第二份oracle或工具源。

**输入→输出**：按DESIGN §3.2建立Flow/Ssa/Typed/Storage/PlannedBody骨干与privateconstructors；fixturemanifest可驱动nativeinputtests、smaliassemblycheck、javac/d8roundtrip。最早验证DeclarationOnly和rawmethod，不把空AST视成功。

**步骤**：
1. 按DESIGN §3固定采用ownershipwrappers；raw视图借用caller-owned bytes，permethodownedarena，ID永不recycle；当前schema按DESIGN §3采用，不自行换成第二套Context体系；仅创建当前实际消费的产物，不预先铺完整AST。
2. MinimalJavaAst支持合法声明与literal/return预留，Validated/Verified不能pub直接new；P9只接受verified输入。
3. 读取B01–B04已冻结manifest/Dcase/expected/toolchain.lock与inputs证书，不在本卡重新填期望或使用引擎输出生成golden；不同publicinterface只改薄adapter。
4. 接入团队可复现的root测试工具/driver，工具来自公开内容锁与checkout cache，不寻找个人SDK、平台jar或参考仓本地目录；缺工具/hash错失败，子进程只测试工具。
5. 连接raw→smali→受控assembler→独立rawprojection比较；按已冻结D059–D061等原始内容证据核对，不为了当前能力删catch/string/payload expected。
6. 将公开开发示例输出Report状态与spans，未来其他测试可以复用，而不是直接调privateprinter。

**不变量 / 限额**：测试fixture自制授权、来源固定；第三方pickle不载入；errornormalizer不删type/catch差异；zero运行数/timeoutnonzero不当pass。

**测试**：B04已完成harness故障证伪；本卡仅验证实际Engine结果接入：真实错误return/漏event/错raw/编译错误被相同comparator拒绝，正确源参考可接受。缺tool/hash错误保持失败，不能因真实引擎未支持而改required成员。S(F02/F03对应固定D案例组)必须真实assembler执行。

**验收**：G0 +公开adapter的非零实际语义/raw测试，引用B04冻结的suite。**完成**：同一固定case/expected/harness接到真实public库，不重新设计oracle；未实现cases仍报告未通过，不假装完整语义已验证。

**不扩展**：大语料campaign、CI自动download私有APK、复制RGcache、生产CLI接线。

## T06 — 第一条 Java 端到端：常量、参数与显式 scalar locals

**依赖**：T05。**Phase**：P2–P9最小profile。**参考**：DX-01/02/08，RG-02。

**文件**：`src/p2_flow/{mod,build,results,effects,verify}.rs`、`src/p3_ssa/{mod,inputs,build,verify}.rs`、`src/p4_calls/mod.rs`、`src/p5_types/{mod,constraints,lattice,solve}.rs`、`src/p6_variables/{mod,locals,definitions}.rs`、`src/p7_regions/mod.rs`、`src/p8_java/{mod,expressions,declarations,verify}.rs`、`src/p9_emit/java.rs`。

**输入→输出**：F05 `ScalarReturn`的选定静态方法经真实schedule输出VerifiedJavaMethod；含未支持constructor等成员的class如实标Mixed，unsupported其他method依然rawsmali。本卡不要求完整普通class为JavaUnit。无call的P4返回emptyvalidbinding，无complexregion的P7返回singleblockplan，不声称它们已全实现。

**步骤**：
1. supportedop：常量、参数move、int简单add/sub/mul、return；normalterminal验证，return后rawcode仍校验。
2. inputwordbinding、opresultValueId、simpletypeconstraints、ValueId→LocalId，explicitassignment不先GVN/inline。
3. intliteral原bits适当signedspelling；Javaexprstmt只call/new/update，arithmetic写localassignment。
4. JavaASTcompletion/returntype/parameter及Identifier验证，P9输出明确method/context状态+originspans；不能readonlyname猜proto。F05编译只用原自制source已知context替换选定方法，constructor/其他成员保持原源码，不生成任何语义stub；因此只计选定method的Java合法性，不把Mixed算完整DianaJavaUnit。
5. 真实输入经已实现的typed transitions产出结果，失败不留下Java残片；以公开interface的语义、错误状态、预算/取消观察验收。artifact/trace仅辅助定位，不新增私有Pass调用顺序、次数或逐phase计数测试。

**不变量 / 限额**：无handler/noMonitor；undefinedread必须拒绝；long/doubleword契约T10才扩充，不knownprofile就Unsupported而非猜int；每effect暂不承诺可inline。

**测试**：T06实际运行B01–B04固定D009（int加法/溢出），不是到开发时再生成answer/inc的新fixture；`answer()->7`等只作实现说明。未定义读/宽值/预算等按D055–D064及任务固定映射接入，范围不足仍未通过，不改expected。D009在已知合法context运行恢复方法，别把未支持的defaultconstructor计整类JavaUnit。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F05)。**完成**：至少一个非空方法真实编译和执行闭环，负例不是默认值“成功”。

**不扩展**：复杂branch、handler、变量merge、表达式美化、通用typedhelper生成。

## T07 — 分支与固定身份 join Phi 的 SSA

**依赖**：T06。**Phase**：P2/P3/P5/P6。**参考**：DX-03/04、RG-03。

**文件**：`src/p2_flow/build.rs`、`src/p3_ssa/{liveness,build,verify}.rs`、`src/p5_types/constraints.rs`、`tests/pipeline/ssa.rs`。

**输入→输出**：normalbranchFlowBody→SsaBody，joininputs以EdgeId标识，ValueId稳定；尚无Javacopylowering完整支持的方法可Smali，但internalSSA须完整验证。

**步骤**：
1. buildblockleaders由entry/target/afterterminal；normalbranches精确true/false，goto为terminator不是effectNOP。
2. 按ALGORITHMS §3反向求只增长的live-in slot集合；normal write kill与throw-before需求分开。每个block每个live-in slot均建固定Phi，包括单predecessor；虚拟EntrySeed提供参数。
3. 每block一次扫描：先绑定全部reads、保存所需throw旧值，再使重叠slot失效/写固定Def。每条incoming一次连接，未填Option不是语义Undefined。
4. 从实际uses保留Phi闭包，Bad与grounded分别单调传播；缺定义/partial-wide/无根定义环拒绝。验证器重放block最新定义与normal/exception来源，不拿normal dominator替代异常检查。
5. 首版不做trivial-Phi消除或别名缩减；类型留有限constraints。封口后drop Env/live scratch，保留必要Phi inputs和use绑定给P5/P6。

**不变量 / 限额**：finiteworklist；looppreallocatedPhi即使本卡不emitloop也可内部测试；branchconditions只绑定一use；每eligibleedge一Phiinput。

**测试**：F06 `DiamondPhi`：choose两个const/param、nesteddiamond、sameblock两edge；M06遗漏Phiinput、wrongedgeValueId、不支配use、undefinedbranchjoin。inputSSA正例+mutation反例。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t07_`。**完成**：join证书真实生成/独立校验，稳定ID与预算测试；T08须接上F06Java行为。

**不扩展**：exceptionSSA、fullheritage、dominancefrontier优化、coalescing。不要只凭变量表叫SSA而省略join。

## T08 — 独立 Local 分配、边 parallel copy 与循环值

**依赖**：T07。**Phase**：P3/P5/P6/P8最小transfer。**参考**：DX-04/05。

**文件**：`src/p6_variables/{locals,copies,definitions}.rs`、`src/p3_ssa/build.rs`、`src/p8_java/structured.rs`的最小diamondadapter、storagetests。

**输入→输出**：TypedBody→StorageBody+normalCopyPlan；每nonliteraldefinition与Phi独立LocalId。F06simplediamond可输出Java；loop先内部plan验证，T09提供通用执行。

**步骤**：
1. 参数/this为pinnedbinding，ValueId不同默认不sharelocals；不把registernumber直接当LocalId。
2. Phi.inputs按EdgeId产生parallelassignments；criticaledge保留edgecopyplacement，不先落sourceblock尾。
3. 严格采用ALGORITHMS §5的parallel sequentialize：dst不在剩余sources才输出，否则保存任一cycle source到exact-type temp并改写所有该source；entry copy在首次进入前，branch/latch copy在选中edge后。
4. DefinitionCertificate独立求originaluse/edgecopyavailability；仅已证明不可观察的普通locals允许defaultinitialization满足JavaDA。
5. loopentry/latch多个Phi预分配/传播，normalcopyplan可以internalinterpreter在test验证；最小diamondP8实现singlecondition求值与transfer。

**不变量 / 限额**：deaddefault不是借口读Undefined；copy周期tempcharge、source-dsttypeproof；先选edge后copy，copy全是pure无throw。Uninit不得null初始化伪装Ref。

**测试**：F06choose正/负条件两driver；F07 `CopyCycleLoop`：`a<-b,b<-a`期望交换、2/3cycle、criticaledge、loopentry/latch；M07naive顺序赋值、copy放wrongbranch、default观察、typecastcopy。T09后补B(F07)。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F06)。**完成**：simplebranch真Java；parallelcopy所有cycle/criticaledge反例validator失败；loopcopy义务清楚不自称全loopJava完成。

**不扩展**：interferencecoloring、局部name角色推断、异常shadow、跨regionleavecopyheuristic。

## T09 — 普通 CFG dispatcher 与端到端循环

**依赖**：T08。**Phase**：P7/P8/P9。**参考**：KU-06/07；方案是新DEXadapter，不复制Cgoto。

**文件**：`src/p7_regions/{normal,verify}.rs`、`src/p8_java/{dispatcher,verify}.rs`、`src/p9_emit/java.rs`。

**输入→输出**：无handlers/Monitor/Uninit的StorageBody→DispatchPlan→VerifiedJava；onecaseperreachableblock，localpc取本次BlockId对应值，不要求跨运行case展示序列相同。

**步骤**：
1. eligibility显式检查，异常图任何handler不进dispatcher；constructoruninit同样拒绝。
2. case内按ops原顺序，Branch两arm各emit对应copy、pcnext、continue；return/throw退出；defaultpc不可达证书。
3. loops完全通过edgepc，不先认while/for；nestedcaseJava括号作用域合法，alllocals在外层可见。
4. StructureCertificate重新枚举ops/edges确认覆盖；合成pc/assignnoheap和noallocation，但defaultassert不可达不依赖userinputguess。
5. support普通irreduciblemultipleentryloop，扩展budget只影响Resource不影响输出语义；nohandlercase不应因自然structuring失败而错fallback。

**不变量 / 限额**：conditionevalonce、edgecopy先于pcassign、case目标/键与block identity对应；互不fallthrough的dispatcher case展示顺序不要求稳定。输出估算limit先reserve，不能fullstring造好再丢。

**测试**：B(F07)交换循环，多次迭代/0次/continue/break；F08 `DispatchGraph`：simplecycle、twoentrycycle、multipleexit、throwterminal；M08漏edge、duplicateeffect、defaultreachable、handlerineligible。人工two-entry CFG用T05测试专用scalar interpreter作独立oracle并与实际恢复Java执行比较；本卡不引入JVM不能直接生成的原始人工CFG源码假替身，exception/monitor不使用此简化oracle。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F07,F08)。**完成**：复杂ordinarygraph有合法Java而非goto伪码；eligibilitynegative严禁接受。

**不扩展**：try/monitor状态机、Object[]寄存器装箱解释器、复制effecttail、字节相同作为自然结构质量指标。

## T10 — Primitive/Null/FP 完整第一档合法化

**依赖**：T09。**Phase**：P2/P5/P8/P9。**参考**：DX-08，RG-09。

**文件**：`src/p5_types/{lattice,constraints,solve,conversions}.rs`、`src/p2_flow/decode.rs`、`src/p8_java/expressions.rs`、`src/p9_emit/literals.rs`。

**输入→输出**：bits/wordtypes+useconstraints→TypedBody/UseConversion→合法primitive/refnullJava。支持boolean、byte/short/char/int/long、float/double、cmp/cmpl/cmpg、conversions和shift；notenoughtypeevidence明确Unsupported。

**步骤**：
1. finite候选type域，constbits不第一次use即固定；numeric/refzero用上下文约束，不全写null或0。
2. widewordpair、aliasoverlap/partialread检查；所有shiftmask、signed/unsigned、narrowing与Dex类型语义表逐opcode对应。
3. booleanbitwise合法truth表达、eqzref==null；任何ambiguous非literal值不直接重解释cast。
4. cmp FP用`x<y?-1:x>y?1:x==y?0:bias`，x/y为locals、bias=-1/+1；不需isNaN库调用，也不折掉NaN分支。FP不reassociate。任意NaN payload/bitcast需明确intrinsic证据，不在T10偷偷添加调用。
5. typedliteral.escape覆盖negativeMIN、Infinity、-0、surrogate；每UseConversionno-extra-runtime-check可核查。

**不变量 / 限额**：有限constraints收敛，Conflict显式拒绝；overflow和Java数值promotion不靠hostRust默认猜。

**测试**：F09 `PrimitiveBits`：booleannegation/bitop、0/null/false、MIN/-1、shift-1/32/64、NaNcmpl/cmpg、negativezero、float→int边界、wideoverlap；M09错误kind/partialwide/unresolvedprimitive；B(F09)按bits/traces比较，不只formattext。F09必须拆成无library-call的核心primitive正例与尚无P4intrinsic证据的bitcast子例：后者本卡预期Unsupported/raw，不能让T10依赖未来T11；T11建立intrinsic声明后新增独立扩档验收。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F09)。**完成**：supportedprimitive矩阵每族有positive/boundary；未验证bitcast/intrinsic不标支持。

**不扩展**：全referencegenerics、arithconstantfold/GVN、浮点代数压缩、万能numericreinterpret。

## T11 — 精确调用契约、外部声明与结果绑定

**依赖**：T10。**Phase**：P0/P2/P4/P5/P8。**参考**：DX-02/06/08，KU-01。

**文件**：`src/p0_knowledge/declarations.rs`、`src/p2_flow/results.rs`、`src/p4_calls/{mod,members,contracts}.rs`、`src/p5_types/constraints.rs`、`src/p8_java/expressions.rs`。

**输入→输出**：rawinvoke+完整/未知catalog→CallEvidence精确kind/owner/proto、receiver+argswordbound、typedresult；缺bindingfacts→Smali，不“调用看起来像就输出”。

**步骤**：
1. invoke格式/range、receiver先、descriptorwords；move-result必须紧随合法producer、正确kind且不是branchtarget；未消费callresult仍保留call。
2. selfmadecompletecatalog提供static/direct/virtual/interface/super、visibility、overloadgroup、throws；Unknown与Partial分开。P4产出descriptor约束与BindingObligation，不要求P5尚未求出的source type。
3. P5完成类型和UseConversion后，P8 discharge精确Javabinding：receiver/staticowner、argstatictypes、重载唯一/dispatch等价；cast必须no-extra-check或原checkcast。
4. callmaythrow保守、unusedresult不DCE；checkedthrows与callerdecl/override一致，不sneakythrow、catchwrap或扩张契约。
5. 报告保存originalMethodRef与EnvironmentId；same-name-different-proto请求精确选择。

**不变量 / 限额**：catalog候选遍历charge；Unknown不fill默认public；参数eval不重排；任何runtimecheck新增必须证据。

**测试**：F10 `CallKinds`：overloadint/long/Ref/null、virtualoverride、interface、private/direct、super、sideeffectargs、unusedthrowingcall；F11 `MissingCatalog`相同输入complete时Java、empty时明确Smali；M10badresult/argwords/staticreceiver/visibility/returnonlyoverload。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F10) + S(F11)。**完成**：所有kind有行为driver和rejectcase；missingcatalog为预期降级而非pass“忽略errors”。

**不扩展**：nativecallingconventions、反射推断、动态invoke-custom/bootstrapJava语义、全平台catalog。

## T12 — 对象初始化身份与受限构造器

**依赖**：T11。**Phase**：P3/P4/P5/P8。**参考**：DX-09，DX-02 RecoverConstructors。

**文件**：`src/p4_calls/allocation.rs`、`src/p3_ssa/{inputs,verify}.rs`、`src/p5_types/lattice.rs`、`src/p8_java/declarations.rs`。

**输入→输出**：邻接new-instance/invoke-directinit→allocation证据和合法Javanew；`<init>`的UninitThis→singlelegalsuper/thischain→正常Refbody。

**步骤**：
1. AllocationId跟踪puremoves，不允许uninitescape/普通call/fieldread；唯一init且正常路径之后referencevalid。
2. 初版要求args先于new准备或literal、无interveningeffect/maythrow；Javanewconstructor组合不能把effect跨过allocation/classinit时机。
3. constructorprologue只允许不观察uninitthis的pureargpreparation和singleexpressiblechain；super/this真实行为不删。
4. ownership/proto访问完整；多个init、joinuninit、不合法Java参数次序→Unsupported，rawfallback保存。
5. finalfieldassignment复杂完整处理T20，本卡simplepostsuper字段写先维持eligibility，不提前移动到initializer。

**不变量 / 限额**：uninit不能defaultnull成为普通Ref；new与init被一个sourceeventconsumed但origin二者保留；无“万能静态helper”。

**测试**：F12 `AdjacentConstruction`：puremovealias、argsreadybeforeallocation、supertrace→bodytrace、thischain；U02 `ConstructorPrologue`：pre-superthisfield、controlflow/monitor/maythrowargs、initdup，应Smali不是Java。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F12) + S(U02)。**完成**：constructortrace顺序一致，所有uninitnegative明确处理，未支持特例不删super掩盖。

**不扩展**：arbitraryconstructorcarrier/helper、anonymouscapture、enumimplicitconstructor、objectallocation大量重写。

## T13 — Reducible if/while 自然结构 adapter
**依赖**：T10。**Phase**：P7/P8。**参考**：KU-06/07，DX-07区域职责。

**文件**：`src/p7_regions/{normal,verify}.rs`、`src/p8_java/{structured,verify}.rs`。

**输入→输出**：normalStorageBody→私有SESE/SCCregionplan→naturalJava；schema不能覆盖则普通dispatcher（T09），不修改原CFG。

**步骤**：
1. 使用算法所需RPO/normaldominance/postdom构造Sequence/Diamond/NaturalLoop，每collapse减少regionnode数；不添加稳定排序/tie-break保证同源码形式。
2. loopheader/latch/exit保留EdgeId；continue/break的copies在transfer前，不能按blockvisited删除尾部。
3. singleentry循环和multipleexit受限profile、`while(true)+if break`合法即可，不做for/foreach糖化。
4. RegionCoverage验证leafBlock集不重叠，Transfer映射精确；origin与effectcoverage不是文本次数。
5. 同fixture用StructuredThenDispatch与DispatchOnly两adapter比行为证据；纯readability不同不要求textidentical。

**不变量 / 限额**：原topology/SSA/CopyPlan不变；privatearena有限；不可归约走已有合法dispatcher，handler/monitor图仍禁止普通fallback。

**测试**：F13 `NaturalControl`：diamond、while0次/多次、nestedloops、conditionalbreak/continue、copyonexit；M11丢边/错误scope/dupregion；B(F13)两mode结果/trace一致。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F13)。**完成**：simpleloop明确naturalAST，hardgraphdispatcher；certificateinvalid不能降成宽松成功。

**不扩展**：NoMoreGotos/taildup/SAILRfullalgorithm、异常region、减少变量数、goto品质当正确性oracle。

## T14 — 整数 switch / payload / fallthrough 结构

**依赖**：T13。**Phase**：P2/P7/P8。**参考**：RG-02、DX-07、KU-06。

**文件**：`src/p2_flow/build.rs`、`src/p7_regions/normal.rs`、`src/p8_java/structured.rs`、`src/p9_emit/java.rs`。

**输入→输出**：已校验packed/sparsepayload→key/EdgeIdmapping→JavaSwitch或dispatcher，target共享/fallthrough保留。

**步骤**：
1. key有序唯一、relativeoffset正确；defaultedge不伪造成最后case。
2. sharedtarget合case但保留原keys与originedges；falls/return/breaktargets显式，不合并不同copyplan。
3. differingPhiinputedges进入同target：case内先执行对应copy再转目标；不因block相同丢edge差异。
4. non-SESE/loopinsidecase优先dispatcher，普通graph结构失败不emitundefinedgoto。

**不变量 / 限额**：selector一次求值；每key语义exact，switchkeys数量和outputbyte受限；payload不算executableop。

**测试**：F14 `IntegerSwitch`：negative/min/maxkey、sharedtarget、default、fallthrough、loopbreak、caseedgecopy；M12duplicatekey/outofrange/branchpayload。B(F14)覆盖每case/default与sideeffectselector。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F14)。**完成**：普通整数switchcoverage完整，string/enum降糖模式暂未支持不fake恢复。

**不扩展**：stringhashswitch/enumordinal糖化、未知bootstrap switch、branchcascade自动重启识别。

## T15 — 字段、数组、显式 check-cast 与可见效果

**依赖**：T12。**Phase**：P2/P4/P5/P8。**参考**：DX-06/08，RG-06。

**文件**：`src/p2_flow/{decode,effects}.rs`、`src/p4_calls/members.rs`、`src/p5_types/{constraints,conversions}.rs`、`src/p8_java/expressions.rs`。

**输入→输出**：iget/iput/sget/sput、arrayload/store/new/fill/length、instanceof/checkcast→准确typedops+合法Java，rawmemberidentity不被source名字覆盖。

**步骤**：
1. descriptorkind与opcode窄/宽/reference兼容，null/arraytype用constraint而不是firstuse猜String。
2. fieldstatic/instance、visibility、volatile/classinitfact来自completecatalog；unknownflags影响effect保守，不自动假定publicfinal。
3. arrayindex/value/receiver按原eval顺序；fillarraybytebits与elementtype一致，未知type拒绝。
4. unusedfield/arrayread、cast依旧生成typedtemp保持throw/init/volatile；原checkcast不能因为结果不用而删。
5. referencejoin只在no-extra-check证据下转换，Object上新增cast不能只是为了javac通过。

**不变量 / 限额**：默认heapread/write/callbarrier，noDCE/hoist；allocationarraycountscharge；staticinit读字段不变成literal常量失trigger。

**测试**：F15 `HeapEffects`：fieldget/set/staticinittrace、arrayload/store顺序/null/bounds、unusedcheckcastthrow、volatile原指令保留、wide/floatarraybits；M13wrongfieldopcode/arraypayloadtype/missingcatalog。B(F15)比较结果、mutationlog、exceptionclass。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F15)。**完成**：supportedheapops都有throw/effectnegative，originalmemberspans定位正确。

**不扩展**：heapaliasoptimizer、静态final常量传播、泛型字段typeprojection、去除unusedthrowing表达。

## T16 — 每抛出 site 的有序异常图与 SSA 环境

**依赖**：T15、T07。**Phase**：P1/P2/P3。**参考**：DX-03/07。

**文件**：`src/p2_flow/{protection,build,verify}.rs`、`src/p3_ssa/{liveness,build,verify}.rs`、`src/p1_input/code.rs`。

**输入→输出**：checkedtry/handlerraw+maythrowops→orderedExceptionEdges；SsaBody含throw前exceptionenvironment与handler ExceptionValue。该卡先internal验证，JavaprotectedprofileT18后完成。

**步骤**：
1. split每受保护maythrowop到terminalsite，try边界也split；orderpreservehandlerlist、catch-all位置；normalgraph和exceptiongraph独立。
2. instruction成功result仅normaledgecommit；throwingdefinition覆盖旧reg的exceptionedge保持旧value，其他opcode-specifictransfer显式列。
3. handler入口move-exception新ValueId，typedchain与commonhandler可能多exceptionedges；incomingPhi按原EdgeId。
4. exceptionSSAvalidator不使用normaldomtree替代异常availability；read-before-def仍Malformed，unknowntype不标正确。
5. normaljumpintohandler、overlap异常区/非法range在raw及flow层明确错误/Unsupported，不排序修复。

**不变量 / 限额**：throwedge每site对应实际opcodeeffect；maythrow分类保守；拆图扩张limitscharge，不把过复杂当无handler。

**测试**：F16 `ThrowState`：`v=old;v=call();catch{use(v)}`、multipletypedcatch/catch-all、sharedhandler、throwinhandler；M14使用normalnewresult、catchrank重排、moveexception非法target、trymidinstruction。tests查environment，不仅source文本。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t16_`。**完成**：异常site/oldstate链证书齐全，尚无Javaprotected能力则不作为该Java功能通过；T18必须补B。

**不扩展**：finally归并、catch-all一律转换finally、generalmonitor、跨trydispatcher。

## T17 — Handler Phi shadow snapshots 与异常 ingress 拷贝

**依赖**：T16、T08。**Phase**：P6。**参考**：DX-05。

**文件**：`src/p6_variables/{exceptional,copies,definitions}.rs`、`tests/pipeline/exception_storage.rs`。

**输入→输出**：exceptionSSA+TypedBody→beforethrowpureShadowSnapshot/HandlerIngress CopyPlan；handlerPhi每incomingthrowsite有对应typedsource证明。

**步骤**：
1. enumerate每handler retainedPhi的eligibleexceptionedges；创建请求内固定身份ShadowLocal，不要求跨运行同编号；不能借普通criticaledges插catch中间代码。
2. 在throw site之前保存该edge的oldstate值，所有可能handler的snapshot均no-throw/no-allocation/localonly；catchingress绑定PhiLocal。
3. 每(handler,Phi)始终独占shadow，nested inner handler中的throw同样给outer保存旧值；不按type合槽。保护归属采用ALGORITHMS §10一层nested规则，shared/crossing无法归属才拒绝，不使用“最近一次寄存器值”。
4. DefinitionCertificate验证每snapshot来源在该site已定义，conversionexact不引入runtimecheck；frame-leveldefaultslot不可观察。
5. P8 placement obligation产出明确列表，后续try构造必须消费同site位置，sourcecopy不能放到另一保护域。

**不变量 / 限额**：shadow只影响合成local、noheap、noextraexception；多个handlersnapshotcopy总数有上限，copycycles也按T08序列化。每throwedge有来源，不依据handler最后一次写猜值。

**测试**：F16旧值internalstorage、F17 `HandlerPhi`两个sites不同oldvalue、looptryshadow、sharedtarget；M15snapshotcheckedcast/Undefined/放在call之后/alias重用错覆盖，validator必须失败。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t17_`；T18后补B(F16,F17)。**完成**：计划与证书可复算，不能“生成了几个shadow变量”即算异常SSA lowering完整。

**不扩展**：复杂cleanup变量合并、把所有exceptions装箱动态解释、异常路径deadcopyheuristic。

## T18 — 受限 try/catch Java 与保护域 ownership

**依赖**：T17、T13。**Phase**：P7/P8。**参考**：DX-07、KU-07。

**文件**：`src/p7_regions/{protection,verify}.rs`、`src/p8_java/{structured,declarations,verify}.rs`。

**输入→输出**：单连续interval、独立handlers、清晰normaljoin→ProtectionCertificate→JavaTry/Catch；不满足profileSmali，ordinarydispatcher不得接异常graph。

**步骤**：
1. exactprotectedoperation集映射Java try；catch顺序按DEX链，catch-all先catchThrowable并保留代码，不猜finally。
2. handlerexclusiveentry、noordinaryreentry、join/return/throw可表示，catchunreachability和checkedthrows同P4合同。
3. 对beforethrowsnapshot找到相同保护域的sourceop之前位置；normalexitcopy和exceptioningresscopy全部消费。
4. copiedcleanup在不同路径原样保留；try大小不能扩到normalcleanup或额外call，以免exception二次捕获行为变化。
5. 一层nested采用ALGORITHMS §10：handler内throw依赖指向outer，先归约inner try+handlers再被outer包含；最终逐site重算catch-stack第一匹配目标。siblings catch不捕获handler自身异常。shared/crossing不匹配则拒绝，不猜finally。

**不变量 / 限额**：每protectedsite原handler选择一致、throw/return完成性一致、catch无法直接向scope外goto；certificate失败为InternalInvariant/Unsupported的定义必须区分算法BUG和不支持形态。

**测试**：B(F16,F17)；F18 `ProtectedControl`：orderedsubtypecatch、unexpectedError走catch-all/rethrow、cleanupreturn/throw、cleanup自己抛出、nestedonelevel；U03 `ComplexProtection`normalreentry/sharedhandlers/release未知需Smali；M16漏edge/try扩大/把call外异常吞掉。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F16,F17,F18) + S(U03)。**完成**：positiveprotected方法真实Java运行，negative保持smali，notfullfinally恢复不影响真实trace。

**不扩展**：arbitraryexceptionownership、全路径finally美化、labelgoto跨try、吞异常使javac通过。

## T19 — Monitor scope 证书与 synchronized

**依赖**：T18、T12。**Phase**：P2/P7/P8。**参考**：DX-07（synchronization）。

**文件**：`src/p7_regions/monitors.rs`、`src/p2_flow/effects.rs`、`src/p8_java/{structured,declarations,verify}.rs`。

**输入→输出**：declarationflag或显式balancedmonitor CFG→MonitorCertificate→Java synchronized；未知/mismatched锁状态→Smali，不输出伪Java关键字。

**步骤**：
1. rawflag语义审阅，method-synchronized隐式lock与explicitmonitor分开；静态lockClass对象、实例lockthis明确。
2. 按ALGORITHMS §11求有限reference-root；One还须root producer在整个持锁scope外且不会重执行，不能凭同静态AllocationId跨迭代认同对象。lock-stack在join一致，enter异常不push，exit精确pop，重入按层计。
3. singleentryscope、null锁throw时机、releasehandlerinterval/protectedsite一致；match后仅以证书替换原enter/exit/catchcleanup。
4. nestedbalancedprofile逐scope核，不证明的路径不跳过monitor；scopebodycall绝不放到synchronized之外。

**不变量 / 限额**：锁effect不是pure，noinline/hoist；证书有all-exitwalkbudget，循环不靠无界enumeration。无法证明停止该profile而非假定balance。

**测试**：F19 `MonitorScope`：return/throw/nulllock、nested/reentrant、methodstatic/instance，body中`Thread.holdsLock`driver、finallythrow后lock释放；U04 mismatchedlock/exitmissing/multientry→Smali；nativeverifynegativecoverage。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F19) + S(U04)。**完成**：有明确语义trace/holdsLock/释放检查（并发stress仅补充不当完整证明），rawflags不直接打印declared_synchronized。

**不扩展**：跨锁aliasvalue猜测、arbitrarymonitorstate、去除“多余”锁、生产运行动态验证。

## T20 — <clinit>/<init>、encoded 初值与 final 字段

**依赖**：T19、T15。**Phase**：P1/P4/P8。**参考**：DX-08/09（initialization）。

**文件**：`src/p8_java/{initializers,declarations,verify}.rs`、`src/p1_input/values.rs`。

**输入→输出**：staticencodedvalues+clinit、constructorfieldwrites→合法classdecl/initializerplan；不能完整shell表示则Mixed保留raw字段/方法，不能给finaldefault再写。

**步骤**：
1. 按ALGORITHMS §12绑定encoded/default：nonfinal非默认值先发static block；final无原写则连default也显式赋一次，有原写只接受default preparation且不另赋值。static final primitive/String不发常量字段initializer，防内联sget/删class-init。
2. clinit按ALGORITHMS §12发static block，return-void转break init label，必定abrupt的body按既定if(true)规则合法化；不造<clinit>方法，不改变read触发初始化。
3. 按ALGORITHMS §12用有限{0,1,Many}路径写次数集合，join并集/写入递增；每条正常构造链/clinit结束须恰1。this委托继承目标写集合、不重复发初值；重写/缺失/未知路径不补default凑编译。
4. 无constructorbody未赋final不能默认初始化“凑编译”；classshellfailure不影响独立methodanalysis，但报告Mixed。
5. 初始化相关throws与classfinalmodifier/rawvisibility证据由P4/metadata而非printedtext猜。

**不变量 / 限额**：sourceconstantfinal可能改变classinittrigger必须核；初始化次数/order和失败路径可见；clinitfield重写不丢origin。

**测试**：F20 `Initialization`：nonconstantstaticfieldtrace、multipleconstructors、thisdelegate、blankfinal、encodednull/false/floatbits、initexception；U05 complexfinalreassignment/default不可证→Mixed/Smali；B(F20)每driver新进程防classinit已发生。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F20) + S(U05)。**完成**：合法unitinittrace一致，无法sourcefinal表达明确decl状态，不删finalcontract隐藏。

**不扩展**：fieldinitializer美化、constructorfactorycarrier、interproceduralinit优化、constant传播。

## T21 — Raw generic、bridge/override 与 source 绑定合法性

**依赖**：T20、T11。**Phase**：P0/P4/P5/P8。**参考**：DX-08及signature/sourceABI问题。

**文件**：`src/p4_calls/{contracts,members}.rs`、`src/p0_knowledge/declarations.rs`、`src/p8_java/{declarations,expressions,verify}.rs`。

**输入→输出**：Signature metadata可保留raw而不必恢复`List<T>`；Javaeraseddecl/override/callbinding证据全通才成功，return-onlycollision不能随意renameexternals。

**步骤**：
1. 审阅DEXdescriptor与Signature差异，保留metadata原记录；rawdecl不改变binarymemberproto，typedcasts不新增runtimecheck。
2. 采用ALGORITHMS §12的MemberEmission::Generated：精确override事实与canonical moves/cast→target call→return匹配，且source会生成同签名同行为bridge才不重复打印。Generated仍有code/原MethodKey，不计DeclarationOnly；单独请求无法独立表达的bridge返回原Smali。
3. Java实现interface/override方法return/throws/visibility与rawcontract一致，genericparentmember绑定不能只依localdescriptor猜。
4. 不完整catalog的相关方法fallback，不让missingtypeexception“忽略javacerrors”变contract。

**不变量 / 限额**：puregeneric美化可省，bindingcorrectness不可省；overloadcomplete状态与EnvironmentId稳定，sourcepublicidentity不改。

**测试**：F21 `ErasedContracts`：selfmadegenericList/identity、covariantbridge、rawinterfaceimpl、nulloverload；U06 return-onlyDEXoverload、bridgeeffect不同、unknownparentcontract→Smali/Mixed；B(F21)配套完整selfmadeclasspath。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F21) + S(U06)。**完成**：raw泛型警告可接受，但真正编译错误不可忽略；legacyfulltypesolver不整体搬入。

**不扩展**：完整GenericTypeSolver/variance/inference美化、通用wildcardprojection、随意sneakythrow/externalsrenaming。

## T22 — 命名 nested class、捕获与 unit ownership

**依赖**：T21、T12。**Phase**：P1/P4/P8。**参考**：DX-09/12，RG-04。

**文件**：`src/p8_java/ownership.rs`、`src/p1_input/catalog.rs`、`src/p8_java/declarations.rs`、`src/p9_emit/spans.rs`。

**输入→输出**：可靠Enclosing/InnerClass/MemberClassesmetadata+declarations→命名nestedJava或Mixed；不要求匿名类源码糖化，非法ownership、missingdependency不wholeclasssilentlydrop。

**步骤**：
1. 元数据ownership图cycle/duplicate验证，`$`名字只是hint，不能独立证明inner。
2. 显式static/instanceouterbinding、constructorcapturefields、privateaccess事实；原MethodRef/FieldRef保持sidecar映射。
3. 可用合法nested名称先实现，匿名original可以reportsyntheticnamedclass但访问/捕获/constructor语义必须一致；unexpressibledecl保留smali。
4. unitpackage/import/name/visibility冲突、顶层static不允许；单innerfragment归入Mixedcontext，不假冒JavaUnit。
5. finalspans统一relocate，UnicodeUTF8边界与originalclasskey可追踪。

**不变量 / 限额**：nestedclosure依赖/深度/输出有请求预算；no`MemberClasses`漏成员，missinginner明示；renameinternal不能改变外部绑定contract。

**测试**：F22 `NamedNested`：staticnested、nonstaticoutercapture、same-simplename、Unicodeidentifier、privateaccess；U07 metadatacycle/missinginner/ambiguousownership→Malformed/Unsupported正确分类；B(F22)全unit真实classpath编译和capturetrace。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F22) + nativeownership/spanmutationtests。**完成**：top-levelunit合法，originalidentitymapping完整，singlefragmentstatus正确。

**不扩展**：匿名inline/lambda恢复、无metadata根据名称猜inner、publicexternalmemberremap。

## T23 — 受限 enum / annotation 与完整 shell 分类

**依赖**：T22、T20。**Phase**：P1/P4/P8/P9。**参考**：DX-08/09，RG-09。

**文件**：`src/p8_java/{enums,declarations,verify}.rs`、`src/p1_input/values.rs`、`src/p9_emit/literals.rs`。

**输入→输出**：canonicalselfmadeenum、annotationinterface与runtimevisiblevalues→合法JavaUnit；无法表达identity/initializer/metadata的shell→Mixed，而独立method仍可Java。

**步骤**：
1. enumconstantfield/order/name/ordinal、canonicalconstructorimplicitEnum参数、values/valueOf契约逐项验证；source必须Javaenum，不ordinaryextendsEnum。
2. 不把任意syntheticmethod删除；canonicalgeneratedmember要有完整behavior证据，多constantbody/extraeffect留Unsupported。
3. annotationelementtype/default/Retention/Target/userannotationliteral真实检查；systemSignature/InnerClass当metadata，不fake`@Signature`Java。
4. rawmetadata保留sidecar但runtimeannotation缺失不能宣称fullclass语义相同；若policy允许inspectionJava片段则Mixed标明非完整unit。
5. statusassembler核对任何Smali/Unavailable/unsupporteddecl存在必Mixed，不能errorcounter=0即JavaUnit。

**不变量 / 限额**：metadatafullsource与inspectiononly不同；annotationarray/stringUTF16limit递归有界；canonicalenum少一个constant/initializer即拒绝。

**测试**：F23 `EnumAnnotation`：enumiteration/name/ordinal/valuesclone、canonicalvalueOf、runtimeannotationreflect/default/array/UTF16；U08 enumcustombody不可证/invalidannotationelement/missingretentionfacts→Mixed；B(F23)本卡只接受canonicalprofile。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F23) + S(U08合法raw部分)。**完成**：支持profile可复算、JavaUnit分类与metadata语义合同吻合，无extendsEnum伪合法代码。

**不扩展**：全enum-switch降糖、annotation平台库大catalog、复杂enumconstantbody、反射名rewrite承诺。

## T24 — 可选纯表达式恢复（不是必需核心）

**依赖**：T23、T14。**Phase**：P8/Pipeline/P0。**参考**：KU-04/05/08，DX-06。

**文件**：`src/p8_java/recovery.rs`、`src/pipeline/schedule.rs`（仅实际接线）、`src/p0_knowledge/policy.rs`。

**输入→输出**：已verifiedJavaAST→可选readabilityrewrite→再次verify；defaultoff，关闭不影响CoreRC。只smallpurelocalcopies/literal和受限loop形式，不搬BDD/gatedPhi。

**步骤**：
1. 仅在P8实现一个真实可选pure函数及明确option，default off；不建PassSpec/registry。按ALGORITHMS §15的Applied/Declined/BUG合同处理，不预建E02–E08插件，必需legalization不可关闭。
2. 只同blocksingleuse、nopossiblethrow/nocall/noheap/nomonitor/nochangingclassinitdomain的pureexpr内联；booleansyntax以已解typefact。
3. rewrite每轮有finite measure（locals/redundantnodes下降）或explicitmaxround/worklimit，unchanged停止。
4. applyonprivateAST，验证失败丢candidate、保持baselineAST并记RecoverDeclined；InternalInvariant仍reportBUG而不是默默吞。

**不变量 / 限额**：noeffect duplication/motion，postverifiedtypes仍valid；defaultoff和on各有sameprofile行为；label/state数量仅qualityproxy。

**测试**：F24 `PureRecovery`：purecopyinline正例、call/read/div/monitornegative；B(F24)on/offdriver一致，mutationduplicatedeffect失败；budget不会禁用requiredcopylowering。

**验收**：G0与本卡公开核心case（统一规则见§4）；B(F24)。**完成**：smalloption真正消费、trace可见、失败保留baseline、不影响T25核心gate。

**不扩展**：本卡不做常量分支改CFG、GVN/LICM、stringbuilder/finally还原、全局restart、registry和持久HARDassertions；这些不是永久禁止美化，后续可按ALGORITHMS.md另起独立可选任务并验收。

## T25 — 固定核心集合加固与 EngineCoreRC

**依赖**：T23、T14。**Phase**：全部/验证。T24为可选美化，不是本卡前置。**参考**：KU-04/08、RG-08、DX-13。

**文件**：`tests/contracts.rs`、薄public adapter、`src/pipeline/control.rs`与实际核心validator，读取root `tests/e2e/manifest.json`/cases/expected/input证书；新crateREADME来源登记和本设计实现状态。不要各crate复制第二套可变fixture/expected。

**输入→输出**：T01–T23的实际supportedprofile→CoreRC证据集：每能力positive/negative/limit/状态、behaviordriver与rawfallback、无panic/mutation、无序输出的身份/内容完整性、source许可证审阅。不是宣布所有DEX可Java。

**步骤**：
1. 核对B01–B04已固定manifest与实际typed transitions，required成员不按当前实现删/skip；将core-java的失败改为Smali成功不可接受。filterzero断言，known038/039/custom/polymorphic未支持列表与rawcoverage分别明确；unknownformat不能fakefallback。
2. 生成字节和artifactmutation测试：count/offset/LEB/payload/handler/type/Phi/copy/site/loop/state/outputbudget，固定seed，取消会终止。
3. 每个knownunsupported合法fixtureSgate；malformedUnavailable/rawexcerpt不冒充smali，BUG不算成功。fallback吞掉exception/type错误的策略必须反例失败。
4. 跑固定D001–D064及D065的engine-seam证据（共65个案例身份），不按当前supported过滤。D065应用适配与D066–D070由H卡负责，H08汇总全部70；总账仍显示70 required，不能把未归engine的应用义务报pass或删除。单列JVM/ART、catalog完整性、metadata、exception/monitor证据。T24off先全core gate，on仅已实现的美化另验。
5. 以不同worker到达次序/HashSeed重复验证记录和产物按原始identity关联、无重无漏、每份实际source合法且spans指向本份文本；不要求生成文本/局部名/声明/诊断序列byte-identical，不为测试加入canonical sort。不同合法调度触发不同预算结果如实分类，但不得因此错语义；不为本卡刻意新建并行。
6. 记录release耗时/峰值内存方法而不是debug断言快；代表input授权后optin才运行，环境/skip明确。不做promised百分比缩减。
7. 套用本仓workspacegates、各crate许可来源和self-contained约束；本卡只验收内部EngineCoreRC，root CLI替换由H05/H08验收，MCP暂不设计。

**不变量 / 限额**：超限Resource不是malformed，totalrequestlimits覆盖多个method，smalllimit不可输出截断“完整Java”；fuzzcase有timeout/seed/range避免unbounded运行。

**测试**：F25 `HardeningMatrix`：所有M/U/Fmanifest登记，实际产物失效、CancelvsFallback、near-boundary分配与reportspans；结果排列/合成ID变化时按原identity核对，不作跨运行全文本相等。knownunsupportedextensions仅raw且有version限制。

**验收**：G0、公开核心t25_行为、固定D001–D065 engine-seam的Java/raw/input/resource/041证据及根四gate。应用domain/extract不成为engine dev依赖，统一由H08验收完整70。required成员不随实现状态变化，运行命令见TASKS §4。

**完成**：可追溯EngineCoreRC报告，其D001–D065核心义务全部按固定预期验收；总70分母仍在产品总账，无empty/skip或Java→Smali改答案；若反例未修，能力保持Unsupported但该requiredcase仍未通过，不能宣布CoreRC。

**不扩展**：生产引擎切换、版本tag/release、自动私有corpus下载、未经测量的cache/parallel、用normalizer修comparison分数。

## 6. 应用任务 H01–H10

## H01 — 产品命名与领域合同冻结

**依赖**：B04。**范围**：Diana应用品牌/CLI/内部crate定位，无MCP新设计。

**文件**：现有 `apps/diana/Cargo.toml`/`src/main.rs` 骨架与未来 `apps/diana/src/lib.rs`、`src/cli/{args,mod}.rs`（均相对 Diana 应用根）；更新README/AGENTS产品说明。旧 rasc 已整体移到 ref-fork，仅参考，不构建/测试；历史release/许可不全局替换；本卡不创建CLI测试文件。

**输入→输出 / Interface**：应用 package/binary `diana`（`apps/diana/`）；内部package`diana-decomp`、path`crates/diana-decomp/`、import`diana_decomp`；原CLI领域/flags/退出码及结果完整性形成基线。输出顺序明确不再保证，独立任务结果按完成/到达顺序发布。rename不代表新算法已替换。

**步骤**：
1. 登记所有live名字消费者：clapname、crateimports、测试binaryenv、skill、releaseartifact/版本断言；MCP相关只登记延期/必要编译引用，不设计servername/schema迁移。
2. 对照现有源码与DESIGN.md核对命令/字段/输出通道与迁移清单，不运行CLI黑盒或建立help/格式golden。旧展示顺序与分页接口有意取消；原始身份、解码完整性和文件安全等复杂领域义务由后续module测试承担。
3. 在 apps/diana/入口与内部crate骨架上实现新产品，根已是 Diana-only workspace；旧 rasc 在 ref-fork，不把其 CLI 当别名/依赖。H08 仍须产品能力证据，旧发布工具已归档，Diana 发布政策独立审阅；历史来源不全替换。
4. 按DESIGN.md冻结11个命令和指定TSV/JSON列、规范at、完整method-selector、默认Java与显式JSONreport；旧getclass/findrefs/skill、class-id/memberprefix/require-java不保留alias。新增fields/methods/xrefsites/extract分别由H09/H10实现，不以旧接口限制新设计。

**不变量 / 限额**：活动 workspace 只含 Diana，旧 rasc 完整归档且不构建/测试；Diana 不转发旧 CLI/引擎；无JVM/Python生产runtime；原版权/来源不改。没有执行输出时不能自编基线数字。

**测试**：不新增自动化测试。Q01是命名/基础接口的人工审阅清单，不是测试族；帮助、flags、列名、alias和分页移除与DESIGN.md对照，不写CLI黑盒或golden。复杂身份/解码/文件安全由后续领域任务测试。

**验收**：格式/lint/构建检查 + Q01人工审阅记录，无h01_测试filter。**完成**：包名/十一命令合同与实现一致，未实施能力标明，不要求基础CLI测试。

**不扩展**：MCPschema/server身份迁移、重写archive/query、创建release/tag、改变版本、全局stringsreplace。

## H02 — 安全输入与 CLI request scope

**依赖**：H01。**范围**：root输入生命周期。

**文件**：`src/analysis/{archive,apk}.rs`，参考`src/analysis/session/mod.rs`的snapshot/AllowedRoot；按需新增`src/analysis/input.rs`、`request.rs`及复杂领域测试`tests/product_contracts.rs`，不强行复用带协议依赖的session类型。

**输入→输出 / Interface**：普通path→`InputLease`（私有不可变字节owner、inputscope身份、受限entry元数据）→生命周期内borrowedviews。CLI不新增open/close/status命令，lease在请求结束释放。

**步骤**：
1. 复用boundedcopy/regularfile/check-before-after思路；明确pathallowed/符号链接策略，允许正常文件但拒绝非普通设备/FIFO。
2. 读取/分配/映射前核input与directory预算；源码在snapshot创建后被截断/替换不会影响lease。
3. CLI统一不可变快照可能新增磁盘复制成本，记录意图与release测量，不称性能字节全兼容。
4. 提供scope内`BytesSource`/borrowedentry，使用安全Rust的owned snapshot或已审阅依赖的safe interface，借用不逃出owner。**活动两包均`unsafe_code=forbid`**，不在输入module开例外；需要unsafe调用的mmap方案不采用。H02/H10所需平台文件primitive若无满足合同的safe interface，先审阅依赖/平台支持决策，不用竞态窗口或放宽安全合同替代。
5. 请求终止/输出错误drop所有临时文件owner，不误删用户输入。

**不变量 / 限额**：len/offsetchecked，复制变化为INPUT_CHANGED；no unsafe `'static`延长；临时文件scope清理，不跨request伪缓存。

**测试**：Q02普通文件/裸DEX/存储ZIP；反例截断竞态、symlinkescape（有限root模式）、FIFO、超input/dir、owner提前drop；测试临时目录不碰用户真实文件。

**验收**：PG + `cargo test -p diana --test product_contracts h02_`。**完成**：source改动隔离、cleanup正确、预算前置与borrow边界可测；成本记录不编造收益。

**不扩展**：长期target协议、MCPopen/schema、全平台文件数据库、为快照重写ZIPcodec。

## H03 — 轻量查询、manifest 与完成顺序输出

**依赖**：H02。**范围**：CLI已有非反编译能力。

**文件**：`src/analysis/{apk,archive,query,manifest}.rs`、`src/analysis/dex/`、`src/cli/mod.rs`；既有scanner/unitfixture可复用，vendoredAXML修改需先PATCHES。

**输入→输出 / Interface**：InputLease→classes/strings/entries/manifest及供H09复用的符号/引用扫描基础，不经过Engine/SSA。DESIGN.md指定列是新输出合同，旧scanner只复用读取/查找算法，不复制旧findrefs方法级结果作为xrefsite。

**步骤**：
1. 依H01基线接新scope，保持rootDEX选择、重复entry规则、最后manifest选择和真实physical/logical/row身份；展示次序不保证，不能按最先完成选择manifest或重复类定义。
2. 保持ASCII/rawsubstring预筛与非ASCIIfallback、exact/fuzzydomainquery；展示string可与engine精确UTF16literal表示不同，必须明确各合同。
3. worker按完成/到达顺序提交row/batch，移除按entry ordinal等待、全量排序、reorder buffer和仅为保持输入顺序的领先窗口。保留资源加权admission、有界byte/row队列、背压/取消；no无界parcollect。不同threads只需完整记录多重集合一致，不要求序列一致。
4. filter保留匹配语义，返回全部匹配结果；删除用户分页参数与对应skip/take、截取计数、分页早停逻辑，不用cursor/preview替代。保留错误/取消/安全预算停止及内部有界batch（batch不是用户分页）。storedentry借用，deflate按需解压；查询不建立fullProgramIndex或反编译IR。
5. 回归字段/诊断类别/exit；不为旧展示序列加sort/stable mode，记录这一有意变化并测量release成本，不编造无序必然加速。

**不变量 / 限额**：同输入定义/引用索引对应同entry，成功查询返回全部匹配记录、不重不漏；noPython/外部JVM；query预算与engine预算不混用。移除用户数量截取不移除安全限额；超预算必须错误，不能只返回前N条并标成功。

**测试**：Q03通过公开查询module interface验证多DEX/重复entry/重复manifest、UTF16匹配、并发记录完整性与资源错误。受控延迟生产者核对无按序等待且buffer有界，按原identity比较内容；不测CLIflag、表头、基础filter路由、空列表打印，亦不assert私有phase调用次数。

**验收**：PG + `cargo test -p diana --test product_contracts h03_` + Diana公开领域interface的输入/解压回归。旧selfcontained/inflatepolicy仅供审阅问题，不运行ref-fork测试，不把其通过数计入验收。**完成**：按新CLI指定列返回全部匹配记录；读取/匹配/原身份完整性保持，输出序和用户分页有意取消；慢entry不阻塞无关结果，快路径无需新engine。

**不扩展**：合并全部DEXparsers、MCP通用分页协议、重写AXML、测量debugbuild后声称加速。

## H04 — 原始 DexImage、041 与定义身份 adapter

**依赖**：H03、T04。**范围**：root→internalengine输入interface。

**文件**：`src/analysis/dex/container.rs`、root`analysis`的selectedclass/engineadapter；T04已提供的原生041输入interface与origin/keys；本卡不再承担engine解析器实现。

**输入→输出 / Interface**：`SelectedClass{scope,entry,logical,classdef,descriptor}`→`Vec<DexImage<'a>>`、ClassKey/MethodSelector与有界DeclarationCatalog；结果identity映射回root原输入。

**步骤**：
1. 按DESIGN.md解析class-at/method-at/field-at/string-at，核physical/logical/class-def/member-index与descriptor；get-class可读名字遇多定义明确ambiguous，不能发入engine又按name任意选。非法at语法/失效索引区分，裸DEX映射e0:l0。
2. 使用T04的原生041读取；只将原container借用及logical header_offset传入，原ImageOrigin映射物理entry/logical member。不得在本卡overlay或复制container给每个member。
3. P1/Engine已按DESIGN §3采用borrowed view；H02 lease先取得必要bytes，同scope创建RunContext并调用open/decompile/render，Report为owned结果，不返回临时IR借用。
4. 依赖declarations用scanner精确定位，在dependencybyte/metadatabudget内生成facts，完整性不够Unknown；不自动解析全APK或信任scanner宽松校验为引擎validator。
5. fields/methods输出at与get-class/get-method选择来自同原始SelectedClass/MemberRef；原runtimeindexes不随Java重命名变，无--members表格前缀。

**不变量 / 限额**：containeroffset/原site/UTF8span坐标不混；native041是输入支持，不等于所有新版opcode支持；超原rawprofile明确Unsupported不overlay蒙混。

**测试**：Q04通过公开输入/定义解析interface验证041多个logicalmembers、重复definition/physicalentry、跨输入过期identity和descriptor/index不一致；核对原definition/proto与未经修改的raw字节。不测CLI选择器参数语法或字符串格式化。

**验收**：PG + `cargo test -p diana --test product_contracts h04_` +引擎T02–T04回归。**完成**：borrow/identity/native041及rawfallback完整，不能以scanner能列出等同engine已支持。

**不扩展**：生产parser全量缓存、永久跨文件ID、切默认旧engine、MCPclass-id返回新schema。

## H05 — CLI 接线与公开反编译接口的语义闭环

**依赖**：H04、T06。**范围**：早期CLI接线，不等全engine完成。

**文件**：root`analysis`新engineadapter、`src/cli/{args,mod}.rs`、`tests/product_contracts.rs`、相关fixtures。

**输入→输出 / Interface**：`get-class/get-method`规范selector→Engine Report→默认Java text或显式JSONdomain projection。默认不输出Mixed/Smali；显式JSON完整report才携语言状态。早期实现profile明确，最终无旧命令alias/长期双engine。

**步骤**：
1. CLI定位class/exactmethod/proto后borrowedengine执行T06fixture，callback/control由scope提供。
2. singlemethodsource的knowncontext验证遵循TASKS §4，不把Mixedclass称JavaUnit；暂不支持方法保留raw与语言marker。
3. 默认text的get-class须VerifiedJavaUnit，get-method须Java或DeclarationOnly且是完整声明/body；无法满足exit1且无stdout/输出文件，不提供--require-java。显式JSON允许标记清楚的完整Mixed/raw报告，requesterror/BUG仍失败。
4. 根据DESIGN.md保存完整method owner/name/proto或method-at，textdiagnosticsstderr、同次stdout/文件payload一致；JSON不serialize私有arena，不设计MCPenvelope。
5. 旧实现只保留在ref-fork供阅读，不留活动旧入口/依赖或双引擎路由。Diana请求禁止静默fallback到dexdec，失败只按原rawsmali合同。

**不变量 / 限额**：请求来源/方法proto精确、outputstatus真实、无stdio污染；reject路径不能凑stub通过。

**测试**：Q05通过公开反编译interface运行F05→恢复Java→javac→行为Driver，验证实际返回/语义、原始身份与降级完整性。复杂overload/跨DEX在F10/Q04覆盖；不启动get-class/get-method进程，不测flags、默认stdout/JSON包装或not-found文本。

**验收**：PG + `cargo test -p diana --test product_contracts h05_` +引擎B(F05)。CLI接线做源码审阅，不加黑盒测试。**完成**：新module真实语义闭环通过，CLI已调用该interface，不仅链接新crate。

**不扩展**：新增MCPgetclass/tool、抛异常stub、未验证shape强行Java、立即删除所有旧依赖。

## H06 — 请求取消、分层预算和输出发布

**依赖**：H05、T09。**范围**：CLI/control/engine输出。

**文件**：rootinput/request/engineadapter/CLIemit；`crates/diana-decomp/src/pipeline/control.rs`；测试privateinput/interrupt/backpressure。

**输入→输出 / Interface**：callerstop/UserInterrupt→RunContext→pipelinecheckpoints；perrequestbyte/work/outputlimits→同一个RunContext；完整text/JSON或明确Resource/Cancel错误。

**步骤**：
1. 取消callback不引入tokio到engine；每decode/worklist/copy/region/emitcheckpoint检查，Cancelled不继续rawfallback。
2. 用户文件/scope结束后不发过期结果；同请求cacheloader/reader的owner/reservation释放，失败无半缓存，nohardtimeout承诺。
3. 创建RunContext时把Limits clamp到scope剩余额度；dependency/main借用按同一物理owner去重计输入，派生数据、所有methods和render累计收费。raw预留只从父配额领取，不能另发一份request预算。
4. 输出分text与JSON精确有界writer，escape/diagnostics/spans计最终字节；JSON超限无截断prefixsuccess，file用临时输出原子发布策略（最终路径替换语义先测平台）。
5. CLIstreamqueries的brokenpipe停止producer/背压；不要把SIGPIPEkill进程称catchable取消保证。

**不变量 / 限额**：取消协作式；libdeflate不可抢占明确；input64MiBengine限制与256MiBinflate限制各自Resource；没有MCPsemaphore/schema新合同。

**测试**：Q06通过公开请求/反编译/提取interface验证长工作取消、owner生命周期、normal/raw预算隔离、大量转义文本的输出字节预算、spans对应实际source、缓存失败reservation、consumer关闭时队列有界。不是简单JSON字段包装或私有phase调用顺序测试；每case有有限timeout。

**验收**：PG + `cargo test -p diana --test product_contracts h06_` +T09/T25control回归。**完成**：exactoutputlimit和取消state有非空执行证据；不能先构造巨String才声称allocationbudget前置。

**不扩展**：MCPclose/status/permit设计、crash隔离子进程、任意实时抢占、跨进程缓存服务。

## H07 — 十一个命令的统一帮助、输出与 agent 文档

**依赖**：H06、H09、H10。**范围**：CLI消费者与使用文本。

**文件**：`src/cli/{args,mod,agent_skill}.rs`、`skill/SKILL.md`、README；历史来源保留。本卡不新增CLItests、help快照或参数路由测试。

**输入→输出 / Interface**：DESIGN.md的11命令、指定列、规范at、默认Java、显式JSON、extract例外正式冻结；agent使用说明作为普通文档/发行资产，不提供公开skill安装命令。MCP仍延期。

**步骤**：
1. 帮助恰含get-class/entries/extract/strings/xref/manifest/classes/get-method/fields/methods/help及指定说明/列；TSVheader/escaping与JSONdomain fields按DESIGN.md，不加旧memberprefix。完整method selector包含name+proto，不再--method/--proto分开组合；at可回传。
2. 默认Java失败exit1、usage2、唯一对象缺失3/歧义4，查询空集合0、显式JSON完整Mixed report0；requestBUG/Resource/Cancel失败。旧getclass/findrefs/skill/require-java/class-id/members/paginationflags明确拒绝。
3. agent文档改用Diana新命令，说明字段/at/JSON、显式外部d8一次转换及复用；不自动移动/删除用户已安装rascskill，不加第十二个skill命令。
4. stdout/file/help/version/Windowsbinarytarget完整；JSON字段version是本地CLIreport，不据此设计未来MCP。
5. 所有liveCLI品牌消费者改名清单核对；legacyrelease/provenance不是remainingbug。

**不变量 / 限额**：unknownflag不可静默忽略；默认source不混Smali，JSONstatus按typedreport；TSV字段控制字符/孤立surrogate转义不丢值，at与spans准确；extract二进制只写-o，不能依通用stdout复制规则污染。

**测试**：不新增自动化测试。Q07是DESIGN.md/源码/agent文档的一致性审阅清单，不是测试族；help、路由、flags、列名、旧alias、默认包装不写黑盒/golden。复杂UTF16保真、身份、spans与提取安全由F03/Q04/Q06/Q09/Q10测试，不重复测基础排版。

**验收**：格式/lint/构建检查 + Q07人工审阅，无h07_测试filter。旧测试保存在 ref-fork，不删除但不进入活动 gate。**完成**：十一命令接线与文档一致，复杂领域验证由对应任务提供，MCP不设计。

**不扩展**：MCPinput/outputschema、自动改用户agent配置、全局旧品牌license/historyrewrite、发布。

## H08 — CLIProductRC 与旧代码退役清单

**依赖**：H07、T25、T14。**范围**：整体验收与发行配置准备，MCP仍延期。

**文件**：活动 rootCargo/lock、未来 Diana release 配置/README/AGENTS、应用 query/engineadapter、`apps/diana/tests/product_contracts.rs`；核对无 ref-fork 依赖，不维护旧MCP构建。

**输入→输出 / Interface**：EngineCoreRC+H01–H07证据→CLIProductRC报告、livecallgraph/依赖退役清单、单diana发行配置。验收无序输出的完整性/身份，不要求跨运行source或报告序列一致；本卡不创建tag或上传release。

**步骤**：
1. DESIGN.md命令/列/路由由源码和文档审阅核对，不新增黑盒；复杂defined-vs-referenced、xrefsite完整性、extract原bytes/CRC/file安全、跨DEX/041身份与引擎行为由Q02–Q06/Q09/Q10和F测试验证。Q01/Q07仅人工清单，Q08汇总，不当执行测试族。
2. 核查活动 CLI/依赖图无旧 dexdec/MCP/helper 或 ref-fork 调用；旧代码完整保存在参考中，不需编译/跑测试，不为 MCP 新设计 adapter。
3. ref-fork 不作为生产依赖或 gate，保留原源码/测试/许可/历史，不以当前参考目录存在当作第二发行产品或必须测试的遗留依赖。
4. 独立审阅未来 Diana releasebinary/version/archive/checksum/Windows 配置；旧 workflow/scripts 已归档不复活，历史 release 不删除，不手工跳版本/发布。
5. 运行仅 Diana 的 PG 与复杂 H/引擎 CoreRC gate，基础 H01/H07 只审阅，不造空测试；授权代表输入的 release 测量明确 not-run，不运行旧 MCP 回归或发布测试。
6. 列knownUnsupported/rawcoverage与MCP延期说明：不把CLIProductRC说成全部projectinterfaces已迁移。

**不变量 / 限额**：无静默跨旧enginefallback；singlebinary，不改版本/tag；活动代码须验证；旧参考不参与本分支 gate；不宣称全DEX语义正确。

**测试**：Q08汇总复杂语义/领域测试：跨DEX/041身份、输入/提取/取消/预算，以及T25的行为和降级完整性。不建立整个CLI命令矩阵或branding/help/flags黑盒；CLI接线、旧依赖和发行配置采用源码审阅/本地构建检查。

**验收**：PG + Q02–Q06/Q09/Q10的实际复杂测试与T25核心gate（非零运行）；Q01/Q07人工核对、Q08汇总，无空h08_测试要求。releaseartifact本地检查，不发布。**完成**：完整D001–D070按固定expected验收，复用T25的D001–D065 engine证据，补齐D065应用映射及D066–D070领域证据；无删除/skip或core-Java改Smali。MCP延期，基础接口不另建自动化测试。

**不扩展**：MCP迁移、直接删除src/mcp、发行/tag、自动下载私有语料、无证据cache/parallel优化。

## H09 — 定义符号清单与跨 DEX 引用 site

**依赖**：H04。**范围**：fields/methods/xref与指定列，不运行反编译器。

**文件**：root `src/analysis/dex/{mod,members,opcodes}.rs`、`src/analysis/query.rs`、`src/cli/{args,mod}.rs`，按需新增`symbols.rs`/`references.rs`；`tests/product_contracts.rs`的Q09。

**输入→输出 / Interface**：每个class_data实际字段/方法定义→field-at/method-at单列；每个实际opcode引用site→`entry, context, match`三列。methods不是全部method_ids references，xref不是旧findrefs的一方法一行；DESIGN.md是唯一字段/选择器规范。

**步骤**：
1. 遍历class_defs/class_data，分别读static/instance fields、direct/virtual methods的checked delta index与flags/codeoff；生成携class-def/member-id/owner/name/proto的规范at，abstract/native/constructor/clinit声明也列。
2. fields/methods `--class`精确选择原定义，`--filter`按语义名称/descriptor字面匹配，输出全部匹配；at与重复classdef核验，不靠排序/位置identity。
3. xref kind string/type/method/field精确解析target，扫描全部rootDEX/logicalmembers的原code，读取opcode operand/refindex，不从恢复Java字符串搜索。记录referencing MethodKey、code-unit offset与actual matched value/ref。
4. context拼method-at加`@cu:0x...`，同方法多个真实site各列；对同site多个operand引用按明确operand-site身份建记录，禁止沿用方法级去重掩盖site。
5. 保持035与已有041扫描格式边界，knownunsupported opcode/profile按原安全合同处理；只为资源背压保留batch，按完成输出，不global sort或分页。
6. 按DESIGN.md输出TSVheader/escaping或JSON items；字符串真实值UTF16精确与root原scanner展示差异要测，不能replacement后做精确xref。

**不变量 / 限额**：defined与referenced、string pool与const-string site分开；每row原身份明确；所有member/count/code/refindex checked，输出/match scratch有界。成功返回全部匹配项、不重不漏，empty正常；不是原数值index相同就跨DEX合并。

**测试**：Q09通过公开符号/引用module interface验证多DEX同名/同proto、重复classdef、外部method_id不算定义、同方法多个xrefsites、未使用stringpool项、孤立surrogate、非法delta/refindex。核对原身份/site/UTF16值，不测试columns、命令参数、JSONitems排版或简单getter。

**验收**：PG + `cargo test -p diana --test product_contracts h09_` +H03/H04回归；非零Q09按原identity/site集合核对。**完成**：fields/methods/xref实际CLI接线，definitions与sites精确，不只是新增help命令。

**不扩展**：模糊regex/裸方法名猜重载、全量DEX对象模型、Java源码xref、MCP工具、分页、新增未经要求的默认列。

## H10 — extract：原始 entry bytes 与安全原子文件

**依赖**：H02。**范围**：单physicalentry提取，无批量解包。

**文件**：`src/analysis/archive.rs`、rootinput/requestscope、`src/cli/{args,mod}.rs`，按需`src/analysis/extract.rs`；Q10producttests。若为CRC加入新依赖，需单独理由/许可/lock变更，禁止未经检查的手写宽松CRC。

**输入→输出 / Interface**：APK/ZIP+exactentryname（可`--entry-index`）+必需`-o`→完整解压payload文件。method0字节不改，method8解压原bytes，不normalizeDEX/切041/生成Java，stdout空。

**步骤**：
1. 目录查找保留所有同名physicalrecords，名字歧义exit4；entry-index和name必须核一致，basename/路径规范化不能改变原名选错记录；bareDEX不当ZIPentry。
2. 先检查压缩方法/加密flag、local/central ranges、declared/actualsize与原CRC。现有codec是否校CRC先查源码测试，不假定它完整；缺必需校验补齐后才标extract成功。
3. 只写用户指定文件，entryname不参与路径拼接。拒绝输出目录、符号链接、输入文件本人/hardlink别名；no-force不覆盖。校验与文件打开/发布需使用安全原子primitive，避免exists检查后rename覆盖竞态。
4. 在目标目录创建私有临时文件，有界借用/解压后写入，完整验证才publish；取消/I/O/CRC/超预算释放临时文件和reservation。`--force`只显式替换允许的普通文件，Windows/Unix语义分别验收。
5. 字节流不经UTF8/JSON转换，no stdoutbinary或成功状态行；debug/errors仅stderr。采用独立binaryfileoutputadapter，不误用通用emit同时stdout/file。

**不变量 / 限额**：输出与原entry完整解压payload逐字相等，raw041header/checksum保持；不自动创建entryname目录树；不删除用户已有文件/输入；CRC/size失败无假成功或半文件，资源预算不是分页。

**测试**：Q10stored/deflated、含NUL及nonUTF8binary、041originalcontainer、duplicateentryname/indexmismatch、ZIPname`../x`仍只写选定-o、CRC/size坏、unsupportedcompression/encrypted、bomb/limit、cancel、输出存在/no-force/force/symlink/hardlink/input-alias、并发发布竞态。测试均自制临时文件，不触碰真实用户路径。

**验收**：PG + `cargo test -p diana --test product_contracts h10_` +input/archive回归；Q10通过公开提取interface核对原payload/hash、失败目标未变、无临时残留；不启动CLI测试stdout/-o路由。**完成**：原字节、CRC、竞态/alias/限额安全证据成立，不是仅解压到Vec。

**不扩展**：batch-unzip/entry路径自动写盘、压缩流/ZIPlocalheader导出、DEXnormalize、CLI外部进程、MCPextract或release。
