# 本次设计交付审查

**当前状态以 §13/STRUCTURE 为准**：活动 workspace 只含 Diana，旧 rasc/dexdec 已归档 ref-fork，不构建/测试；§1–§12 是此前逐轮记录，不是当前 gate 要求。

**范围更新说明**：用户已将整个项目（不是仅引擎）定名为 Diana；root包/CLI为`diana`，内部crate为`diana-decomp`，环境变量`DIANA_`。PROJECT与PROJECT_TASKS补充CLI/输入/轻量查询整合；MCP明确暂缓，没有新协议/schema设计。已有`feat/dena`实验分支已按用户要求删除；不迁移其代码。下文原审查表是先前引擎文档的历史审查，不代表新增H线代码已验收。

## 1. 交付范围与状态

本次只新增 `docs/diana/` 文档集：README、DESIGN、MODULES、ALGORITHMS、IMPLEMENTATION_PLAN、TASKS、VERIFICATION、SOURCE_MAP、RECOVERY_EXTENSIONS、本文。

没有实现新引擎，没有修改生产 Rust、Cargo、CLI、版本或测试 fixture；没有发布、提交或切换生产引擎。新 Diana 从零设计，不以现有实验分支为兼容或迁移目标。未来任务中的命令不是本次已执行的新 Diana 测试。

用户补充“美化可以可选，以后扩展”已落实：RECOVERY_EXTENSIONS 定义独立开关、候选回合、Applied/Declined/BUG、前置证据与预算；E01–E08 不成为 CoreRC 依赖。T24 是首个可选实现任务，T25 核心加固不依赖它。

## 2. 已实际执行的文档检查

最终审查使用只读 Python 检查文档内容，以及 Git/本仓审计命令：

| 检查 | 实际结果 |
|---|---|
| 任务卡与依赖图 | 25 张 T01–T25，25 个依赖节点，唯一 ID、依赖一致、无环 |
| 每卡字段 | 文件、Phase、输入输出/interface、步骤、不变量/限额、测试族、验收命令、完成定义、不扩展范围均存在 |
| Fixture 族一致性 | TASKS 与 VERIFICATION 中 F00–F25 共 26 族一致；其正例/拒绝例/工具要求均为未来实现合同 |
| 需求覆盖 | VERIFICATION 的 R01–R16 全有设计定位、任务与证据层 |
| 源码证据 ID | 30 个 KU/DX/RG 锚点；文档引用 ID 无未知项；SOURCE_MAP 固定三个参考提交与审阅状态 |
| 链接 | 相对文件链接指向文档；README 有全部交付文件入口。未来源码路径明确标为未来，不要求当前存在 |
| 接口一致性 | ClassKey/MethodSelector 与本地 MethodKey/FieldKey、外部 MethodRef/FieldRef 分离；P7 返回 PlannedBody，不丢 StorageBody |
| 调度交接 | P4 生成声明事实与 binding obligations，P5 求类型/use，P8 最终验证 source binding；不循环要求 P4 提前知道 P5 类型 |
| Raw 校验交接 | RawMethodView 只保证 ranges，ValidatedRawMethod 才能进入完整 smali adapter；确定损坏输入不因 raw 格式曾通过而标 Smali 成功 |
| 局部不动点 | P3 固定身份环境/liveness/availability 与 P5 有限类型 worklist 分开；未把整个 band 反复重跑 |
| 方法/类验收 | T06 明确 method-in-known-context，不把未支持 default constructor 的 Mixed class 算 JavaUnit；完整 unit 验收不 splice 修补 |
| 输入/异常/锁关键反例 | ALGORITHMS 与 VERIFICATION 列 parallel cycle、critical edge、throw 前旧值、扩大 try、新增 cast、monitor identity/release、enum/constructor 限制 |
| `git diff --check` | 通过；由于新增文件尚未 tracked，另逐文件检查行尾空白/NUL，而不只依赖 Git 检查 |
| `python3 tools/audit/desensitize_check.py --range HEAD` | 通过（9 个敏感 token）；该命令仅覆盖 tracked diff |
| 新文档逐词扫描 | 使用本仓同一词表扫描全部新增 Markdown 内容与路径，未命中；避免 untracked 文档不在 tracked diff 中漏检 |
| 工作区范围 | `git status --porcelain=v1` 仅报告新增 `docs/diana/`，未改 tracked 代码 |

