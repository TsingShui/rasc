# 关键算法实施附录

本页细化 [DESIGN.md](DESIGN.md) / [MODULES.md](MODULES.md) 的算法义务。是实现指引，不是已经实现或形式化证明。每项对应任务卡；超过支持 profile 的情况保持原始 fallback，不从这里的伪代码推出“全部 DEX 都正确”。

## 1. Decoder 单表合同（T03/T04/T10/T25）

### 1.1 格式表 schema

```text
OpcodeSpec {
  opcode: u8,
  min_dex_version,
  format: Format,
  width_code_units: u8,       // 普通指令；payload单独checkedlength
  operands: OperandLayout,   // bit fields、signed literal、index kind
  register_rule: RegRule,    // narrow/wide、implicit words、reads-before-write
  raw_mnemonic,
  java_capability: Core | Deferred(reason),
  effect_class: EffectClass,
}
```

`java_capability` 不是 `known_format`。一条 invoke-custom 可以格式已识别、原始 smali可打印，但Java未支持；unknownwidth则不能跳过去继续声称完整方法。版本035允许的opcode集合与038/039扩展分开；reserved/optimized/quickened指令不按普通标准DEX猜。

指令跨度**全部使用 code units**。radguard部分接口使用bytes[RG-02]，移植decoder时不得只复制同名offset字段。每format宽度、operand位域在新tests固定，不跨层乘2两次。

第一档格式族（完整byte表在T03按已审阅DEX规范与参考decoder登记，不从本表盲猜opcode）：

| family | widths（code units） | 关键检查 |
|---|---|---|
| 10x/10t/11x/11n/12x | 1 | opcode高padding、4bit/8bitreg、signed4 literal、signed8 branch |
| 20t/21s/21h/21c/21t/22x/22s/22b/22c/22t/23x | 2 | high/padding、第二wordsignedliteral/index、nibble或byte寄存器位域分别检查 |
| 30t/31i/31c/31t/32x/35c/3rc | 3 | i32relativePC、jumboindex、wordreg、35c count≤5、range start+count不overflow |
| 51l | 5 | 64-bit bitslittleendian，不把hostf64格式化作truth |
| newer 45cc/4rcc 等 | 4（具体按规范） | method+proto双index、版本gate；Java延后但raw校验不能省 |
| packed-switch payload | `4 + 2*n` | 32bitfirstkey及n个32bittarget，相对原switchPC；keyaddoverflow |
| sparse-switch payload | `2 + 4*n` | n个32bitkey+n个32bittarget，检查格式要求的key唯一/递增与targetboundary；不是为了展示稳定排序修复原输入 |
| fill-array payload | `4 + ceil(width*n/2)` | width只能允许的scalar宽度；n*width checked；末padding不算element |

### 1.2 两遍解析

```text
pass1(raw code units):
  offset=0
  while offset < count:
    charge(Decode,1)
    lookup(opcode/version) or UnknownFormat(offset)
    若 payload：checked读取长度/对齐、记录RawPayload；不建Op
    否则：checked读取全部words/operand、登记InsnStart与RawInsn
    offset = checked_add(offset,width)
pass2:
  对每RawInsn的branch/payload引用：
    relative_target = checked_i64(original_pc + signed_delta)
    要求落在范围内且目标种类正确
  register/index bounds，invoke words 与proto，result/exception入口约束
  try/catch/debug metadata结构和边界
  所有范围/身份完整才seal ValidatedRawMethod
```

payload可以有多个引用但不能当正常可执行目标。unreachableinstructions也必须结构校验；不可达unknownwidth不借DCE跳过。合法真实NOP可以没有sourceeffect但保留RawOrigin；goto必须是Terminator而非丢失的NOP。

### 1.3 move-result 合并与异常边

producer与move-result的site均保留；move-result必须immediatelyfollow（按合法format规范），非branchtarget且returnkind匹配。invoke正常完成才定义result；异常snapshot发生在invoke之前。若try区间只覆盖invoke不覆盖move-result，生成Java赋值表达包含no-throwlocalcommit可以等价，但证书必须保留保护site是invoke而非扩大到额外表达式。若无法隔离operand求值/effect顺序则Unsupported，而非按syntax猜正确。

## 2. effect 表的最低要求（T06/T10/T11/T15/T16）

| Op | reads/writes/call/init/monitor | may_throw / 移动策略 |
|---|---|---|
| local move/const literal | 无heap/call | exact typed localcopy通常pure；string/classliteral另核resolution |
| int/long add/sub/mul/bit/shift | 无heap/call | 不引入exception，溢出按Javabit语义；noGVN初版 |
| integer div/rem | 无heap/call | 除0抛异常；不能unused就删，不能跨trydomain |
| float/double arithmetic | 无heap/call | 不因除0抛整数异常；NaN/-0、不重结合、不用hostfastmath |
| check-cast | resolution/referencecheck | 可能LinkageError/ClassCastException；unused仍保留 |
| array get/store/length/new/fill | heapread/write或allocation | null/bounds/type/negativecount/allocation异常；exactorder |
| field get/put/static | heapread/write、possiblevolatile/init | null/resolution/access/init等保守；unknownfieldflag不猜非volatile |
| invoke任意kind | unknownread/write/call/init保守 | throws默认unknown；resultunused不是DCE理由 |
| new-instance + init | allocation/call/init | 不让args从new后移到allocation前；受限相邻profile |
| monitor enter/exit | synchronization | locknull、释放规则；所有protectedpathcertify后才转synchronized |
| return/throw/branch/switch | control | throw含异常；conditionevalonce，edgecopiespure |

