# 新 Diana 设计与实施计划

**Diana（黛安娜）是整个项目名，不再是 rasc 里的另一个反编译器。** 目标产品/命令是 `diana`（应用包在 `apps/diana/`），内部反编译库是 `diana-decomp`（`crates/diana-decomp/`），开发环境变量使用 `DIANA_`。设计位于 `Diana` 分支的 `docs/diana/`。

**当前范围：CLI、APK/DEX查询、原始entry提取、manifest、输入安全与反编译；MCP暂时不设计。** 旧MCP随 rasc 归档到 `ref-fork/rasc/`，不删除/重写协议，不参加主 workspace/CI。**状态：Diana-only 包骨架已创建，功能待实现；旧实现是参考工程，不再构建/测试。** 实际目录与旧设计路径映射见 [STRUCTURE.md](STRUCTURE.md)。 反编译算法从零设计，复用现有rasc的有用查询与安全能力；不迁移已删除的实验分支。

未来 GUI 与 Web UI 已按用户要求在 `apps/diana-gui/`、`apps/diana-web-ui/` 预留 README-only 目录；不实现、不引入框架/依赖、不加入 workspace 或 CI，详见 [应用入口索引](../../apps/README.md)。当前任务与 E2E 范围仍是 CLI/引擎。

## 1. 核心决定

- **E2E案例先于开发**：已有70个具体D案例设计，B01–B04先审阅冻结inputs/独立expected、准备可复验工具/source/fixture/基准/harness；通过后再开始T01/H01功能实现；用户明确要求的纯包结构已先搭建，不算任务完成。开发只接薄公开adapter和让固定case转绿，不按actual随意改答案或删requiredcase。
- **团队协作不依赖作者机器**：参考用公开URL+commit+相对符号，工具用内容锁，fixture源/expected/recipe签入；不依赖个人home/AndroidSDK/兄弟仓cache。radguard实际独立仓commit和许可边界已更正。
- **公开CLI按用户指定的十一项**：`get-class / entries / extract / strings / xref / manifest / classes / get-method / fields / methods / help`。参数、列与选择器以[CLI.md](CLI.md)为唯一规范；默认get只Java，显式JSON才返回清楚标记的降级报告，不提供旧命令alias、skill安装命令或分页。
- **一个项目、两条内部路径**：root CLI/输入/轻量查询保留现有优势，内部 `diana-decomp` 承载 P0–P9；不强行让列表查询先建完整反编译对象。MCP延期，不预先冻结新工具或schema。
- **不要求稳定输出顺序**：独立查询/方法结果按完成或到达输出，不做全局排序、按输入序等待或重排缓冲。不提供查询分页或数量截取参数，返回全部匹配记录；跨运行文本/局部名无需一致。程序指令、参数求值、catch、锁和初始化的语义顺序仍必须保持，原始身份与本次spans不能错。
- **查询无分页**：移除 `--offset`/`--limit`，保留filter和流式输出。安全预算不是分页：超预算明确错误，不静默截断为成功结果；内部字节offset、原始索引和有界batch仍保留。
- **测试集中复杂语义**：不新增CLI黑盒、帮助/参数/路由/列名/alias等简单测试；这些接口靠源码/文档审阅和构建核对。E2E从公开反编译库进行Java行为与raw/smali回环，投入Phi/循环/类型/调用/异常/构造器/锁；输入、跨DEX身份与提取文件安全走公开领域接口。现有测试本次不删除。
- **语义忠实优先**。能可靠表示的方法以合法 Java 为目标，不能做到则明确逐方法 smali；损坏/未知格式无法可靠反汇编时 Unavailable。混合输出不冒充完整 Java 编译单元。
- **用 Kuna 管职责，不照搬顺序与规模**。阶段有产物所有权、真实调度、预算与 trace；首版只采用类型化 transition、固定身份的 SSA/定义传播和有限类型 worklist，不做通用插件、无消费者的 restart 或另一份 TOML schedule。
- **保守主路**：安全 raw → Op CFG → SSA → 调用证据 → 类型 → 独立 locals/edge copies → region → Java → printer。保留显式计算，不整体复制 dexdec 在 structured Semantic IR 上的高级 ValueRecovery。
- **结合两种参考的实用优势**：dexdec 提供异常旧状态、Phi 平行拷贝、类型与 Java 合法性义务；radguard 提供紧凑数据流与成员来源 span 的经验。既不把“大”当正确，也不把“小”当安全。
- **结构自然恢复失败时**，只有无 handler、无 monitor、无未处理 uninitialized object 的普通 CFG 可转合法 Java dispatcher；其他难表达方法回到原始字节生成 smali。
- **首版代码量控制靠不实现可选美化**：通用 local coalescing、泛型原形推断、GVN/LICM、finally/字符串糖化等可后续独立开关扩展，不成为核心依赖；万能 constructor helper 不作为普遍可行方案。primitive/null、调用绑定、异常与锁的正确性不是美化，不能删。
- **按端到端能力切片实施**，不是写完一个 phase 的全部算法才进入下一个。每张卡包含文件、接口、步骤、反例、预算、验收和不扩展范围。