这是文档一致性与可执行性审查，不是算法运行成绩或形式化证明。没有在本目标里重新运行旧语料比较、编译新 Diana 或生成真实行为基线。

## 3. 审查中修正的重要问题

1. 撤回早先“javac 通过即语义正确”“忽略缺依赖的编译错误算通过”“LOC 有确定可删比例”等过强结论，SOURCE_MAP 单列说明。
2. 一定义一 local 只省变量合并；正常平行 copy、异常 shadow、定义可用性和类型约束仍必需。
3. 明确 P3 也有阶段内有限传播，不再说仅 P5 有 fixed point；所有循环必须工作收费。
4. 将 raw ranges 安全与完整指令/metadata 校验分开；尚无 metadata 支持的输入不能标“半个完整 smali”。
5. 将原始 DEX identity 与 Java 静态绑定证据分阶段处理，消除 calls/type interface 的先后矛盾。
6. 统一 Java 源码目标 `--release 8` 与可使用 JDK 21 工具；构造器不能通过升级语言规则暗中放宽支持。
7. 明确 default constructor 尚未支持时的早期方法测试上下文，不通过删构造器/插语义 stub 假装整类通过。
8. 明确 normal dispatcher 仅在无 handler/monitor/未处理 uninit 且类型/storage 验证通过时使用；不把它当通用 Java 解释器。
9. T10 的无 P4 intrinsic 证据 bitcast 子例保持 Unsupported，不能用将来的 T11 成为 T10 隐含依赖。
10. 可选美化不等于允许错语义；Declined 保留基线，BUG 仍是失败信号，未来扩展逐项验收。

## 4. 仍需实现阶段验证的风险

详细风险见 DESIGN §12、VERIFICATION §8，最低试验如下：

- **异常 SSA / shadow**：F16、F17 和 M15 验证失败前寄存器值、多个 throw site、精确 snapshot 类型与位置。共享 handler、跨区域 join 不能由简单正例推广。
- **Java 绑定 / raw generic**：F10、F21 检查 null overload、bridge、visibility、throws、父类契约；缺 catalog 要局部降级，不能填默认 public 事实。
- **构造/初始化**：F12、F20 与 U02/U05 验证 allocation/init 顺序、super/this、final 单赋值、class-init trigger。万能 helper 不是承诺。
- **SSA/类型表示**：F06、F09、F16 与 M06/M09 区分 pending、undefined、conflict、literal 多 use、wide overlap；不能默认转 Object/null。
- **结构与 monitor**：F18、F19 核查保护域、catch 顺序、lock identity、all-exit release；复杂 protected graph 拒绝普通 dispatcher。
- **验证环境**：JVM 源码闭环不等于直接 ART 双 DEX 行为验证。人工 smali 的 scalar case 只用受限 tests-only oracle；异常/monitor 没有适用执行环境时不能算 V5 已通过。
- **许可与 fixture**：复制具体算法前逐文件确认 LICENSE/NOTICE/修改登记；本次没搬 RG corpus/pickle，也没有假设它们授权已知。

这些是明确的实施任务/支持限制，不以“计划完成”冒充“算法验证完成”。如实现中的反例迫使改变总体成功契约，先询问用户，不通过改 normalizer 或缩分母自改目标。

## 5. 可立即开始的执行顺序

产品线先H01冻结命名/CLI合同；H02/H03复用安全输入和快查询，与引擎线交错，H04/T04→H05/T06尽早贯通CLI。引擎首批 **T01 → T02 → T03 → T04 → T05 → T06**。先获得安全输入、明确 raw 失败通路与真实可执行的静态方法闭环，再按能力扩展，不先写完每个 phase 的全部内容。

Agent 接手：README → PROJECT/PROJECT_TASKS → DESIGN → MODULES/ALGORITHMS → IMPLEMENTATION_PLAN → 对应H/T任务卡与验证。T卡不自行切root默认，H05/H08负责Diana CLI新路径接线与替换；MCP不设计、不删除。未来命令并未执行，未支持形态保持raw。

