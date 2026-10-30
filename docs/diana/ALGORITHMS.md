# Diana 核心算法实施方案

本文是当前采用的实施方案，不再是“以后研究”的问题清单。配合 [DESIGN.md](DESIGN.md)
的合同与数据结构、[TASKS.md](TASKS.md) 的任务和验收规则使用。
尚无生产实现；算法必须经过既定核心案例验证，不能把本文称为形式化证明或已完成能力。
不实现自动修复/重试框架，不新增非核心测试。原始字节、程序顺序与既定70案例不改变。

## 1. Reader、格式表与原始指令（T02–T04）

### 1.1 地址空间与读取函数

`Cursor`持有`bytes: &[u8], pos: usize, end: usize`；所有读取返回Result，不索引后再检查。
`range(off,n,stride)`按`n.checked_mul(stride)`、`off.checked_add(size)`核验，随后才取slice。
固定记录保留borrowed view；需要解码的变长记录先核remaining/最小记录尺寸、累计预算，
再try_reserve。偏移分别用`FileByte`、`CodeUnit`，不在调用点裸乘2。

- 接受已定义的035/037/038/039/040/041容器；036及未知版本为UnsupportedContainer。
  版本识别不代表该版本的全部Java opcode已支持。reverse-endian首版明确Unsupported。
- 035–040要求header_offset=0、header_size=0x70、file_size等于本次物理DEX长度。
- 041要求header_size=0x78；0x70/0x74字段分别为container_size/header_offset。
  从0起按每个header的file_size checked前进，最后恰到container_size；所有header的
  container_size与物理长度相同、header_offset与实际位置相同，步长至少0x78。
- 041的section/data/code/debug偏移都是**物理container起点**的绝对偏移，可引用后续
  logical member的数据，不能引用当前header之前的数据；不得用logical file_size截断
 共享数据，也不得加header_offset两次。data_size/data_off在041不用作数据区边界。
- 所有输入view均是原container+header_offset，不overlay header、不重算“原始字节”。
  一次校验物理header链，选中logical member才建其catalog；相同物理slice不重复计输入字节。
- 首版header checksum/signature只保留原字段，不作为内存安全证明，也不宣称完整ART
  文件认证；fixture内容锁和ZIP提取CRC是独立合同，不因此取消。

固定表stride：string_id/type_id=4，proto_id=12，field_id/method_id=8，class_def=32。
先验证map_list的`4+12*n`、类型唯一、规范要求的offset次序、固定表与header一致性，
再允许读引用。type/proto数量上限65535；其他表受实际范围、索引宽度和资源配额约束，
不把u32计数先cast成u16。class_data四个count后分别累计field/method的ULEB delta，
每组从0开始、checked_add、索引/所属owner/重复定义校验后才发布声明。

ULEB32最多5字节，第5字节不得续接且payload<=0x0f；SLEB32第5字节payload只能
0x00..0x07或0x78..0x7f。先用u64累积，再做符号扩展/范围核验；不在MIN上abs。
不自行增加规范没有要求的“必须最短编码”限制。

`code_item`：16字节头→insns_size*2→奇数insns且tries>0时2字节padding→8*tries→
encoded_catch_handler_list。handler_off相对**handler list起点**，branch/payload相对原
指令PC；handler size用i64处理负数。try区间有序不重叠、起止在合法指令边界、目标不是
payload。abstract/native的code_off=0只产声明，不运行method pipeline。

MUTF-8输出`Vec<u16>`：00作终止，C0 80作NUL；一/二/三字节单元按规范验证，保存
surrogate单元，不接受普通UTF-8四字节路径替代；声明utf16长度须精确匹配。
字符串池literal可有孤立surrogate；descriptor/name另外按DEX声明语法验证，不能lossy
转换后查成员。display、Java literal和原始身份是三个不同用途。

### 1.2 格式位域与完整opcode登记

令`w0..w4`为u16 code units；`a8=w0>>8`、`a4=(w0>>8)&15`、`b4=w0>>12`。
`s8/s16/s32`表示按该宽度符号解释；32/64-bit words始终little-endian拼接。

| Format | code units | 解码字段 |
|---|---:|---|
| 10x / 12x / 11n / 11x / 10t | 1 | 无operand且高字节0 / A=a4,B=b4 / A=a4,lit=s4(b4) / A=a8 / delta=s8(a8) |
| 20t / 22x | 2 | 高字节0,delta=s16(w1) / A=a8,B=w1 |
| 21t / 21s / 21h / 21c | 2 | A=a8，分别为delta=s16(w1)、lit=s16(w1)、high-bits=w1、index=w1 |
| 23x / 22b | 2 | A=a8,B=w1&255，C=w1>>8 / lit=s8(w1>>8) |
| 22t / 22s / 22c | 2 | A=a4,B=b4，分别为delta=s16(w1)、lit=s16(w1)、index=w1 |
| 30t / 32x | 3 | 高字节0,delta=s32(w1|w2<<16) / 高字节0,A=w1,B=w2 |
| 31i / 31t / 31c | 3 | A=a8，分别为bits32、signed payload delta、unsigned jumbo index，由w1/w2拼成 |
| 35c | 3 | count=b4<=5,G=a4,index=w1；C/D/E/F为w2低到高四nibble，实际寄存器取[C,D,E,F,G]前count项 |
| 3rc | 3 | word_count=a8,index=w1,first=w2；count>0时checked核first+count<=registers_size |
| 45cc / 4rcc | 4 | 分别沿35c/3rc布局，w3额外为proto index；038+ |
| 51l | 5 | A=a8,bits64=w1|w2<<16|w3<<32|w4<<48 |

opcode登记按以下闭区间展开；同一条`OpcodeSpec`包含format、version、ref kind、
read/write slot、effect和Java lowering kind，decoder与raw printer只读此表：

