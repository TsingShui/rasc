# Diana 参考源码、证据与复用约束

状态：设计阶段源码审阅记录，不是跨引擎性能或正确性评测。本文固定参考版本；链接中的行号只对该版本有效。新 Diana 从零设计，不以既有实验分支为迁移目标。

## 1. 团队可获取的参考版本

参考由**公开仓库URL、固定commit、仓库相对文件/符号**定位，不依赖作者的home目录、已删除worktree、父仓磁盘布局或SSH权限。公开内容核查：三个独立仓库的固定版本入口均可通过HTTPS获取；这不证明所有fixture的授权或算法正确。

| 标识 | 公开来源 | 固定提交 | 相对根 / 核查说明 |
|---|---|---|---|
| DX | [TsingShui/rasc](https://github.com/TsingShui/rasc) | `ef8a4c92c6b89e8afed9ccbc1552feae67b6982d` | DX后续路径相对`crates/dexdec/`；root/rusty-dex路径另明确标注。[固定manifest](https://github.com/TsingShui/rasc/blob/ef8a4c92c6b89e8afed9ccbc1552feae67b6982d/crates/dexdec/Cargo.toml) |
| KU | [Noelo-Lab/kuna](https://github.com/Noelo-Lab/kuna) | `29c44b57f8c1d9b35044aa4ee3f636c193288009` | 路径相对该仓根。[固定phases文档](https://github.com/Noelo-Lab/kuna/blob/29c44b57f8c1d9b35044aa4ee3f636c193288009/docs/phases.md) |
| RG | [MG1937/radguard](https://github.com/MG1937/radguard)，`0.1.3` | **`290de20c925c4a2c7642b1a63c9997185a06e130`** | 路径相对radguard自己的仓根。[固定manifest](https://github.com/MG1937/radguard/blob/290de20c925c4a2c7642b1a63c9997185a06e130/Cargo.toml)。此前误把父仓commit当radguard commit；父仓只含gitlink，此处更正 |

按[REPRODUCIBILITY.md](REPRODUCIBILITY.md)在当前checkout内显式获取固定参考；参考只用于审阅/实现，不是生产或E2E测试运行的隐含依赖。离线协作者使用按相同commit验证的源码包；不能自动改用main或作者机器路径。

补充产品范围：Diana是整个项目，root产品/CLI为`diana`，内部引擎为`diana-decomp`；现有rasc源码作为CLI/输入/轻量查询的复用来源，见[PROJECT.md](PROJECT.md)。MCP明确延期，本轮不设计新协议、schema或迁移。反编译算法仍从零设计，不移植已删除的实验分支。

本轮只做源码/公开入口/文档核查，没有重新运行三引擎语料比较，也没有验证旧对话中的覆盖率数字。后续HEAD移动不自动改变本表；任何reference更新须独立审阅，不能静默用另一个版本。RG父仓gitlink不把父仓Apache声明自动传递为子仓许可，见第6节。

## 2. Kuna：借机制，不借完整引擎

| 证据 ID | 文件 / 符号（固定版本） | 源码观察 | 新 Diana 决策 |
|---|---|---|---|
| KU-01 | `docs/phases.md`：Phases、Band B、Source layout | 阶段是产物分类，P0 正交；Band B 内顺序不由目录编号决定 | 物理目录按产物所有权，实际执行顺序独立列出；不假设 P1 到 P9 只各跑一次 |
| KU-02 | `decompiler/crates/kuna-decomp/src/infra/universalaction.rs:46–101`：`RuleRow`、`SchedNode`、`materialize` | 调度树有 Action/Pool/Group/RestartGroup，构造函数实际生成执行树 | 初版仅保留类型化 transition、显式 Sequence 与有预算的本地 Repeat；不搬动态插件、整套 ActionDatabase |
| KU-03 | 同文件 `universal_sched:296`，`mainloop:550`、`fullloop:627` | `ActionBlockStructure` 就在 mainloop 中；types、heritage、rule pool 与后续控制图变换交织 | “Kuna 一定先做完全部数据流再结构化”不是准确执行描述。Diana 自己选一个可验证、少反馈的调度 |
| KU-04 | `docs/spec/00-overview.md §0.7`（约 `3432–3475`） | 逐条列出 trigger、持久状态、重入范围；有未实现反馈边的明确标注 | 只为已实现的生产者/消费者建立反馈；原始字节和用户配置不被算法重写 |
| KU-05 | `decompiler/crates/kuna-decomp/phases.toml`；`build.rs`；`build/registry.rs` | registry 与 codegen 提供 option/catalog 元数据，不等同于实际 schedule | 初版 Rust 静态表从实际 schedule 描述生成；不引入 TOML/build.rs 第二份真相源。确有外部配置需求再引入生成器 |
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
| DX-09 | `src/analysis/java_backend/constructor_syntax.rs`：`apply_with_method_returns` | 多种 pre-super 提取/调度/生成 helper；处理 Java 与 DEX 构造约束差异 | 初版只接受可证明的相邻 new/init 和合法 constructor prologue；不做“任意前导都能静态 helper 化”的保证 |
| DX-10 | `src/analysis/java_backend/declaration_lowering.rs:1381`：`failure_body` | 恢复失败可生成抛 `UnsupportedOperationException` 的方法体 | 不继承该成功表达；失败保留 raw 方法并在结果模型显式降级 |
| DX-11 | `src/decoder/method_decoder.rs:650–675` | 未匹配 opcode 可生成 nop；word_at 的缺失值可为 0 | 新 reader/decoder 严格区分合法 NOP、未知 opcode、截断；禁止该默认路径 |
| DX-12 | `src/api/decompiler.rs:28–55,105–138`；`src/api.rs` | request isolation 清分析 scope，但 parsed metadata 保留；请求内 hierarchy/ABI 仍被消费 | 不能称缓存“全部死代码”。内部 Diana Engine 借用 root-owned DexImage/request scope，parser/IR/source缓存延后到实测需要；root已有解压复用有独立budget |
| DX-13 | `Cargo.toml [lib] test=false`；`src/lib.rs` lint allow；`src/analysis/java_backend/generator.rs` 等 Rayon 调用 | workspace 普通 test 不自动执行 legacy lib tests；并行 map/collect 可能维持顺序 | 不删测试换“体积”；新模块无blanket allow，必需测试进入gate；Diana不要求并行结果稳定序，首版单线程仅简化实现 |

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

## 5. 前面对话中需要撤回或收窄的结论

1. **javac 通过不是语义正确**。忽略 missing dependencies 的编译报错后得出的“通过率”也不是完整 javac 通过率；旧的 31/40、8/40、7/40 不能比较为质量排名。类/方法 denominator、依赖环境、混合输出、inner fragment 必须分别报告。
2. **不存在已证明的 55–60k 行核心地板或 -33% 收益**。第一处 `#[cfg(test)]` 可能在文件中段，不能把其后全部算测试；LOC、函数数更不是算法深度指标。本设计不承诺代码量收益。
3. **Phi 降低并非“永远正确”的简单赋值**。parallel copy、critical edge、旧值异常状态、循环/handler ingress、类型和词法作用域都必须保留。
4. **默认给 locals 0/null 不自动是死写**。只可用于已证明原输入未读未定义值、且新 copy/异常路径不会观察初值的局部变量；否则 fallback，不掩盖错误。
5. **Enum 不能直接打印成 `class E extends Enum`**。Java 禁止普通类直接继承 Enum；enum 的声明与 name/ordinal/values 契约必须单独处理或降级。
6. **构造器 helper 不普遍可用**。可能依赖未初始化 this、实例字段、锁、访问权限、参数求值顺序；少量受限 profile 才能用，初版不实现万能 helper。
7. **不动点不等于阶段全部互相重跑**。SourceVariableAllocation 有不可逆身份降低；不能直接塞进无条件 Repeat。Kuna registry 是元数据，不是调度解释器。
8. **不强制生成文本跨运行相同**。用户取消稳定输出序要求，复杂/简化fallback及纯性能修改均按原identity、语义轨迹、结构覆盖与合法性验收，不额外要求byte-identical。原始输入内容与程序语义顺序仍必须准确保留。
9. **DAD diff 不能成为唯一验收，也不应拿现成夹具当授权已知**。其 comparator 的 normalization 会隐藏形态差异，冻结参考也可能有 bug。
10. **删除 Rayon 或取消排序不能据此保证更小/更快**。参考源码有稳定合并不等于新Diana继承该合同；现按用户要求取消结果/文本稳定序，性能影响需release测量。blanket lint allow、测试范围与缓存的实际消费者要精确核查。

## 6. 复用登记与许可检查

实施时每次移植一个算法，新增登记：来源项目/固定提交/符号、原许可与 copyright、适配差异、不变量、测试、是否仍依赖原 helper。原则是“理解后重写或有记录地局部移植”，不是把目录搬进新 crate 再慢慢启用。

- DX 与 rusty-dex 有 Apache-2.0 许可文件和本仓 NOTICE 记录；任何复制须保留许可/copyright、记录修改并更新新分发清单。
- KU 的工作空间声明许可，含 Ghidra/angr 来源；复制具体文件前检查该文件头、根 LICENSE/NOTICE 和上游归属，不只看 Cargo 一行。
- RG是独立git仓库/gitlink，固定版本manifest没有`license`字段，也未找到该仓自己的LICENSE。**父仓Apache-2.0不能自动覆盖gitlink子仓**。引用设计/符号不等于获准复制代码；在权利人明确许可前，阻止radguard程序/fixture移植与分发，采用自行实现+自制fixture。此前“子目录沿父许可”的假设撤回；DAD衍生归属仍需核查。
- 本文引用符号和算法思想；本次不复制第三方程序或 corpus。若许可、fixture 授权无法确认，则阻止该项复制，保留自行实现与自制 fixture 路线；不阻止其他设计工作。

下一步：[目标架构](DESIGN.md)；实施者的首批入口：[实施路线](IMPLEMENTATION_PLAN.md)。