本次设计交付可以用于开始实施；它不宣称新 Diana 已存在、达到 CoreRC、减少具体行数或超越任何参考引擎。

## 6. CLI 产品范围更新的文档检查

后续用户确认Diana是整项目，并明确MCP暂不设计。已新增PROJECT/PROJECT_TASKS，H01–H08产品任务与T01–T25引擎任务相互衔接；root包/CLI=`diana`，内部crate=`diana-decomp`。移除了拟议的新MCPschema/envelope/tool迁移合同，现有MCP代码保持不动/必要回归，不成为CLIProductRC前置。

实际只读检查：33个H/T依赖节点联合无环，各卡依赖/文件/interface/步骤/不变量/反例/验收字段一致；相对文件链接、逐词敏感信息/行尾空白检查通过；网页数据刷新后8张H卡/25张T卡一致。Node语法检查通过，headless浏览器验证项目总览、H→T跳转、内部crate路径、搜索和MCP延期标记通过。只更新docs/diana，未执行未来Rust任务、改Cargo/CLI/MCP或发布。原第2节的30源码锚点/25卡审查只针对当时引擎计划，不冒充H线代码验收。

## 7. 输出顺序合同的有意取消

用户明确不需要稳定顺序。PROJECT §6、DESIGN §1.1及H03/T25已调整：查询、独立方法/类报告与跨任务diagnostic/trace按完成/到达输出，不引入稳定排序/tie-break、按输入ordinal等待或重排缓冲，不设stable-output模式；这有意识改变原仓库的确定性合同。实现时更新相应现有测试/指导，本次只修改计划，不改AGENTS/源码或延期MCP行为。

验收改为全量结果按原identity核对多重集合、无重无漏、每份source合法与spans指向本次文本，不要求跨运行字节/合成ID/局部名相同。用户随后取消查询分页/数量截取：成功查询返回全部匹配记录，不再支持 `--offset`/`--limit`；文档看板按任务/phase编号排列仅用于阅读，不是执行要求。

保留原始程序的指令、控制转移、参数/副作用求值、catch优先级、monitor和初始化顺序；保存原始定义身份与请求内引用不意味着跨运行排序。取消排序是否实际增益需release测量，未承诺任何性能比例。

## 8. 查询分页能力移除

用户明确不需要offset/limit。PROJECT/PROJECT_TASKS/README/VERIFICATION及网页已删除“按本次到达流计数、跨调用不同子集”的方案。未来H01/H03/H07移除CLI分页flags、skip/take截取逻辑和相关help/skill用法，不新增cursor/preview替代；验收旧flag拒绝、成功查询全部匹配记录不重不漏。

这不删除DEX字节offset、原始身份索引、资源limits或内部有界batch/backpressure。filter与流式结果保留；安全预算耗尽明确报错，不能返回前N条且伪装完整成功。本次只更新计划/网页，未改当前CLI源码或延期MCP接口。

## 9. 十一项 CLI 接口重新约定

用户指定get-class、entries、extract、strings、xref、manifest、classes、get-method、fields、methods、help，新增CLI.md作为公开命令、指定columns、参数、转义、at与退出码的唯一规范。旧getclass/findrefs/skill、class-id/memberprefix/require-java、分页和stable模式不保留；历史源码符号仅作出处。

默认text获取只Java，不把Mixed/Smali伪装Fetch as Java；显式JSON报告保留引擎语言状态。fields/methods列实际定义而不是外部ID引用，xref逐指令site不是一方法一行；extract只将原physicalentry完整解压payload写必需-o，不改DEXheader/checksum、不切041，文件覆盖/alias/CRC/资源失败合同明确。

新增H09定义符号/xrefsites、H10原entryextract，两图现在共35个H/T任务。H07依赖H06/H09/H10后统一全部命令，H08验收Q01–Q10。网页按CLI.md生成十一命令/指定列和H01–H10任务详情。本次未修改生产CLI、生成代码/fixtures、发布或变更MCP。新增语义需要实施时实际工具/行为/安全测试，文档存在不代表已实现。

## 10. 测试范围收窄：复杂语义优先，不测基础CLI

