# Diana 产品任务卡（CLI 优先，MCP 延期）

这些是**未来功能实施任务**；纯包骨架已按用户最新要求先行，不代表 H01/T01 完成。实际结构以 [STRUCTURE.md](STRUCTURE.md) 为准：下文 root 产品/src 指 Diana 应用根 `apps/diana/`，固定旧源码锚点不改，当前 checkout 对应 `ref-fork/rasc/`；主 workspace/CI 不构建测试它；产品测试在 `apps/diana/tests/`，workspace Cargo/lock、共享 `tests/e2e/` 与工具仍在仓库根。H01–H10负责整个Diana产品；T01–T25负责内部`diana-decomp`。H编号与现有U01–U08 Unsupported fixture不同，不得混用。总体合同见[PROJECT.md](PROJECT.md)，公开命令/列/选择器以[CLI.md](CLI.md)为唯一规范，反编译合同见[DESIGN.md](DESIGN.md)。

MCP暂不设计：不创建新tool/schema/envelope/会话协议。旧MCP完整归档在 ref-fork，与旧 workspace/lock/tests 独立；不改行为、不删除源码，也不编译或运行其回归来验收 Diana。主工程无旧调用/依赖。

## 1. 依赖与共享实施合同

**测试范围（最新约定）**：不新增CLI黑盒测试，不为help/version、参数解析、命令路由、列名、基础格式、alias或分页flag建测试/golden。这些接口仍按CLI.md实现，通过源码/文档审阅与编译核对。自动化投入反编译行为与复杂领域边界：Phi/循环、类型/调用、异常/锁、构造器、跨DEX/041身份、原始字节/降级完整性、文件安全、资源/取消。测试通过公开库/module interface，不启动Diana CLI进程。现有测试不在本次删除范围。

```text
host-deps H01: B04
host-deps H02: H01
host-deps H03: H02
host-deps H04: H03 T04
host-deps H05: H04 T06
host-deps H06: H05 T09
host-deps H07: H06 H09 H10
host-deps H08: H07 T25 T14
host-deps H09: H04
host-deps H10: H02
```

B01–B04测试设计/fixture/公开工具/独立基准/harness先于T01/H01，详见[TEST_PREPARATION.md](TEST_PREPARATION.md)；所有case与source/expected可由干净checkout复建，不用作者机器路径。T线不反向依赖H线，B准备不依赖引擎实现，因此没有跨图循环。T01采用本文既定命名/interface即可独立实现；H01不能另选同名内部crate。先H01/T01确定共享interface，再让H02/H03与T02–T06交错；H05尽早跑一个真实CLI方法。用户未授权时不启动并行子Agent。

未来产品gate `PG`：root `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`、`cargo test --workspace --all-features`、`cargo build --release`。这些命令只选 Diana，不会进入 ref-fork；旧工程 lint/测试不再作为本分支 gate，也不补 MCP 回归或新协议合同。

H02起为实际复杂领域任务创建`tests/product_contracts.rs`，只测试公开输入/查询/提取module interface，不启动CLI。H02–H06/H09/H10使用`cargo test -p diana --test product_contracts hNN_`且运行非零用例；H01/H07只做实现/人工核对，无测试filter，不造空测试凑数；H08汇总已有复杂测试。引擎gate用`-p diana-decomp`，Diana 应用用 `-p diana`；根 Cargo 只选择 Diana 两包，没有 `-p rasc` 成员。旧工程不参与当前 gate，骨架构建不算功能/E2E通过。

每卡交付focused代码、接口/doc同步、编译与审阅结果；复杂语义/安全任务接入B01–B04已冻结的自制fixture/独立expected/harness，不在实现时重新设计答案。新反例可追加，改旧oracle须独立证据与审阅；不得按引擎当前能力删case。基础CLI卡不强求新增测试。旧测试保存在 ref-fork，不删除也不运行作为主 gate，更不扩成新CLI黑盒。不改版本/tag、不推发布；目标module仅按实际需要创建。

## H01 — 产品命名与领域合同冻结