## 2. 阅读入口

**交互网页**：[打开架构与实施看板](index.html)。自包含 HTML，可离线直接打开；包含CLI命令契约、项目总览、H01–H10产品任务、内部反编译架构、T01–T25实施路线和可选美化。MCP明确延期；看板展示计划，不是实现进度。

网页数据从设计 Markdown 提取，修改计划后刷新：

```sh
python3 docs/diana/build_board.py
```

该脚本只更新 `index.html` 的内嵌数据，不修改引擎代码，不访问网络。JavaScript/CSS 保留在网页中，无需安装前端依赖或运行服务。

| 文档 | 回答的问题 |
|---|---|
| [DEVELOPMENT.md](DEVELOPMENT.md) | 开发流程唯一入口：有界批次、完成/停止条件、实际批次记录模板；按批准备前置的调整仍待确认 |
| [STRUCTURE.md](STRUCTURE.md) | Diana-only workspace 与 ref-fork 参考隔离、默认构建/CI 范围、旧源码定位 |
| [PROJECT.md](PROJECT.md) | Diana整项目的CLI优先架构、已有rasc能力复用、输入/041/身份/取消/预算与MCP延期边界 |
| [REPRODUCIBILITY.md](REPRODUCIBILITY.md) | 公开参考来源、固定工具内容锁、仓库相对fixture/cache、干净checkout准备与路径验收 |
| [TEST_PREPARATION.md](TEST_PREPARATION.md) | B01–B04在引擎实现前完成：case冻结、源/fixture/工具、独立基准、harness证伪 |
| [E2E_CASES.md](E2E_CASES.md) / [e2e_cases.json](e2e_cases.json) | 70个具体语义/安全case、inputs、独立expected、mutation-killer和oracle；状态仅设计稿 |
| [CLI.md](CLI.md) | 用户指定的11个命令、参数形式、TSV/JSON列、可回传at、Java默认规则、extract文件安全与退出码 |
| [PROJECT_TASKS.md](PROJECT_TASKS.md) | H01–H10产品任务：品牌、输入、查询、原始view、CLI闭环、控制/输出、定义符号/xrefsite、extract和CLIProductRC |
| [SOURCE_MAP.md](SOURCE_MAP.md) | 三项目哪些机制有真实源码依据？哪些旧结论须撤回？怎样记录来源与许可？ |
| [DESIGN.md](DESIGN.md) | 输出契约、P0–P9 产物、调度、SSA/Phi/类型/调用/异常/monitor/Java 合法性、风险与架构取舍 |
| [MODULES.md](MODULES.md) | 未来目录和内部 interface、具体 product schema、worklist/copy/验证器实现义务 |
| [ALGORITHMS.md](ALGORITHMS.md) | decoder 两遍校验、固定身份 SSA、类型/调用交接、Phi copy 放置、结构证书与 smali 组装的实施细节 |
| [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) | 端到端增量、依赖 DAG、首批可执行任务、实施纪律与 CoreRC 定义 |
| [TASKS.md](TASKS.md) | T01–T25 的 Agent 任务卡；T24 是可选美化，不阻塞核心验收 |
| [VERIFICATION.md](VERIFICATION.md) | V0–V7 验证、F00–F25 自制 fixture、工具链与 Driver、需求覆盖、证据局限 |
| [RECOVERY_EXTENSIONS.md](RECOVERY_EXTENSIONS.md) | 可选美化的 slot、独立开关、E01–E08候选与后续扩展验收，不成为核心依赖 |
| [REVIEW.md](REVIEW.md) | 本次文档检查与设计风险记录；区分实际完成的文档审查和未来验收 |

建议阅读顺序：本页 → DEVELOPMENT → REPRODUCIBILITY/TEST_PREPARATION/E2E_CASES → CLI/PROJECT/PROJECT_TASKS → DESIGN → MODULES/ALGORITHMS → IMPLEMENTATION_PLAN → 当前任务卡 → 对应 VERIFICATION；复制算法之前回查 SOURCE_MAP。使用文档内 symbol/文件定位，而不是依赖前面对话中的 LOC 或粗略 benchmark。

## 3. 架构一眼图

```text
                         Diana CLI / agent 文档
                                │
               输入/快照/ZIP/请求控制与预算
                                │
              ┌─────────────────┴────────────────┐
       查询/原entry提取/manifest            内部diana-decomp
       不建完整反编译IR                    P0–P9流水线
              └─────────────────┬────────────────┘
                       CLI text/JSON/diagnostics

           MCP：延期，不在本轮设计/验收路径
```