用户明确不需要CLI黑盒或简单测试。VERIFICATION/PROJECT_TASKS/CLI/README/TASKS已更新：help/version、参数/命令路由、列名/基础包装、alias/分页flag、简单enum/getter/转发只做实现和源码/文档审阅，不新增golden或自动化案例。H01/H07不要求测试filter；Q01/Q07为人工清单，Q08为汇总，不能作为“已执行E2E”计数。

保留公开库/module interface的Java行为回环、raw/smali组装与内容比较，聚焦Phi/循环、类型/FP、调用/构造器、异常/cleanup、monitor/初始化、降级完整性；输入/原始身份/041/引用内容/提取CRC与文件竞态/资源取消属于复杂领域安全，也不通过Diana CLI子进程测试。现有测试代码本次不删除。

提前准备fixture/独立期望/harness与逐能力接入执行区别明确，不声称场景穷举完所有DEX。本次仍只改设计/网页，没有开始编写E2E代码或生成binaryfixtures。

## 11. 测试设计前置与团队可复现路径

用户要求案例先于引擎开发、共同开发不得依赖个人路径。新增E2E_CASES/e2e_cases.json：70个D001–D070具体场景、输入recipe/参数、独立期望、应抓错误、oracle与seam；状态是`designed-not-built/draft-1`，**不是源fixture、DEX或已通过E2E**。TEST_PREPARATION定义B01–B04：规范审阅冻结→公开工具内容锁/自制源→独立输入与基准认证→harness故障证伪。B04是T01/H01前置，T05只接真实public库，不等开发时再改案例/填工具/expected。

required suite成员不能按当前实现能力过滤/skip，本应Java的case不可改Smali成功；新增反例可追加，真oracle错误须独立证据/单独审阅修正。F/Q只作历史能力映射，任务→D的固定组在E2E_CASES指定，简单CLI仍不测。示例源码recipe可含伪代码，B01先修正规范，B02/B03先落实/验证关键DEX指令义务再开发引擎。

SOURCE_MAP改成公开HTTPS repo+commit+相对符号，去除作者home/兄弟仓目录。发现radguard是独立git仓而非父仓普通目录，改用公开MG1937/radguard的实际commit `290de20c925c4a2c7642b1a63c9997185a06e130`；公开manifest/README/source入口可获取。独立仓无明确LICENSE、manifest无license，父仓许可不自动覆盖gitlink；在权利人许可确认前禁止程序/fixture复制，采用自行实现/自制输入。

REPRODUCIBILITY定义公开工具坐标/version与B02完整checksum/transitive依赖锁、仓库相对tests/e2e资产、可重建.cache/e2e及系统临时目录API。去除个人SDK/配置绝对路径；smali用明确mainclass+锁classpath，不假定裸jar可java-jar。当前仅公开入口存在性核查，工具锁/source/fixture/runner未生成，B02/B03尚未执行，不claim跨平台准备通过。

文档与网页数据更新后检查：70case ID/字段一致，39个B/H/T依赖节点无环、B04先于T01/H01，各卡依赖与源码/文件链接有效，无个人绝对路径token依赖；网页新增测试准备与70case详情。只改docs/diana，无生产代码/实际引擎测试/发布。

## 12. 同仓双项目结构先行（用户后续明确授权）

最新要求先搭项目结构：根 rasc 的生产代码/入口/版本保持不变，新增 `apps/diana/` 独立 binary package 和 `crates/diana-decomp/` 文档级空库，使用独立未发布骨架版本 0.0.0、publish=false、unsafe_code=forbid。当前 binary 只报告未实现并失败，没有 Engine、CLI 命令或空 phase；Cargo 依赖只确立 diana→diana-decomp，旧 rasc→dexdec→rusty-dex 不变。STRUCTURE.md 是当前路径事实与早期 root 设计路径的覆盖说明；新产品应用根在 apps/diana，不重命名/移动旧源码。

用户授权仅纯包/构建/发布隔离先行，B04 仍是 H01/T01 功能前置，不把空库可编译或零测试当任务完成。70 个案例规范与独立 expected 保持逐字不变，fixture/tool lock/runner 仍未生成。不新增简单 CLI/getter 黑盒测试。