| opcode | format / 含义 |
|---|---|
| 00 | 10x nop；0100/0200/0300完整word另识别payload signature |
| 01..09 | move / move-wide / move-object各三条，依次12x、22x、32x |
| 0a..0d / 0e / 0f..11 | 11x result/result-wide/result-object/exception；10x return-void；11x return/return-wide/return-object |
| 12..15 | 11n const/4、21s const/16、31i const、21h const/high16 |
| 16..19 | 21s const-wide/16、31i const-wide/32、51l const-wide、21h const-wide/high16 |
| 1a..1c | 21c const-string、31c const-string/jumbo、21c const-class |
| 1d..1f / 20 / 21 | 11x monitor-enter/exit；21c check-cast；22c instance-of；12x array-length |
| 22..27 | 21c new-instance、22c new-array、35c/3rc filled-new-array[/range]、31t fill-array-data、11x throw |
| 28..2c | 10t/20t/30t goto[/16,/32]；31t packed/sparse-switch |
| 2d..31 | 23x cmpl-float/cmpg-float/cmpl-double/cmpg-double/cmp-long |
| 32..37 / 38..3d | 22t if-eq/ne/lt/ge/gt/le；21t对应if-*z |
| 44..51 | 23x aget七种/对应aput七种：普通、wide、object、boolean、byte、char、short |
| 52..5f / 60..6d | 22c iget/iput同七种；21c sget/sput同七种 |
| 6e..72 / 74..78 | 35c virtual/super/direct/static/interface；3rc对应range |
| 7b..8f | 12x neg/not与primitive转换，按Dalvik bytecode名称表顺序登记 |
| 90..af / b0..cf | 23x binary及12x /2addr；int/long各11种，float/double各5种 |
| d0..d7 / d8..e2 | 22s /lit16：add,rsub,mul,div,rem,and,or,xor；22b /lit8再加shl,shr,ushr |
| fa/fb/fc/fd | 038+：45cc/4rcc invoke-polymorphic[/range]、35c/3rc invoke-custom[/range] |
| fe/ff | 039+：21c const-method-handle、const-method-type |

unary 7b..8f顺序为neg-int,not-int,neg-long,not-long,neg-float,neg-double,
int-to-long/float/double,long-to-int/float/double,float-to-int/long/double,
double-to-int/long/float,int-to-byte/char/short。binary int/long顺序为
add,sub,mul,div,rem,and,or,xor,shl,shr,ushr；FP只有add,sub,mul,div,rem。
long/double operand/result是Two；long shift的计数仍是One；cmp结果总是One；
conversion按名称决定两侧width，/2addr先读旧A/B再写A，不当成只读B。

其余035 reserved/unused值不猜宽度、不当nop；optimized/quickened格式不混入标准表。
const/high16左移16位，wide/high16左移48位；wide/16和wide/32先符号扩展再成64位。
35c/3rc的count是**word数**，不是参数个数；wide实参必须占相邻两个word。

payload长度：packed=`4+2*n`，sparse=`2+4*n`，fill-array=`4+ceil(element_width*n/2)`。
用checked整数先算；payload需4-byte对齐，array width只接受1/2/4/8，odd byte尾padding
不算元素。switch keys按规范核递增/唯一，不重排修复；target均相对引用它的switch PC。

### 1.3 两遍校验与CFG切分

```text
pass1:
  每次先charge/check_cancel，checked读取完整format/payload
  记录InsnStart/PayloadStart、RawInsn(original words, operands, refs, origin)
pass2:
  核register跨度、index种类、branch/payload目标与对齐
  核try/debug边界、handler入口、invoke word/proto和result/exception邻接
  全部通过后才封口ValidatedRawMethod
```

不可达代码也校验宽度/引用，不用DCE掩盖截断。普通fallthrough到payload、落出code末尾、
跳入指令中间均拒绝。goto/goto16与if-*的delta不得0，goto32允许0；不能一概拒绝合法自环。
CFG leader为入口、跳转目标、终结后、try边界、handler入口，
以及受保护may-throw操作之后。每个受保护may-throw为block最后一个操作，normal/
exception edge分开；同target不同case/rank仍有不同EdgeId。

invoke/filled-new-array与紧随的move-result绑定为一个语义producer，保留两个raw site；
result只在normal路径写目标寄存器，丢弃result也保留call。result kind/return kind不符、
branch进入move-result、非法move-exception入口拒绝。原保护域只覆盖真正may-throw site，
不得把Java operand求值或其他调用一起扩大进try。

### 1.4 metadata解码的确定规则

- encoded_value头低5位为tag、高3位为value_arg。byte/short/int/long及对应引用索引
  按允许width读取，整数符号扩展；float/double是右补0恢复原bits，不做数值转换。
  array/annotation/null的arg必须0，boolean的arg为0/1且无后续payload。
- annotation/array用显式有界栈；每层先charge元素数和深度，引用索引分别核对应表。
  debug读取line_start、参数名ulebp1；NO_INDEX不变成池index0。
- debug opcode 0结束，1 advance_pc，2 advance_line，3/4 start_local[/extended]，
  5 end_local，6 restart_local，7/8 prologue/epilogue，9 set_file；>=0x0a按
  `adj=op-0x0a; pc+=adj/15; line+=-4+adj%15`更新。地址/line checked，local生命周期有效。
- 未完整解出所需metadata不能返回“完整Smali”。header结构、method局部损坏与当前
  metadata不支持分开报告，不抛弃其他可定位方法。

## 2. Op与effect合同（T06/T10/T11/T15）

采用三地址Op，operand是寄存器slot或ValueId，不存递归expression。`Slot={start:u16,
width:One|Two}`；每个read先读取旧值，随后才做所有normal writes，支持move-wide重叠。

| Op | reads / normal writes | effect与Java主路 |
|---|---|---|
| 数值const/move | 无或source slot / 一个slot | pure；const保存bits，move不做类型猜测 |
| int/long算术、位运算、转换 | operands / result | 保留宽度/溢出；div/rem可ArithmeticException，其余不因算术新增异常 |
| FP算术/cmp | operands / result | IEEE语义，不reassociate；cmp保留NaN bias |
| string/class const | pool ref / result | resolution/may-throw；不是可随意复制的pure数值literal |
| array/field/cast | receiver/index/value/ref / 结果或无 | heap、null/bounds/type/resolution/access、volatile/init保守；unused不删 |
| invoke | receiver后按word顺序参数 / 可选result | unknown read/write/call/throw/init保守，不DCE、不提前或重复求值 |
| new/init | allocation与参数 / uninit再ref | AllocationId贯通，按§9配对，不把构造器当普通函数 |
| monitor | lock ref / 无 | 同步；enter成功才push，exit按特殊异常PC处理 |
| branch/return/throw | selector/condition/value / 无 | Terminator；throw无normal successor |

