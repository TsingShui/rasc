# Agent 实施任务卡

本页是**将来的实现任务**，本轮只交付计划，没有创建 `crates/diana-decomp`、fixture、脚本或实现测试。目标是从零引擎，不迁移既有实验分支。所有路径除另有说明均相对于未来 `crates/diana-decomp/`；在仓库根执行命令。

必读：[DESIGN.md](DESIGN.md)（契约）、[MODULES.md](MODULES.md)（接口）、[ALGORITHMS.md](ALGORITHMS.md)（关键算法步骤）、[VERIFICATION.md](VERIFICATION.md)（fixture/harness规则）。依赖以 [IMPLEMENTATION_PLAN.md §4](IMPLEMENTATION_PLAN.md#4-任务依赖可用作机械检查输入) 为准。

## 0. 所有任务共同的执行合同

- **范围**：T01–T25只创建当前内部 `diana-decomp` 任务会编译/测试的 Module。crate 默认 `unsafe_code=forbid`，无 blanket warnings allow，默认 lib tests 打开；不增加生产外部进程/JVM/Python依赖。root `diana` CLI替换由H任务验收，T卡不自行切默认入口。**MCP暂不设计或重写**。
- **交付**：实现、interface/doc同步；有复杂语义/输入/资源义务的能力提供输入→结果正例与反例、fixture和来源登记。测试跨公开库/module interface，重点捕获错误语义/原始内容丢失/安全失效，不给简单enum、getter、转发、参数路由、help/列名或机械格式各写一个测试，也不靠测试私有调用次数。CLI接线人工审阅；现有测试本次不删。
- **基本gate G0**（未来 T01 后存在）：`cargo fmt --all -- --check`；`cargo clippy -p diana-decomp --all-targets -- -D warnings`；`cargo test -p diana-decomp --all-targets`；集成提交再跑根 `AGENTS.md` workspace命令。缺crate时不能运行这些命令并宣称pass。
- **任务filter**：`cargo test -p diana-decomp --test contracts tNN_`。T01创建`tests/contracts.rs`，按任务注册子模块，测试名必须带对应filter，harness须检查运行计数非零；0tests不是验收。
- **语义gate B(F)**：F只是原能力族索引，**实际按E2E_CASES.md第4节的任务→D固定映射运行**，不把宽F族全部作为早期任务的未来功能。B01–B04先冻结/认证输入与expected，T05只接真实库；例如T06执行`DIANA_DEV_ROUNDTRIP=1 DIANA_CASE=D009 cargo test -p diana-decomp --test contracts behavior_roundtrip -- --nocapture`。可comma-list D编号；缺工具/fixture失败，不根据当前实现动态删除成员。详见VERIFICATION.md，正常G0可运行已签入自制DEX的Rust测试，不假装运行了外部roundtrip。
- **smali gate S(F)**：F为历史族映射，实际D编号按E2E_CASES固定选；B01–B04先锁工具/input/expected，T05只接adapter，例如`DIANA_DEV_SMALI=1 DIANA_CASE=D061 cargo test -p diana-decomp --test contracts smali_roundtrip -- --nocapture`。只用于已校验且metadata完整的standard-smali；raw excerpt不纳入roundtrip成功。assembler要求与版本在manifest记录。
- **变更粒度**：先最小profile正例，再拒绝边界；核心不变量失败不得以放宽assert/修改normalizer吸收。unsupported保留raw，与内部BUG、Malformed、Resource区分。
- **预算**：所有count/offset checked arithmetic，集合/arena创建前charge+try_reserve；每个worklist/graph/rewrite循环收费，取消有测试；输出text/spans合计也受请求限额。
- **输出顺序**：不保证独立任务结果、合成名字、源码/诊断跨运行一致，不加sort/stable tie-break或按输入ordinal等待。固定ValueId/PhiId仅是本次分析身份；原始指令/参数/副作用/catch/monitor/初始化语义顺序不能取消。验收比较身份完整性与语义，非文本排列。
- **完成**：下面的输入输出、反例、命令和范围均达成；尚未支持的扩展留knownUnsupported登记。任务要求Java的fixture不能以“有smali输出”算该项完成。

下面复杂能力卡的完成条件包含G0、实际非零测试、指定B/Sgate和manifest结果；基础接口/机械接线仅做编译和源码审阅，不造空测试凑数。有外部语义gate要求而tool不可用则blocked/not-run，不伪装通过。下文内部artifact/invariant反例可作为定位辅助，但E2E不依赖私有phase形状或转发/调用顺序断言。

## T01 — 可编译的引擎合同与最小 Kuna 调度骨架

**依赖**：B04。**Phase**：P0/Pipeline/P9。**源码参考**：KU-01/02/04/05，RG-04。

**目标文件**：未来 `crates/diana-decomp/Cargo.toml`（其余路径按本页约定相对该crate）、`src/{lib,engine}.rs`、`src/pipeline/{schedule,registry,control,diagnostics,trace}.rs`、`src/p0_knowledge/{mod,policy}.rs`、`src/substrate/{ids,origin}.rs`、`tests/contracts.rs`。package=`diana-decomp`，Rust导入=`diana_decomp`，与root产品`diana`分开。加入workspace，旧live生产依赖由H08分析退役，版本按正式流程，不手工推动release。

**输入→输出 / Interface**：实现DESIGN §2和PROJECT §4的`DexImage<'a>/Engine<'a>/RunContext/Options/Request/Report/MethodOutput/ClassOutput`、分类诊断；未接线Java路径只能Unsupported/Unavailable，不能占位成功。建立`PhaseId/PassId/PassSpec`、`RunControl::charge/reserve/check_cancel`；registry是实际schedule定义的投影，不是另一份手填pass顺序。

**步骤**：
1. 创建最小crate与测试注册；enum和private Verified构造器只给owning validator访问。
2. 基础enum、interface、转发与错误包装通过编译/源码审阅确认，不写“枚举值等于自身”或机械getter测试。
3. 实现独立normal/fallback预算池，unknown输入不消耗无界fallback；trace sink有event数量/字符串上限。
4. 不为私有Pass调用顺序写测试，也不为未实现phase创建空算法；通过真实公开请求的资源耗尽/取消行为验证调度是否停止，trace仅辅助定位。
5. 固定diagnostic字段：code、severity、method/site、phase/pass、kind、boundeddetail，WallTime不得进入文本golden。

**不变量 / 限额**：所有状态可穷举；requiredpass不可被policy禁用；重复PassId拒绝；work charge不得溢出/退款重试绕限。Cancelled停止请求，不触发fallback。

**测试**：F00聚焦复杂资源控制：公开请求恰在预算边界与超额、资源错误后的fallback预算隔离、长工作取消后停止、重复申请/失败不会绕限制或泄漏。基本状态enum/包装/私有Pass调用顺序不测试；真实Malformed/Unsupported在后续输入/语义fixture验证。T01未有可运行真实请求时这部分作为后续接入要求，不造无效测试凑数。

**验收**：G0和源码/interface审阅；有真实资源/取消请求时运行非零`t01_`领域测试，接口尚未接线时明确待T05接入，不写空green。**完成条件**：crate编译、资源控制实现有明确验证落点；不要求基础enum/路由/转发测试，不能把未接线功能当成功。

**不扩展**：TOML/build.rs生成器、插件ActionDatabase、persistentassertion/restart、并行cache、独立console。

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

**输入→输出**：MUTF-8→`DexString(Vec<u16>)`；validswitch/arraypayload/encodedvalue/debugraw→完整Method/Declaration raw记录，standard-smali对应directive。diagnosticlossystring不得成为literaltruth。

**步骤**：
1. decode MUTF-8 NUL、surrogate、invalidencoding、声明utf16_len；remainingbytes与stringunits先核，绝不replacement改变literal。
2. switchpayload对齐/target基于switch-PC、检查原格式要求的keys递增/唯一与数量（不为稳定输出重排原输入）；fill-array宽度countchecked且保留bytesbits；payload不是executableblock。
3. debugprogram逐opcode有限解码，地址增量/line/signoverflow检查；若无法可靠decode该metadata标Malformed/Unsupported，不能打印完整smali承诺。
4. annotation/encodedarray有界栈，非法valuewidth/tag与index拒绝；systemmetadata保留原身份，不写Java关键字。
5. Java/smaliescaping分别测试NUL/newline/quote/backslash-u/isolatedsurrogate；JavaUnicode词法预处理的newline必须转正确stringescape。

**不变量 / 限额**：保留每u16、float/doublebits；allpayloadbytecoverage，nestedcountdepthcharge；no`Vec::with_capacity(input_count)`先于校验。

**测试**：F03 `Utf16Payload`：unpairedhigh/low、NUL、`\\u000a`文字、NaNbitpattern、sparse/packedswitch、fillarray；M04负relativepayload、overlap、oversizedarray/annotation、baddebugtarget/unterminatedMUTF8。futureS(F03)校验smali→DEXliteral/payload等价。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t04_`；T05后S(F02,F03)。**完成**：rawroundtripcomparison可逐field说明精确保留或明确unsupported，无静默lossy值。

**不扩展**：源级annotation/generic美化、stringbuilder拼接、全部新版DEXmetadata。

## T05 — 接入已冻结 E2E 与最小 typed product/AST

**依赖**：T04。**Phase**：Pipeline/Substrate/P9、测试体系。**参考**：KU-08、RG-08、DX-13。

**文件**：`src/substrate/{cfg,java}.rs`、`tests/contracts.rs`、薄`tests/support/e2e_adapter.rs`、`examples/inspect_class.rs`；读取B01–B04先准备的root `tests/e2e/` cases/manifest/expected/锁定harness，不复制第二份oracle或工具源。

**输入→输出**：按MODULES §2建立Flow/Ssa/Typed/Storage/PlannedBody骨干与privateconstructors；fixturemanifest可驱动nativeinputtests、smaliassemblycheck、javac/d8roundtrip。最早验证DeclarationOnly和rawmethod，不把空AST视成功。

**步骤**：
1. 按MODULES固定采用ownershipwrappers；raw视图借用caller-owned bytes，permethodownedarena，ID永不recycle；schema确定后所有card使用同命名，不自行换成第二套Context体系。
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
5. artifact/trace要求实际走P2…P9且每步counts正确；error在指定pass后无Java残片。

**不变量 / 限额**：无handler/noMonitor；undefinedread必须拒绝；long/doubleword契约T10才扩充，不knownprofile就Unsupported而非猜int；每effect暂不承诺可inline。

**测试**：T06实际运行B01–B04固定D009（int加法/溢出），不是到开发时再生成answer/inc的新fixture；`answer()->7`等只作实现说明。未定义读/宽值/预算等按D055–D064及任务固定映射接入，范围不足仍未通过，不改expected。D009在已知合法context运行恢复方法，别把未支持的defaultconstructor计整类JavaUnit。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t06_` + B(F05)。**完成**：至少一个非空方法真实编译和执行闭环，负例不是默认值“成功”。

**不扩展**：复杂branch、handler、变量merge、表达式美化、通用typedhelper生成。

## T07 — 分支与固定身份 join Phi 的 SSA

**依赖**：T06。**Phase**：P2/P3/P5/P6。**参考**：DX-03/04、RG-03。

**文件**：`src/p2_flow/build.rs`、`src/p3_ssa/{liveness,build,verify}.rs`、`src/p5_types/constraints.rs`、`tests/pipeline/ssa.rs`。

**输入→输出**：normalbranchFlowBody→SsaBody，joininputs以EdgeId标识，ValueId稳定；尚无Javacopylowering完整支持的方法可Smali，但internalSSA须完整验证。

**步骤**：
1. buildblockleaders由entry/target/afterterminal；normalbranches精确true/false，goto为terminator不是effectNOP。
2. sparseuse/kill/live-in；为每需join的BlockId预分配固定Phi，不每iteration新建；不额外排序BlockId追求跨运行相同ID。
3. worklist更新edgeRegEnvironment、Phi.inputs，fixeddefinitionIDs；temporaryPending与finalUndefined分离。
4. normaldominatorvalidator、Phi-inputedgeavailability、multipleedges同target不能随意合并。
5. typed scalarjoin用constraints，不先根据第一branchtype随意定finaltype。

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
3. sequentialize依据“dst不在剩余sources”，cycle拆temp，sourceexacttype；no-extra-cast/boxing。
4. DefinitionCertificate独立求originaluse/edgecopyavailability；仅已证明不可观察的普通locals允许defaultinitialization满足JavaDA。
5. loopentry/latch多个Phi预分配/传播，normalcopyplan可以internalinterpreter在test验证；最小diamondP8实现singlecondition求值与transfer。

**不变量 / 限额**：deaddefault不是借口读Undefined；copy周期tempcharge、source-dsttypeproof；先选edge后copy，copy全是pure无throw。Uninit不得null初始化伪装Ref。

**测试**：F06choose正/负条件两driver；F07 `CopyCycleLoop`：`a<-b,b<-a`期望交换、2/3cycle、criticaledge、loopentry/latch；M07naive顺序赋值、copy放wrongbranch、default观察、typecastcopy。T09后补B(F07)。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t08_` + B(F06)。**完成**：simplebranch真Java；parallelcopy所有cycle/criticaledge反例validator失败；loopcopy义务清楚不自称全loopJava完成。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t09_` + B(F07,F08)。**完成**：复杂ordinarygraph有合法Java而非goto伪码；eligibilitynegative严禁接受。

**不扩展**：try/monitor状态机、Object[]寄存器装箱解释器、复制effecttail、字节相同作为自然结构质量指标。

## T10 — Primitive/Null/FP 完整第一档合法化

**依赖**：T09。**Phase**：P2/P5/P8/P9。**参考**：DX-08，RG-09。

**文件**：`src/p5_types/{lattice,constraints,solve,conversions}.rs`、`src/p2_flow/decode.rs`、`src/p8_java/expressions.rs`、`src/p9_emit/literals.rs`。

**输入→输出**：bits/wordtypes+useconstraints→TypedBody/UseConversion→合法primitive/refnullJava。支持boolean、byte/short/char/int/long、float/double、cmp/cmpl/cmpg、conversions和shift；notenoughtypeevidence明确Unsupported。

**步骤**：
1. finite候选type域，constbits不第一次use即固定；numeric/refzero用上下文约束，不全写null或0。
2. widewordpair、aliasoverlap/partialread检查；所有shiftmask、signed/unsigned、narrowing与Dex类型语义表逐opcode对应。
3. booleanbitwise合法truth表达、eqzref==null；任何ambiguous非literal值不直接重解释cast。
4. cmpl/cmpg展开保留NaNbias和signedzero；FP不reassociate/fastmath。rawNaNpayload相关bitoperations以保守profile验收；用intrinsic必须有P4语义/环境证据，无法证明则fallback。
5. typedliteral.escape覆盖negativeMIN、Infinity、-0、surrogate；每UseConversionno-extra-runtime-check可核查。

**不变量 / 限额**：有限constraints收敛，Conflict显式拒绝；overflow和Java数值promotion不靠hostRust默认猜。

**测试**：F09 `PrimitiveBits`：booleannegation/bitop、0/null/false、MIN/-1、shift-1/32/64、NaNcmpl/cmpg、negativezero、float→int边界、wideoverlap；M09错误kind/partialwide/unresolvedprimitive；B(F09)按bits/traces比较，不只formattext。F09必须拆成无library-call的核心primitive正例与尚无P4intrinsic证据的bitcast子例：后者本卡预期Unsupported/raw，不能让T10依赖未来T11；T11建立intrinsic声明后新增独立扩档验收。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t10_` + B(F09)。**完成**：supportedprimitive矩阵每族有positive/boundary；未验证bitcast/intrinsic不标支持。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t11_` + B(F10) + S(F11)。**完成**：所有kind有行为driver和rejectcase；missingcatalog为预期降级而非pass“忽略errors”。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t12_` + B(F12) + S(U02)。**完成**：constructortrace顺序一致，所有uninitnegative明确处理，未支持特例不删super掩盖。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t13_` + B(F13)。**完成**：simpleloop明确naturalAST，hardgraphdispatcher；certificateinvalid不能降成宽松成功。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t14_` + B(F14)。**完成**：普通整数switchcoverage完整，string/enum降糖模式暂未支持不fake恢复。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t15_` + B(F15)。**完成**：supportedheapops都有throw/effectnegative，originalmemberspans定位正确。

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
3. nestedhandlerexceptiongraph先只支持无outeralias的simpleprofile，其他返回Unsupported：不通过同type就隐式共享影子槽。
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
5. 第二档可支持**一层**lexicalnestedtry只有ownership全证据；sharedtarget复杂或overlapprotectionremainUnsupported，不用排序修复。

**不变量 / 限额**：每protectedsite原handler选择一致、throw/return完成性一致、catch无法直接向scope外goto；certificate失败为InternalInvariant/Unsupported的定义必须区分算法BUG和不支持形态。

**测试**：B(F16,F17)；F18 `ProtectedControl`：orderedsubtypecatch、unexpectedError走catch-all/rethrow、cleanupreturn/throw、cleanup自己抛出、nestedonelevel；U03 `ComplexProtection`normalreentry/sharedhandlers/release未知需Smali；M16漏edge/try扩大/把call外异常吞掉。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t18_` + B(F16,F17,F18) + S(U03)。**完成**：positiveprotected方法真实Java运行，negative保持smali，notfullfinally恢复不影响真实trace。

**不扩展**：arbitraryexceptionownership、全路径finally美化、labelgoto跨try、吞异常使javac通过。

## T19 — Monitor scope 证书与 synchronized

**依赖**：T18、T12。**Phase**：P2/P7/P8。**参考**：DX-07（synchronization）。

**文件**：`src/p7_regions/monitors.rs`、`src/p2_flow/effects.rs`、`src/p8_java/{structured,declarations,verify}.rs`。

**输入→输出**：declarationflag或显式balancedmonitor CFG→MonitorCertificate→Java synchronized；未知/mismatched锁状态→Smali，不输出伪Java关键字。

**步骤**：
1. rawflag语义审阅，method-synchronized隐式lock与explicitmonitor分开；静态lockClass对象、实例lockthis明确。
2. lockValueId identity而非name/stringequivalence；相同lock重入count、enter失败不要求release，成功后各normal/exceptionexit精确一次release。
3. singleentryscope、null锁throw时机、releasehandlerinterval/protectedsite一致；match后仅以证书替换原enter/exit/catchcleanup。
4. nestedbalancedprofile逐scope核，不证明的路径不跳过monitor；scopebodycall绝不放到synchronized之外。

**不变量 / 限额**：锁effect不是pure，noinline/hoist；证书有all-exitwalkbudget，循环不靠无界enumeration。无法证明停止该profile而非假定balance。

**测试**：F19 `MonitorScope`：return/throw/nulllock、nested/reentrant、methodstatic/instance，body中`Thread.holdsLock`driver、finallythrow后lock释放；U04 mismatchedlock/exitmissing/multientry→Smali；nativeverifynegativecoverage。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t19_` + B(F19) + S(U04)。**完成**：有明确语义trace/holdsLock/释放检查（并发stress仅补充不当完整证明），rawflags不直接打印declared_synchronized。

**不扩展**：跨锁aliasvalue猜测、arbitrarymonitorstate、去除“多余”锁、生产运行动态验证。

## T20 — <clinit>/<init>、encoded 初值与 final 字段

**依赖**：T19、T15。**Phase**：P1/P4/P8。**参考**：DX-08/09（initialization）。

**文件**：`src/p8_java/{initializers,declarations,verify}.rs`、`src/p1_input/values.rs`。

**输入→输出**：staticencodedvalues+clinit、constructorfieldwrites→合法classdecl/initializerplan；不能完整shell表示则Mixed保留raw字段/方法，不能给finaldefault再写。

**步骤**：
1. 合并DEXstaticfieldsencodedorder与clinit原effectorder；常量default/explicitliteral准确，nullString不改成空串。
2. clinit生成static block，不造可调用`<clinit>`method；保持readtriggerclassinitialization，不把staticnonconstant读inlineconst。
3. simpleblankfinal在constructors/allpaths赋一次，delegatingthischain和super先后核对；不合法Java单赋值profile明确UnsupportedDecl。
4. 无constructorbody未赋final不能默认初始化“凑编译”；classshellfailure不影响独立methodanalysis，但报告Mixed。
5. 初始化相关throws与classfinalmodifier/rawvisibility证据由P4/metadata而非printedtext猜。

**不变量 / 限额**：sourceconstantfinal可能改变classinittrigger必须核；初始化次数/order和失败路径可见；clinitfield重写不丢origin。

**测试**：F20 `Initialization`：nonconstantstaticfieldtrace、multipleconstructors、thisdelegate、blankfinal、encodednull/false/floatbits、initexception；U05 complexfinalreassignment/default不可证→Mixed/Smali；B(F20)每driver新进程防classinit已发生。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t20_` + B(F20) + S(U05)。**完成**：合法unitinittrace一致，无法sourcefinal表达明确decl状态，不删finalcontract隐藏。

**不扩展**：fieldinitializer美化、constructorfactorycarrier、interproceduralinit优化、constant传播。

## T21 — Raw generic、bridge/override 与 source 绑定合法性

**依赖**：T20、T11。**Phase**：P0/P4/P5/P8。**参考**：DX-08及signature/sourceABI问题。

**文件**：`src/p4_calls/{contracts,members}.rs`、`src/p0_knowledge/declarations.rs`、`src/p8_java/{declarations,expressions,verify}.rs`。

**输入→输出**：Signature metadata可保留raw而不必恢复`List<T>`；Javaeraseddecl/override/callbinding证据全通才成功，return-onlycollision不能随意renameexternals。

**步骤**：
1. 审阅DEXdescriptor与Signature差异，保留metadata原记录；rawdecl不改变binarymemberproto，typedcasts不新增runtimecheck。
2. bridge/synthetic只metadata不是Javakeyword；候选同name同parametererasure的covariantbridge若可由Javacompiler生成且行为忠实才省rawbridge，否则该decl/methodUnsupported。
3. Java实现interface/override方法return/throws/visibility与rawcontract一致，genericparentmember绑定不能只依localdescriptor猜。
4. 不完整catalog的相关方法fallback，不让missingtypeexception“忽略javacerrors”变contract。

**不变量 / 限额**：puregeneric美化可省，bindingcorrectness不可省；overloadcomplete状态与EnvironmentId稳定，sourcepublicidentity不改。

**测试**：F21 `ErasedContracts`：selfmadegenericList/identity、covariantbridge、rawinterfaceimpl、nulloverload；U06 return-onlyDEXoverload、bridgeeffect不同、unknownparentcontract→Smali/Mixed；B(F21)配套完整selfmadeclasspath。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t21_` + B(F21) + S(U06)。**完成**：raw泛型警告可接受，但真正编译错误不可忽略；legacyfulltypesolver不整体搬入。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t22_` + B(F22) + nativeownership/spanmutationtests。**完成**：top-levelunit合法，originalidentitymapping完整，singlefragmentstatus正确。

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

**验收**：G0 + `cargo test -p diana-decomp --test contracts t23_` + B(F23) + S(U08合法raw部分)。**完成**：支持profile可复算、JavaUnit分类与metadata语义合同吻合，无extendsEnum伪合法代码。

**不扩展**：全enum-switch降糖、annotation平台库大catalog、复杂enumconstantbody、反射名rewrite承诺。

## T24 — 可选纯表达式恢复（不是必需核心）

**依赖**：T23、T14。**Phase**：P8/Pipeline/P0。**参考**：KU-04/05/08，DX-06。

**文件**：`src/p8_java/recovery.rs`、`src/pipeline/{schedule,registry}.rs`、`src/p0_knowledge/policy.rs`。

**输入→输出**：已verifiedJavaAST→可选readabilityrewrite→再次verify；defaultoff，关闭不影响CoreRC。只smallpurelocalcopies/literal和受限loop形式，不搬BDD/gatedPhi。

**步骤**：
1. option由actualpassspec投影，earliestconsumer=P8；requiredlegalization不可关闭。按RECOVERY_EXTENSIONS.md的Applied/Declined/BUG合同实现一个真实purepass，不预建未实现E02–E08插件。
2. 只同blocksingleuse、nopossiblethrow/nocall/noheap/nomonitor/nochangingclassinitdomain的pureexpr内联；booleansyntax以已解typefact。
3. rewrite每轮有finite measure（locals/redundantnodes下降）或explicitmaxround/worklimit，unchanged停止。
4. applyonprivateAST，验证失败丢candidate、保持baselineAST并记RecoverDeclined；InternalInvariant仍reportBUG而不是默默吞。

**不变量 / 限额**：noeffect duplication/motion，postverifiedtypes仍valid；defaultoff和on各有sameprofile行为；label/state数量仅qualityproxy。

**测试**：F24 `PureRecovery`：purecopyinline正例、call/read/div/monitornegative；B(F24)on/offdriver一致，mutationduplicatedeffect失败；budget不会禁用requiredcopylowering。

**验收**：G0 + `cargo test -p diana-decomp --test contracts t24_` + B(F24)。**完成**：smalloption真正消费、trace可见、失败保留baseline、不影响T25核心gate。

**不扩展**：本卡不做常量分支改CFG、GVN/LICM、stringbuilder/finally还原、全局restart和持久HARDassertions；这些不是永久禁止美化，后续可按RECOVERY_EXTENSIONS.md另起独立可选任务并验收。

## T25 — 全 supported profile 加固与发布前证据（不切生产）

**依赖**：T23、T14。**Phase**：全部/验证。T24为可选美化，不是本卡前置。**参考**：KU-04/08、RG-08、DX-13。

**文件**：`tests/contracts.rs`、薄public adapter、`src/pipeline/{registry,control,trace}.rs`，读取root `tests/e2e/manifest.json`/cases/expected/input证书；新crateREADME/NOTICE登记和本设计实现状态。不要各crate复制第二套可变fixture/expected。

**输入→输出**：T01–T23的实际supportedprofile→CoreRC证据集：每能力positive/negative/limit/状态、behaviordriver与rawfallback、无panic/mutation、无序输出的身份/内容完整性、source许可证审阅。不是宣布所有DEX可Java。

**步骤**：
1. 核对B01–B04已固定manifest与actualregistry/schedule，required成员不按当前实现删/skip；将core-java的失败改为Smali成功不可接受。filterzero断言，known038/039/custom/polymorphic未支持列表与rawcoverage分别明确；unknownformat不能fakefallback。
2. 生成字节和artifactmutation测试：count/offset/LEB/payload/handler/type/Phi/copy/site/loop/state/outputbudget，固定seed，取消会终止。
3. 每个knownunsupported合法fixtureSgate；malformedUnavailable/rawexcerpt不冒充smali，BUG不算成功。fallback吞掉exception/type错误的策略必须反例失败。
4. 跑B01/B04固定的D001–D054核心Java集合与其他raw/安全/identity套件，不根据当前engine-supported过滤分母；未实现仍未通过。单列JVM/ART、catalog完整性、metadata、exception/monitor证据。T24off先全core gate，on仅已实现的美化另验。
5. 以不同worker到达次序/HashSeed重复验证记录和产物按原始identity关联、无重无漏、每份实际source合法且spans指向本份文本；不要求生成文本/局部名/声明/诊断序列byte-identical，不为测试加入canonical sort。不同合法调度触发不同预算结果如实分类，但不得因此错语义；不为本卡刻意新建并行。
6. 记录release耗时/峰值内存方法而不是debug断言快；代表input授权后optin才运行，环境/skip明确。不做promised百分比缩减。
7. 套用本仓workspacegates、NOTICE许可来源和self-contained约束；本卡只验收内部EngineCoreRC，root CLI替换由H05/H08验收，MCP暂不设计。

**不变量 / 限额**：超限Resource不是malformed，totalrequestlimits覆盖多个method，smalllimit不可输出截断“完整Java”；fuzzcase有timeout/seed/range避免unbounded运行。

**测试**：F25 `HardeningMatrix`：所有M/U/Fmanifest登记，registryinvalidationscope、CancelvsFallback、largecountnearboundary、repeatnochange与reportspans；结果排列/合成ID变化时按原identity核对，不作跨运行全文本相等。knownunsupportedextensions仅raw且有version限制。

**验收**：G0；publicinterface复杂t25_测试；B(B01/B04固定core-java Dcase集合)、S(固定raw-integrity集合)及input/resource/identity/domain/extract套件，根四gate。required成员不随实现状态变化，运行命令见VERIFICATION。

**完成**：可追溯CoreRC报告，required Dcase全部按固定预期验收，无empty/skip或Java→Smali改答案；若反例未修，能力保持Unsupported但该requiredcase仍未通过，不能宣布CoreRC。

**不扩展**：生产引擎切换、版本tag/release、自动私有corpus下载、未经测量的cache/parallel、用normalizer修comparison分数。

## 26. Agent 接手时的通用提示模板

```text
执行 Diana TNN；先读 docs/diana/DESIGN.md、MODULES.md、TASKS.md 的对应卡、VERIFICATION.md。
在新工作分支按从零架构实现内部diana-decomp，不迁移既有实验分支；root Diana CLI由PROJECT_TASKS的H线统筹，T卡不自行切默认，MCP暂不设计。
确认前置任务已有证据，别把存在文件等同于依赖已验收。
先最小 public seam fixture，再 negative/limit/internal invariant，最后指定 B/S gate。
只创建会编译和测试的 Module；复制参考代码前检查 SOURCE_MAP.md 和许可。
回报实际文件、接口偏离、非零运行测试数、命令结果、unsupported状态、尚未验证的义务。
核心语义无法达成时不要偷偷降门槛：保留raw/fallback，提出接口决策或询问。
```

本次文档验收不会执行以上未来任务。任何“完成代码”“测试全绿”的说法只能由实施者在对应环境运行后提供。
