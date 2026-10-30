# Diana 验证、fixture 与验收合同

状态：**内部diana-decomp验证设计，尚未运行新引擎**。具体70个case设计见[E2E_CASES.md](E2E_CASES.md)/[e2e_cases.json](e2e_cases.json)。[TEST_PREPARATION.md](TEST_PREPARATION.md)的B01–B04先完成规范、可复现fixture/工具锁、基准与harness验收，再开始T01/H01；T05仅接真实库，不临时设计案例。团队路径/工具合同见[REPRODUCIBILITY.md](REPRODUCIBILITY.md)。整体Diana CLI以[CLI.md](CLI.md)的十一命令/列为唯一规范，H01–H10/Q01–Q10验证见[PROJECT_TASKS.md](PROJECT_TASKS.md)：包括get默认Java/显式JSON、定义字段/方法、site级xref、原entryextract字节与文件安全。MCP暂不设计、不作为本轮CLI产品验收。旧对话中的比较数字不作为本设计基线。

本页的核心原则：**javac 通过、文本一致、结构相似、测试运行成功是不同证据，不可互换；有限测试也不是任意程序语义等价证明。**

## 测试投入边界（最新约定）

不新增CLI黑盒测试，不测试help/version、参数解析/路由、命令alias/分页flag、列名或基础展示包装，不为它们维护golden。CLI.md是实现规格，这些简单接口通过代码/文档审阅和构建核对；现有测试本次不删除。

语义E2E走公开反编译库interface：自制DEX→实际恢复Java→编译/行为Driver、原raw→smali→组装/语义字段比对。优先复杂Phi/循环、primitive/ref/FP、调用/构造器、异常旧值/cleanup、monitor、初始化与降级；不耦合私有phase实现或mock内部调用。原始DEX/041身份、输入/提取文件安全和资源/取消另走公开领域module interface，不通过CLI子进程测试。

**测试先于开发**：B01–B04必须在T01/H01之前冻结D001–D070行为/inputs/expected/required suite、准备可复验source/fixture/toolchain/harness。实现期间只接薄公开interface adapter、逐固定case组转绿，不自动改expected、降为Smali成功或按当前能力删/skip失败case。真实oracle错误只允许独立证据和单独审阅修正；新反例可追加。不穷举全部DEX、不写简单CLI或未定私有artifact测试。

## 1. 分层验证与失败信号

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

## 2. 成功状态与计数分母

### 2.1 报告必须逐方法计数

统计以下互斥状态，并同时输出 denominator：

- `Java` executable methods（有 code，经过 P8 验证）及其 behavior-tested 子集。
- `DeclarationOnly`（abstract/native，无 body）——不能抬高 executable Java 恢复率。
- `Smali`（已校验 raw + 全部指令/规定 metadata）；与 `Unavailable` 分开。
- `Unavailable`（损坏/未知宽度/无法可靠 fallback/资源失败），诊断原因为 Malformed、Unsupported、BudgetExceeded、InternalInvariant。
- `JavaUnit` 与 `Mixed` class 数；单 inner 片段、缺 shell metadata、任意 fallback 不算完整 JavaUnit。
- RequestError 与 Cancelled 单列；BUG 必須为红色 gate 信号，即使能输出 raw。
- missing catalog、known unsupported instruction、exception profile、monitor profile、constructor profile 各原因数。

例如：`Java=12/20 executable, DeclarationOnly=3, Smali=5, Unavailable=3; JavaUnit=1/4; BehaviorVerified=10/12 in env X`，而不是“17/20 能输出就是 85% 成功”。无法真实编译的 Java 片段即便 P8 成功，也不能计为 V4 已通过。

### 2.2 冻结基线纪律