`RawOrigin`保留操作与producer/result site；每个effect site在Java计划中有明确落点。
没有优化器：不进行heap CSE、调用纯度推断、常量字段传播或异常路径删减。
`monitor-exit`异常handler选择按Dalvik“PC已前进”的规则取下一PC，不能统一套throw前PC；
接受的synchronized profile另证明合法释放不发生锁错配异常。

## 3. 固定入口Phi建造：一次绑定，不迭代猜ValueId（T07/T16）

### 3.1 采用的算法与数据

**首版选择：live-in slot → 固定入口Phi → 一次扫描block → 连接incoming → 有限验证。**
不在传播中反复切换“单值别名/Phi”，不做trivial-Phi消除；多一些Phi/locals换取简单闭环。
包括单predecessor block也分配live-in Phi，跨block use统一经过入口Phi，不直接绑定外块
OpResult。参数有虚拟EntrySeed incoming；因此实际入口有回边也能正确区分首次/循环值。

```text
Sym = Def(ValueId) | Phi(PhiId) | Bad(Undefined | PartialWide)
Phi { block, slot, result: ValueId, inputs: Vec<(IncomingEdge, Sym)> }
IncomingEdge = EntrySeed | Normal(EdgeId) | Exceptional(ExceptionEdgeId)
Env = sparse map<Slot, Sym>
```

每条参数、OpResult、normal check-cast refinement、handler ExceptionAt先分配固定ValueId。
纯数值const也有ValueId/原bits；P8可逐use拼写。`move-exception`定义独立异常值，不与
handler incoming普通寄存器混同。ID不重用、不要求跨运行相同。

### 3.2 live-in：唯一需要的数据流固定点

先以normal+exception边求结构可达集合；不做路径谓词推理。slot需求只来自实际Op/终结
read及后继需求。写入与slot重叠则kill旧slot，包括wide被覆盖一个word；normal gen与
exception-before-write分开。

```text
LiveIn[*] = empty
worklist = reachable blocks
反向扫描block:
  live = union LiveIn[normal successors]
  对每个op逆序:
    handler_live = union LiveIn[eligible handlers at this throw site]
    live = Reads(op) union (live minus SlotsOverlapping(NormalWrites(op))) union handler_live
  若live增大，更新LiveIn[b]并enqueue其normal/exception predecessors
```

集合仅增长，宇宙为有限的`(block, slot)`需求；每个新元素/扫描步先charge。存稀疏slot
集合，不先建register_count×edge_count矩阵。callback在pop、op扫描及长集合操作中检查。

### 3.3 一次绑定、连接与封口

```text
为每个reachable block的每个LiveIn slot创建固定Phi与result ValueId
逐block（任意就绪/遍历序，不要求稳定输出）:
  env = {slot -> 本block入口Phi}
  逐op原序:
    Uses(op) = read(env, requested slots)          # 先读所有source
    若may-throw，保存各handler LiveIn所需的旧env查询结果
    对每个normal write:
      删除env中与write重叠的slot
      env[write.slot] = 该op已预分配的Def
  记录normal successor所需slot的env查询，不clone完整frame
逐Phi逐incoming:
  EntrySeed从descriptor参数/this取值；其余从对应normal/throw snapshot取值
  缺slot为Bad；重叠但不完整的wide为PartialWide，绝不猜高word
  每条eligible incoming恰连接一次；未连接是内部错误，不是默认0
```

`read`优先精确slot；没有该slot但有不兼容重叠slot时返回PartialWide，否则Undefined。
block入口可以同时记录待验证的不同slot查询；最终incoming必须能提供各自正确宽度。
每个普通Op use只能引用本block入口Phi或先前local Def；跨块值只出现在Phi inputs。

封口使用两次图传播，而非ValueId fixed point：

1. 从所有实际Op/terminator uses反向追踪Phi，得到retained Phi闭包；无用Phi不参与
   语义错误判定/存储分配，但原始指令仍保留、reachable Op的每个read仍须验证。
2. 任一retained Phi输入Bad即标bad，沿Phi user反向表传播；每个bad标记只发生一次。
   任何实际use直接为Bad或依赖bad Phi，按已验证的定义/word约束拒绝Malformed。
3. 从Parameter/OpResult/ExceptionAt输入给Phi播种grounded，沿Phi user传播；保留的Phi
   若没有任何可追溯具体定义，则是无根定义环，不用默认初值填充。
4. 核normal输入是commit后、exception输入是对应opcode transfer的旧状态；不能用
   normal dominator替代此检查。typed/uninit的Java可表达性留P4/P5，不混称Undefined。
5. 验证后释放Env/live工作表；保留Phi inputs、绑定uses和必要edge事实供P5/P6消费。

终止度量明确：liveness最多插入有限需求；ID一次分配；binding每block一次；连接每edge/
Phi一次；retained/bad/grounded标记各最多一次。不存在Phi↔别名反复切换，也不把“预算
用完”叫收敛。POPL 2023提供有限域/命名的依据，本方案不用其GVN联合域或直接借其证明。

验证器从raw read/write顺序独立重放block，核本地最新定义、Phi来源和throw快照；不信任
builder自报bool。D001/D002/D003/D029/D030是公开行为证据，不冻结私有Phi数量/顺序。

## 4. 类型与Java表达式（T10/T11/T15）

### 4.1 域、约束与求解顺序

P4先给声明/proto约束，P5再求值/use类型，P8最后核源码绑定。定义身份不因类型变化重建。

```text
ValueFacts:
  width: One|Two
  candidates: finite primitive/null/ref/uninit kinds
  required_uses: finite set<UseRequirement>
UseConversion = Identity | LiteralView | PrimitiveCast | BooleanToInt | Proven01ToBoolean
              | ProvenUpcast | OriginalCheckCast
```