原始DEX已存在的可能异常必须保留相应site/handlerdomain。保守effect可能让更多caseUnsupported，正确性优先；以后减少barrier须用独立intrinsic/profile证据，而不是猜常用Java方法一定pure。

## 3. SSA fixed-identity 建造（T07/T16）

### 3.1 数据结构与状态

- 参数在`registers_size-ins_size`开始，参数word数量由descriptor核，实例this加1。起点使用checked_sub，ins_size大于registers_size是Malformed，禁止saturating变0。
- 读取wide值要求Low/High同ValueId且相邻；写入一word会invalid与其重叠的原widepair，不能留下高word别名。
- FixedDefId分配：参数、op正常result、checkcast成功type-refinement、handler ExceptionValue；checkcast失败边维持旧state。
- 环境格状态：Pending（尚未算）/Defined(ValueId,kind)/Undefined（路径无定义）/InvalidWide；Pending不是合法值，不能当null。
- JoinPhiId为`(targetBlock, register-start, width-kind)`预分配，finite；inputs按每EdgeId更新。Phi发生在join，不能用firstpredecessor的version覆盖第二条edge。

### 3.2 固定身份、有限传播（不要求稳定遍历顺序）

```text
collect_read_write_sets → liveness_worklist
preallocate fixed op defs + candidate join Phis
queue = reachableblocks（采用算法所需RPO或就绪worklist，不另加输出稳定排序）
while queue not empty:
  b = pop; charge(SSAWork,1)
  in = join eligible predecessor environments
       同identity/kind可直接绑定；多个distinct → 已有PhiId
       任一实际read路径Undefined → final invalid-use obligation
  state = in
  for op in original_order(b):
    bind each use from state (Pending暂不seal)
    throw_before = clone relevant live registers only
    apply opcode-specific normal writes/state refinements
    if op terminal-maythrow:
      exception_out(site) = throw_before + explicit exception rules
  changed edge envs/PhiInputs only enqueue affected successors
seal:
  noPending, noUndefinedactualuse, completePhiInputs, validwordpairs
  independently verify definitions/use availability and exception-oldstate
```

不要每一round生成新Phi；不要“只看到一个predecessor已算就先删Phi”，回边会使结果变化。循环0次entryedge和latch返回边都要输入。无法解析的uninit/typejoin可能合法DEX但当前Javaprofile不足，区分Unsupported；确定非法的实际undefinedread才Malformed。

初版可以保留过多Phi减少构造复杂度，但deadPhi若有Undefinedincoming不能自动成为最终错误：必须先按retainedactualuses决定是否需要它；如果不用就独立剔除/标nonsemantic，不用默认值补它。这个细节在T07要有unreachable/non-live-in反例。

## 4. 类型约束与 Java 用法证据（T10/T11/T21）

P4收集原始proto与完整候选声明→P5求value/use→P8验证sourcebinding，不循环要求P4预知P5。

类型求解采用有限facts两类，不让一个enum在int/ref/float无限来回猜：

1. `RequiredUses(value)`：每opcode/proto/field/return要求，按use-site添加，只增不减。
2. `CompatibleCandidates(value)`：候选primitive/null/ref/uninit，只收窄或有限ref-LUB证据增长，Conflict明确。

给const32保存bits + alluses；它没有heap/call，若使用同时要求`int`与`float`，可以在literaluse分别spell（仅literal）。对nonliteral仅在原DEX规范允许、且Java表达有准确bit语义证据时转换，否则Unsupported，不能拿numericcast代替reinterpret。`int0`、`booleanfalse`、`null`仅从已证use选，别在decoder提前定。

例子：

```text
const v0, 0
if-eqz vRef → ReferenceEqualNull（不是ifRef==0）
return-boolean use(v0) → literal false
invoke(float)param(v0) → +0.0f（如果是纯constuse且bits满足）
```

literal分裂属于表达spelling，但ValueOrigin仍同RawInsn；不能把有sideeffectresult当literal重复生成。Phi如果跨两种incompatiblewordkind无合法type，当前profile失败，不改全Object再cast。

Java “增补 cast”的证明分三种：已知referenceassignable的源码staticcast不新增check失败；原checkcast生成相同runtime检查；正确primitivewiden/narrowuse。未知cast、boxingcall、泛型cast可能额外throw/init/call，必须拒绝或显式待验证，而非为了javac转绿。

## 5. Phi destruction 的 placement 义务（T08/T17/T18）

### 5.1 正常边