每次baseline包含：引擎commit、options/profile/schema、输入checksum、该请求catalog标识、toolchainversions、运行命令、按原identity关联的方法状态/trace、已测试输入集。不冻结查询/跨方法结果序列，不要求跨运行局部名或生成源码相同；text样本仅作诊断，source spans对本次真实输出逐项校验。复杂查询内容通过公开module核对原identity多重集合，不提供分页或数量截取参数；旧flag移除只做源码/文档核对，不新增CLI测试。安全预算不足通过领域interface验证明确失败，不前N条成功。行为变化仍须独立审阅。

参考 dexdec、radguard、DAD 输出只作 triage：相同可能同错，不同可能都是等价形态。不能冻结宽松 normalizer 后宣称真值；comparison 规则变更与引擎变更必须分别记录。本设计不复制 RG pickle/DEX fixtures，不运行未授权缓存，不让 fixture 名揭示真实 corpus 身份。

## 3. Fixture manifest 与工具合同

### 3.1 自制 fixture 基本结构

B02/B03在引擎实现前创建团队共享`tests/e2e/manifest.json`，T05/产品任务只读取并接入，不各自复制另一份expected。执行case用D001–D070，F/Q仅作能力映射。每项schema至少包括：

```json
{
  "id": "D029",
  "name": "ThrowState",
  "source": ["tests/e2e/sources/D029/Case.java", "tests/e2e/sources/D029/Dependencies.java"],
  "smali_source": [],
  "dex": "tests/e2e/inputs/D029/classes.dex",
  "dex_sha256": "schema-example-only: B03 seals the actual SHA-256 before engine development",
  "dex_version": "035",
  "compile": {"javac_release": "8", "d8_min_api": 21},
  "toolchain_lock": "tests/e2e/toolchain.lock.json",
  "environment": {"catalog": "tests/e2e/sources/D029/declarations.json", "classpath_recipe": "self-made-sources"},
  "request": {"class": "Lfixtures/D029/Case;", "methods": ["test()I"]},
  "validation_scope": "method-in-known-context",
  "expected_file": "tests/e2e/expected/D029.json",
  "expected_language": "java",
  "checks": ["V0", "V1", "V2", "V3", "V4", "V5"],
  "driver": {"cases": "tests/e2e/sources/D029/driver_cases.json", "timeout_ms": 5000},
  "preparation": "inputs-certified",
  "license": "self-made; contributor-authorized"
}
```

上面只是schema示意，不是可执行manifest。**B02/B03（引擎实现之前）**必须填写真实tool/fixturehash并审阅独立expected；占位hash或未锁传递依赖阻止准备验收。签入binary只能小型自制且与source/recipe对应；.gitignore仅局部审阅允许fixture，不放开整个corpus。引擎Agent不可根据actualoutput重写manifest/expected。

Dex version 必须从实际 header 读取确认；d8 的 min-api 或 javac-release 不单独保证 version/opcode 范围。基础 profile 035；T25 的 newer version/raw 格式另登记，未登记不能假设接受。

### 3.2 工具可复现获取，不依赖本地SDK

按[REPRODUCIBILITY.md](REPRODUCIBILITY.md)固定公开artifact：Temurin21.0.8+9、R8/D8 8.3.37、smali/dexlib2 2.5.2；B02锁所有JAR传递依赖/OSJDK资产URL和SHA-256，B03真实验证，不只“作者机器上有d8”。工具放checkout`.cache/e2e/tools/`，fixture/依赖源在`tests/e2e/`；核心suite不require AndroidSDK/platformjar/兄弟仓库或私有cache。

准备器未来interface：`python3 tools/e2e.py fetch-tools`显式联网按lock验证；`prepare/verify-inputs/verify-harness`离线读已锁工具和签入source，不扫描home、不auto-download。B04验收通过才进入T01/H01。runner以参数数组调用`com.android.tools.r8.D8`/`org.jf.smali.Main`与锁classpath，不假设裸smali.jar可`java -jar`执行。子进程只测试工具，生产不引用。