纯数值const仅保留bits与width，使用处选int/float/null/boolean等LiteralView；有effect的
string/class const及非literal不能逐use复制。参数/field/invoke返回/转换op给确定producer
约束；Phi result与每incoming保持可赋值关系。worklist只添加use事实、收窄候选或按有限
已知hierarchy增长ref join；每次实际改变才enqueue users，空队列才封口。

求解后按以下规则选择source类型，不依赖第一个use或HashMap顺序：
- 确定producer类型优先保留；byte/short/char作为Java算术operand提升int，写回处显式窄化。
- boolean与int分开。已知boolean需要int视图时为`b ? 1 : 0`；反向仅接受已证0/1或原
  boolean定义，不把任意int写成`x != 0`掩盖类型冲突。保留一个有限Known01事实：boolean
  参数/0或1 literal/已知boolean位运算以及`x & 1`可证明结果0/1；只有该证据才允许
  非literal int以`x != 0`转boolean。其余范围分析不在首版实现。
- Phi需一种可表示的结果类型；primitive/ref或不兼容32/64-bit解释不混到Object。
  literal可在不同Phi/use选不同LiteralView，非literal不允许numeric cast冒充bitcast。
- refs只有完整subtype事实才能做不抛异常upcast；未知hierarchy可以用Object保存join，
  但任何narrower use仍需无新增runtime check的证明，否则Unsupported。
- 已完成初始化的Ref与Uninit不同；AllocationId/UninitThis不进入普通Object Phi/默认null。

封口后每个retained Value和use都有类型/转换或明确失败，P6不再自行猜类型。

### 4.2 必需数值lowering

| 原语义 | 采用的Java表达 |
|---|---|
| int/long add/sub/mul、bitwise、shift | 对应Java运算；原结果width不提升；long移位计数是int，mask由Java定义保持 |
| rsub literal | `literal - value`，不得误写value-literal |
| byte/char/short转换 | `(byte)x` / `(char)x` / `(short)x`；原定义处保存结果，不在后用处重新算 |
| div/rem | 对应`/`、`%`，即使result不用也赋typed temp；MIN/-1、除零由Java语义保留 |
| boolean and/or/xor | `&`、`|`、`^`，不是短路`&&/||`；原CFG决定的短路只在结构层表达 |
| eqz/nez | int与0比较、boolean取反/自身、ref与null比较，按已解use类型选 |
| cmp-long | `x < y ? -1 : x > y ? 1 : 0` |
| cmpl/cmpg FP | `x < y ? -1 : x > y ? 1 : x == y ? 0 : bias`，bias=-1/+1 |
| FP→int/long、double→float | 对应Java cast，不用host Rust cast预计算原运行结果 |

cmp的x/y都是已物化locals，三次读取不重复原operand effect；该展开无需Float.isNaN等
额外调用，保留NaN和±0，不把cmp+branch一概折成`<`。不做FP重结合/fastmath。

有限FP literal从原bits解读后用round-trip spelling及f/d后缀；-0显式`-0.0f/d`，Infinity
用`1.0f/0.0f`或其d版本（负无穷取负）。canonical NaN可用`0.0f/0.0f`，仅在不要求
payload精确保留的profile使用。任意NaN payload/bit reinterpret若需标准库调用，必须在T11登记
intrinsic的准确签名/行为；T10不能隐式引入未验证调用，相关扩档仍按固定案例要求计状态。
JVM-only验证使用既定JDK21/Java8源码目标，不据此保证所有历史JVM/ART实现相关FP细节。

Java literal：普通NUL用合法octal/string escape，换行/制表用`\n/\t`；不输出会被Java
Unicode预处理变成源码换行的`\u000a`。孤立surrogate输出对应UTF16 escape；实际反斜线
使用`\\`避免形成意外Unicode escape。诊断lossy文本不进入literal。

## 5. Local、Phi copies与异常shadow（T08/T17）

### 5.1 正常边parallel copies

Parameter绑定原参数；其他非pure-literal retained Value与Phi各有独立Local，不coalesce。
对每个Phi incoming建立`CopySet[IncomingEdge]`，EntrySeed copy放方法入口，其他copy只
在**选中该edge后**执行。类型来自P5，source是已物化local或pure literal。

```text
删除相同Local且类型/转换也相同的self-copy
while pending非空:
  charge并检查取消
  若存在dst不在任何剩余source中的copy:
    输出该copy并移除
  否则:
    选一个剩余local source s；新建exact-type temp t
    输出 t=s；把所有source==s改成t
```

每次输出copy或打断有限cycle；temp/扫描/输出逐项计费，不需稳定tie-break。
`a<-b,b<-a`可得到`t=b;b=a;a=t`，三循环同样保存覆盖前source。
来源与Boissinot Out-of-SSA、DX已审局部对应，不照搬其coalescing/宽松cast。

Definition检查从EntrySeed开始按每个block的入口copy/local写/uses重放；所有路径遵守
“入口Phi已赋值→Op读旧值再commit→边parallel copy”的归纳关系。Java为DA所加0/null
只允许在该关系证明不可观察的普通local；不修复Undefined，不初始化未构造对象。

### 5.2 handler ingress

异常边没有可执行edge block。对每个`(handler, retained Phi)`创建独立ShadowLocal；
在该site可能选中的每个handler所需shadow上保存exception incoming，紧挨throw op之前。
所有source已物化、可用、初始化完成且转换不抛异常；不在snapshot里cast/boxing/call。
多个snapshot不共享slot，不做跨handler别名优化。catch入口先绑定异常对象，再把shadow
复制到handler Phi locals；原move-exception的定义/来源仍被消费，不重复定义异常对象。

D029：call成功值只进入normal Def；catch所用Phi的source来自call前snapshot。
D030：每site保存其各自旧值，不能读handler“最近一次”全局寄存器表。
嵌套catch中的throw按同一规则保存外层所需shadow，slot由handler+Phi唯一标识。
无法满足可用性/type/保护位置时停止该Java profile，不添默认值猜测。

## 6. 普通结构与dispatcher（T09/T13/T14）

### 6.1 必须可执行的dispatcher

