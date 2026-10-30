# 可选美化：保留扩展能力，不成为核心依赖

用户补充决定：美化部分可做成可选，后续有精力再扩展。它与本计划“语义忠实、保守默认”一致。本页是扩展合同，不要求现在实现这些 pass，也不把它们纳入 CoreRC 的完成条件。

## 1. 三类职责不可混淆

| 类别 | 例子 | 可选吗？ | 关闭的后果 |
|---|---|---|---|
| 核心语义 lowering | boolean/int/null 区分、原 check-cast 保留、Phi edge copy、catch 顺序、monitor release | **不允许关闭** | 关闭会错语义，不提供这种 option |
| 必需 Java 合法化 | 原始成员精确绑定、合法 constructor/enum、throws/override、作用域/DA | **不允许关闭** | 无法合法化就明确 raw fallback，不发非法 Java 成功 |
| 可选 readability recovery | 少些临时变量、for/foreach、StringBuilder 拼接、泛型拼写、匿名类源形、重复 cleanup→finally | **可以逐项开启** | 保留当前保守 Java/结构形态；核心结果不依赖它 |

“高级”不一定是美化。例如完整 Phi 消除或 monitor 证书复杂但仍属核心；“只加一个 cast”很短却可能改变异常行为。分类依据语义义务，不依据行数或开发难度。

## 2. 扩展 slot 的最小合同

首版 T24 只实现实际存在的 pure recovery option，不预建插件框架。后续每项实现为 P8 内部 Module，使用具体 AST/证书或只读 provenance，接口可采用：

```text
recover(candidate: &mut JavaAst,
        facts: &RecoveryFacts,
        control: &mut RecoveryControl)
    -> Applied(Delta, Obligations) | Declined(Reason) | InternalInvariant(Diagnostic)
```

- 输入是基线已验证 Java AST 的**私有候选副本**或可回滚有限 edit-log，CFG/SSA/StorageBody 不被修改。
- Applied 必须明确触碰的 nodes、原 effect/transfer/类型证据引用、SourceOrigin 重定位义务；只返回“changed=true”不够。
- verifier 再验证 Java 绑定/控制/效果/声明约束，行为 fixtures 比较 on/off。证书验证仍不等于任意程序等价证明。
- Declined / optional BudgetExceeded：丢候选，保留基线；日志写 `RecoveryDeclined`，不改变 Java/Smali 状态。Cancelled 仍停止请求，不继续美化。
- InternalInvariant：丢候选以防污染，报告 BUG，开发 gate 红；不能默默叫 Declined。
- 单次候选预算与全方法预算均有上限；optional 的失败不能耗尽已保留基线的 emit 预算，不能反复重试换 profile 绕限。
- 选项有明确的协议/配置 id、默认值、前置能力、最早消费phase、失效范围、测试族；它是选项身份而非结果排序保证，未实现项不要先出现于public catalog。

只有真实存在第二 adapter/策略时才暴露 seam。没有动态加载第三方 pass、TOML rule language、LLM imperative IR edits。

## 3. 何时运行以及能读什么

```text
core P8 lower/verify → BaselineJava
       → 可选回合（固定次序、有限预算，默认关闭）
       → P8 reverify → P9 pure printer
```

默认美化模式 `RecoveryPreset::Core`：所有 readability pass 关闭；`Readable` 将来只开启已实现、已验收、默认策略明确的子项。每项显式开关可覆盖preset；未实现项返回配置错误，不静默忽略。

P9 只做不涉及绑定/语义的格式：缩进/空行/括号策略等，格式变化也要保持 literal 和 spans。所有需要类型、求值顺序、调用精确绑定或结构转换的美化仍在 P8。

不要为了未来可选pass在核心复制全部已删除分析。`RecoveryFacts` 初版只暴露只读已有事实：OriginalOp/Origin、TypeTable、BindingTable、CopyPlan/StructureCertificate 摘要。后续确实需要 reaching-domain/cleanup equivalence 等，再由该扩展声明自己的 analysis 与 budget；不会自动把 BDD 或整套 ValueRecovery 变为核心依赖。

## 4. 候选扩展与前置义务