测试默认 G0 运行 checked-in 自制 raw/contract fixture，不 require JVM/d8。**显式启用** roundtrip/assembly 时，missing tool/fixture/classpath、非零 exit、超时、0-selected-cases、unsupported checksum 都必须 fail。未启用可记录 not-run，但不能计 V4/V5/V6 已通过。

### 3.3 开发级完整命令示例

这些是B02/B03先验证、T05接入真实库的命令合同，不是本轮已执行的工具成绩。`${TOOL_JAVA}`/`${TOOL_JAVAC}`/`${R8_CP}`等由锁定工具准备器解析成checkout-relative运行参数，不是让每个开发者填写个人绝对路径；下面shell仅作Unix示意，实际runner跨平台组classpath：

```sh
# 每次在新临时目录工作，不共享原始/恢复 classfiles。
"${TOOL_JAVAC}" --release 8 -d .cache/e2e/original-classes tests/e2e/sources/D009/Case.java
mkdir -p .cache/e2e/dex-original
"${TOOL_JAVA}" -cp "${R8_CP}" com.android.tools.r8.D8 --min-api 21 --output .cache/e2e/dex-original .cache/e2e/original-classes/fixtures/D009/Case.class

# Rust tests/support 调用 Engine，并保存 JavaUnit 的 package/name 匹配文件；
# Method-only/Mixed 不可直接写成 Case.java 冒充完整 compilation unit。
# 内部库开发runner在T05定义；生产get-class/get-method由H05/H07按CLI.md接线。

"${TOOL_JAVAC}" --release 8 -cp "${SELFMADE_CP}" -d .cache/e2e/recovered-classes .cache/e2e/recovered-source/fixtures/D009/Case.java
"${TOOL_JAVA}" -cp "${ORIGINAL_DRIVER_CP}" fixtures.Driver D009
"${TOOL_JAVA}" -cp "${RECOVERED_DRIVER_CP}" fixtures.Driver D009
# harness 比较结构化 JSON 轨迹；shell 中的 stdout 和 $? 仅展示，不替代 harness。
```

**方法级 context-spliced 验收**（例如 T06 的 F05）：允许在已知自制源码的合法 owner 中，仅替换目标方法的完整声明/body，其余 constructor、依赖和成员保留原始自制源码。harness 必须按明确的 method/proto 定位替换，不靠宽松 regex 匹配；不得修改目标方法或生成 stub。manifest 增加 `validation_scope: "method-in-known-context"`、目标 MethodSelector 与 context checksum；该测试只计目标方法 V4/V5，不计 Diana 整个 class 为 JavaUnit。完整 class 验收用 `validation_scope: "full-unit"`，必须直接编译 Diana 的全部输出，不 splice 修补。

原始class含多个文件时用显式classfile列表或self-made jar输入d8；不是将目录直接传给d8。需要库声明/desugaring时显式`--lib`及所有依赖，由manifest记录；不要默认平台API缺失warning无害。annotation/enum/neweropcodecase用自己的toolchainprofile。

跨平台 classpath 由 Rust `std::env::join_paths` 生成，以上冒号是Unix示例，不复制到Windows实现。driver运行目录/变量隔离、不共享静态状态；clinitcase每个输入新JVM防初始化缓存。编译错误不得 grep/filter “expected missing package”转绿；提供依赖或标not-run。

G0 / B / S 实际驱动命令：

```sh
cargo fmt --all -- --check
cargo clippy -p diana-decomp --all-targets -- -D warnings
cargo test -p diana-decomp --all-targets
# 不允许 0 个用例匹配：
DIANA_DEV_ROUNDTRIP=1 DIANA_CASE=D009 cargo test -p diana-decomp --test contracts behavior_roundtrip -- --nocapture
DIANA_DEV_SMALI=1 DIANA_CASE=D061 cargo test -p diana-decomp --test contracts smali_roundtrip -- --nocapture
# 固定B01/B04 suite成员，不能按引擎当前支持情况动态删除required案例：
DIANA_DEV_ROUNDTRIP=1 DIANA_CASE=core_supported cargo test -p diana-decomp --test contracts behavior_roundtrip -- --nocapture
DIANA_DEV_SMALI=1 DIANA_CASE=raw_supported cargo test -p diana-decomp --test contracts smali_roundtrip -- --nocapture
```