eligibility为无handler、无monitor、无未处理uninit，且P5/P6封口。所有locals在method
scope声明，pc初值为entry block；每个reachable block一case，执行原ops及terminator：

```text
Jump(e):             copies(e); pc=target(e); continue
Branch(c,t,f):       if(c){copies(t);pc=target(t);}else{copies(f);pc=target(f);} continue
Switch(v,keys,d):    对原key/default选同一EdgeId，copies(edge); pc=target(edge); continue
Return/Throw:       执行原return/throw，不设置虚假fallthrough
```

pc default分支只作为不可达内部错误；验证pc初值、每次赋值的target和case集合闭合。
每case入口local store对应原block entry，copy先于pc更新；这是执行关系，不是数span。
condition/selector均是已求好的值。没有Object[]寄存器解释器、异常dispatcher或动态装箱。

### 6.2 自然结构的确定识别规则

在只读CFG上求normal RPO、dominators、postdominators（return/throw汇到虚拟Exit），
用显式栈而非无界递归。dom可用RPO迭代立即支配者/intersect祖先链；postdom对反图同算。
每次改变enqueue相关节点，ancestor walk按实际步收费；SCC用显式栈Tarjan，图不改写。

私有RegionArena初始一block一node；每次仅接受以下已实现schema，归约后节点数下降：
- Sequence：a只有下一条normal边到b，b唯一normal入口，保护context相同；保留边copy。
- Diamond：branch两臂到共同postdom join，臂单入口且内部可按当前schema归约；head
  ops只执行一次，两个transfer各消费自己的CopySet；两臂abrupt也可用虚拟Exit作follow。
- Loop：backedge target支配source，reverse predecessor闭包求自然loop，多latch并入同
  header；外部入口必须只到header。统一输出有label的`while(true)`，header ops每轮执行。
  header退出边先copy再break；latch到header先copy再continue。额外退出只接受当前或
  enclosing loop已确定follow，分别emit具名break；其他多出口形态交dispatcher。
- IntegerSwitch：selector一次；同target且同incoming copy可合case labels。fallthrough
  只接受无环的case-body链，链上必须保留边copy且不能重复执行direct-entry copy。
  共享tail/copy冲突或case跳转无法用自然fallthrough表达时，整方法用dispatcher；不先造
  trampolines或复制有effect的尾部来“优化”源码。

按现有图就绪关系尝试schema即可，不为文本稳定排序，也不求全局最优。候选前先算
新增node/transfer/条件/复制的计费上界；超限不建候选。普通不匹配继续下一schema，
没有自然结构但eligible就选dispatcher；证书漏edge/site是BUG，不叫模式不支持。
D007等required Java案例允许合法dispatcher，不以自然源码形态作为成功条件。

### 6.3 结构验证器

核`BlockId→执行node`、`EdgeId→transfer`及每个block原ops顺序；允许producer/result
合一个源表达式和synthetic pure步骤，但不能遗漏或重复动态effect。normal transfer的
CopySet/target、loop continue/break target、switch key/default、return/throw分别核。
保护/monitor检查按§10/§11另外执行，不把exception边塞normal dominator。

## 7. 简单raw输出

只有ValidatedRawMethod能进standard smali printer；按原指令位置输出mnemonic、寄存器、
ref、label、payload与完整所需metadata。无法可靠识别格式则Unavailable+有限excerpt。
Java失败只走这一个出口，不重试其他引擎/更宽profile。assembler/独立projection是开发
验收，不是生产依赖；细节归TASKS §4，不在核心算法里扩测试工具框架。

## 8. 支持范围与交付顺序

T06先D009方法级链，T07/T08接D001，T09接D002/D003，T10扩primitive；调用、对象、
异常、锁和完整class随后按卡依赖实现。method-in-known-context不等于完整JavaUnit。
只读请求内原身份、语义事件顺序与错误分类，不要求输出文本/合成名字跨运行相同。
required Java尚未实现仍未通过，不能因为§7可输出raw就改expected或宣称CoreRC。

## 9. 调用绑定、对象与构造器（T11/T12）

### 9.1 CallEvidence生成与discharge

P4保存原owner/name/proto/invoke kind、word operands、完整/未知声明集合、access/throws/
class-init事实。每个owner/name组包含继承/hiding候选，未提供完整候选不得标Complete。
P4只生成P5约束和BindingObligation，不要求提前得到P5 source types。

P8对每次调用按以下固定顺序处理：
1. 依P5把receiver与实参固定为locals/literal views，保持原receiver/参数求值顺序。
2. 令每实参的source static type与目标formal descriptor精确一致；只允许Identity、
   已证明upcast、原check-cast或已验证primitive转换。`(T)null`安全；不任意增加downcast。
3. 枚举完整且accessible的候选，按固定formal types核Java fixed-arity overload选择；
   varargs作为其原array参数使用，不新增拆包/装箱。要求选中的erased member/dispatch
   与原invoke一致；若两个候选仅return type不同或存在不明泛型绑定，则Unsupported。
4. static用可证明相同解析/初始化目标的type qualifier；virtual/interface保留receiver；
   direct仅当前合法private/constructor形式；super用`super.m`且核相同super解析目标。
   interface-special、跨owner访问或source shell无法表达时不改成virtual蒙混。
5. 对static final primitive/String读，另核`JavaConstantKind=Constant|NonConstant|Unknown`。
   内部unit采用§12的blank-final/static-block方案，标NonConstant；外部若是Constant，
   Java源码读可能被内联而丢class-init/原读取，不能直接lower sget，返回Unsupported。
   Unknown同样不能猜；不通过反射/helper额外触发初始化。
6. 累积escaping checked exceptions，减去按序catch可捕获项；剩余throws必须与原override/
   声明contract相容。未知throws/父契约时拒绝，不sneaky throw、catch包装或扩大public契约。

无call的方法产空但有效BindingTable；不是必须构造全平台catalog。const/primitive运算
不用虚构库事实；确需intrinsic时在同一声明表给出签名、init/effect与语义profile。

### 9.2 allocation/init