workspace 默认成员为根 rasc，裸 Cargo 命令保持旧产品选择，Diana 用 -p 明确选择。两产品共享根 Cargo.lock/target，新 lockfile 仅增加两个本地包，不更新外部依赖。发布 workflow 明确 -p rasc --bin rasc，版本脚本只处理 rasc/dexdec/rusty-dex；扩充现有发布准备测试，验证 rasc 发布不改变 Diana manifest/version。新 Diana workflow 仅格式/lint/构建，不运行零测试冒充 E2E，也不发布。

另按用户要求删除仓库根 NOTICE，移除 README 活链接、当前发布打包及任务中对根文件的引用；不新增/修改 LICENSE，各 crate 现有 license 原样保留，历史 release 记录不改写。没有发布、tag、commit 或 push。

本地验证（macOS）：
- Rust 1.93.0：Diana 两包格式/clippy/release build 通过；workspace all-target/all-feature clippy 通过。
- 当前 Rust 1.98：workspace 格式、测试与 release build 通过；既有测试 165 通过、6 个 opt-in/benchmark 忽略，不算真实 APK 验证。新骨架零测试不计 Diana 能力验收。
- Rust 1.98 workspace clippy 被未修改的 rusty-dex 两条新 lint 阻塞（classes.rs:971 collapsible_match、code_item.rs:85 chunks_exact_to_as_chunks）；未通过放宽 lint 或改旧代码吸收。
- 发布准备现有 8 个测试通过。首轮子脚本误用系统 Python3.9 缺 tomllib，改用 Python3.14 的 python3 PATH 后重跑，不修改期望；CI 原有 Python3.12 配置不变。
- workspace metadata/dependency tree、无旧源码改动、看板刷新/文档链接、70 案例未变、敏感信息/空白检查通过。Linux/Windows CI 未执行，不声称跨平台或 E2E 已验证。

## 13. ref-fork 归档：本分支只开发/构建 Diana

用户否决同 workspace 双项目继续构建测试的安排。已将全部 rasc src/tests/skill、dexdec/rusty-dex、axmldecoder/vendor、旧 MCP 文档/release 记录、发布脚本及 workflow 迁入 `ref-fork/rasc/`，保留独立旧 Cargo workspace/lock。root 变为 Diana-only virtual workspace，两包均为 members/default-members，exclude 指定 ref-fork/rasc；删除活动旧 package/dependencies/patch。主 lock 仅 Diana 两个本地包，旧 lock 与 HEAD 原锁逐字相同。

旧 workflow 位于参考工程的嵌套 .github/workflows，GitHub 不发现/触发；旧 release tests/code-mode 工具也随工程归档。撤回前一轮为双活动 workspace 添加的旧发布测试改动，归档脚本恢复已提交版本；没有运行这些脚本或旧工程 build/test/lint/format。活动 CI 使用 root workspace 命令，仅 Diana 的格式/lint/release build，没有空测试或发布。root 与 package AGENTS/README、STRUCTURE/PROJECT/H任务/看板均同步当前边界，不再要求旧 MCP 回归或旧 lint。

迁移前记录 268 个 tracked 源/测试/说明/工具文件的内容 hash，迁移后 264 个完全一致；4 个明确差异为两份旧 release 脚本/测试恢复 HEAD，以及输入排序测试的中性包名、PATCHES 的中性语料说明。后两项为防止文件移动重新引入审计禁词，不改旧生产逻辑、算法或测试期望义务。旧 crate LICENSE 原样保留，根 NOTICE 不恢复，不新增许可 bundle。没有移动私有 cache/旧 target/忽略 binary，也不自动清理用户已有构建目录。

验证只针对活动 Diana：Rust 1.98 与最低 1.93 的 workspace clippy(all-target/all-feature)/release build，以及格式检查通过；完整 cargo metadata/tree 中仅两个 Diana 包，无 ref-fork 依赖。参考仅静态核对 manifest/本地依赖路径/lock 与源码 hash，不启动任何旧 build/test。没有跑零用例 cargo test 冒充能力通过。

70 个设计案例/expected 与 HEAD 逐字不变；B01–B04、Engine/CLI 功能仍未实现，不因主工程构建成功标 H01/T01 或 E2E 完成。文档链接、看板/DAG/JavaScript、敏感信息扫描和空白检查通过；CI Linux/Windows 未运行。迁移尚未提交或发布。