**依赖**：B04。**范围**：Diana应用品牌/CLI/内部crate定位，无MCP新设计。

**文件**：现有 `apps/diana/Cargo.toml`/`src/main.rs` 骨架与未来 `apps/diana/src/lib.rs`、`src/cli/{args,mod}.rs`（均相对 Diana 应用根）；更新README/AGENTS产品说明。旧 rasc 已整体移到 ref-fork，仅参考，不构建/测试；历史release/许可不全局替换；本卡不创建CLI测试文件。

**输入→输出 / Interface**：应用 package/binary `diana`（`apps/diana/`）；内部package`diana-decomp`、path`crates/diana-decomp/`、import`diana_decomp`；原CLI领域/flags/退出码及结果完整性形成基线。输出顺序明确不再保证，独立任务结果按完成/到达顺序发布。rename不代表新算法已替换。

**步骤**：
1. 登记所有live名字消费者：clapname、crateimports、测试binaryenv、skill、releaseartifact/版本断言；MCP相关只登记延期/必要编译引用，不设计servername/schema迁移。
2. 对照现有源码与CLI.md核对命令/字段/输出通道与迁移清单，不运行CLI黑盒或建立help/格式golden。旧展示顺序与分页接口有意取消；原始身份、解码完整性和文件安全等复杂领域义务由后续module测试承担。
3. 在 apps/diana/入口与内部crate骨架上实现新产品，根已是 Diana-only workspace；旧 rasc 在 ref-fork，不把其 CLI 当别名/依赖。H08 仍须产品能力证据，旧发布工具已归档，Diana 发布政策独立审阅；历史来源不全替换。
4. 按CLI.md冻结11个命令和指定TSV/JSON列、规范at、完整method-selector、默认Java与显式JSONreport；旧getclass/findrefs/skill、class-id/memberprefix/require-java不保留alias。新增fields/methods/xrefsites/extract分别由H09/H10实现，不以旧接口限制新设计。

**不变量 / 限额**：活动 workspace 只含 Diana，旧 rasc 完整归档且不构建/测试；Diana 不转发旧 CLI/引擎；无JVM/Python生产runtime；原版权/来源不改。没有执行输出时不能自编基线数字。

**测试**：不新增自动化测试。Q01是命名/基础接口的人工审阅清单，不是测试族；帮助、flags、列名、alias和分页移除与CLI.md对照，不写CLI黑盒或golden。复杂身份/解码/文件安全由后续领域任务测试。

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
4. 提供scope内`BytesSource`/borrowedentry，不将用户文件mmap借用逃出owner；必要unsafe集中在输入module并写safety前提，`diana-decomp`保持unsafe forbid。
5. 请求终止/输出错误drop所有临时文件owner，不误删用户输入。

**不变量 / 限额**：len/offsetchecked，复制变化为INPUT_CHANGED；no unsafe `'static`延长；临时文件scope清理，不跨request伪缓存。

**测试**：Q02普通文件/裸DEX/存储ZIP；反例截断竞态、symlinkescape（有限root模式）、FIFO、超input/dir、owner提前drop；测试临时目录不碰用户真实文件。

**验收**：PG + `cargo test -p diana --test product_contracts h02_`。**完成**：source改动隔离、cleanup正确、预算前置与borrow边界可测；成本记录不编造收益。

**不扩展**：长期target协议、MCPopen/schema、全平台文件数据库、为快照重写ZIPcodec。

## H03 — 轻量查询、manifest 与完成顺序输出

**依赖**：H02。**范围**：CLI已有非反编译能力。

**文件**：`src/analysis/{apk,archive,query,manifest}.rs`、`src/analysis/dex/`、`src/cli/mod.rs`；既有scanner/unitfixture可复用，vendoredAXML修改需先PATCHES。

**输入→输出 / Interface**：InputLease→classes/strings/entries/manifest及供H09复用的符号/引用扫描基础，不经过Engine/SSA。CLI.md指定列是新输出合同，旧scanner只复用读取/查找算法，不复制旧findrefs方法级结果作为xrefsite。