new-instance定义`Uninit(AllocationId)`，pure move/checkcast允许追踪同对象别名，但
init前不能作为普通Ref、传给任意调用或进入一般Phi。首档匹配同block唯一new→init，
中间只允许无throw/effect的alias moves；参数已经在new之前物化或为pure literal。
生成一个`new T(args)`，消费allocation/init两个site，初始化成功后所有该allocation
的alias视为同一Ref；异常路径保留各原site保护域。保护域不同且无法准确映射则拒绝。

真正constructor第一条Java statement必须为合法`super(...)`/`this(...)`。只允许把前导
参数move/数值literal拼进这个调用，不在它前面输出local声明、field写或helper调用。
随后才发普通locals/body。继承、access、ctor overload与checked throws由同一绑定规则核。
T22对编译器生成的outer capture有明确专用规则，不开放一般pre-super重排。

## 10. try/catch与保护域（T16–T18）

### 10.1 输入与第一档匹配

每个throw site保留有序`HandlerChain=[(catch type|all,target)]`，不按类型/target排序。
normal结构和各handler入口分别按§6的自然schema构造，不对protected graph用dispatcher。
共享handler入口允许多个raw catch引用，但不能有普通边进入move-exception。

单try匹配算法：
1. 从raw try起点/结束leader选单入口body region，normal退出为共同follow或return/throw；
   所有参与blocks与handler bodies的所有权集合必须互斥（共同follow不归handler）。
2. handler从其入口到follow或abrupt出口独立归约；不接受跳回protected body、跨catch
   普通reentry或共享而未分离的effect tail。仅消费原effect，不复制cleanup。
3. 按原chain生成catch。多个类型共享相同handler body时，类型互不为子类型且body不对
   catch参数重赋值才可multi-catch；否则各catch发各自原路径，或明确不支持该profile。
4. 普通subtype catch必须在supertype前；catch-all为Throwable。P4给throws义务，P8最终核。
   before-throw shadow放对应op紧前；handler exception值绑定catch参数，ingress copy在首部。
   rethrow若经pure moves/同根Phi仍是该handler原异常，则直接输出`throw catchParam`，保持
   catchParam effectively-final，不能改成`throw throwableTemp`丢失Java precise-rethrow
   typing。相应escaping checked集合取原protected操作的声明throws经catch过滤后的集合，
   再并入cleanup自身throws，不一律扩大为Throwable；未知或异常被替换则不能用此规则。
5. 重算输出每个可能抛出操作的实际catch选择；原try外cleanup不得被包进旧catch。

### 10.2 嵌套一层与验证算法

T18采用**有界bottom-up try归约**，不是猜“catch-all就是finally”：
- 候选handler组来自原chain的连续prefix；未消费suffix记为ambient保护context。
- 若handler体内throw sites指向另一handler，记录inner-handler→outer-handler依赖；
  只接受无环且可组成一层词法包含的关系，先归约inner try连同其handler成为一个node。
- body候选是同一normal region中连续whole nodes或一个完整子树；边界切穿尚未归约
  的if/loop则先归约其内部/分割sequence，仍无法解释则不支持，不复制condition/effect。
- 外层可把已归约inner try及其handler作为一个whole node包含。原chain被前项覆盖的
  后项可仅在完整subtype事实证明下忽略；否则必须保留顺序/拒绝，不能猜不可达。
- 每次归约消耗至少一个未归属handler组；候选数量、node访问、subtype查询逐步收费。
  不符合独占入口/词法包含的shared/crossing图不继续扩大搜索。

最终validator对AST递归维护catch-stack（显式栈实现）：对每个原throw site，将其外层
Java catches按inner→outer展开，按完整catch类型关系去掉永不胜过更早项的shadowed项，
要求与原chain对所有可能类型的第一匹配目标一致。未知类型关系不能据此删项；对
新增pure copy无throw，不产生额外catch site。handler本体的catch-stack只含其外层，
不能让同一try的兄弟catch捕获handler自己抛的异常。

这直接覆盖D029/D030旧值、D031优先级、D032 try外cleanup、D033/D034显式return/throw
cleanup路径及D035一层nested。保留已有重复cleanup与Throwable rethrow，不做finally
糖化；若实现未达到某required行为，该case仍未完成，不改成Smali期望。

## 11. monitor与synchronized（T19）

先算有限reference-root事实：Parameter/this/其他Ref producer各有root，pure move及成功
check-cast沿source取root；Phi union incoming，使用`None→One(root)→Many`有限域，只有
One可用于同根判断（grounded另已验证）。literal null有独立常量root；Many不猜同对象。
该事实同样供§10原异常rethrow使用，不重写SSA身份或做一般ValueRecovery。

`LockIdentity`还需**动态新鲜性检查**：定义该root的Op必须在匹配enter之前、候选整个
body/release-handler之外；候选单入口且所有出口已释放，不能在持锁期间重执行root
producer。Parameter/this本来不可重定义。仅AllocationId/ValueId相同不足以证明循环
两次执行的new/field-read给同一对象，不能据此接受exit新对象。unknown alias/freshness
不猜；将已求得的锁值物化一次再用于synchronized。

对每条normal/exception transfer传播有界lock stack（root+enter site），entry为空；enter成功push、enter
失败保留旧stack；exit成功只pop同identity栈顶。join要求同stack，单次传播栈高受
max_nesting约束；不通过无界路径枚举验证。受保护exit的异常PC先按§2规范取值。

匹配一个synchronized region须同时成立：
- 单入口在enter成功之后，锁expression已经求值一次；所有normal离开经同锁exit；
- body抛异常经原catch-all释放同锁后rethrow**原异常**，没有额外cleanup effect；
- 各return值在release前已求好，嵌套/重入按stack层次分别匹配，每条路径恰释放一层；
- 证据能排除接受profile内的null exit/锁错配；不把enter失败路径当已经持锁；
- 所有body op及原try边界都保留在同scope，不能输出空synchronized再在外执行调用。

匹配后输出`synchronized(lockLocal){body}`，消费原enter/exit/纯释放handler。方法级
modifier只在匹配**整个方法**的this/static Class锁且无前后effect时使用；managed方法的
DECLARED_SYNCHRONIZED标记本身不授权新增锁。native无body仅按其真实执行flag发声明。
raw flags仍独立保留。无法证平衡或有额外释放逻辑则该Java profile不支持。

