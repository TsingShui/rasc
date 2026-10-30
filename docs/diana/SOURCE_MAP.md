# Diana 参考源码、证据与复用约束

状态：设计阶段源码审阅记录，不是跨引擎性能或正确性评测。本文固定参考版本；链接中的行号只对该版本有效。新 Diana 从零设计，不以既有实验分支为迁移目标。

## 1. 团队可获取的参考版本

参考由**公开仓库URL、固定commit、仓库相对文件/符号**定位，不依赖作者的home目录、已删除worktree、父仓磁盘布局或SSH权限。公开内容核查：三个独立仓库的固定版本入口均可通过HTTPS获取；这不证明所有fixture的授权或算法正确。

| 标识 | 公开来源 | 固定提交 | 相对根 / 核查说明 |
|---|---|---|---|
| DX | [TsingShui/rasc](https://github.com/TsingShui/rasc) | `ef8a4c92c6b89e8afed9ccbc1552feae67b6982d` | DX路径相对`crates/dexdec/`；root/rusty-dex路径另明确标注。[固定manifest](https://github.com/TsingShui/rasc/blob/ef8a4c92c6b89e8afed9ccbc1552feae67b6982d/crates/dexdec/Cargo.toml) |
| KU | [Noelo-Lab/kuna](https://github.com/Noelo-Lab/kuna) | `29c44b57f8c1d9b35044aa4ee3f636c193288009` | 路径相对该仓根。[固定phases文档](https://github.com/Noelo-Lab/kuna/blob/29c44b57f8c1d9b35044aa4ee3f636c193288009/docs/phases.md) |
| RG | [MG1937/radguard](https://github.com/MG1937/radguard)，`0.1.3` | **`290de20c925c4a2c7642b1a63c9997185a06e130`** | 路径相对radguard自己的仓根。[固定manifest](https://github.com/MG1937/radguard/blob/290de20c925c4a2c7642b1a63c9997185a06e130/Cargo.toml)。此前误把父仓commit当radguard commit；父仓只含gitlink，此处更正 |

按[SOURCE_MAP.md](SOURCE_MAP.md#10-可复现输入与工具)在当前checkout内显式获取固定参考；参考只用于审阅/实现，不是生产或E2E测试运行的隐含依赖。离线协作者使用按相同commit验证的源码包；不能自动改用main或作者机器路径。

当前 checkout 的旧源码已整体归档到 `ref-fork/rasc/`，不参加活动构建/测试。读取上表 DX/旧 root 的固定路径时在 checkout 加此仓库相对前缀；公开 URL/commit/path 锚点不改，见 [DESIGN.md](DESIGN.md#1-产品边界与工程范围)。

补充产品范围：Diana是整个项目，活动产品/CLI为`diana`，内部引擎为`diana-decomp`；现有rasc源码作为CLI/输入/轻量查询的复用来源，见[DESIGN.md](DESIGN.md#1-产品边界与工程范围)。不设计MCP协议、schema或迁移。反编译算法独立实现，不移植实验分支。

核查范围为源码/公开入口/文档，不据此声明语料覆盖率。固定commit不跟随HEAD变化；任何reference更新须独立审阅，不能静默用另一个版本。RG父仓gitlink不把父仓Apache声明自动传递为子仓许可，见第6节。

## 2. Kuna：借机制，不借完整引擎

| 证据 ID | 文件 / 符号（固定版本） | 源码观察 | 新 Diana 决策 |
|---|---|---|---|
| KU-01 | `docs/phases.md`：Phases、Band B、Source layout | 阶段是产物分类，P0 正交；Band B 内顺序不由目录编号决定 | 物理目录按产物所有权，实际执行顺序独立列出；不假设 P1 到 P9 只各跑一次 |
| KU-02 | `decompiler/crates/kuna-decomp/src/infra/universalaction.rs:46–101`：`RuleRow`、`SchedNode`、`materialize` | 调度树有 Action/Pool/Group/RestartGroup，构造函数实际生成执行树 | 使用typed函数直接调用和Module内有界worklist；不搬动态插件、ActionDatabase或通用Repeat调度 |
| KU-03 | 同文件 `universal_sched:296`，`mainloop:550`、`fullloop:627` | `ActionBlockStructure` 就在 mainloop 中；types、heritage、rule pool 与后续控制图变换交织 | “Kuna 一定先做完全部数据流再结构化”不是准确执行描述。Diana按DESIGN §4的typed调用次序执行 |
| KU-04 | `docs/spec/00-overview.md §0.7`（约 `3432–3475`） | 逐条列出 trigger、持久状态、重入范围；有未实现反馈边的明确标注 | 只为已实现的生产者/消费者建立反馈；原始字节和用户配置不被算法重写 |
| KU-05 | `decompiler/crates/kuna-decomp/phases.toml`；`build.rs`；`build/registry.rs` | registry 与 codegen 提供 option/catalog 元数据，不等同于实际 schedule | 直接typed函数接线，不建registry/静态元数据生成器；PhaseId只标诊断。不引入TOML/build.rs第二份真相源 |
| KU-06 | `p8_structure/blockaction.rs`：`ActionBlockStructure`；`p8_structure/region_structurer.rs` | 有不同结构化策略及拒绝后的回退。部分模块头注释描述历史默认值，不能覆盖 live 配置 | 以实际默认/调用点为准。新 Diana 的例外是 Java 无 goto：普通图可回退状态机，异常/monitor 图不能无条件用它 |
| KU-07 | `docs/spec/07-regions.md §7.1`；`p7_regions/kuna_regiongraph.rs` | 私有图上归约，真实 CFG 不被结构化分析破坏；参考实现对输出相关遍历使用稳定次序 | 只借只读CFG/私有region arena和请求内identity，不继承其稳定排序/确定性输出要求；Diana独立结果按完成顺序发布 |
| KU-08 | `docs/observatory.md`；`docs/spec/08-structuring.md §8.4` | 可观测阶段产物、真实调度及质量代理；质量度量不自动证明语义 | 设计 `TraceEvent` 与 block/edge 覆盖证书，label/state 数只衡量可读性 |

不复用 SLEIGH、p-code、native calling-convention 推断、代码发现、栈帧布局、二进制别名分析。DEX 提供显式方法、寄存器和大部分声明，但仍有调用分派、解析、访问权限、异常及源码合法性问题，不能因此删除调用语义分析。

## 3. dexdec：借正确性问题清单与算法局部

| 证据 ID | 文件 / 符号 | 源码观察 | 新 Diana 决策 |
|---|---|---|---|
| DX-01 | `src/lib.rs:5–15`；`src/analysis/java_backend/method_pipeline.rs:53–179` | CFG/SSA → ExceptionAnalyzer → RegionGraphBuilder → RegionReducer → ValueRecovery → TypeSolver → SourceVariableAllocation → SourceSyntaxRecovery | 这是该引擎的算法依赖，不是 DEX 格式要求。新引擎不移植其结构化 Semantic IR 上的 ValueRecovery，因此可以先在 CFG 上求类型，再结构化 |
| DX-02 | `src/ir/passes/cfg_pipeline.rs:120–234` | input/SSA 验证、move-result 绑定、monitor/critical-edge 分割、SSA、constructor recovery | 保留这些问题的职责，但简化主路；所有拓扑编辑必须发生在 SSA 前或显式重建 SSA |
| DX-03 | `src/ir/passes/ssa_transform.rs`：`validate_exception_boundaries`（约 `200`），exceptional state（约 `288`、`427`） | 有异常边的 block 必须以唯一可能抛出的指令结束；异常状态不一定是正常执行后的状态 | Diana 每个受保护 may-throw 指令有独立 site，记录正常/异常寄存器环境；不能在 catch 上直接使用指令成功结果 |
| DX-04 | `src/ir/analysis/source_variables.rs:45–267` | 活跃性/干扰、coalescing、正常/异常 Phi placement、类型、词法退出处理互相配合 | 只去除**可选合并**；保留 Phi 类型、边拷贝、异常入口保存和词法 transfer 放置义务 |
| DX-05 | `src/ir/analysis/source_variables/phi.rs:1274–1361`：`ExceptionalCopyPlacement`；`:1785–1847`：`sequentialize` | parallel copy 周期要临时量；异常边拷贝要验证可用性并在抛出 site 前保存 | 独立 locals、显式 edge copy，正常/异常两种 copy plan 分开；不把一 Phi 一赋值说成完整解决方案 |
| DX-06 | `src/ir/analysis/effects.rs`：`InstructionEffects`、`ThrowEffect` | 读/写/调用/抛异常/同步等共享事实；未知调用保守 | 使用更少字段的 effect 表但默认保守；未使用的返回值不能成为删调用、删字段读、删异常的理由 |
| DX-07 | `src/ir/exception.rs`；`src/ir/exception/cleanup.rs`；`src/ir/region/{exceptions,synchronization,exits}.rs` | 处理 DEX 区间、handler 归属、cleanup 等价、monitor 及跨 region leave | 不整体复制；以受限可表达 profile、明确结构证书和 smali fallback 控制复杂度 |
| DX-08 | `src/language/java/{dex,source_types,literals,assignment}.rs` | boolean/int、null/0、泛型/擦除/重载、字面量、定值等合法化不是单纯排版 | primitive/null/调用精确绑定属于核心；泛型美化可后置，但不能忽略擦除契约与重载歧义 |
| DX-09 | `src/analysis/java_backend/constructor_syntax.rs`：`apply_with_method_returns` | 多种 pre-super 提取/调度/生成 helper；处理 Java 与 DEX 构造约束差异 | 接受范围为相邻new/init和合法constructor prologue；不做“任意前导都能静态 helper 化”的保证 |
| DX-10 | `src/analysis/java_backend/declaration_lowering.rs:1381`：`failure_body` | 恢复失败可生成抛 `UnsupportedOperationException` 的方法体 | 不继承该成功表达；失败保留 raw 方法并在结果模型显式降级 |
| DX-11 | `src/decoder/method_decoder.rs:650–675` | 未匹配 opcode 可生成 nop；word_at 的缺失值可为 0 | 新 reader/decoder 严格区分合法 NOP、未知 opcode、截断；禁止该默认路径 |
| DX-12 | `src/api/decompiler.rs:28–55,105–138`；`src/api.rs` | request isolation 清分析 scope，但 parsed metadata 保留；请求内 hierarchy/ABI 仍被消费 | 不能称缓存“全部死代码”。内部 Diana Engine 借用 root-owned DexImage/request scope，parser/IR/source缓存延后到实测需要；root已有解压复用有独立budget |
| DX-13 | `Cargo.toml [lib] test=false`；`src/lib.rs` lint allow；`src/analysis/java_backend/generator.rs` 等 Rayon 调用 | workspace 普通 test 不自动执行 legacy lib tests；并行 map/collect 可能维持顺序 | 不删测试换“体积”；新模块无blanket allow，必需测试进入gate；Diana不要求并行结果稳定序；引擎在调用线程执行 |

`crates/rusty-dex/src/dex/instructions.rs` 的 payload 与 `bytes()` 路径需要逐种核对；不把第三方 Instruction 对象当 fallback 原始字节。新设计选安全只读 reader，可参考现有解析实现，但绝不先信任分配后再加 guard。

## 4. radguard：借小型数据流与输出溯源，拒绝宽松成功

| 证据 ID | 文件 / 符号 | 源码观察 | 新 Diana 决策 |
|---|---|---|---|
| RG-01 | `README.md:3–13` | 自称 Java-like pseudocode，不承诺 Java 可编译 | 不将其输出当语义 oracle；“伪码”不自动意味着行为错误，也不能替代类型合法性 |
| RG-02 | `src/decompiler.rs`：`write_method`；`src/dataflow.rs:479–519`：`optimize_impl`；`src/control_flow.rs:6–33` | lowering → Graph → def/use、split_variables、DCE/propagation → structure → writer | 借鉴紧凑 dense ID、worklist 与分离分析/打印；新 Diana 明确 SSA 及验证，不整体套平坦 Graph |
| RG-03 | `src/ir/mod.rs:7–12`：`VariableId`；`src/dataflow.rs`：`ReachingDefinitions`、`split_variables_at` | 没有直接名为 SSA/Phi 的公共层，但有版本变量和 def-use 分析 | “grep 没 SSA 就没有定义身份”过强。应比较 join/exception/类型能力，不比较命名 |
| RG-04 | `src/writer.rs:17–43`：`MemberReference`、`SourceOutput::append` | 文本 byte span 记录方法/字段原始描述符；append 修正 offset | 纳入输出 OriginSpan；Java 局部重命名不丢原始成员身份；所有 span 指向最终 UTF-8 字节 |
| RG-05 | `src/writer.rs:140–176`：`visit_node` | writer 直接遍历 graph，跟踪 follow/visited/condition | 新 printer 只遍历 VerifiedJava AST；不能让 visited 丢 block 成为隐含 fallback |
| RG-06 | `src/dex/mod.rs:847–925`：`parse_code_item`、`parse_catch_handlers` | 有 checked_mul/range 检查，但 typed_count 可在完整验证前 Vec::with_capacity | Reader 先按剩余输入和总预算验证条目数，再分配；小体积不证明安全 |
| RG-07 | 同文件 `read_mutf8:1059–1121` | 对孤立 surrogate 用 replacement char 转 Rust String | Diana 用 UTF-16 code units 保存 DEX string，不接受语义改变；人类诊断文本与 literal 表达分开 |
| RG-08 | `tests/quality_gate.py:420–456`：`decompile_dad_cached`；各 `normalize_dad_*` | 冻结 DAD 输出、带 DEX-aware normalization；缓存缺失不自动生成。pickle 只宜可信内部输入 | 借“固定 comparator/明确基线”的纪律；不复制 fixture、pickle loader 或 permissive normalization；差分只是线索 |
| RG-09 | `src/types.rs`、`src/decompiler.rs` 的 `encoded_value` | access flag/source literal 到字符串的转换需要核查 Java 合法性 | synthetic/bridge/declared-synchronized 不能直接变 Java 关键字；原 flag 保留在 metadata |

## 5. 算法适用条件

1. 固定ValueId/PhiId只固定命名，不证明状态传播收敛；集合域、transfer与有限度量按
   ALGORITHMS §3–§4定义，不能继承参考论文对另一种域的证明。
2. Phi赋值必须按parallel copy语义；任意逐条发赋值可能丢旧值，检查实际edge placement。
3. 可编译、结构覆盖、有限输入行为和任意程序等价是不同证据，不能互相替代。
4. constructor helper不能合法访问任意未初始化this/锁/字段；按相邻new/init条件拒绝
   不可表达形态，不提供万能helper。
5. enum用语言规则生成，不以class extends Enum或删除synthetic成员凑成可编译输出。
6. 参考实现体积/流行度/发表场所不构成正确性证明；不预设可删LOC比例或覆盖率。

## 6. 复用登记与许可检查

实施时每次移植一个算法，新增登记：来源项目/固定提交/符号、原许可与 copyright、适配差异、不变量、测试、是否仍依赖原 helper。原则是“理解后重写或有记录地局部移植”，不是把目录搬进新 crate 再慢慢启用。

- DX 与 rusty-dex 各自保留 Apache-2.0 许可文件；任何复制须保留许可/copyright、记录修改并更新新分发清单。
- KU 的工作空间声明许可，含 Ghidra/angr 来源；复制具体文件前检查该文件头、根 LICENSE/NOTICE 和上游归属，不只看 Cargo 一行。
- RG是独立git仓库/gitlink，固定版本manifest没有`license`字段，也未找到该仓自己的LICENSE。**父仓Apache-2.0不能自动覆盖gitlink子仓**。引用设计/符号不等于获准复制代码；在权利人明确许可前，阻止radguard程序/fixture移植与分发，采用自行实现+自制fixture。此前“子目录沿父许可”的假设撤回；DAD衍生归属仍需核查。
- 本文引用符号和算法思想；不复制未授权第三方程序或corpus。若许可、fixture 授权无法确认，则阻止该项复制，保留自行实现与自制 fixture 路线；不阻止其他设计工作。

## 7. 一键获取的补充资料与阅读落点

```sh
python3 tools/fetch_references.py
```

[下载脚本](../../tools/fetch_references.py)顶部目录表是实际 URL/源码提交/选取目录的
唯一清单；当前 46 项，可 `--list` 查看或 `--only <id> ...` 只取本批材料。
保存到 `.cache/references/`，本地入口 `README.generated.md`。论文/讲稿只保留同名
UTF-8 TXT，不保留 PDF；已有 TXT 不重复下载，原始 URL 用于按需在线查阅。参考下载按用户要求
不做 checksum，不覆盖已有修改、不构建/测试，不导入生产依赖。详见
[SOURCE_MAP.md](SOURCE_MAP.md#10-可复现输入与工具)。

| 脚本 ID | 内容 / 主要用途 | 对应任务与限制 |
|---|---|---|
| dx / ku / rg | 本文三个既有参考版本；原始安全、流水线纪律、数据流与文本来源 | 固定版本不变；rg 只阅读，许可未明不复制 |
| dex-format / dalvik-bytecode / dex-constraints | AOSP 格式、指令和静态/结构约束 | P1/P2、D055–D059；优先规范，不照抄宽松解析行为 |
| jls-expressions / jls-execution / jvms-verification | JLS/JVMS 8 的求值/调用、初始化与验证 | T10–T12/T19–T23；语言目标和实际 JVM/ART 证据分开 |
| jd-tester / java-decompiler-bugs | 2024《Understanding and Finding Java Decompiler Bugs》与 artifact 说明 | 参考可执行小例子、失败分类、缩减方法；不下载 release 可执行物/bug corpus，不作为输出 oracle |
| jadx | 实现与 IntegrationTest/SmaliTest 的 Java 测试支撑源码 | E2E 准备时学输入/重编译/执行观察，不照搬文本 golden、内部 seam 或 skip 策略 |
| ssa-construction / cranelift | Braun 等 2013 SSA 论文与 Rust SSA builder/IR verifier | T07；借固定身份/Phi 与验证思想，不自动替换既定算法；仍需 DEX 异常状态证明 |
| out-of-ssa | Boissinot 等 2009 Out-of-SSA 论文 | T08/T09，D002/D003；先正确处理平行拷贝，再考虑可选变量合并 |
| smali | Google smali/dexlib2 的实现 | raw 投影/回环；不是 E2E 工具版本升级，assembler 成功不等于语义正确 |
| soot / dexpler | Soot Dexpler 类型/异常图源码与 2012 论文 | 类型/异常批次参考，不搬整个 Jimple/Soot 框架 |
| phoenix / sailr | 2013 Phoenix 与 2024 SAILR 结构恢复论文 | T13/T14；面向原生/C，不能直接套 Java 的无 goto、handler/monitor 限制 |
| vineflower | Java 声明与恢复源码 | 构造器/nested/enum 批次按需阅读，不把参考恢复文本当真值 |

每批最多选择一篇主要算法资料和一个实现参考，产出一个决策、不变量或反例；
不把全文精读/全项目构建变成新的开工前置。新增参考先在脚本固定 URL/提交，
记录用途与不适用范围，不自动更新旧期望或扩大实现范围。下载材料仅供开发查阅，
不因公开可下载便推断代码/fixture 可复制或重新分发。

## 8. 扩充研究：顶会、经典基础与成熟工程

这轮从“有哪些项目”补到“哪些结论能支持或挑战我们的设计”。选择顺序是：
**直接相关性 → 可核的一手证据 → 发表场合/长期影响 → 是否有公开实现**，
不为了凑齐四大而推荐无关工作。语言语义仍以 DEX/JLS/JVMS 等规范为依据，
顶会发表或高引用不会自动证明某个实现/fixture 正确。安全四大指 IEEE S&P、USENIX Security、CCS、NDSS；
EuroS&P 不算 IEEE S&P。PLDI/POPL/OOPSLA 是编程语言顶会，ISSTA 属软件测试/分析，
TOPLAS/JAR 是期刊；SCAM 2019 是直接 Java 证据，但不冒称 FSE/ICSE。

Cytron SSA、Cousot 抽象解释、Csmith、JMM、CompCert 作为经典基础精选；不编造
即时引用量，不把搜索服务不同口径的 citation 数当稳定排名。新近论文也不因
引用尚少而排除。旧版论文的局限不自动等于所下载新工程 HEAD 的局限。

**核查深度**：下面除明确注明外，已获取公开论文原件；现在只保存提取 TXT，
不保留 PDF。已审阅摘要、算法/语义模型、
与 Diana 相关的讨论或限制，并非全部全文逐行复现。新工程核固定提交与关键源码，
没有构建或跑其测试。TXT 不是无损原文，OCR/公式/表格需按原 URL 在线核对。
Johnny 当前只取得官方讲稿；两篇 ISSTA 论文只核官方入口/
摘要，不伪称已读全文。ACM PDF 在当前网络返回访问挑战，Johnny 作者 PDF 无法连接；
不绕过访问限制，也不以未读全文支持具体算法结论。

### 8.1 安全四大与结构化综述

下载 ID 与本地文件由脚本维护；下表的 DOI/会议入口用于学术定位，不要求团队另行
寻找作者本机的材料。论文讲述的“证明”必须按其语言、模型和可信基讨论。

| 下载 ID / 论文 | 场合 / 一手入口 | 关键贡献与阅读位置 | 对 Diana 的意义 / 不适用处 |
|---|---|---|---|
| dream — *No More Gotos: Decompilation Using Pattern-Independent Control-Flow Structuring and Semantics-Preserving Transformations* | **NDSS 2015**；[DOI](https://doi.org/10.14722/ndss.2015.23185)，[公开论文](https://www.ndss-symposium.org/wp-content/uploads/2017/09/11_4_2.pdf) | pattern-independent structuring 与 CFG 变换；重点看条件组合与去非结构控制流 | T13/T14 的候选思想，不是“总能高效输出忠实 Java”的保证；不能重复求值有副作用条件，也不自动覆盖 Java handler/monitor/uninit |
| johnny — *Helping Johnny to Analyze Malware: A Usability-Optimized Decompiler and Malware Analysis User Study* | **IEEE S&P 2016**；[DOI](https://doi.org/10.1109/SP.2016.18)，[官方会议/摘要](https://www.ieee-security.org/TC/SP2016/program.html)，[讲稿](https://www.ieee-security.org/TC/SP2016/slides/yakdan.pdf) | 将去编译形态与分析人员使用效果联系；当前只核摘要/讲稿，未读全文 | 不只追求少 goto/少行数；可读性应围绕实际分析任务。不引入用户实验平台或把研究结论直接变成产品评分阈值 |
| d-helix — *D-Helix: A Generic Decompiler Testing Framework Using Symbolic Differentiation* | **USENIX Security 2024**；[会议入口](https://www.usenix.org/conference/usenixsecurity24/presentation/zou)，[论文](https://www.usenix.org/system/files/usenixsecurity24-zou.pdf) | 函数级重编译、符号模型比较、启发式定位；§4.2 的 loop bounding / memory / external-call 模型尤其重要 | 支持语义检查和定位负责决策的阶段；其超时后限制分支执行、dummy return、内存初始化/枚举等模型不能当任意程序等价证明，更不能搬进 Diana 生产语义。TUNER 是调试思路，不意味着核心正确性做成开关 |
| fidelity-taxonomy — *A Taxonomy of C Decompiler Fidelity Issues* | **USENIX Security 2024**；[会议入口](https://www.usenix.org/conference/usenixsecurity24/presentation/dramko)，[论文](https://www.usenix.org/system/files/usenixsecurity24-dramko.pdf) | 对四个 C 反编译器的输出做 open coding / thematic analysis，归纳名称/类型以外的 fidelity 问题 | 将语义错误、声明/绑定错误、可读性缺陷分开；输出不像原源码不自动是行为 bug，长而正确也不该判失败。T24 仍是可选美化 |
| everparse-paper — *EverParse: Verified Secure Zero-Copy Parsers for Authenticated Message Formats* | **USENIX Security 2019**；[会议入口](https://www.usenix.org/conference/usenixsecurity19/presentation/delignat-lavaud)，[论文](https://www.usenix.org/system/files/sec19-ramananandro_0.pdf) | LowParse 的安全/正确性性质与生成器外置的可信基；§2–§3 区分 parser 安全、格式正确性、非可塑性 | 加强 P1 checked-view 的证明边界：safe Rust 不自动给出长度一致、预算或格式正确。TLS/TLV 的唯一编码性质不定义 DEX，不能因此 canonicalize 原始字节，也不引入 F*/生成器依赖 |
| verified-lifting — *Verifiably Correct Lifting of Position-Independent x86-64 Binaries to Symbolized Assembly* | **CCS 2024**；[DOI](https://doi.org/10.1145/3658644.3690244)，[作者论文](https://www.ssrg.ece.vt.edu/papers/ccs24.pdf) | x86-64 → symbolized NASM；证书、重编译以及 Ghidra/Isabelle 支撑的符号化正确性验证 | 借“生产者提供见证、检查器消费明确义务”的组织；这不是 DEX→Java 证明，更不能把 OriginSpan/edge 覆盖证书冒称同等级语义证明 |
| structuring-sok — *SoK: No Goto, No Cry? The Fairy Tale of Flawless Control-Flow Structuring* | **EuroS&P 2025（不是四大 IEEE S&P）**；[DOI](https://doi.org/10.1109/EuroSP63326.2025.00032)，[公开论文](https://oaklandsok.github.io/papers/behner2025.pdf) | §3–§5 系统化四个相互关联的难题：可解析子图定义、不可解析子图处理、解析顺序、实用性；比较 pattern-based / independent 方法 | 是对“无 goto 算法能一招解决”的直接约束。支持先小型自然结构、受限 dispatcher、显式预算；不得导入其链接的恶意软件样本/大语料 |

已有的 **Phoenix（USENIX Security 2013）**、**SAILR（USENIX Security 2024）**仍保留。
新增 DREAM 与 SoK 是为了对比限制而不是替换它们。SAILR 的 compiler-aware pattern
不自动适配 D8/R8；Java 无 goto 是目标语言的合法性约束，选择自然结构还是合法
状态机才是可读性/成本取舍，不能混为一谈。

### 8.2 软件分析与编程语言研究

| 下载 ID / 论文 | 场合 / 一手入口 | 关键贡献与阅读位置 | 对 Diana 的意义 / 不适用处 |
|---|---|---|---|
| c-decompiler-correctness — *How Far We Have Come: Testing Decompilation Correctness of C Decompilers* | **ISSTA 2020**；[DOI](https://doi.org/10.1145/3395363.3397370)，[官方议程](https://conf.researchr.org/track/issta-2020/issta-2020-papers) | 反编译正确性及类型/优化问题的实证工作；当前只核出版信息与摘要 | 作为风险分类补充，不从未取得的全文推导比较器实现或覆盖结论，不复用其 C corpus 当 Diana 验收 |
| decompiler-effectiveness — *Evaluating the Effectiveness of Decompilers* | **ISSTA 2024**；[DOI](https://doi.org/10.1145/3650212.3652144)，[官方摘要](https://2024.issta.org/details/issta-2024-papers/40/Evaluating-the-Effectiveness-of-Decompilers) | 分开考察语义一致性与可读性；只核摘要和公开 artifact README，未读全文/复现评测 | 对应“先语义、后美化”；摘要的约 55% 等数字只属于其数据/工具/方法，不能当 C/DEX 全行业准确率，也不用于 Diana 进度 |
| ssa-abstract-interpretation — *SSA Translation is an Abstract Interpretation* | **POPL 2023**；[DOI](https://doi.org/10.1145/3571258)，[作者完整稿](https://binsec.github.io/assets/publications/papers/2023-popl-full-with-appendices.pdf) | §2、§4–§6 将 SSA 建造成抽象解释，循环命名/全局值图控制递归，讨论未完成不动点；有自身 soundness/completeness 论证 | 帮 T07 审核域、join、循环身份与收敛。其联合 GVN/SCCP/LICM 分析用于更强精度，并不证明 Diana 应改成全 band 联合求解；不把它当现有 P3 伪代码已被证明 |
| alive2-paper — *Alive2: Bounded Translation Validation for LLVM* | **PLDI 2021**；[DOI](https://doi.org/10.1145/3453483.3454030)，[作者论文](https://web.ist.utl.pt/nuno.lopes/pubs/alive2-pldi21.pdf) | bounded translation validation、LLVM refinement/UB 模型；§3.8 明列当时的未支持项 | “检查没有反例”须附模型/边界，不等于 DEX/Java 普遍等价；2021 论文列的 exception/volatile 等限制不直接投射为当前 HEAD 状态。生产不依赖SMT |
| csmith — *Finding and Understanding Bugs in C Compilers* | **PLDI 2011**；[DOI](https://doi.org/10.1145/1993498.1993532)，[作者稿](https://www2.cs.utah.edu/~regehr/papers/pldi11-preprint.pdf) | 生成有效且避开 C undefined/unspecified behavior 的程序，以差分发现 wrong-code | 独立输入认证比大规模生成更先；Java/DEX 的溢出、异常、移位等有自己的语义，不能沿用 C 生成器的排除规则。不引入随机生成平台 |
| emi — *Compiler Validation via Equivalence Modulo Inputs* | **PLDI 2014**；[DOI](https://doi.org/10.1145/2594291.2594334)，[作者稿](https://people.inf.ethz.ch/suz/publications/emi.pdf) | 以具体测试输入的覆盖约束生成等价变体，检验编译器；“modulo inputs”是关键限定 | 支持独立行为 oracle 与定向变体；通过有限输入不证明全域等价。不能以优化/裁剪为由改变 frozen required 案例或删除其语义触发点 |
| java-decompiler-study — *The Strengths and Behavioral Quirks of Java Bytecode Decompilers* | **SCAM 2019（不是 FSE）**；[DOI](https://doi.org/10.1109/SCAM.2019.00019)，[作者稿](https://arxiv.org/pdf/1908.06895v1) | §III–§IV 区分 syntactic correctness、syntactic distortion、EMI；javac/ecj 的输入差异 | 与 DEX 目标更接近：编译成功、所测输入行为、原源码相似是三件事。来源编译器影响输入形态，支持 B03 逐案例核结构，不把文本相似设 gate |

已有 *Understanding and Finding Java Decompiler Bugs* 是 **OOPSLA 2024 / PACMPL**
（[DOI](https://doi.org/10.1145/3649860)），是比泛原生/C 研究更直接的 Java bug 分类来源。
仍与 JADX 源码、既有 Out-of-SSA / Braun SSA 论文配合，不重复建立另一套测试方法。

### 8.3 经典高影响基础

这些文献的价值是定义模型和不变量，不是要求开工前通读，也不为 Diana 自动带来
形式化证明或论文所报告的性能。

| 下载 ID / 论文 | 正式来源 | 具体阅读落点与设计作用 |
|---|---|---|
| ssa-cytron — *Efficiently Computing Static Single Assignment Form and the Control Dependence Graph* | **TOPLAS 1991**；[DOI](https://doi.org/10.1145/115372.115320)，[公开教学副本](https://rsim.cs.illinois.edu/arch/qual_papers/compilers/toplas91.pdf) | dominance/frontier、Phi 与重命名的经典定义；检查 P3 的 use/def 与控制依赖，不强迫改成论文的具体构造策略 |
| abstract-interpretation — *Abstract Interpretation: A Unified Lattice Model for Static Analysis of Programs by Construction or Approximation of Fixpoints* | **POPL 1977**；[DOI](https://doi.org/10.1145/512950.512973)，[作者页与正文](https://www.di.ens.fr/~cousot/COUSOTpapers/POPL77.shtml) | 有序域、保序传递与不动点；检查 P3/P5 的单调性/有限高度，而不是“对象个数有限所以必收敛” |
| java-memory-model — *The Java Memory Model* | **POPL 2005**；[DOI](https://doi.org/10.1145/1040305.1040336)，[作者稿](https://rsim.cs.illinois.edu/Pubs/popl05.pdf) | 同步、volatile、causality、data-race-free 与顺序一致性；论文明确省略 final fields 的详细处理，须补 [JLS 8 §17](https://docs.oracle.com/javase/specs/jls/se8/html/jls-17.html)，不能从论文推断 final/初始化任意重排合法 |
| compcert-paper — *A Formally Verified Compiler Back-end* | **JAR 2009**；[DOI](https://doi.org/10.1007/s10817-009-9155-4)，[作者稿](https://xavierleroy.org/publi/compcert-backend.pdf) | §2 区分语义保持关系，含终止/发散/外部事件；后续以 simulation 分解转换。Cminor→PowerPC 的证明不传递给 Java/DEX；只借逐阶段义务/可信基的组织，不使用Coq |

### 8.4 新增成熟工程：固定提交、具体符号和不借什么

提交与 sparse 路径由脚本固定；下列链接也固定在同一提交，不依赖 main 的行号。
它们只供源码审阅，没有成为 Cargo/JVM/外部进程依赖，论文版本与代码 HEAD 分开。

| 下载 ID | 固定源码证据 | 源码证据 | 对 Diana 的落点 / 禁止直接照搬 |
|---|---|---|---|
| art | Android 15 `android-15.0.0_r1` 的 peeled commit；[method_verifier.cc](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/runtime/verifier/method_verifier.cc#2015)、[register_line.cc](https://android.googlesource.com/platform/art/+/bebbc3cc49f2d9d5420197df0a336fbc3fcbea40/runtime/verifier/register_line.cc#331) | may-throw 前复制 `saved_line_`，明确不把 check-cast 成功类型带入 catch；`PushMonitor/PopMonitor` 跟踪锁深度/别名，并有 `VERIFY_ERROR_LOCKING` 分类 | 直接支持 P2/P3 异常旧状态、T12/T16/T19。运行时 verifier 的 hard/soft/locking 失败及版本策略不能机械对应 Diana Malformed/Unsupported；不复制整个 ART，也不自动要求设备/ART 执行 |
| r8 | 8.3.37 peeled commit；[BasicBlock.java](https://r8.googlesource.com/r8/+/3ccc501316db4344828180e7152a4adb97522bf8/src/main/java/com/android/tools/r8/ir/code/BasicBlock.java#1163)、[IRBuilder.java](https://r8.googlesource.com/r8/+/3ccc501316db4344828180e7152a4adb97522bf8/src/main/java/com/android/tools/r8/ir/conversion/IRBuilder.java) | `readCurrentDefinition(..., EdgeType)` 区分异常边；`writeCurrentDefinition` 保存 throwing result 覆盖前的定义；incomplete Phis 待 predecessor 完整再 seal | 是异常 SSA 的直接工程证据；借状态义务，不借负寄存器索引编码、不抄全部优化。源码版本相同不代表 B02 工具 jar/依赖已锁定 |
| ghidra | [heritage.cc](https://github.com/NationalSecurityAgency/ghidra/blob/6bed26c590229dde4cca7ff5759dc49b5db19d47/Ghidra/Features/Decompiler/src/decompile/cpp/heritage.cc)、[blockaction.cc](https://github.com/NationalSecurityAgency/ghidra/blob/6bed26c590229dde4cca7ff5759dc49b5db19d47/Ghidra/Features/Decompiler/src/decompile/cpp/blockaction.cc)、[action.cc](https://github.com/NationalSecurityAgency/ghidra/blob/6bed26c590229dde4cca7ff5759dc49b5db19d47/Ghidra/Features/Decompiler/src/decompile/cpp/action.cc#L865) | SSA heritage、block structuring、action/rule 执行各有职责，`ActionPool` 遍历 ops 分配规则处理 | 用于与 Kuna 的职责/规则调度对照；不是把成熟度当算法正确证明，不引入 SLEIGH、native discovery 或一整套 restart engine |
| alive2 | [README](https://github.com/AliveToolkit/alive2/blob/adacbd5180ccf95075e9867c4f0c259f59c9fef2/README.md)、[tv/tv.cpp](https://github.com/AliveToolkit/alive2/blob/adacbd5180ccf95075e9867c4f0c259f59c9fef2/tv/tv.cpp)、[ir/state.cpp](https://github.com/AliveToolkit/alive2/blob/adacbd5180ccf95075e9867c4f0c259f59c9fef2/ir/state.cpp) | IR、symbolic execution、refinement、SMT 分层；当前 README 明确 inter-procedural transformation 不受支持、可能有伪反例 | 学“验证域显式化”，不是直接拿 LLVM UB/refinement 模型验证 DEX；不下载 LLVM/Z3 或启用生产求解器 |
| everparse | [README](https://github.com/project-everest/everparse/blob/43b28b1e5bcf1d8ede6c4ae021cb6e227be4e498/README.md)、[src/3d](https://github.com/project-everest/everparse/tree/43b28b1e5bcf1d8ede6c4ae021cb6e227be4e498/src/3d)、[doc](https://github.com/project-everest/everparse/tree/43b28b1e5bcf1d8ede6c4ae021cb6e227be4e498/doc) | 当前包含不同 frontend 与 C/Rust 组件，不等同 2019 论文单一工具状态；证明对象/生成器边界依组件而异 | 参考 validated views / 检查后访问的设计；手工改写成 Rust 不继承上游证明，不新增格式 DSL、代码生成平台或 F* 运行要求 |
| wasm-tools | [BinaryReader](https://github.com/bytecodealliance/wasm-tools/blob/6860dc1222ecbc374b8bc12efcbc980bfcc9001f/crates/wasmparser/src/binary_reader.rs#L27)、[Validator](https://github.com/bytecodealliance/wasm-tools/blob/6860dc1222ecbc374b8bc12efcbc980bfcc9001f/crates/wasmparser/src/validator.rs#L115) | 借用输入并保留原 offset；Parser/Payload 与 Validator 语义状态分开。`BinaryReader::new` 默认所有已知 feature，且文档明确极端 offset/长度会 panic | 支持原字节/原位置与分层校验；**不照搬默认全 feature 或 panic 构造器**。Diana 的版本门控和 malformed 不 panic 要自行满足；WebAssembly 规则不等于 DEX |

## 9. 设计取舍与证据边界

本节关联来源与具体实现规则。算法步骤及数据定义以DESIGN/ALGORITHMS为准，
本文仅记录出处与证据适用范围。

### 9.1 已有设计得到支持，维持不变

1. **语义优先，成功与降级分开**：D-Helix、Java 实证研究与 CompCert 都提示“文本看起来像源码/可以编译”不足以代替行为；继续 Java/Smali/Unavailable，不造异常桩成功。
2. **三地址 CFG → 固定身份 SSA → type/storage → region/Java**仍是合理的保守工程选择；新 POPL 工作展示了联合分析的另一条路线，并没有证明独立阶段错误或必须搬 GVN/BDD。
3. **Phi parallel copy、异常旧状态、严格保留 may-throw 操作**更有直接证据：R8/ART 都明确区分异常与正常状态；不把所有 predecessor 合一套成功状态。
4. **受限 dispatcher 保底、复杂保护域诚实降级**不因 DREAM/SAILR 推翻；无 goto 不等于无类型/异常/锁证明义务。
5. **核心正确性不可开关，美化默认关闭**；TUNER 可启发开发定位，不能将错误 phase 关闭后算 Java 成功。报告和文本仍不要求跨运行稳定序。

### 9.2 已采纳的具体设计

| 来源 | 采用的工程规则 | 当前落点 |
|---|---|---|
| R01 Cousot / POPL 2023 / R8 | live-in有限只增；固定入口Phi、block一次绑定、incoming一次连接；retained/bad/grounded有限传播，不简化Phi | ALGORITHMS §3、DESIGN §3.3.3、T07；不声称继承论文证明 |
| R02 SoK / DREAM / Ghidra | Sequence/Diamond/自然loop/受限switch明确schema，复制前计容量，失败用符合profile的dispatcher，不求全局最优 | ALGORITHMS §6；DESIGN给产品默认限额，不冒称由论文推出 |
| R03 verified lifting / CompCert / Alive2 | raw/value/copy/edge/Java绑定各读原事实验证；finite Driver与coverage分别说明，不称全域证明 | ALGORITHMS各validator、TASKS §4 |
| R04 JLS/JMM / ART | 原volatile/init/lock顺序不变；lock root需动态新鲜性，final不放常量字段initializer导致sget内联 | ALGORITHMS §9/§11/§12；不新建并发平台 |
| R05 DEX / ART / EverParse / wasmparser | checked ranges/ULEB/MUTF16/metadata、041物理地址明确；raw-valid/Java-profile/资源/BUG分类分开 | ALGORITHMS §1、DESIGN §2；不复制上游panic或unsafe |
| R06 Java研究 / fidelity taxonomy | Engine按JSON acceptanceGate核全部核心required调用，Product汇总全部required；Generated executable不计DeclarationOnly，有限行为不等编译即等价 | TASKS §2、TASKS §4 |
| R07 R8存活期 / 具体调用链 | 一个RunContext贯穿open/render，累计派生分配，used/held分账；Env/worklist最后消费后drop、Report自持结果facts | DESIGN §2/§10、DESIGN §3.2/§3.7 |
| R08 Dalvik fill / ART verifier | 填充既有primitive array，null可通过静态类型验证，不能删零元素操作的NPE；Java采用length read及已证FreshArray stores | ALGORITHMS §2.4；[bytecode](https://source.android.com/docs/core/runtime/dalvik-bytecode)、固定ART method_verifier.cc的FILL_ARRAY_DATA分支 |
| R09 JVMS 8格式界限 | §4.3.3参数words<=255；§4.7.3的code/max_locals/max_stack及§4.4常量池界限独立于DEX/分析预算；生成模板的保守上界为Diana自身合同，不借规范自动证明其数值 | ALGORITHMS §12.4–§12.6；[JVMS 4.3.3](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.3.3)、[4.7.3](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.7.3) |

案例修订的独立规范依据（R10）：[DEX access_flags/encoded_value](https://source.android.com/docs/core/runtime/dex-format)区分managed declared同步标记、显式值tag与缺省；[JLS §8.9.3](https://docs.oracle.com/javase/specs/jls/se8/html/jls-8.html#jls-8.9.3)给enum声明序；[JLS §12.4.1](https://docs.oracle.com/javase/specs/jls/se8/html/jls-12.html#jls-12.4.1)与[§13.4.9](https://docs.oracle.com/javase/specs/jls/se8/html/jls-13.html#jls-13.4.9)界定常量内联/初始化；[ZIP APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)给entry CRC/size/identity。D069的TargetExists假通过反例直接由DESIGN §6.7的no-force规则推出；原子发布依据ALGORITHMS §14.3的native primitive，不由有限并发采样证明。修订记录在TASKS §4及JSON revisionReason；规范出处不替代实际输入认证或独立复评。

## 10. 可复现输入与工具

| 类型 | 必须使用的形式 | 禁止 |
|---|---|---|
| 源码与fixture | 仓库相对路径、公开URL+commit | 个人绝对路径、兄弟仓库目录、未签入文件当必需输入 |
| 固定工具 | artifact坐标/version、公开下载源、锁文件SHA-256 | 自动找作者SDK目录、latest、只写“装一个d8”、未锁依赖的fat jar |
| E2E生成/cache | 当前checkout的`.cache/e2e/` | 把预存cache当前提；工具/fixture缺内容锁或版本检查 |
| 参考下载/cache | 当前checkout的`.cache/references/`；明确URL/源码commit | 个人预存目录当必需输入；静默覆盖已有修改；参考资料不要求checksum |
| 临时运行产物 | harness调用系统temporary-directory API分配，日志按case归档到`.cache/e2e/runs/` | 写死某OS的临时目录、共享固定临时名、实际路径进golden |

`.cache/`是本仓明确的不提交生成目录，不是私有机器依赖；**任何必需内容都必须能重建**。输入source、expectation、manifest、recipe与hash记录签入；不得用“我机器上有”解决缺文件。运行command工作目录由脚本按自身/checkout定位，不要求当前shell恰好在某目录。

参考实际URL/commit/sparse路径仅由tools/fetch_references.py顶部REFERENCES维护。
显式运行才联网，--list不联网/写缓存；已有非空TXT复用、不重新下载PDF/覆盖修改。
源码核HEAD/origin/干净状态，不做资料checksum；不构建/测试参考工程、不运行下载脚本。
论文/讲稿只保同名UTF8 TXT，扫描件OCR标记，原URL保留，公式/图表有疑义回查原文。
E2E工具/fixture仍必须完整内容锁，合同统一在TASKS §3–§4，不享资料checksum豁免。