**步骤**：
1. 依H01基线接新scope，保持rootDEX选择、重复entry规则、最后manifest选择和真实physical/logical/row身份；展示次序不保证，不能按最先完成选择manifest或重复类定义。
2. 保持ASCII/rawsubstring预筛与非ASCIIfallback、exact/fuzzydomainquery；展示string可与engine精确UTF16literal表示不同，必须明确各合同。
3. worker按完成/到达顺序提交row/batch，移除按entry ordinal等待、全量排序、reorder buffer和仅为保持输入顺序的领先窗口。保留资源加权admission、有界byte/row队列、背压/取消；no无界parcollect。不同threads只需完整记录多重集合一致，不要求序列一致。
4. filter保留匹配语义，返回全部匹配结果；删除用户分页参数与对应skip/take、截取计数、分页早停逻辑，不用cursor/preview替代。保留错误/取消/安全预算停止及内部有界batch（batch不是用户分页）。storedentry借用，deflate按需解压；查询不建立fullProgramIndex或反编译IR。
5. 回归字段/诊断类别/exit；不为旧展示序列加sort/stable mode，记录这一有意变化并测量release成本，不编造无序必然加速。

**不变量 / 限额**：同输入定义/引用索引对应同entry，成功查询返回全部匹配记录、不重不漏；noPython/外部JVM；query预算与engine预算不混用。移除用户数量截取不移除安全限额；超预算必须错误，不能只返回前N条并标成功。

**测试**：Q03通过公开查询module interface验证多DEX/重复entry/重复manifest、UTF16匹配、并发记录完整性与资源错误。受控延迟生产者核对无按序等待且buffer有界，按原identity比较内容；不测CLIflag、表头、基础filter路由、空列表打印，亦不assert私有phase调用次数。

**验收**：PG + `cargo test -p diana --test product_contracts h03_` +对应现有selfcontained/inflatepolicy测试。**完成**：按新CLI指定列返回全部匹配记录；读取/匹配/原身份完整性保持，输出序和用户分页有意取消；慢entry不阻塞无关结果，快路径无需新engine。

**不扩展**：合并全部DEXparsers、MCP通用分页协议、重写AXML、测量debugbuild后声称加速。

## H04 — 原始 DexImage、041 与定义身份 adapter

**依赖**：H03、T04。**范围**：root→internalengine输入interface。

**文件**：`src/analysis/dex/container.rs`、root`analysis`的selectedclass/engineadapter；`crates/diana-decomp/src/p1_input/{header,code,catalog}.rs`与origin/keys。

**输入→输出 / Interface**：`SelectedClass{scope,entry,logical,classdef,descriptor}`→`Vec<DexImage<'a>>`、ClassKey/MethodSelector与有界DeclarationCatalog；结果identity映射回root原输入。

**步骤**：
1. 按CLI.md解析class-at/method-at/field-at/string-at，核physical/logical/class-def/member-index与descriptor；get-class可读名字遇多定义明确ambiguous，不能发入engine又按name任意选。非法at语法/失效索引区分，裸DEX映射e0:l0。
2. 041原container+logicalheader_offset原生校验，sections使用正确container地址空间；保留rawbytes，不传旧normalizedheader。
3. 改P1/Engine为borrowedviews，ownerlease由H02scope保持；scope中先取得依赖bytes，再一次调用engine，不能返回悬空borrow。
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
2. singlemethodsource的knowncontext验证遵循VERIFICATION，不把Mixedclass称JavaUnit；暂不支持方法保留raw与语言marker。
3. 默认text的get-class须VerifiedJavaUnit，get-method须Java或DeclarationOnly且是完整声明/body；无法满足exit1且无stdout/输出文件，不提供--require-java。显式JSON允许标记清楚的完整Mixed/raw报告，requesterror/BUG仍失败。
4. 根据CLI.md保存完整method owner/name/proto或method-at，textdiagnosticsstderr、同次stdout/文件payload一致；JSON不serialize私有arena，不设计MCPenvelope。
5. 保留旧实现直到明确各入口替换；已选Diana请求禁止静默fallbackdexdec，失败只按原rawsmali合同。