## 12. class、初始化与编译器生成成员（T20–T23）

### 12.1 unit装配顺序

先确定owner/package/kind/hierarchy和成员声明，再分析方法；以原ClassKey/MethodKey
关联结果，不靠source name或到达次序。先验证shell，再逐方法结果，最后做unit级
final/constructor/override/generated-member检查。任何缺证据的shell或成员使class为Mixed，
独立显式方法仍可Java。非法Java原始成员名不全局重命名；仅synthetic locals可安全命名。

标识符按Java8规则验证：首字符为字母类别Lu/Ll/Lt/Lm/Lo/Nl、Sc或Pc，后续可再含Nd/
Mn/Mc；首版拒绝identifier-ignorable控制/Cf字符。关键字/true/false/null不作identifier，
构造器特殊名另处理。采用native Unicode general-category库并由Cargo锁版本，不用JVM
Character查询或把ASCII以外都lossy改名。非支持Unicode情况明确source不支持。

### 12.2 static/final与constructor chain

- encoded_values按static_fields原顺序绑定，缺省值按DEX类型给出；不按field名排序。
- source字段先只声明。非final字段的非默认encoded初值放`static { ... }`开头，接原
  clinit。final字段若原clinit没有写，则把encoded或隐式default值也显式赋一次；若有
  原写且preparation值是default，则不加赋值、由原写完成；非默认preparation加后续原写
  会造成两次final赋值，拒绝该decl profile。读发生在blank final赋值前且Java DA不能证明
  的形态也拒绝，不删除原读或补赋值。
- **static final primitive/String不写常量表达式字段initializer**，避免javac内联sget
  并消去原class-init触发。Generated enum/inner隐藏成员由其owner规则消费，不重复发初值。
- final采用有限`{0,1,Many}`路径写次数集合；join取并集、写入使0→1、1/Many→Many。
  只核正常完成路径，abrupt初始化失败照原异常传播。每个正常构造链/静态初始化必须
  恰一次；存在重写、缺失或未知路径则不伪造default赋值补齐。
- this()-委托图先核无环及目标身份；delegating ctor继承被调ctor的写集合，不重复发
  字段初始化。super/this调用在body前，父行为不省略。原field写位置不美化到声明处。
- clinit发`static { initExit: { ... } }`，原return-void转`break initExit`，不在Java
  initializer里打印return。若原body必定abrupt，包一层`if (true) { ... }`满足Java
  initializer的正常完成检查；条件是常量、无false动态路径，不是异常桩或跳过原body。
  已知无code的clinit无需造方法；初始化异常不包装，由目标VM类初始化规则处理。

### 12.3 generated member不是DeclarationOnly

bridge、enum的values/valueOf/隐藏构造参数、inner outer-capture属于语言生成形态。
P8使用`MemberEmission::Explicit | Generated(owner-node, rule)`；Generated仍是有code的
Java成员证据，不计abstract/native DeclarationOnly。unit printer仅发owner语法，不能
同时打印与javac生成成员冲突的重复声明；raw身份仍映射到该owner的实际source spans。
请求单独get-method而无可独立Java声明的generated成员时返回该原方法Smali，不伪造wrapper。

具体只接受以下规则：
- Covariant bridge：bridge/synthetic标记、完整parent/override事实，body只有参数moves/
  必需check-cast→对同receiver精确target调用→return，没有额外effect/handler。仅当选用
  的source override/generic声明确定会生成同签名同转换行为的bridge才标Generated；
  否则class Mixed，return-only overload不靠重命名规避。
- Named inner：InnerClass/Enclosing元数据确定owner，不按`$`猜。nonstatic inner的唯一
  synthetic outer字段/隐藏第一个ctor参数及其canonical赋值匹配后，用`Outer.this`和
  `outer.new Inner(args)`表达；该隐藏赋值由javac拥有，不开放一般pre-super写字段。
  其他capture/副作用不匹配就不生成匿名/lambda或helper。
- Enum：constant静态字段/顺序、name/ordinal隐藏参数、唯一私有storage、values复制、
  valueOf调用必须是canonical模式；隐藏storage没有模式以外的实际引用。只发Java enum
  constants/用户成员，把匹配的隐藏成员记Generated；不能发`class extends Enum`。
- Annotation：interface元素return/default合法、runtime retention与值完整保留；数组/
  String按UTF16/原bits输出，class/enum值用精确descriptor。system元数据不伪造Java注解。

Generated是检查后的core emission规则，不是“看见synthetic就删”。任一规则失败不能
给任意方法造异常桩；对应required full-unit案例仍保持未通过状态。

## 13. 控制与资源的具体接线

所有open/index、方法分析及render调用使用同一个`&mut RunContext`；Limits归该context，
Options只保存源码策略。context不存IR、不spawn、不全局共享；callback在每个循环pop、
instruction、集合增长/复制和长字符串分块处检查。上游取消立即结束，不转raw出口。

采用**累计、不退款**的request配额；method另有normal与预留raw的小额子配额。先checked
估计分配字节并charge，再try_reserve；后者失败映射BudgetExceeded，不能当格式损坏。
Env/Phi inputs/metadata/UTF16、HashMap容量、候选AST、spans/diagnostics/输出都计费。
正常分析产物用完即drop，raw出口只借原始输入；drop降低实际内存但不恢复累计额度。

每method开始先从总request配额划出raw工作/输出保留量，normal不能使用它；预留仍在
总硬界限内，不能每个方法另发一份无限request额度。unused raw预留在本method结束后
可作为未消费额度归还父账，已经实际消费的work/bytes永不退款，不构造重试绕限。
P8候选先算增量容量再创建，失败丢候选；P9 writer写每段转义后字节前收费，不能String
全造好才check。Report只留结果必需数据，不把整套Flow/SSA环境带到输出层。

默认具体上限、report-owned数据和public interface以DESIGN为准。这里的计费
是保守逻辑上界，不承诺allocator/RSS完全等于账本；性能只在授权release输入上测量。

## 14. 应用输入、扫描与提取