内部反编译链：

```text
                            P0 RequestFacts / Options / EnvironmentId（只读）
                                      ↓ 每阶段消费
P1 ProgramIndex / ValidatedRawMethod
  → P2 FlowBody（原始指令与 normal/有序 exception edges）
  → P3 SsaBody（固定定义身份、异常失败前环境、Phi）
  → P4 ResolvedBody（原始成员/kind/proto、Java 绑定与 throws 证据）
  → P5 TypedBody（有限类型 worklist，不修改 CFG 拓扑）
  → P6 StorageBody（独立 LocalId、正常平行 copy、异常 shadow、定义证书）
  → P7 PlannedBody（只读图的 Region/Protection/Monitor 证书）
  → P8 VerifiedJavaMethod / Unit 或 Mixed 分类
  → P9 RenderedReport（纯 Java printer / 原始 smali adapter + UTF-8 spans）

分析失败 ── 丢弃 partial AST ── 已校验原始字节 ── P9 Smali / Unavailable
取消     ── 停止请求（不逐方法继续 fallback）
```

P7 是分类归属，返回的 `PlannedBody` 同时携带 P6 StorageBody 与 RegionPlan，不丢前面产物。P1 catalog 先于 P0 请求事实冻结；P0 不是强制“第零个运行一次”的分析阶段。

## 4. 先测试准备，再启动实现

用户要求的结构准备已完成：`apps/diana/` CLI 骨架与 `crates/diana-decomp/` 空库加入 workspace，根默认只含 Diana；旧 rasc/dexdec 全量移入 ref-fork 独立 workspace，不参与 build/test/CI，Diana 不依赖旧引擎且未发布。没有实现 Engine、CLI 命令或空 phase，也没有用零测试冒充验收。下一步仍先执行B01→B02→B03→B04：审阅70case规范、写自制Java/smali/Driver源和独立expected、锁公开工具/fixture、独立认证输入/参考轨迹、证明harness能拒绝错误结果。**当前仅案例设计完成，source/binaryfixture/toolhash/runner仍未实现；不是E2E已通过。** B04准备验收后才进入引擎与产品实现。

准备通过后H01在应用根 `apps/diana/` 实现 `diana` 产品/CLI 领域合同，内部库仍 `crates/diana-decomp/`，H02/H03复用安全输入和轻量查询；引擎第一批 **T01 → T02 → T03 → T04 → T05 → T06**：

1. 最小 crate、明确输出/错误状态、预算、实际 schedule。
2. 安全 DEX catalog/raw method 范围。
3. opcode 格式表、raw smali 拒绝通路。
4. UTF-16/payload/metadata 无损表示。
5. T05将已冻结fixture/expected/harness接到真实公开库，不重新设计案例或填写工具版本。
6. 第一个非空 Java executable 方法真正贯通，编译并与基准 Driver 比较行为。

H04/T04→H05/T06尽早贯通get-method真实CLI，不等待全部phase；H09补fields/methods/xrefsite，H10补安全extract；此后两条任务线按卡扩充。T06 前不能把空 phase Module 当成果；fallback出了文本不等于对应Java能力完成。EngineCoreRC与CLIProductRC分开，MCP延期。

## 5. 已知限制与验收原则

- Java 编译通过 ≠ 语义等价；参考反编译器文本相同 ≠ 真值；默认 0/null ≠ 未定义值已被正确处理。
- 一定义一 local 简化了变量合并，但还必须处理 critical edge、parallel copy 周期、异常旧值与作用域。
- 构造器/enum/checked throws/inner metadata 受 Java 语法与绑定规则限制，不能“多加 cast/删 super/普通类 extends Enum”解决。
- 第一档异常只支持可证明的保护区与 handler 形态；monitor 必须有 all-exit release 证书。复杂情况应明确 fallback，不弱化契约。
- 开发测试可以显式使用 javac/d8/assembler，生产不依赖它们。缺工具、依赖或 ART 环境须标 not-run；不能忽略错误算通过。
- 不承诺 LOC 缩减、性能提升或所有合法 DEX 可表达；CoreRC 按实际 supported profile 与完整证据定义，不等于全功能反编译器。

## 6. 本轮改动范围

设计资产已在 `Diana` 分支提交。用户随后要求的本轮结构工作新增 `apps/diana/` 与 `crates/diana-decomp/` 最小包、workspace 和独立构建/发布范围，具体见 STRUCTURE。用户进一步要求旧 rasc/CLI/MCP/tests/deps 归档到 `ref-fork/rasc/`，生产源码原内容保留但不再构建/测试；主 workspace/lock/CI 只含 Diana 失败入口和文档级空库；没有 Engine/70份可运行 fixture/E2E runner，没有发布。结构构建检查不是 H01/T01 或语义 E2E 通过，后续功能仍等 B01–B04。