**不变量 / 限额**：请求来源/方法proto精确、outputstatus真实、无stdio污染；reject路径不能凑stub通过。

**测试**：Q05通过公开反编译interface运行F05→恢复Java→javac→行为Driver，验证实际返回/语义、原始身份与降级完整性。复杂overload/跨DEX在F10/Q04覆盖；不启动get-class/get-method进程，不测flags、默认stdout/JSON包装或not-found文本。

**验收**：PG + `cargo test -p diana --test product_contracts h05_` +引擎B(F05)。CLI接线做源码审阅，不加黑盒测试。**完成**：新module真实语义闭环通过，CLI已调用该interface，不仅链接新crate。

**不扩展**：新增MCPgetclass/tool、抛异常stub、未验证shape强行Java、立即删除所有旧依赖。

## H06 — 请求取消、分层预算和输出发布

**依赖**：H05、T09。**范围**：CLI/control/engine输出。

**文件**：rootinput/request/engineadapter/CLIemit；`crates/diana-decomp/src/pipeline/control.rs`；测试privateinput/interrupt/backpressure。

**输入→输出 / Interface**：callerstop/UserInterrupt→RunContext→pipelinecheckpoints；perrequestbyte/work/outputlimits→有效Options；完整text/JSON或明确Resource/Cancel错误。

**步骤**：
1. 取消callback不引入tokio到engine；每decode/worklist/copy/region/emitcheckpoint检查，Cancelled不继续rawfallback。
2. 用户文件/scope结束后不发过期结果；同请求cacheloader/reader的owner/reservation释放，失败无半缓存，nohardtimeout承诺。
3. clampengine预算到scope剩余额度，dependency与mainview合计收费；已有queryinflatelimits不误套methodASTlimits。
4. 输出分text与JSON精确有界writer，escape/diagnostics/spans计最终字节；JSON超限无截断prefixsuccess，file用临时输出原子发布策略（最终路径替换语义先测平台）。
5. CLIstreamqueries的brokenpipe停止producer/背压；不要把SIGPIPEkill进程称catchable取消保证。

**不变量 / 限额**：取消协作式；libdeflate不可抢占明确；input64MiBengine限制与256MiBinflate限制各自Resource；没有MCPsemaphore/schema新合同。

**测试**：Q06通过公开请求/反编译/提取interface验证长工作取消、owner生命周期、normal/raw预算隔离、大量转义文本的输出字节预算、spans对应实际source、缓存失败reservation、consumer关闭时队列有界。不是简单JSON字段包装或私有phase调用顺序测试；每case有有限timeout。

**验收**：PG + `cargo test -p diana --test product_contracts h06_` +T09/T25control回归。**完成**：exactoutputlimit和取消state有非空执行证据；不能先构造巨String才声称allocationbudget前置。

**不扩展**：MCPclose/status/permit设计、crash隔离子进程、任意实时抢占、跨进程缓存服务。

## H07 — 十一个命令的统一帮助、输出与 agent 文档

**依赖**：H06、H09、H10。**范围**：CLI消费者与使用文本。

**文件**：`src/cli/{args,mod,agent_skill}.rs`、`skill/SKILL.md`、README；历史来源保留。本卡不新增CLItests、help快照或参数路由测试。

**输入→输出 / Interface**：CLI.md的11命令、指定列、规范at、默认Java、显式JSON、extract例外正式冻结；agent使用说明作为普通文档/发行资产，不提供公开skill安装命令。MCP仍延期。

**步骤**：
1. 帮助恰含get-class/entries/extract/strings/xref/manifest/classes/get-method/fields/methods/help及指定说明/列；TSVheader/escaping与JSONdomain fields按CLI.md，不加旧memberprefix。完整method selector包含name+proto，不再--method/--proto分开组合；at可回传。
2. 默认Java失败exit1、usage2、唯一对象缺失3/歧义4，查询空集合0、显式JSON完整Mixed report0；requestBUG/Resource/Cancel失败。旧getclass/findrefs/skill/require-java/class-id/members/paginationflags明确拒绝。
3. agent文档改用Diana新命令，说明字段/at/JSON、显式外部d8一次转换及复用；不自动移动/删除用户已安装rascskill，不加第十二个skill命令。
4. stdout/file/help/version/Windowsbinarytarget完整；JSON字段version是本地CLIreport，不据此设计未来MCP。
5. 所有liveCLI品牌消费者改名清单核对；legacyrelease/provenance不是remainingbug。