以下采用确定的native方案，不搬旧session/MCP，不搭通用文件框架。安全文件访问使用
cap-std的Dir/普通文件safe interface，跨平台文件身份用same-file，私有临时文件用tempfile；
压缩沿用已审阅的bounded native inflate方式，CRC用crc32fast。依赖在实现时加入/锁定并
检查许可，不在本次改Cargo，也不因依赖内部有unsafe而在Diana代码开unsafe例外。

### 14.1 InputLease与目录

1. 先固定允许root及Dir handle，按capability-relative规则打开显式输入；禁止设备/FIFO，
   Unix safe OpenOptionsExt加O_NONBLOCK/O_NOCTTY，避免FIFO/终端打开阻塞或改进程终端；
   Windows拒绝device/pipe namespace。得到handle后再次核regular file、len与文件身份。
2. 小输入直接bounded copy进owned bytes，大APK复制到私有snapshot file；每读固定块
   检查取消/累计input配额，最多读limit+1用于拒绝增长。前后核原handle身份/len/mtime，
   观察到变化报INPUT_CHANGED并丢弃snapshot。这里只承诺复制后immutable，不声称stat
   检查能证明原文件任意并发写时的原子版本；后续只分析已复制的内容，不再读原path。
3. 大APK只读中央目录和当前需要的entry范围；被materialize的entry各有独立owned buffer，
   stored借该buffer、deflate借解压buffer，均由lease持有。无unsafe mmap，不复制整APK
   到每个DexImage，不宣称这条safe copy路径是零拷贝。
4. EOCD从尾部最多65535+22字节搜索，comment长度须对应文件尾；single-disk，目录
   count/bytes先核上限。遇classic sentinel，核ZIP64 locator/EOCD记录的签名、长度、
   single-disk字段与64-bit目录范围，再读目录；不将0xffffffff当真实分配量。
5. 每个central record按原ordinal存EntryKey，检查固定46字节+name/extra/comment长度。
   UTF8 flag名称严格解码，其他按ZIP CP437；保留原name bytes，不basename/normalize。
   同名record全保留，名字查找不唯一必须要求原entry-index；选manifest按原ordinal规则，
   不能由worker完成次序决定。local header另核签名/flags/method与data范围，bit3数据
   descriptor的最终CRC/size以中央目录及实际解压校验为准。

### 14.2 轻量定义/xref扫描

1. H03/H09不建Engine/SSA。class_defs→class_data四组delta分别累加，列**实际定义**，
   不把所有field_ids/method_ids引用当定义。每row携scope/physical/logical/class/member原key。
2. 字符串匹配先ASCII/raw预筛；非ASCII/精确xref必须解MUTF8到u16后比较，不把replacement
   文本作查找truth。display escaping与匹配分开；只对内容做filter，结果全部返回。
3. 指令layout复用engine的stateless `raw::instruction_layout(code_bytes, pc_cu, version)`，
   返回checked width与至多两个`(RefKind,index,operand_slot)`，不初始化ProgramIndex。
   该function由T03的同一format表实现；engine内部完整decode也用它，不复制两份宽度表。
   metadata/index仍由当前scanner核自己的logical表。unknown width导致本扫描明确失败，
   不猜下一条地址、不漏site后报成功。
4. 每个actual direct ref operand生成site，identity含MethodKey+code_unit+operand_slot；
   不扩展为签名的传递引用，不按方法去重。相同数字index但不同logical image绝不合并。
5. 首版可单worker，后续多worker用有界byte/row队列与weighted admission；按完成发布，
   无global sort/按ordinal等待。consumer关闭即停止producer；途中失败显式失败，不把
   已发prefix当完整成功。没有分页、preview或成功截断。

### 14.3 原entry提取与发布

1. 精确选EntryKey，核加密/method/local/central范围；method0要求压缩/解压长度一致。
   method8在声明长度与限额内bounded inflate，核实际长度与结束状态。原payload不经
   UTF8/JSON、DEX normalize或header改写。CRC32遍历实际payload并与central CRC一致。
2. output只用显式-o，不拼entry name。一次打开输出parent Dir并持有；拒绝目录/符号链接
   目标与输入同handle/hardlink身份。所有临时/最终操作相对该Dir，不重新解析可变parent路径。
3. 在该Dir以create_new生成mode0600临时文件，碰撞重试有固定上限；写入/flush并完成
   CRC/size/预算检查才publish。错误/取消只清自己的temp，不删用户目标。
4. no-force以同Dir **hard_link(temp, final)** 原子发布，存在任何final项就失败，再删除temp。
   两发布者最多一个成功；不采用exists后rename。文件系统不支持该primitive时明确失败，
   不降为有覆盖竞态的实现；D070平台证据未具备仍not-run。
5. force在入场时只允许普通、非输入alias目标，然后相对持有Dir做原子rename替换目录项，
   不跟随最终symlink去写别处。它不是“只替换此前inode”的CAS接口，不承诺外部进程不能
   在检查后改名；固定输入path本身禁止作输出，源handle字节不得被写。不能满足这些safe
   primitive的平台不做弱化实现。原子可见性不冒称断电持久性。
6. 输出文件成功后stdout无binary/状态行；query/get的文本/JSON与本binary输出adapter分开。
   BrokenPipe或I/O错误停止请求，文件发布前错误目标不变、无半文件。默认get文本先核
   Java/JavaUnit资格并生成完整有界payload后再写，不先发半段Java再发现Mixed。

H02/H10核心文件安全测试继续走公开领域interface与既定自制目录；上述依赖、路径包装
或命令解析不另建机械/辅助测试。原始权限/来源、跨平台not-run及release成本如实记录。

## 15. 可选美化

T24只做一个真实pure recovery，默认Core/off；E01–E08不成为CoreRC依赖。不建插件/registry。
候选只读已有facts，在有界副本/有限edit-log中修改；构建前收费，Applied后重验绑定/控制/
effects/spans；Declined或optional-budget保留基线，Cancelled停止，BUG不得包装Declined。
on/off跑同一固定行为与拒绝反例；未实现option报配置错误，不先出public catalog。
不为将来美化保留整套已释放分析；需要新facts时由实际扩展单独提出。

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