`CopySet(edge)`并行赋值；每组source来自edgeenvironment、dst是targetPhiLocal，staticconversion不throw。branch/case/looptransfer选择后才执行。sequentialization详见DESIGN §7.2，cycle每次至少替换一个source并消除一条可调度assignment或引入有限temp；temp数≤待处理cycle节点数，charge每扫描/copy。

需独立验证：`apply_parallel(original_values)`等于`execute_sequential(emitted_copy_stmts)`对测试值及symbolicmap一致；不能仅看是否出现temp。no-opselfcopy只能相同LocalId/type，不能type-refinement的原checkcast当selfcopy删。

### 5.2 异常边

异常不能执行任意edgeblock。`ShadowSlot(handler,Phi)`储存在throw之前；每eligiblethrowedge均提供definedsource，type-compatible且copypure。catch入口读shadow（其定义来自实际throw路径，不是默认0）并绑定ExceptionValue。

条件：

- 每条可能**选中**该handler的throwedge入值都available；impossibleedge排除须有证明，不靠throweffect猜不会发生。
- 多handler/多site影子槽用请求内明确identity key，不能在同一sitealias互盖；不要求跨运行同合成ID，宽值完整两个words成一个typedcopy。
- Snapshot source已经成为显式local/literal，不能snapshot复杂call/cast/arrayexpr。
- P8插位置处于与原op相同保护domain，紧邻op前；Catch中的restore是puretype-compatible，不新增runtimecheck。
- nestedhandler使用影子槽时可能有跨scopeconflicts；未支持ownershipprofile就decline，不让同名shadow误复用。

null/default影子值**不可观察**由DefinitionCertificate重算可用性；若有throwedge没写shadow却catch能读，必须失败。F16/F17/M15构成扩档前最低门。

## 6. natural structure 与 dispatcher 的校验

### 6.1 normal dispatcher

每originalreachableblock→onecase；entrypc对应entryBlock；singlecase执行原ops+normalTransfer，复制后更新pc。Return/Throw不更新到fakefallthrough；defaultpc在映射闭包中不可达。

验证器核对两张表：`BlockId -> CaseNode`和`EdgeId -> TransferNode`，再核对每case的op order与转移/conditionuse。case/op文本origin数不代表动态次数，loop依照originalCFG重复执行。SSA/typed/storage先valid，不能因为“goto改switch”绕过type/conflict/handler问题。

禁止dispatcher：有handler/protectedsites未映射、monitor、尚未lower的Uninit。这些转smali更安全。新增intpc默认entry是localunobservable，defaultAssertionError unreachable要可证而不是躲到“用户不会触发”。

### 6.2 natural schemas 具体第一档

- Sequence：a只有normalnext=b、b唯一normalpredecessor=a、sameprotectiondomain；collapse不漏edgecopy。
- Diamond：条件head两个arm各single-entry，postdomjoin明确；arm内可sequence，joinPhiCopies各在自己的transfer。
- NaturalLoop：backedgeheadnormaldominateslatch、single-entry；headercondition和body/latch transfers按原order，exitcopy放exit前；多个exit用明确break/continuetarget，不tailduplicateeffects。
- IntegerSwitch：selectoronce，key/defaultmapping完整，falls与sharedtarget分开；sameblockdifferentCopySet不能无条件合case。

每collapse减少私有region节点，原CFG不改。schema失败ordinarygraph用dispatcher，malformedcertificate不靠fallback遮盖内部BUG。try/monitor用专门certificate，不加到normaldominance当普通branch。

## 7. raw smali assembly harness（T05/T25）

单MethodOutput不是完整smalifile。测试须从同一原始classmetadata生成**只用于assembly校验的合法owner shell**：`.class/.super`、fields与必要methoddecl，插入该完整method及其try/debugmetadata；其他method可以直接用同原始rawmethod，不生成语义stub。assembler允许外部symbolreference，不代表这些外部类可运行。

重组后比较semanticrawfields：opcode族、寄存器/常量bits、branch对应instructionidentity、payloadvalues、handler顺序与受保护site集、memberdescriptor、UTF16、声明flags/encodedvalues/annotations/debugsequence（允许符号池index与合法padding/offset重排）。缺任何required项都V6失败。不要只用“assemblerexit0”证明没丢字节。

反例：删catchdirective、把floatbits当numericliteral、unknownword改nop、wideinvoke只发一个word、losssurrogate、debugrange跨bound。原输入损坏或未知width返回Unavailable+rawexcerpt，不参加“smali完整roundtrip”。

## 8. 低覆盖率不是修复算法的理由

起始staticmethodfixture可能由javac自动生成defaultconstructor。T06不得为了整JavaUnit省掉不支持constructor：F05使用无构造code的自制普通interface staticmethods（`--release 8`可表达），或manifest显式method-only返回Java并在合法独立测试上下文验证。任何因其他methodfallback而classMixed都要如实计数。

正式constructor/instance/fullclass支持在T12/T20，enum/annotation/nested在T22/T23。closure缺失时不要从单method已通过直接推出整unit可重编译；boundaries由statusmodel表达，不由“多数方法输出了文本”推断。

相关任务：[TASKS.md](TASKS.md)；验证证据局限：[VERIFICATION.md](VERIFICATION.md)。