**不变量 / 限额**：unknownflag不可静默忽略；默认source不混Smali，JSONstatus按typedreport；TSV字段控制字符/孤立surrogate转义不丢值，at与spans准确；extract二进制只写-o，不能依通用stdout复制规则污染。

**测试**：不新增自动化测试。Q07是CLI.md/源码/agent文档的一致性审阅清单，不是测试族；help、路由、flags、列名、旧alias、默认包装不写黑盒/golden。复杂UTF16保真、身份、spans与提取安全由F03/Q04/Q06/Q09/Q10测试，不重复测基础排版。

**验收**：格式/lint/构建检查 + Q07人工审阅，无h07_测试filter。旧测试保存在 ref-fork，不删除但不进入活动 gate。**完成**：十一命令接线与文档一致，复杂领域验证由对应任务提供，MCP不设计。

**不扩展**：MCPinput/outputschema、自动改用户agent配置、全局旧品牌license/historyrewrite、发布。

## H08 — CLIProductRC 与旧代码退役清单

**依赖**：H07、T25、T14。**范围**：整体验收与发行配置准备，MCP仍延期。

**文件**：活动 rootCargo/lock、未来 Diana release 配置/README/AGENTS、应用 query/engineadapter、`apps/diana/tests/product_contracts.rs`；核对无 ref-fork 依赖，不维护旧MCP构建。

**输入→输出 / Interface**：EngineCoreRC+H01–H07证据→CLIProductRC报告、livecallgraph/依赖退役清单、单diana发行配置。验收无序输出的完整性/身份，不要求跨运行source或报告序列一致；本卡不创建tag或上传release。

**步骤**：
1. CLI.md命令/列/路由由源码和文档审阅核对，不新增黑盒；复杂defined-vs-referenced、xrefsite完整性、extract原bytes/CRC/file安全、跨DEX/041身份与引擎行为由Q02–Q06/Q09/Q10和F测试验证。Q01/Q07仅人工清单，Q08汇总，不当执行测试族。
2. 核查活动 CLI/依赖图无旧 dexdec/MCP/helper 或 ref-fork 调用；旧代码完整保存在参考中，不需编译/跑测试，不为 MCP 新设计 adapter。
3. ref-fork 不作为生产依赖或 gate，保留原源码/测试/许可/历史，不以当前参考目录存在当作第二发行产品或必须测试的遗留依赖。
4. 独立审阅未来 Diana releasebinary/version/archive/checksum/Windows 配置；旧 workflow/scripts 已归档不复活，历史 release 不删除，不手工跳版本/发布。
5. 运行仅 Diana 的 PG 与复杂 H/引擎 CoreRC gate，基础 H01/H07 只审阅，不造空测试；授权代表输入的 release 测量明确 not-run，不运行旧 MCP 回归或发布测试。
6. 列knownUnsupported/rawcoverage与MCP延期说明：不把CLIProductRC说成全部projectinterfaces已迁移。

**不变量 / 限额**：无静默跨旧enginefallback；singlebinary，不改版本/tag；活动代码须验证；旧参考不参与本分支 gate；不宣称全DEX语义正确。

**测试**：Q08汇总复杂语义/领域测试：跨DEX/041身份、输入/提取/取消/预算，以及T25的行为和降级完整性。不建立整个CLI命令矩阵或branding/help/flags黑盒；CLI接线、旧依赖和发行配置采用源码审阅/本地构建检查。

**验收**：PG + Q02–Q06/Q09/Q10的实际复杂测试与T25核心gate（非零运行）；Q01/Q07人工核对、Q08汇总，无空h08_测试要求。releaseartifact本地检查，不发布。**完成**：复杂能力有真实证据，MCP延期明确，基础接口不另建自动化测试。