`core_supported/raw_supported`改为B01/B04固定suite成员选择器，**不得按引擎当前支持状态过滤/移除requiredcase**。实现阶段可以显式选固定D编号子集运行，全core suite仍报告未实现/未通过分母，emptyselector失败。下面F选择器若保留仅作为D案例映射，不是另一份可变expected；准备规范/fixture/tool锁早于T01，T05只接adapter。

## 4. 行为 Driver 的精确定义

### 4.1 Driver 输出结构

本项目不保证分析结果稳定顺序，但**被分析程序的执行轨迹不是展示顺序**。driver中的event seq、调用参数求值、异常/catch、cleanup、锁和初始化顺序必须逐项保留，不能按事件kind排序或当多重集合比较。独立方法/查询报告才按原identity关联且忽略展示次序。

比较规范字段的JSON结构与语义event序列，不比较打印Java文本或JSON对象键排列。每 case 独立执行原始自制源码与恢复源码的相同public入口，依赖Driver/Logger与catalog版本一致。

```json
{
  "case": "F16-old-value-on-throw",
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

## 5. 能力族映射与固定案例

**执行规范是[e2e_cases.json](e2e_cases.json)中D001–D070，完整具体inputs/expected/oracle先由B01–B04审阅、准备、认证。** 下列F00–F25/U/M只是能力族和反例命名映射，不是让实现Agent自行补expected或按当前能力删case；映射见[E2E_CASES.md](E2E_CASES.md)。

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
| F25 HardeningMatrix | 所有M/U、smallbudget、cancel、registry/schedule/spans、known038/039rawextension | 基线支持/状态一致、无panic/0tests、BUGgate红 | T25；V0–V7 |

F07/F08手写smali部分必须在manifest指出referenceinterpreter/ART/JVM实际证明范围；F17sharedhandler若未实现ownershipprofile则作为U03式降级case，不以它不Java而否定第一档T18，亦不列为其positiveJava支持。

## 6. 几个必须具体写出的反例

### 6.1 Phi cycle（不要“先看起来写两个赋值”）

输入 edge copy：`a <- b, b <- a`，初值`a=10,b=20`。输出应`a=20,b=10`。错误序列`a=b;b=a`会得到20/20；V2必须独立执行copyplan捕获，而不只检查有一个temp。

### 6.2 Critical edge

`pred`两successors，只有`join`需要某Phi拷贝；另一arm调用`observe(oldLocal)`。将copy一律放pred末尾会改变非joinarm可见值。Expected：选join才执行copy，两armdriver都跑；mutation把copy提前应红。

### 6.3 异常写回前状态

自制支持profile：`v=7; try { v=Producer.alwaysThrows(); } catch(ExpectedError e) { Logger.old(v); }`。原始DEXthrowedge用7；正常result即使写同physicalreg也不commit到exceptionstate。snapshot在call后或catch默认0都错误。catch里面抛另外Error，outerhandler应该收到对应old state，不得沿用上一次site影子槽。

### 6.4 不扩大 try 范围

原try只保护`Producer()`，normalcleanup `Cleanup.mayThrow()`在其外；若恢复成包住cleanup的try，cleanup的exception会被原handler多捕获。原expectedeventprefix与exceptionclass明确；这是“原样重复cleanup”是否真的保持语义的硬反例。

### 6.5 新增 cast 不等于源码合法化成功

两个真实Ref值join到Object，某use因不完整catalog被猜为NarrowType；为了javac插`(NarrowType)`会额外ClassCastException。V4静态合法可能通过，V5真实driver与no-extra-check证书必须拒绝此转换。

### 6.6 Monitor 不是空try糖

锁对象先求值、monitorenter成功后call在锁内，throw也释放。错误恢复“空try/catch+后面call”会让holdsLock=false；把lock重新取一次可换对象。V3lockidentity和all-exitrelease证明加V5具体driver，不用“未死锁”当充分证据。

### 6.7 Java enum / constructor source restrictions

`class E extends Enum`即使字节码可类比，javac不允许。pre-superthisfieldwrite/monitor/arbitrarybranch不是“helper都能解决”；成功profile只接受已明确表述的窄case。negative必须降级保留raw，不能删除原super/field/lock以通过编译。

### 6.8 初值不可观察与格式安全

Unknown/Undefined register不是null，badUTF-16不是replacementchar，truncatedoperand不是0。mutation分别覆盖三条曾经宽松成功的路径；V0/V1/V2拒绝后Unavailable不能计Smali完整。

## 7. 需求—任务—验证覆盖矩阵

| Requirement | 设计定位 | 任务 | 证据 / 完成条件 |
|---|---|---|---|
| R01 从零Kuna骨架且只借必要机制 | DESIGN §3/4/10；MODULES §1/5 | T01/T25 | actualscheduletrace==registry、requiredpass不可禁；无largeundeclared迁入 |
| R02 原生安全input与rawfallback | DESIGN §2/5/10；MODULES §6 | T02–T05/T25 | V0/V6，Malformed/unknownwidth不fake-smali，取消不走fallback |
| R03 请求内ID/op/edgecoverage | DESIGN §5；MODULES §2 | T03/T06/T07/T09/T25 | 原始site→op/edge→AST/spans连续、lost/dup拒绝；不要求跨运行合成ID/文本相同 |
| R04 SSA/Phi/normalcopy | DESIGN §5.3/7；MODULES §2.3/2.4 | T07–T09 | V1/V2；cycle/criticaledge/loop/badundefined |
| R05 primitive/ref/null/types | DESIGN §6.1；MODULES §4 | T06/T10/T15 | V4/V5；NaN/shift/wide/refjoin/no-extra-cast |
| R06 callkind/proto/access/throws | DESIGN §6.2；MODULES §3 | T11/T21 | 完整selfmadecatalog真实callee/trace、Unknown单列fallback |
| R07 allocation/constructor | DESIGN §6.3/9 | T12/T20 | V4/V5，uninit/superorder/finalsinglewrite证据，复杂caseU02/U05 |
| R08 exceptionoldstate/catchdomain | DESIGN §5.2/7.3/8.3 | T16–T18 | V1/V2/V3/V5，F16/F17/F18与M14/M15/M16 |
| R09 monitor与同步 | DESIGN §8.4 | T19 | V3/V5，lockidentity/allreleases/holdsLock/null，U04 |
| R10 normalstructure与dispatcher | DESIGN §8.1/8.2 | T09/T13/T14 | V3/V4/V5，同profile两adapter/complexgraph；handlereligibility拒绝 |
| R11 classshell/init/generic/nested/enum/annotation | DESIGN §9 | T20–T23 | V4/V5/V7，metadata/runtime完整性，不能静态片段冒充unit |
| R12 输出状态、spans、诊断 | DESIGN §2/9；MODULES §4 | T01/T03/T05/T22/T25 | denominator正确、UTF8half-open有效、Mixed明确，BUG不计成功 |
| R13 保守默认与可选readability | DESIGN §1/10/11；RECOVERY_EXTENSIONS.md | T24可选/T25，E01–E08后续独立任务 | on/off证据、effectnegative、Declined保留baseline；不用textsame衡量语义 |
| R14 预算、终止、取消、失效 | DESIGN §10；MODULES §5/6 | 每卡/T25 | 前置allocationlimits、workstep内charge、repeatfinite、requesttotallimits |
| R15 原始参考源码与许可 | SOURCE_MAP §1–6 | 每次reuse/T01/T25 | provenance/commit/license/NOTICE登记，fixture授权不猜 |
| R16 非phase闭门开发、Agent可执行任务 | IMPLEMENTATION_PLAN §1–5；TASKS | T01…T25 | 每卡接口/文件/steps/negative/gate/完成定义，依赖无环 |

## 8. 风险的最小试验与停止条件

| 风险 | 最小验证 | 停止 / 降级规则 |
|---|---|---|
| R1 exceptionSSA与shadowcopies | F16旧状态 + F17两个site + M15placement/type失败 | 不能证no-throw/exactsource则该Java方法Unsupported；BUG证书失败仍红 |
| R2 Java绑定与rawgeneric | F10nullOverload + F21bridge + emptycatalog | 不完整candidate不猜；报告BindingEvidenceMissing，补catalog是新能力 |
| R3 constructor/final/init | F12argsready + U02pre-super + F20final/clinit | 不删super、不默认final；无法表达decl则Mixed/raw |
| R4 defaultlocals/dispatcher | F07cycle + F08irreducible + M05/M07undefined | 未定义输入拒绝，不能default0；handwrittengraph无oracleV5不算通过 |
| R5 try/monitorownership | F18try扩大反例 + F19holdsLock | crossing/reentry/unmatched释放profile降级，不启异常dispatcher |
| R6 JVM/ART/真实依赖 | 完整selfmadeclasspath、标JVM-only，授权ART再双DEXdriver | 无environment不跑真实APK，not-run不算behaviorpass |
| R7 reader/metadata/许可 | M01–M04count/UTF16+F23runtimeannotation+复制前LICENSE审阅 | 许可阻止复制但可以自制；无法声明完整metadata不JavaUnit |
| R8 SSA/类型的边输入/bit interpretation | F06join、F09多种use、F16exceptionedge，M06/M09错误输入 | pending不当Undefined、Conflict不改Object；类型证据不足该Java profile拒绝 |

若这些实验揭示总体契约必须改变（例如必须允许Java-like伪码或放弃runtimeannotation语义），须向用户确认，不能通过改golden/缩denominator自行更改目标。

## 9. 文档阶段的验证（本次可实际运行）

与上面的未来引擎命令不同，本轮验证只包括：

1. `docs/diana/README.md`能定位各文档；相对链接指向现存文档（未来目录标明future，不要求现在存在）。
2. `deps TNN:`与TASKS任务卡一致；`host-deps HNN:`与PROJECT_TASKS一致，两图联合ID唯一、依赖无环；T24是optional，T25不依赖它；H与U型拒绝fixture不混用。
3. 复杂能力卡具备文件/interface/步骤/不变量/限额/测试/命令/完成/不扩展；H01/H07基础CLI卡明确只审阅、不新增测试filter。Q01/Q07是人工清单、Q08汇总，Q02–Q06/Q09/Q10只测试复杂领域。结果序列/文本确定性不验收，程序语义与原身份完整性仍在；不测试旧flag拒绝等CLI黑盒。
4. P0–P9product/interface/实际顺序/消费与失效一致；所有failurepath回到raw，不要求运行尚不存在的crate。
5. 源码出处固定版本与符号，个人绝对路径/敏感corpus标识不进入文档；许可不确定点明确记录。
6. `git diff --check`、改动范围复核和本仓desensitize检查。`tools/audit/desensitize_check.py --range HEAD`扫描工作区相对 HEAD 的 tracked diff；新 untracked 文档要额外逐词扫描、检查行尾空白，不能以 tracked diff 中没有它们就 claim 通过。

本页不是测试成绩报告；实际文档验证结果由交付说明列出。本目标完成仅指设计/计划就绪，不代表新引擎算法、性能或真实APK行为已验证。