以下只是后续路线，不是已经可实现或已验证的保证。按照“先最便宜可证的”排序。

| 扩展 ID | 形态变化 / conservative baseline | 必须保留的证据 | 明确拒绝的情况 / 最小反例 | 依赖 |
|---|---|---|---|---|
| E01 pure-local-inline | `v=a+1; return v` → `return a+1` | same block/single use、no-throw/no-heap/no-call、operands稳定、原evaluation顺序 | integer div、字段读、volatile、cast、call跨site；两个use导致duplicate | T24 + T25 |
| E02 condition / loop form | `while(true){if(!c)break;...}` → while/for | condition一次、latch/exit EdgeId copy顺序、continue target与update时机 | continue跳过update、Phi搬运提前、condition里call顺序变化 | T13/T14/T25 |
| E03 string-building syntax | `StringBuilder.append(...).toString()` → concatenation | 特定已知intrinsic调用语义、overload（char[]/Object/primitive）、null、单次求值、所有builder uses/escape | builder逃逸/alias、append(char[])变Object.toString、null/call时机变化、额外allocation语义不能无条件忽略 | T11/T15/T25；单独intrinsic审阅 |
| E04 generic source spelling | raw List/cast → `List<String>`等 | erased member identity、完整generic声明、overload/bridge选择不变、无新增runtimecheck | wildcard/owner projection未知、generic方法选择另一个overload、cast改变异常 | T21/T25；扩展自有约束求解 |
| E05 named synthetic→anonymous/lambda | 命名nested syntheticclass → anonymous/lambda | capture/outer-this/constructor/kind、class identity与可观察行为适用profile、binding | getClass/identity/serialization/reflection可观察差异、constructor side effects、不同this | T22/T25；匿名类与lambda不共用宽松规则 |
| E06 initializer/source placement | 保留constructor/staticblock写 → 字段initializer/聚合 | 原求值顺序、final单赋值、class-init触发、每构造链次数 | this委托重复初始化、static读触发点移动、arrayallocation/throw顺序 | T20/T25 |
| E07 repeated cleanup→finally | catchall与normalpath重复 → `finally` | 全路径cleanup等价、exception保护域、return值捕获与finally自身throw、handler顺序 | finally覆盖return、cleanup被扩大try多捕获、rethrow变新异常 | T18/T19/T25；独立cleanup equivalence analysis |
| E08 local coalescing / naming | 多ValueId独立Local→共享源码local | 活跃性/干扰、types、异常shadow/edge copies/DA、命名不改成员绑定 | copy-cycle丢旧值、catch观察覆盖值、reference不同type、defaultobservable | T08/T17/T25；不是仅同register就merge |

这些项默认不进CoreRC。实现其中一项时新增focused扩展任务卡，列算法、文件、effect invariant、counterexample、on/off行为和工具链验证，不能只“把开关打开”。E03/E05等对可观察allocation/class identity敏感，不可称普遍语义等价；若无法在当前契约证明，则Declined而非开启宽松模式。

## 5. 交付与验收纪律

1. 每个pass独立开关，基线Core运行所有requiredtests；Readable/on运行同一positivecase+拒绝反例。
2. **语义测试失败不能因为是美化就算可接受**。可以输出更丑、更长，但不能改返回/异常/副作用/锁/成员身份。
3. 包括纯性能重构在内，不要求生成文本跨运行byte-identical或独立结果稳定序。按原始identity、语义义务与本次实际SourceSpan验收；这不能取消程序自身求值/异常/初始化顺序。
4. 对某case Declined不是Java失败；报告分别计`applied/declined/optional-budget/bug`，不能用applied数量当质量或覆盖证明。
5. 新扩展需要 earlierfacts时先声明最早consumer和invalidationclosure；初版整方法重算更安全，不承诺缓存局部rewind。
6. 默认ON需明确测试、counterexample、release成本与许可来源；用户允许未来扩展，不等于授权任何未验证变换默认开启。

相关位置：[DESIGN.md §10](DESIGN.md#10-调度registry失效与预算)、[T24](TASKS.md#t24--可选纯表达式恢复不是必需核心)、[验证矩阵](VERIFICATION.md)。