**不扩展**：MCP迁移、直接删除src/mcp、发行/tag、自动下载私有语料、无证据cache/parallel优化。

## H09 — 定义符号清单与跨 DEX 引用 site

**依赖**：H04。**范围**：fields/methods/xref与指定列，不运行反编译器。

**文件**：root `src/analysis/dex/{mod,members,opcodes}.rs`、`src/analysis/query.rs`、`src/cli/{args,mod}.rs`，按需新增`symbols.rs`/`references.rs`；`tests/product_contracts.rs`的Q09。

**输入→输出 / Interface**：每个class_data实际字段/方法定义→field-at/method-at单列；每个实际opcode引用site→`entry, context, match`三列。methods不是全部method_ids references，xref不是旧findrefs的一方法一行；CLI.md是唯一字段/选择器规范。

**步骤**：
1. 遍历class_defs/class_data，分别读static/instance fields、direct/virtual methods的checked delta index与flags/codeoff；生成携class-def/member-id/owner/name/proto的规范at，abstract/native/constructor/clinit声明也列。
2. fields/methods `--class`精确选择原定义，`--filter`按语义名称/descriptor字面匹配，输出全部匹配；at与重复classdef核验，不靠排序/位置identity。
3. xref kind string/type/method/field精确解析target，扫描全部rootDEX/logicalmembers的原code，读取opcode operand/refindex，不从恢复Java字符串搜索。记录referencing MethodKey、code-unit offset与actual matched value/ref。
4. context拼method-at加`@cu:0x...`，同方法多个真实site各列；对同site多个operand引用按明确operand-site身份建记录，禁止沿用方法级去重掩盖site。
5. 保持035与已有041扫描格式边界，knownunsupported opcode/profile按原安全合同处理；只为资源背压保留batch，按完成输出，不global sort或分页。
6. 按CLI.md输出TSVheader/escaping或JSON items；字符串真实值UTF16精确与root原scanner展示差异要测，不能replacement后做精确xref。

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

## 2. 产品验收矩阵

| 记录 / 复杂测试族 | 产品义务 | 相关任务 | 证据 |
|---|---|---|---|
| Q01 人工审阅 | root/internal命名与基础CLI规格 | H01/H07/H08 | 源码/文档核对和构建，不是自动化测试族 |
| Q02 InputLease | 普通文件/快照/借用/变化竞态/limits | H02/H06 | private自制文件隔离、无悬空borrow、资源释放 |
| Q03 QuerySemantics | 多DEX/重复entry/manifest选择、记录与UTF16完整性 | H03/H08 | 公开module的identity/值、并发与资源边界；不测CLI |
| Q04 OriginalViews | 041/重复definition/规范at往返 | H04/H05/H08/H09 | 未改rawbytes、原identity/proto精确、invalidkeys拒绝 |
| Q05 DecompileBehavior | 公开反编译接口→Java行为回环 | H05/H08 | F05语义/身份/降级证据；CLI仅人工接线审阅 |
| Q06 ControlBudget | 取消/bytes/JSONescape/spans/cache/backpressure | H06/H08 | 阶段检查点、独立fallback预算、原子输出与loaderreservation |
| Q07 人工审阅 | CLI.md与命令/帮助/列/agent文档一致性 | H07/H08 | 代码与文档对照，无help/路由/列名黑盒测试 |
| Q08 验收汇总 | 复杂语义/安全证据与接线/发行审阅 | H08 | 复用已有复杂测试，不另建简单CLI矩阵 |
| Q09 SymbolsXref | fields/methods definitions、xref实际sites/指定列 | H09/H07/H08 | at往返、defined不含外部reference、同方法多个site、UTF16/identity精确 |
| Q10 OriginalExtract | 原payload/CRC/覆盖竞态/alias/资源失败 | H10/H08 | 公开提取interface字节相等、无normalize、失败目标未变；不测CLI输出/参数路由 |

本轮不运行这些未来命令。产品与引擎任务只通过文档一致性审查，真正完成须由实施Agent在对应代码和工具链运行后提供证据。
