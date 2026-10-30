# Diana 核心算法技术规范

本文规定DEX解码、分析、Java/Smali表示的算法、状态转换与拒绝条件。
interface及ownership见[DESIGN.md](DESIGN.md)，验证规则见[TASKS.md](TASKS.md)。

## 0. 语义模型与正确性条件

**规则**定义算法，**推导**明确其正确性前提；TASKS定义的独立运行证据不能由推导替代。
下述论证不是机器检查证明。

### 0.1 对象与状态

| 记号 | 定义 | 不允许混同 |
|---|---|---|
| `N` / `pc` | physical字节数 / method内code-unit位置 | `FileByte`、`CodeUnit`、UTF16与source UTF8 byte |
| `I,B,E,X` | 原指令数、reachable block数、normal edge数、exception edge数 | 同target的不同edge仍是不同选择 |
| `R,S,P,A` | register words、被请求的Slot集合、保留Phi数、Phi incoming总数 | register不等于ValueId或LocalId |
| `sigma` | 运行时原寄存器store，wide为两个相关word | 未定义不是0，uninit不是null |
| `lambda` | 生成程序的typed locals store | DA补的初值不是语义定义 |
| `H,K` | heap / 当前lock stack | 局部copy不能假装heap操作或锁操作 |
| `tau` | 调用、读写、异常、同步、初始化等有序可观察轨迹 | 独立分析报告的展示顺序不属于tau |

ValueId是**静态定义位置**，不是跨循环永远不变的运行时值。证明中可写
`value(v, activation)`区分同一Op/Phi在两次block进入时产生的实例；实现不存activation
计数器。每次入口Phi赋值/Op执行后，lambda代表当前实例，而不是历史实例的集合。
这也是§11为什么除了同root还要检查持锁期间不会重新执行root producer。

### 0.2 保持关系与适用前提

block入口的关系`Rel(b,sigma,lambda)`：b的每个live-in slot，其Phi local按source type
解释后的word值等于sigma；本block先前已经执行的Def也对应当前实例。heap与锁的
应用可观察部分一致，原事件前缀tau一致。非live slot、未读的DA初值和pc/temp不要求相等。
纯literal用其原bits的选定视图解释，不要求它有local。

目标是：每个原正常/异常步骤由有限个Java步骤模拟，新增步骤只读写不可观察locals；
选同一edge、产生同一可观察事件和后续Rel。反向也不能多出可观察路径；dispatcher的
非法pc分支、静态初始化的`if(true)`假分支需证明不可达。有限copy/调度步骤不能无限
stutter而让原本终止的方法发散。有限Driver只提供所测输入证据，不替代这条论证。

前提包括匹配的声明/链接环境、选定的Java语义profile及未触发实现相关VM资源失败。
异步异常、实际OOM/GC/栈帧差异不在本稿的逐指令模拟保证内；这不授权移动原allocation、
may-throw、volatile、锁或class-init。同步异常与原保护域仍逐site保持。JVM21的证据不
自动升级成历史JVM或ART全部行为的保证；尤其不能把单线程轨迹叫完整JMM证明。

### 0.3 决策的取舍

| 决策 | 选它的原因 | 明确不采用 |
|---|---|---|
| 原地址+borrowed bytes | 一套坐标可核原身份/范围，041共享数据不失真 | 改header后再把副本称raw truth |
| atom后切CFG | producer/result不可被leader规则拆成错误定义时机 | 将成功result暴露给catch |
| 固定入口Phi | 把身份问题与信息传播分开，易给出有限度量 | 别名/Phi交替重写直到“看起来不变” |
| family选择后再定source type | bits解释、名义类型和use转换职责分开 | 一个Unknown变量同时做join/猜cast/打印类型 |
| 独立locals+parallel copy | 牺牲紧凑度，避免coalescing干扰与旧值丢失 | 按物理reg复用Java local |
| 明确transfer与词法scope | 可以逐edge/site检查copy、catch与锁 | coverage数量等于语义证明 |

## 1. Reader、格式表与原始指令（T02–T04）

### 1.1 地址空间与读取函数

`Cursor`持有`bytes: &[u8], pos: usize, end: usize`；所有读取返回Result，不索引后再检查。
`range(off,n,stride)`按`n.checked_mul(stride)`、`off.checked_add(size)`核验，随后才取slice。
固定记录保留borrowed view；需要解码的变长记录先核remaining/最小记录尺寸、累计预算，
再try_reserve。偏移分别用`FileByte`、`CodeUnit`，不在调用点裸乘2。

- 接受已定义的035/037/038/039/040/041容器；036及未知版本为UnsupportedContainer。
  版本识别不代表该版本的全部Java opcode已支持。reverse-endian为Unsupported。
- 035–040要求header_offset=0、header_size=0x70、file_size等于本次物理DEX长度。
- 041要求header_size=0x78；0x70/0x74字段分别为container_size/header_offset。
  从0起按每个header的file_size checked前进，最后恰到container_size；所有header的
  container_size与物理长度相同、header_offset与实际位置相同，步长至少0x78。
- 041的section/data/code/debug偏移都是**物理container起点**的绝对偏移，可引用后续
  logical member的数据，不能引用当前header之前的数据；不得用logical file_size截断
 共享数据，也不得加header_offset两次。data_size/data_off在041不用作数据区边界。
- 所有输入view均是原container+header_offset，不overlay header、不重算“原始字节”。
  一次校验物理header链，选中logical member才建其catalog；相同物理slice不重复计输入字节。
- header checksum/signature只保留原字段，不作为内存安全证明，也不宣称完整ART
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

不可达代码也校验宽度/显式引用，不用DCE掩盖截断。显式branch跳入指令中间始终拒绝；
fallthrough到payload/落出code末尾是reachable执行检查，不把return之后的dead padding
误判为实际执行越界。raw仍保留所有已解码指令，published FlowBody只含reachable部分。goto/goto16与if-*的delta不得0，goto32允许0；不能一概拒绝合法自环。
CFG leader为入口、跳转目标、终结后、try边界、handler入口，
以及受保护may-throw操作之后。每个受保护may-throw为block最后一个操作，normal/
exception edge分开；同target不同case/rank仍有不同EdgeId。

invoke/filled-new-array与紧随的move-result先绑定为一个语义atom，再按atom切CFG，
保留两个raw site；result只在normal路径写目标寄存器，丢弃result也保留call。result kind/return kind不符、
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

### 1.5 范围判断的推导与执行次序

对`n`条stride为`s`的记录，合法条件是`off<=N`且`n<=(N-off)/s`（s>0）。
先做除法式上界/checked乘加再取slice，不能先算可能溢出的`off+n*s<=N`。record count
满足最小字节界限只是必要条件：之后逐record核其变长尾部和ref。读取不分配；需要
Vec时先核输入事实、语义count和资源配额，再try_reserve，三者不能互相替代。
若已读字段的最小字节数可直接证明超出remaining，先记Malformed；不因同一count也
超过分析限额而改报BudgetExceeded。尚未读到该字段前的取消/请求admission资源失败
仍按各自原因返回，不为诊断强行继续解析。

```text
read_table(off, n, stride):
  reject stride==0 or off>N
  reject n>(N-off)/stride
  end=checked_add(off, checked_mul(n,stride))
  return bytes.get(off..end) or Malformed
```

这是局部内存安全推导，不证明map/type语义正确。041另有三个区间：physical `[0,N)`、
logical span `[h,h+file_size)`、当前member可引用的后续物理范围；span是header遍历区间，
不是引用数据的上界。比如h=0x100、file_size=0x80、N=0x500时，数据0x400可能合法，
0x80不是该member可引用的前向数据。section类型/对齐/重叠规则仍另核，不因在physical
范围内就把另一个header当string/code。所有地址保持原值，不加h或拷贝共享区域修正。

### 1.6 atom、leader与result跨try边界

`Atom { raw_sites, reads, normal_defs, throw_site, protection }`是P2私有建造表示，不新增
public seam。先验证raw邻接/禁止branch进入move-result，再将该result并入producer。
保护链按真正throw_site的原pc查询，不按atom的最后一个raw site查询。

若raw try恰在move-result前开始/结束，不能既强制该pc为leader，又把result当独立可
抛出的操作。仅对这个已证pure result commit，将**图切分点**投影到atom之后；原try
范围、raw metadata和原pc不改。若其他真实操作/目标被跨越，拒绝该融合，而不是搬它。
因此call在try内/result在try外与call在try外/result在try内均有准确throw保护链。

执行次序固定：读取receiver/args → 保存所需旧frame → 执行producer → 成功时commit
result。assignment必须无需boxing/新downcast，否则不能借“commit不抛异常”作推导。
这条规则同时解释D029：即使目标reg与旧变量同号，catch仍只能读失败前的值。

### 1.7 CFG建造与坏尾部

1. 对完整raw流记录boundary表和payload范围，核所有显式target/ref，即便其site不可达。
2. 构造atom边界与raw-pc到atom映射；try切分遵守§1.6，handler入口仍必须是合法入口。
3. 从entry沿候选normal/exception边求结构reachability。终结后可有raw数据/死代码，
   但一个reachable可继续执行的atom必须有合法next；没有next不能造return/nop。
4. 只对reachable atoms发布Block/Edge；每个受保护may-throw/throw terminator有独立
   exception site，chain顺序不变。未捕获异常是abrupt exit，不造handler伪节点。

扫描、边界核验和reachability基础成本O(I+E+X)，存储O(I+B+E+X)；变长metadata与
payload按实际元素另计。边/leader插入及queue操作前收费。未知width在第一遍停止：
后面即使碰到像return的word也不能恢复可信指令边界。

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
invoke-custom[/range]的已知format、call_site/method_handle/proto等metadata由raw层完整
验证和保留；Java主路为Unsupported/InvokeCustomUnsupported，不猜普通invoke或未知
宽度。拒绝只影响该方法，class仍可展示其他已恢复成员及该方法原Smali。
没有优化器：不进行heap CSE、调用纯度推断、常量字段传播或异常路径删减。
`monitor-exit`异常handler选择按Dalvik“PC已前进”的规则取下一PC，不能统一套throw前PC；
接受的synchronized profile另证明合法释放不发生锁错配异常。

### 2.1 统一transfer与“旧状态”的准确含义

对Op定义两个不同的frame transfer，而不是给所有successor复用一个out环境：

```text
args = read_all(sigma, Reads(op))
normal:      (result,H',events)=execute(op,args,H); sigma'=commit(sigma,result)
exceptional: (ex,H',events)=execute_throw(op,args,H); frame'=sigma
```

异常只是不commit**该Op的目标register**，绝不表示heap回滚。callee先写字段再throw时，
catch同时观察旧register和已发生的heap effect。check-cast成功路径的新类型也不进入
exception frame。throw terminator本身没有normal successor，但有operand read和可能
的handler transfer，不能因它不在ops数组就漏掉liveness/snapshot。

### 2.2 宽值与重叠写的次序

Slot可视为half-open word区间`[start,start+width)`；两个slot重叠当且仅当区间相交。
读wide必须取得完整同一值的pair，不能独立拼两个各有定义但不属于同一wide的word。
一条Op先读完全部source再使重叠旧slot失效，故`move-wide v1,v0`的重叠copy不丢source。
相反，`const-wide v0; const v1; return-wide v0`的高word已被覆盖，必须拒绝，不靠旧
ValueId还存在就继续读；D058的独立事实正是这个不变量。

### 2.3 不引入Java隐藏求值

所有heap/call/array operands先按原序物化，Java表达式只读这些locals及安全literal。
`arr[i]=value`不能顺手变`arr[i]+=call()`；自动boxing/unboxing、复合赋值、重复receiver
或把多条volatile读合一都可能新增/移动异常与访问。unused field/array/cast/div仍发
合法typed赋值保留其执行；no-result call仍发call。primitive narrowing属于原Op或
原store的明确语义，不是为让Phi copy“能编译”任意添的转换。

### 2.4 fill-array-data的确定展开

该Op向**既有primitive array**写入payload，始终登记may-throw，不能因第三方opcode表
漏标CAN_THROW而删除其检查。P1核payload范围/count/element_width；P5另证component与
width一致：Z/B=1，S/C=2，I/F=4，J/D=8。已知reference-array不符合该DEX约束；只有
信息不足时不能凭width猜int[]或float[]。

P8仅构造以下两种FillPlan，其他情况为Unsupported/ArrayFillShape并保留raw：

| 条件 | Java展开 |
|---|---|
| count=0，已知primitive component/width | 在原site执行`int unusedLength = a.length;`，无store |
| count>0，满足下列FreshArray证明 | 同一位置先读`a.length`，再原序发a[0]..a[count-1]的typed stores |

FreshArray证明必须包含：
1. a来源唯一new-array，长度是已证非负整数常量L，L>=count；allocation原位置保留，
   不把fill和new合成一个会提前执行元素计算/改变异常位置的新表达式。
2. 从new的normal完成到fill为无环单入口normal链。数组值只经过move-object或单incoming
   的pass-through Phi；没有外部normal入口/回边让另一个实例进入fill。
3. 链中只允许数值const、上述alias move及纯control直通，没有调用、heap访问/发布、
   monitor或其他may-throw操作。数组尚未写入field/其他数组、传参或返回；所有aliases
   均计入检查，不能只看当前物理reg。
4. element逐项能按§4.8精确发出。Z的payload限0/1，B/S有符号、C无符号；F/D按原bits
   解释，保留-0/Infinity；无法用所规定Java literal精确保持的NaN payload拒绝Java。

```text
lower_fill(op, facts):
  require known primitive component and matching width
  if count>0: require FreshArray(op.array, length>=count)
  require every payload element has exact Java literal
  charge all length-temp/store/literal nodes before construction
  emit unusedLength = arrayLocal.length at original protection context
  for i in 0..count: emit arrayLocal[i] = exact_literal(payload[i])
```

count=0仍保留length read，所以null参数照样NPE，不因循环零次而正常返回。count>0的
accepted形态已证非null且长度足够，primitive stores不会发生NPE/bounds/ArrayStore异常；
数组未逃逸、展开中无call/effectful operand，故中间写入不可被程序观察。结束后前count
个元素与payload相等，尾部仍为原默认值，register store不变。

长度不足的普通参数不能逐项展开：它可能先改前几个元素再抛，与原bulk操作检查/写入
合同不符。这里不合成异常对象、补helper、插入新库调用或建立通用array recovery。
FillExpansion统一拥有原site，length-check与stores是其有序children，覆盖器不把它们误算
为多次原fill执行；snapshot放在整个expansion之前。D027作为正例须由独立输入证书
逐项证明int/float新数组形态与payload，不能由Java literal文本推断原DEX必含fill。
运行成本O(count)，AST/常量按实际增长计费，仅检查显式配额；不预测JVM code费用。
payload本身借原bytes，不先复制巨大Vec。

## 3. 固定入口Phi建造：一次绑定，不迭代猜ValueId（T07/T16）

### 3.1 采用的算法与数据

**算法：live-in slot → 固定入口Phi → 一次扫描block → 连接incoming → 有限验证。**
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
  live = Reads(terminator) union LiveIn[normal successors]
         union LiveIn[eligible handlers of a throwing terminator]
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
  绑定terminator reads；若terminator可throw，保存其handler所需旧env
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
   任何实际use直接为Bad或依赖bad Phi，不生成Java。确定违反DEX定义/word约束或有
   实际reachable坏读证据时为Malformed（D057/D058）；若只是保守边集合导致无法证明
   可用性，则为Unsupported/DefinitionAvailabilityUnproved，不把合法性未知伪称损坏。
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

### 3.4 方程、有限性与复杂度

令`S`仅含实际read/后继需要的合法Slot，Kill是“与normal write区间重叠”的slot集合。
逆序transfer为`Reads union (LiveAfter - Kill) union HandlerNeedsBeforeWrite`。
集合差的Kill固定，与当前迭代信息无关，所以transfer对集合包含关系单调；从空集开始
只会增加。任一`(block,slot)`最多首次加入一次，故终止不依赖调度顺序或ID是否相等。
这不授权先把Unknown当Undefined；incoming未连接仍是建造态Option。

最简单实现每次重新扫描被影响block，worklist最坏可有O(B*|S|)次有效增长，单次扫描
还需访问Op和相关需求集合；不能声称整个P3总是线性。扫描中的set元素/edge/input也收费。
Phi数P不超过最终live-in总量，incoming数A按真实eligible边计；values/phi_inputs/分配
预算仅在显式配置时限制增长。连接和后续三次图传播为O(P+A+uses)，额外存储同阶；不预分配R×E矩阵。

### 3.5 diamond的逐项走查

```text
entry: if c goto L else R
L:     r = a; goto J
R:     r = b; goto J
J:     return r
```

J的terminator首先产生live-in(r)，反传到L/R后由各自write kill，再产生a/b需求。
为J/r创建固定p；L/R各自绑定到入口参数Phi，写出固定dL/dR；p的两个incoming分别是
`(L->J,dL)`、`(R->J,dR)`。P6只在选中L/R那条实际edge执行`local(p)=local(dL/dR)`。
参数链上的冗余Phi保留，不影响关系。若把这两条copy都放在entry，后一条会覆盖前者；
若漏掉J的return read，甚至不会创建p。这说明“终结operand也算read”不是实现细节。

### 3.6 归纳不变量与错误归属

- 初始：EntrySeed提供参数/this，其余不凭空有定义。
- block内：第k个Op读取的是前k-1个normal commit后的最新env；所有source先于write。
- 边：normal输入取最后commit后env；exception输入取该site执行前env；handler异常
  对象是ExceptionAt的新定义，不是incoming reg的别名。
- 循环：回边与EntrySeed都是固定incoming，一次连接全，不能因首次只见entry就seal。
  Phi local每次进入都被重赋值；静态PhiId不声称不同迭代的动态值相同。

从真实uses反向保留Phi闭包，是为了不因一个永不被读取slot的Undefined拒绝方法；
但不能以“result没用”为由跳过一个reachable Op的实际输入/effect。grounded仅排除
无根定义环，不能单独证明每条路径有定义；Bad传播也必须独立完成。结构reachability
不是完整路径可行性证明，不使用SMT/路径求解器；合法性未知按上文分类而不是补0。
验证器重放raw读写及保存的edge来源，不能只检查builder生成的同一份计数。

## 4. 类型与Java表达式（T10/T11/T15）

### 4.1 域、约束与求解顺序

P4先给声明/proto约束，P5再求值/use类型，P8最后核源码绑定。定义身份不因类型变化重建。

```text
Family = Integral32 | Float32 | Integral64 | Float64 | Reference
ValueFacts { width, family_group, producer_origins, integral_bits, integral_range, use_requirements }
UseConversion = Identity | LiteralView | PrimitiveCast | ProvenNarrow
              | BooleanToInt | Proven01ToBoolean | ProvenUpcast | OriginalCheckCast
```

不使用一个变量同时“收窄类型/扩大join/替换Unknown”。采用§4.3–§4.5的分离算法：
先闭合表示family约束，再单调传播producer/值事实，最后一次选择source type并逐use检查。
uninit的可用性由§9单独给出，不能混入普通Reference family后消失。

纯数值const仅保留bits与width，**每次use单独选视图**；有effect的string/class const及
非literal不能逐use复制。P4给固定声明/proto事实，不要求先知道P5最终source type。
一个已封口source type不在P6/P8随打印需要重选；检查失败不能靠任意cast修复。

求解后按以下规则选择source类型，不依赖第一个use或HashMap顺序：
- 确定producer类型优先保留；byte/short/char作为Java算术operand提升int，写回处显式窄化。
- boolean与int分开。已知boolean需要int视图时为`b ? 1 : 0`；反向仅接受已证0/1或原
  boolean定义，不把任意int写成`x != 0`掩盖类型冲突。保留一个有限Known01事实：boolean
  参数/0或1 literal/已知boolean位运算以及`x & 1`可证明结果0/1；只有该证据才允许
  非literal int以`x != 0`转boolean。§4.9只传播producer边界用于无损窄化，不进行算术range推断。
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
| boolean参与的DEX位运算 | 必要时以`b?1:0`取得int carrier，再用`&`/`|`/`^`，Known01结果可转回boolean；不能生成`boolean ^ 1`，也不能改成短路`&&/||` |
| eqz/nez | int与0比较、boolean取反/自身、ref与null比较，按已解use类型选 |
| cmp-long | `x < y ? -1 : x > y ? 1 : 0` |
| cmpl/cmpg FP | `x < y ? -1 : x > y ? 1 : x == y ? 0 : bias`，bias=-1/+1 |
| FP→int/long、double→float | 对应Java cast，不用host Rust cast预计算原运行结果 |

cmp的x/y都是已物化locals，三次读取不重复原operand effect；该展开无需Float.isNaN等
额外调用，保留NaN和±0，不把cmp+branch一概折成`<`。不做FP重结合/fastmath。

有限FP literal从原bits解读后用round-trip spelling及f/d后缀；-0显式`-0.0f/d`，Infinity
用`1.0f/0.0f`或其d版本（负无穷取负）。canonical NaN可用`0.0f/0.0f`，仅在不要求
payload精确保留的语义条件下使用。任意NaN payload或非literal bit reinterpret若无
现有表达规则则Unsupported；不为恢复新增标准库调用。原程序已有调用仍按§9绑定。
JVM-only验证使用既定JDK21/Java8源码目标，不据此保证所有历史JVM/ART实现相关FP细节。

Java literal：普通NUL用合法octal/string escape，换行/制表用`\n/\t`；不输出会被Java
Unicode预处理变成源码换行的`\u000a`。孤立surrogate输出对应UTF16 escape；实际反斜线
使用`\\`避免形成意外Unicode escape。诊断lossy文本不进入literal。

### 4.3 表示family：有限约束，不是全局猜类型

Family不是最终Java类型：boolean/byte/short/char/int同属Integral32，但各producer可以
保留自己的合法声明类型。I32与F32、I64与F64不能因为同word宽度而互换；numeric cast
会改数值，不是bit reinterpret。Reference与Integral32也不因const 0而合并。

建两个不同的图，不能拿“类型相容”当“值相同”：

- family等式图：move/Phi输入输出应同family；双寄存器if-eq/ne两边需相同可比较family。
  equality只约束表示，不传播对象身份或值。scalar opcode、descriptor、move-object/
  move-wide等给出允许family集合；if-*z可允许Integral32或Reference，FP条件须经cmp。
- 值事实图：只有value-preserving move/Phi边传播producer origins；比较两数不使其
  origins相同。check-cast/primitive conversion是独立producer，不跨它传播旧类型结论。

family选择前，先将descriptor已确定的origins沿move/Phi及array component规则求有限
闭包，给aget/aget-wide补来自实际array component的family约束；不能先默认I32再发现
输入是float[]。无array证据就不能封该load的Java类型。这个descriptor闭包只做union，
不依赖最终source storage type，因此不形成P4/P5互相等待。

pure const按`LiteralUse=(ValueId,use-site)`创建**类型视图记录**，不分配新SSA ValueId。
同一个const 0的一次Reference use与一次Integral use不是同一个family节点；否则会把
多态literal错误变成whole-method类型冲突。非literal只靠真实值流/约束关联，不复制视图。

```text
为普通value和LiteralUse建family节点
用union-find合并等式边（Phi逐incoming、move、需要同family的eq/ne）
对每组G：Allowed[G] = intersection(全部成员的width/opcode/descriptor要求)
对LiteralUse再限制：32bit可I32/F32，0可Ref；64bit可I64/F64
若Allowed为空：该Java表示不支持，不默认Object
若仍多种：按已收集use选唯一所需family；无区分性use则One默认I32、Two默认I64
封口后检查所有原operand约束，不在P6重新选择
```

默认仅消解没有producer/use能区分的literal解释，不覆盖一个已知F32 producer。
多个非literal use要求不兼容family时不搜索cast组合/引入bitcast库。family图构建近线性
（union-find摊还O((values+constraints)*alpha(values))），每条边/合并/集合交均收费。

### 4.4 producer与Known01：第二个有限固定点

descriptor origins按上节先闭合以约束typed load；family封口后继续核值事实/整数域，
仍只使用有限集合的union，不反向改family：

```text
Origins(v): finite set<primitive producer tag | qualified reference type>
IntegralBits(v): subset {Zero, One, Other}
```

参数/field/返回/conversion等给具体producer seed；move复制source facts，Phi union其
incoming。原类型名只来自已读descriptor/catalog及其有界数组component，不无限合成
泛型类型。普通参数int的IntegralBits为三项，boolean为{Zero,One}，const 0/1为单项，
其他整型literal为{Other}；其余arithmetic默认三项，不能随意推断range。

`and-int x,1`可直接给{Zero,One}；and/or/xor的两边均只可能0/1时，结果也只可能0/1。
事实未到达的空集是Bottom，不是“已证为false”。转移函数在可能值集合包含关系上
单调，未知来源只会扩大可能集合；worklist每次实际增加才传播。最终非空且不含Other
才Known01，不能因未封口集合只有0便删分支或封类型。

array-get的component facts从已证明array origins推导，strip一个`[`取得component；
没有array证据不造强转。每个推导的component仍属于有限descriptor闭包。ref集合可能
增长较大，按新fact逐项计constraints/字节；不声称这一步与输入字节严格线性，最坏
约为可到达facts数量乘其传播边数，并由method/request配额限制。

### 4.5 一次确定source storage type

producer本身保留声明/原语义决定的类型；只为Phi选择共同storage type：

| family / incoming facts | Phi结果选择 | copy/use规则 |
|---|---|---|
| Integral32，全部非literal来源为boolean | boolean | literal限0/1；int use用BooleanToInt |
| Integral32，含byte/short/char/int或其他数值来源 | int | 窄producer按Java promotion读；boolean来源显式解为0/1 |
| 仅literal的Integral32合流 | 全部typed use为boolean且bits均0/1可boolean，否则int | 不是把任意非0值称true |
| Float32 / Integral64 / Float64 | float / long / double | literal按bits view；禁止跨family数值cast冒充copy |
| Reference，含非null来源 | 已知共同名义superclass；无法求更窄时Object | interface/数组规则只用已证明assignability；后续narrow use仍要证据 |
| Reference，所有incoming是null | 可同时满足所有use的已知ref type；无约束用Object | 多个不相容narrow use无单一type时Unsupported，不生成万能downcast |
| Uninit或Ref/primitive混合 | 不属于普通Phi profile | 交§9的受限构造规则或拒绝Java |

共同superclass沿完整已知super链求交；缺hierarchy不猜subtype。所有Java reference都
可安全upcast Object是语言事实，但不能由此声称Object可传给任意String/array formal。
类型选择次序是语义政策，不是为稳定源码排序；不能用“第一个到达的producer”选类型。

每个UseRequirement随后检查`interpret(source type, value) == 原word解释`并生成唯一
已证转换。Phi edge只允许值保持且no-throw的view/widening；ordinary use允许§4.9的
ProvenNarrow，其对所有可能值均不截断。真正改变值的byte/char转换必须对应原conversion/
store语义；array/field的narrow store不作为Phi修复手段。
检查Unknown即Unsupported/TypeEvidenceMissing；确定输入类型违规则须独立规则证据，
不能把所有类型表示冲突都报Malformed。没有通用CSP回溯或隐式boxing求解器。

### 4.6 三个易错例子的推导

- D013：boolean参数可保持boolean local，DEX int位运算的operand用`a?1:0`取得word。
  xor/and的Known01闭包使return处`v!=0`安全。也可在已证boolean结果形态发非短路位运算，
  但不能把原boolean与数字1直接拼到Java运算符两边；不要求某一种打印文本。
- D014：literal 0在ref use发null，在boolean use发false，在int use发0；这三次解释不
  把一个真实Object value变成int。literal经过非literal合流后，仍受Phi单一storage规则。
- D015：cmp结果是int(-1/0/1)，原FP inputs只读已物化local；NaN bias是producer语义，
  不能因后继看起来是`<`便把它当类型简化删除。±0相等且不走NaN分支。

### 4.7 终止与检查器职责

family阶段只做有限合并/交集，facts阶段只做有限union；source type只在两者封口后选。
因此不存在同一变量一会儿变Object、一会儿变String再重跑的振荡。该算法有意不完备：
某些真实等价Java存在，但超出本profile仍失败，不能称“所有合法DEX已表达”。
检查器逐raw operand核producer来源、所选family、Phi incoming转换和original check-cast
site；primitive模板还需公开行为证据。类型自洽不证明把sub写成add也正确。

### 4.8 literal编码：从bits构造，不靠漂亮字符串猜值

I32/I64可统一用8/16位hex（I64加L），例如0x80000000和0x8000000000000000L，避免
把正的2147483648误拼成非法int token。boolean仅0/1，Reference literal仅0；窄literal
view须保持原word解释，不能把原int 255在普通call里任意改成byte -1。原narrow store/
conversion允许的截断要留在对应Op。

有限FP可直接从IEEE bits发精确Java hex literal，无运行时库调用：

| 类型 | normal | subnormal |
|---|---|---|
| float | `[-]0x1.<(frac<<1):06hex>p(E-127)f` | `[-]0x0.<(frac<<1):06hex>p-126f` |
| double | `[-]0x1.<frac:13hex>p(E-1023)d` | `[-]0x0.<frac:13hex>p-1022d` |

frac为原fraction整数，符号独立，exponent按十进制有符号整数发出；zero/Infinity/NaN
走§4.2的单独分支。float末尾补的1个0 bit只是hex对齐，不做rounding；最小float
subnormal为0x0.000002p-126f。也可使用已证round-trip spelling，但不能host求值后
丢掉负零/NaN策略。

String按UTF16单元逐个escape，不正规化NFC/NFKC，不把surrogate pair合成后再计长度。
quote/backslash/CR/LF用语言专用escape，原backslash成对输出以避免意外Unicode escape；
Java与smali不是同一个escape函数。Java classfile的CONSTANT_Utf8还有65535-byte界限：
每u16单元MUTF8长度为0→2、1..127→1、128..2047→2、其余→3。Java源码中的六字节
`\uXXXX`与这个长度不是一回事；超界不能靠拼几个编译期常量规避，javac会折叠。
不为绕限引入StringBuilder、分块helper或改变intern identity；超界为
Unsupported/JavaLiteralTooLarge，按§8选择raw。

### 4.9 窄整数经过Phi后的无损use转换

Java carrier为int不代表值域就是全部I32。对Integral32附加以下producer边界，不从测试
actual值、debug名字或“调用形参想要byte”反推范围：

| producer | 固定范围 |
|---|---|
| boolean参数/字段/返回/定义 | [0,1] |
| byte参数/aget-byte/iget-byte/返回/int-to-byte | [-128,127] |
| short对应producer/int-to-short | [-32768,32767] |
| char对应producer/int-to-char | [0,65535] |
| 普通int参数/字段/返回及整数算术 | [-2147483648,2147483647] |
| Integral literal view | 原bits按signed I32解释的单点 |

int-to-byte等**建立新producer**，不能把它前面的int范围继续当成结果；反过来，普通
add/sub即使operands都是byte也重新给完整I32范围，不推断x+0、x-x等代数结论。
move复制边界，Phi取incoming闭区间的hull；Bottom是尚无事实，不是[0,0]。下界只下降、
上界只上升，端点均来自固定producer边界/literal，故为有限单调闭包。每次变更/传播
计work；不添加通用interval/widening求解器。

facts封口后，已有Known01证据可将检查时范围与[0,1]相交，不回写/重启前面的闭包。
对于目标B/S/C，只有最终范围非空且完全包含于相应目标范围，才生成ProvenNarrow：

```text
prove_narrow(v, target):
  require family(v)==Integral32 and source carrier is int-like
  R = finalized producer range; if Known01(v): R = R intersect [0,1]
  require R != Bottom and R subset range(target)
  return explicit (byte)/(short)/(char) cast + range evidence
```

该primitive cast不抛异常、不分配，且对R内每个值都是identity；不是用截断修复错误值。
它只用于return/call等ordinary use的source静态类型匹配，不改变Phi carrier或在edge上
任意窄化。boolean仍使用Known01规则，不存在合法Java的`(boolean)int`。

例：`byte id(byte x){return x;}`的EntrySeed为byte，入口Phi可存int，但范围仍为
[-128,127]，故`return (byte)phiLocal;`保持值。byte+short的Phi不能无条件转byte；byte
参与加法后也不能借旧origin转byte。未知/越界范围拒绝该Java use，不添加运行时if或
改原descriptor。D012的原始有损转换仍在原Op执行，不受本规则取代。

## 5. Local、Phi copies与异常shadow（T08/T17）

### 5.1 正常边parallel copies

Parameter绑定原参数；其他已初始化、非pure-literal retained Value与Phi各有独立Local，
不coalesce。§9.3的uninit aliases使用DeferredAllocation，不分配假null local。
对每个Phi incoming建立`CopySet[IncomingEdge]`，EntrySeed copy放方法入口（constructor
受§9.2的prologue替代/normal完成后物化规则约束），其他copy只
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
constructor prologue用§9.2已证formal/literal替代关系重放，不能把尚未物化的Phi local
假称已赋值；super/this normal完成并执行deferred entry copies后，恢复上述普通body不变量。

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

### 5.3 parallel copy为什么按destination选叶子

设入场store为lambda0，集合`d_i <- s_i`的规范是所有source从lambda0读取后同时写。
destination唯一（同一个Phi只在本edge赋一次），source可fan-out，亦可literal。若某d
不再是任何pending source，写d不会破坏剩余读；故可以先发该copy。条件反过来则错：
`a<-b,b<-c`应先a=b，先b=c会使a丢掉旧b。

若无这样的d，剩余有cycle。先`t=s`保存旧值、将**全部**对s的pending读改为t，之后
对原s的覆盖不丢旧值，且这个cycle被打断。例如：

```text
入场 a=A,b=B,c=C
并行 a<-b,b<-c,c<-a
顺序 t=b; b=c; c=a; a=t
结束 a=B,b=C,c=A
```

自环Identity可删，但不同转换不能仅因LocalId相同就删。simple实现可O(k²)，每次扫描/
替换元素均计work，不是只在外层循环charge一次；temps不超过cycle打断次数，先计容量。
不必为了论文的最少copy目标引入coalescing或全局调度器。

### 5.4 shadow与DA初值不可观察的推导

对每个可能进入handler h的site s，其所需shadow赋值必须在s紧前且全是no-throw local/
literal copy。若s抛出，则该次snapshot已全部完成，catch ingress读到的就是sigma_before_s。
如果s正常返回，snapshot之后被下一site重写也无害：catch只消费实际抛出那次snapshot。
不同handler独占slot避免inner cleanup污染outer所需值。

Java DA可能仍要求catch外shadow有0/null初值。可加的理由不是“正常情况下会覆盖”，
而是**每一条被接受的exception edge**都有完整snapshot，且snapshot本身不抛异常；
任何counterexample缺一条即不能Verified。throw terminator与nested handler内throw
也必须纳入，不能只枚举带result的call。heap并未由snapshot回滚。

EntrySeed copy、正常edge copy和before-throw snapshot三处具有不同placement，不合成
一个“块尾统一赋值”。同target的两条边也不能只按target合并；选择原EdgeId后才执行。

## 6. 普通结构与dispatcher（T09/T13/T14）

### 6.1 必须可执行的dispatcher

eligibility为无handler、无monitor、无未处理uninit，且P5/P6封口。所有locals在method
scope声明，pc初值为entry block；每个reachable block一case，执行原ops及terminator：

```text
Jump(e):             copies(e); pc=target(e); continue
Branch(c,t,f):       if(c){copies(t);pc=target(t);}else{copies(f);pc=target(f);} continue
Switch(v,keys,d):    对已物化v用整数if/else链选同一EdgeId；copies(edge); pc=target(edge); continue
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
- IntegerSwitch：selector一次；令n为key数，R=0（n=0）或checked i64(max-min)+1。
  只有R<=4*n+16才发Java switch；更稀疏的图用dispatcher内if/else链，避免目标table膨胀。
  同target且同incoming copy可合case labels。fallthrough
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

### 6.4 dominator/postdominator的数学规格

normal可达子图中：`Dom(entry)={entry}`，其他`Dom(b)={b} union intersection Dom(pred)`。
用全可达集合初始化其他节点、反复intersection得到只减的有限参考规格；实现沿§6.2
用RPO/idom树避免常驻B×B矩阵。intersect沿RPO编号较后的节点向其idom走，每次祖先
访问计费；不能只限制outer rounds而让一次祖先链走到无界。

postdominator只在能到达所选normal Exit的反向子图求；真实return/throw或当前region
的已定义follow可连接分析用Exit。**永不终止的SCC不加虚假的运行时exit**，其ipdom
可以没有值，不能据此造break/return。无出口自然loop可while(true)，不能证明的结构
仍用符合eligibility的dispatcher。exception catches不混到normal postdom图里。

### 6.5 结构归约的局部执行关系

Sequence连接点恰消费一次原edge transfer；Diamond的head ops与condition只执行一次，
且两臂不各自再计算condition；Loop把header保留在每轮体内，continue先做latch copy；
Switch的selector已物化，case key/default选原EdgeId。归约减少RegionArena节点，不
修改原CFG；同一effect节点不得拥有两个source owner来“共享尾部”。

每个schema需要的入口/follow/出口映射不全便Declined，不靠printer猜。候选空间预估
包括同时活着的旧plan和新nodes；work/内存不够时不能先创建巨大候选。普通图可选择
已有dispatcher表示，受保护图不能借此跳过catch/monitor约束。逻辑BUG仍红，不叫
“另一种不支持”。字面相似、span覆盖与路径执行关系是不同检查。

## 7. 简单raw输出

有code的方法必须为ValidatedRawMethod；无code的abstract/native必须为ValidatedRawDeclaration，
后者只输出完整flags/proto/annotations及.method/.end method，不伪造.registers或body。
standard smali printer按原位置输出mnemonic、寄存器、ref、label、payload与完整metadata。无法可靠识别格式则Unavailable+有限excerpt。
Java失败只走这一个出口，不重试其他引擎/更宽profile。assembler/独立projection是开发
验收，不是生产依赖；细节归TASKS §4，不在核心算法里扩测试工具框架。

### 7.1 raw闭包与labels

raw可输出性不是“opcode有名字”。在生成字符串前，核该方法所有实际使用的opcode、
ref、handler、payload、literal和必要metadata在所选standard-smali语法中有完整表示。
缺任一项即Unavailable+有限excerpt，不能漏annotation/catch后仍叫完整Smali。

code labels按已验证原boundary关联；branch/catch/try/end/debug各保留原逻辑落点，
payload label与实际instruction label分开。switch payload的target offset相对**引用它
的switch pc**，不能相对payload pc。若一个payload被多个switch引用，要逐ref核上下文；
不能任取一个owner打印。无法用单一standard表示忠实表达的共享形式为UnsupportedRawSyntax，
不私自复制payload、删ref或放宽固定projection。

原instruction form/mnemonic和operands按原序发出，寄存器使用绝对v编号可避免参数p别名
误算。原无效width不可用.word兜底。可合法重新编码的offset/string-pool index不当byte
相等要求，但instruction/ref/target/payload/handler/UTF16/metadata必须经独立projection
比对；不能靠assembler接受证明这些内容没丢。

### 7.2 一次出口，不是恢复系统

normal失败先停止消费其图并drop不再需要的scratch，再只读已验证raw，在请求剩余
资源及显式raw配额（若有）内完整打印。Malformed不走可信raw；Cancelled直接结束；BUG即使附raw也保留BUG状态。
raw失败不重试normal、不换engine、不扩大配置。生产只打印，不spawn assembler；
D061的回环是在开发工具中，且声明不完整造成Java拒绝不应破坏原raw成员身份。

## 8. 表示选择的确定规则

每个方法只运行一条分析链，按以下条件选择结果：

1. 取消立即停止request，不逐method继续raw。
2. 已证明Malformed则Unavailable，不输出可信Smali。abstract/native只有声明和目标表示
   检查均通过才产DeclarationOnly，否则返回完整Smali声明或Unavailable。
3. 全部Java检查通过则Java；失败方法不打印其未验证的半段AST，但不丢弃其他已完成方法。
4. Unsupported/BudgetExceeded只进入一次raw路径。若normal尚未完成raw校验，raw路径用
   可用资源及显式raw配额从原RawMethodView完成校验，再打印。已确认Malformed不能靠重读撤销。
5. raw校验/语法/预算不能满足则Unavailable。raw新发现确定损坏时保留Malformed事实；
   InternalInvariant即使附raw也保留BUG。不得重新运行normal或更换engine/限额。

调用者显式设置normal work为0而raw仍可用时可得到完整Smali；显式raw配额也为0则Unavailable。
None不是0，默认不限配额不得被当作预算耗尽。
这是失败结果到只读raw出口的一次分支，不是恢复调度器。

### 8.1 结果展示与Java资格分开

P8判定语言资格，P9只读已有结果，应用不因class为Mixed而拒绝调用renderer：
1. JavaUnit用完整Java printer；单方法Java/DeclarationOnly打印相应声明/body，不添索引前缀。
2. Mixed先标原class身份与“非完整Java编译单元”，按真实DeclarationOutput保留shell/
   字段。成员按原MethodKey关联逐段展示：Java保留已验证声明/body；Generated只引用
   已证owner语法及原key映射，不伪造独立方法；Smali保留完整raw及原因；Unavailable
   保留身份、原因和已有excerpt。不可解释的shell同样标记，不据此删掉可用方法。
3. language/原因标记与source段分别计UTF8bytes，OriginSpan重定位到实际本次文本；
   不把raw塞进可意外闭合的Java注释/假body，不用stub补足class。untrusted名字/消息
   使用既定display escaping，字符串/payload内容仍按各语言完整保留。无输出排序要求。
4. text/json共用相同Report，不重跑分析或触发第二条fallback。完整渲染在原配额内进行，
   写出前核payload；允许Mixed不等于允许截断。单个成员Unsupported不成为整类展示禁令。
5. 结果完整性/语言资格与错误分类分开：完整Unsupported结果可正常展示；Malformed、
   BudgetExceeded、InternalInvariant等若已有可渲染Report也不清空成果，但保留失败状态；
   Cancelled不合成Report继续raw，render/I/O失败不冒充完整交付。

## 9. 调用绑定、对象与构造器（T11/T12）

### 9.1 CallEvidence生成与discharge

P4保存原owner/name/proto/invoke kind、word operands、完整/未知声明集合、access/throws/
class-init事实。每个owner/name组包含继承/hiding候选，未提供完整候选不得标Complete。
P4只生成P5约束和BindingObligation，不要求提前得到P5 source types。

P8先按§12.1固定class请求的MemberEmission。下列步骤用于实际打印的普通Java call；
已由完整Generated模板证据消费的原call由该规则验证，不再假装为独立源码call做重载
选择。普通call候选是source-visible声明投影，不把已证明的compiler bridge当成另一个
手写return-only重载；原runtime目录仍完整保留，用于原invoke/dispatch身份核对。

P8对每个普通Java call按以下固定顺序处理：
1. 依P5把receiver与实参固定为locals/literal views，保持原receiver/参数求值顺序。
2. 令每实参的source static type与目标formal descriptor精确一致；只允许Identity、
   已证明upcast、原check-cast或已验证primitive转换。`(T)null`安全；不任意增加downcast。
3. 枚举完整且accessible的source-visible候选，先按继承/override/hiding事实折叠被
   覆盖的parent声明，再按固定formal types核fixed-arity overload；不能把正常covariant
   override误当两个手写重载。varargs按原array参数，不拆包/装箱。要求选中erased
   member/dispatch与原invoke一致；剩余有效声明若仅return不同且无合法override/Generated
   解释，或仍有不明泛型绑定，则Unsupported。
4. static用可证明相同解析/初始化目标的type qualifier；virtual/interface保留receiver；
   direct仅当前合法private/constructor形式；super用`super.m`且核相同super解析目标。
   interface-special、跨owner访问或source shell无法表达时不改成virtual蒙混。
5. 对static final primitive/String读，另核`JavaConstantKind=Constant|NonConstant|Unknown`。
   内部unit采用§12的blank-final/static-block方案，标NonConstant；外部若是Constant，
   Java源码读可能被内联而丢class-init/原读取，不能直接lower sget，返回
   Unsupported/JavaConstantFieldRead。Unknown为Unsupported/BindingEvidenceMissing；
   不通过反射/helper额外触发初始化。
6. 累积escaping checked exceptions，减去按序catch可捕获项；剩余throws必须与原override/
   声明contract相容。未知throws/父契约时拒绝，不sneaky throw、catch包装或扩大public契约。

无call的方法产空但有效BindingTable；不是必须构造全平台catalog。const/primitive运算
不依赖虚构库事实。只绑定原程序已有call，不为primitive或bit reinterpret新增intrinsic。

### 9.2 allocation/init

new-instance定义`Uninit(AllocationId)`，仅pure move-object追踪初始化前的别名，但
init前不能作为普通Ref、传给任意调用或进入一般Phi。配对必须满足同block唯一new→init，
中间只允许无throw/effect的alias moves；参数已经在new之前物化或为pure literal。
生成一个`new T(args)`，消费allocation/init两个site，初始化成功后所有该allocation
的alias视为同一Ref；异常路径保留各原site保护域。保护域不同且无法准确映射则拒绝。

真正constructor第一条Java statement必须为合法`super(...)`/`this(...)`。只允许把前导
参数move/数值literal拼进这个调用，不在它前面输出local声明、field写或helper调用。
随后才发普通locals/body。固定入口Phi若唯一incoming为EntrySeed，prologue可沿该
pass-through Phi及纯参数move追溯原formal/literal，直接用于super/this实参；UninitThis
同样仅经这条受限入口链作receiver。不得开放多incoming/回边Phi或全局合并ValueId。
涉及的local声明、EntrySeed copies与prefix move物化只在调用normal完成后执行，且须
证明调用前无其他use/handler能观察它们；失败路径不物化已初始化this。这样既不在
super前读未赋Java local，也不靠给uninit赋null凑编译。继承、access、ctor overload与
checked throws仍由同一绑定规则核。T22对outer capture另有专用规则，不开放一般
pre-super重排。

### 9.3 uninit是use位置的状态，不是全局改一个ValueType

AllocationId标静态allocation site；初始化成功与否随控制位置变化。不能在发现一个
init后把该ValueId全局改成Ref，使init失败的catch也凭空拥有已完成对象。

同block配对按原序检查：new到唯一init之间只许alias moves，receiver是该A，
所有普通heap/call/return/throw use都不允许；init的normal完成后方可普通使用。
跨block use须由这个normal完成支配，exception transfer保留Uninit状态；无法证明则
不支持当前Java profile。P4把每个use的availability交P5，不给SSA补新物理写入。

P6使用`LocalBinding::DeferredAllocation(A)`：pre-init aliases没有可读Java local，也
不赋default null。P8的一个ConstructorExpr消费new/init两个site，成功时物化obj local；
之后这些aliases映射到同一obj，原reg后来被覆盖则是另一个ValueId。这里是构造器
合法化的专门storage规则，不是开放一般local coalescing。

它保持allocation→init→后用顺序，拒绝`new; effectfulArg(); init`被打印成先算arg再
new。真正支持这种更宽形态须证明Java new的allocation/argument/init内部顺序；
不造carrier/helper或默认重排。构造器内UninitThis到super/this同样只在normal完成后
视作Ref，前导local声明不能抢到Java constructor invocation之前。

### 9.4 Java绑定的可执行判定

对目标formal向量F，把actual的source static types固定为F后再判断候选，而不是仅看
打印出来像哪个overload。对完整候选m，fixed-arity适用条件是arity相同且每个F_i可用
本profile允许的identity/widening转换到m.formal_i；判定不执行boxing、泛型推断或varargs
展开；不能把影响选择的这些形态当“不存在的竞争候选”。候选包含这些可能影响选择的形态则拒绝Java绑定。适用者中只接受唯一most-specific且原owner/proto/dispatch均匹配的目标。

D019的null literal须具有Object formal静态类型，否则pick(String)会胜出；D021的super
不是同receiver的普通virtual。varargs调用按array实参fixed-arity，声明本身若需要保留
varargs flag仍须合法`T...`形式，不能把调用不展开等同于随意改声明。

完整性是owner/name/继承/access范围的声明，不是“池里有该method_id”。missing parent、
隐藏候选、return-only collision都不能默认成功。静态qualifier、field与type spelling也
要在实际source scope查找：全限定文本也可能受同名field/type遮蔽，不能只拼descriptor。
source name无法忠实绑定则拒绝相关Java，原key/名字不改。无需引入完整全平台catalog。

### 9.5 checked throws和声明证明的边界

P4收集的是声明/绑定义务，不提前按catch参数Throwable扩大方法throws。P8在词法catch
确定后分离原操作的声明checked集合、按优先级已捕获部分、handler自身新异常及原异常
rethrow；§10的precise-rethrow规则只对effectively-final原catch参数使用。constructor/
initializer/override各自有不同允许throws，不能统一加`throws Throwable`。

Unknown事实不等于没有checked exception，Complete也不证明外部callee在运行时绝不
用sneaky/native方式throw别的对象；它首先是Java编译绑定所用声明合同。语义仍保留原
call/异常传播，不靠声明throws删真实exception边。原catch优先级也不按throws集合重排。

## 10. try/catch与保护域（T16–T18）

### 10.1 输入与匹配条件

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

P7先匹配canonical monitor/release-handler组，再将其作为带ownership的whole node交
normal/try归约；不能先用一般try profile拒绝尚未识别的monitor cleanup。嵌套scope按
lock stack逐层核，body内自然loop/sequence照原图分析，不复制effect或修改catch链。
拒绝锁scope/identity/freshness证明时为Unsupported/MonitorScopeUnproved，不补锁或
默认为格式损坏；其他已确认Malformed事实仍不可撤销。

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
显式max_nesting（若有）约束；默认不设深度数值上限，仍不用宿主递归或无界路径枚举验证。受保护exit的异常PC先按§2规范取值。

匹配一个synchronized region须同时成立：
- 单入口在enter成功之后，锁expression已经求值一次；所有normal离开经同锁exit；
- body抛异常经原catch-all释放同锁后rethrow**原异常**，没有额外cleanup effect；
- 各return值在release前已求好，嵌套/重入按stack层次分别匹配，每条路径恰释放一层；
- 证据能排除接受profile内的null exit/锁错配；不把enter失败路径当已经持锁；
- 所有body op及原try边界都保留在同scope，不能输出空synchronized再在外执行调用。

匹配后输出`synchronized(lockLocal){body}`，消费原enter/exit/纯释放handler。方法级
modifier只在原有声明同步标记且匹配**整个方法**的this/static Class锁、无前后effect时
使用，避免把普通block擅自改成可反射的同步方法；managed方法的
DECLARED_SYNCHRONIZED标记本身不授权新增锁。native无body仅按其真实执行flag发声明。
raw flags仍独立保留。无法证平衡或有额外释放逻辑则该Java profile不支持。

## 12. class、初始化与编译器生成成员（T20–T23）

### 12.1 unit装配顺序

先确定owner/package/kind/hierarchy和成员声明，以原ClassKey/MethodKey关联结果，不靠
source name或到达次序。class请求在普通方法lowering之前，先从P1已验证raw及完整声明
事实按§12.3的直接模板检查确定MemberEmission；模板核参数/receiver来源、必要cast、
精确target/field/order及无额外effect，不依赖普通Java overload推断或丢掉冲突后补证。
只有完整证据才选Generated；不匹配则保留raw/Explicit及Mixed原因，不能静默删除成员。
该表只保存owner/原key/模板证据，不持有整类SSA，也不引入推测后重跑方法的调度。

随后验证source-visible shell并逐个分析Explicit方法；shell源码资格失败只记录class
原因，原raw仍可定位时继续各方法分析，不能因某个Unsupported提前跳过余下成员。
只有取消/全局损坏/请求资源等硬终止按请求规则停止，不能把未处理prefix冒充完整结果。
最后做unit级final/constructor/
override/Generated实际生成布局检查。Generated原有code由模板规则消费、预算与目标
admission同样执行；不要求其body先成为可独立Java方法。单方法请求没有owner生成
语法时仍按§12.3返回该原方法Smali。任何缺证据的shell/成员使class为Mixed，独立显式
方法仍可Java。非法Java原始成员名不全局重命名；仅synthetic locals可安全命名。

标识符按Java8规则验证：首字符为字母类别Lu/Ll/Lt/Lm/Lo/Nl、Sc或Pc，后续可再含Nd/
Mn/Mc；拒绝identifier-ignorable控制/Cf字符。关键字/true/false/null不作identifier，
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

### 12.4 Java目标：直接检查与真实编译分开

Java目标为Java8/classfile 52，开发编译合同为Temurin21.0.8+9的
`javac --release 8 -proc:none -g:none`。生产不调用javac，也不预测编译器的代码大小。
P8只对原语义、源码规则、声明事实以及能直接确定的格式项目作检查。

| 直接可确定的项目 | 规则 |
|---|---|
| 方法参数words | implicit this（非static为1）+sum(formals的J/D为2、其余含array为1) <=255 |
| 实际descriptor/name/String/annotation等MUTF8单项 | 编码字节数<=65535，不按source escape长度 |
| 已确定的每class field/method数量 | 包含已证compiler-owned成员，分别<=65535 |

原binary proto是参数检查的truth：inner hidden outer参数、enum隐藏name/ordinal也计入；
return type不计入。静态255个int与实例254个int加this可通过，静态256或实例255加this
必须拒绝；abstract/native虽无Code也不能省略声明检查。禁止删unused参数、改名、拆
方法/class或造helper绕过真实限制。直接确认超限时返回Unsupported/JavaTargetLimit，
带实际项目/计数/界限，不误报Malformed或分析BudgetExceeded。

最终classfile仍须满足真实JVMS限制：有Code时`0 < code_length < 65536`；
max_locals/max_stack、exception rows及各u2数量字段按格式核界；constant_pool_count
包括index0及long/double双slot且<=65535。这些是实际产物限制，不是AST或ValueId配额。
V4真实编译并独立读取classfile检查；生产没有实际classfile时，不宣称已证明其大小，
也不以未知编译大小作Unsupported。Verified只标识已完成上述原生检查，不是编译成功证书。
required Java的真实javac失败仍是验收失败，不能用“未预测大小”把失败算通过。

### 12.5 源码规模不作估算拒绝门槛

不按每AST节点固定字节费、方法固定开销、DEX code units、source local数量或catch/
monitor数量估算JVM code/locals/stack/handler/pool并拒绝Java；不设JavaTargetCostUnknown。
不建立第二套JVM backend或估算器，也不把未知费用转换成raw理由。

三地址operands与打印宏仍按实际source语义展开，cmp-with-bias、BoolToInt、ProvenNarrow、
FillExpansion的每项操作/类型/求值顺序均须验证。语法只含已有明确lowering规则的表达式、
赋值、if/loop/integer switch、try/catch和synchronized；缺少语义规则的string concat、
boxing、lambda、foreach、try-with-resources或finally不会因此变成已支持能力。
实际容器/文本增长只检查平台容量和调用者显式配额（若有），默认不加规模阈值。
D077的长调用序列必须完整恢复并通过真实编译，不靠删call、造loop或helper缩小方法。

### 12.6 compiler-owned成员与声明完整性

Generated规则必须提供实际binary成员清单、签名、隐藏参数及行为证据，不能只给owner名。
bridge、enum values/valueOf/backing storage、inner outer-capture按§12.3逐项匹配；
不为这些成员附加猜测的code或pool费用。共享type/descriptor/name不能按重复mention
乘固定系数变成class拒绝理由。D078的2200个abstract接口方法必须保留为完整JavaUnit，
不造constructor/body，实际pool由独立classfile读取检查。

encoded初值导致source产生语言初始化成员时，将其纳入layout并核实际初始化顺序，
不当作新public helper。compiler-owned方法/字段/隐藏参数与语义模板不符，或source会
生成未被证明的成员，仍不能构造JavaUnit；这属于成员语义/完整性缺证，不是大小估算。
普通class无原constructor时须处理javac自动default constructor问题，不能伪造一个原
不存在的异常构造器。规则失败保留Mixed/raw及原key，不能静默丢成员。
V4将实际生成成员与原manifest/规则比对，行为另由Driver核验；不信builder填true。

## 13. 控制与资源的具体接线

open/index、方法分析及render共用一个`&mut RunContext`，Limits只归该context。
默认所有数值配额为None，既不加固定阈值，也不预留固定raw work/内存。context不存IR、
不spawn、不全局共享；每个循环pop、instruction、集合增长/复制和长字符串分块都检查
取消。取消立即结束，不转raw。分析收敛由各算法的有限状态/单调度量保证，不靠固定步数
掩盖不能终止的算法。

只有调用者显式Some(n)的项目才核额度，累计used不退款。normal与raw工作子配额独立，
共同计入request；method派生分配合计包含normal/raw。仅显式有限的fallback work/
分配值从父配额claim，normal不能用held；未配置时raw只用剩余资源，不领取无限预留。
未消费held可归还，已用额度不回退，不建立重试绕限路径。

所有分配先核平台可表示大小、输入最小记录尺寸及适用显式配额，再try_reserve；
真实分配失败明确报Resource，不改成格式损坏。Env、metadata、UTF16、AST、spans及
文本都遵守此规则。不可信深度的遍历及释放使用显式栈/arena，不依赖宿主递归或递归Drop；
max_nesting只有显式设置才作数值门槛。
产物最后消费后drop，Report只留必要owned facts，不挂整套SSA/Flow历史图。

normal/raw/report输出只核各自显式设置的长度项；最终JSON也不设默认硬cap，见
DESIGN §7/§10.3。按实际转义后的UTF8增量检查，不能先造巨String后才声称预算前置。
同一成员拼report不重复扣成员长度，真实work/分配仍累计；未配置的上限不偷偷补默认值。
性能记录来自授权release运行，不以账本替代RSS或实际耗时。

## 14. 应用输入、扫描与提取

以下采用确定的native方案，不搬旧session/MCP，不搭通用文件框架。安全文件访问使用
cap-std的Dir/普通文件safe interface，跨平台文件身份用same-file，私有临时文件用tempfile；
压缩沿用已审阅的bounded native inflate方式，CRC用crc32fast。依赖由Cargo锁定。Diana代码不使用unsafe，文件、解压与CRC调用safe interface。

### 14.1 InputLease与目录

1. 先固定允许root及Dir handle，按capability-relative规则打开显式输入；禁止设备/FIFO，
   Unix safe OpenOptionsExt加O_NONBLOCK/O_NOCTTY，避免FIFO/终端打开阻塞或改进程终端；
   Windows拒绝device/pipe namespace。得到handle后再次核regular file、len与文件身份。
2. 小输入复制进owned bytes，大APK复制到私有snapshot file；按入场文件长度分块读取，
   检查取消及显式input配额（若有）。提前EOF或入场长度之外的字节，以及前后原handle
   身份/len/mtime变化，均报INPUT_CHANGED并丢弃snapshot；不限配额也不追逐持续增长文件。这里只承诺复制后immutable，不声称stat
   检查能证明原文件任意并发写时的原子版本；分析只读取snapshot，不再读原path。
3. 大APK只读中央目录和当前需要的entry范围；被materialize的entry各有独立owned buffer，
   stored借该buffer、deflate借解压buffer，均由lease持有。无unsafe mmap，不复制整APK
   到每个DexImage，不宣称这条safe copy路径是零拷贝。
4. EOCD从尾部最多65535+22字节搜索，comment长度须对应文件尾；single-disk，目录
   count/bytes先核实际目录范围/最小记录尺寸及显式上限（若有）。遇classic sentinel，核ZIP64 locator/EOCD记录的签名、长度、
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
5. worker数量按DESIGN §7确定，使用有界byte/row队列与weighted admission；按完成发布，
   无global sort/按ordinal等待。consumer关闭即停止producer；途中失败显式失败，不把
   已发prefix当完整成功。没有分页、preview或成功截断。

### 14.3 原entry提取与发布

1. 精确选EntryKey，先核local/central头、flags/method一致及data范围。结构有效后先拒绝
   任何加密entry（EncryptedEntry，不接受密码/解密路径），再拒绝非0/8压缩方式
   （UnsupportedCompression，不把格式有效的不支持方式报Malformed）。两项均在解码、
   明文size/CRC校验及创建输出前判定；加密stored的compressed_size含加密header，不能
   提前套未加密的等长规则。未加密method0才要求compressed/uncompressed长度一致；
   method8在声明长度及显式限额（若有）内bounded inflate，核实际长度/结束状态并遍历CRC32对照
   central CRC。原payload不经UTF8/JSON、DEX normalize或header改写。
2. output只用显式-o，不拼entry name。一次打开输出parent Dir并持有；拒绝目录/符号链接
   目标与输入同handle/hardlink身份。所有临时/最终操作相对该Dir，不重新解析可变parent路径。
3. 在该Dir以create_new生成mode0600临时文件，碰撞重试有固定上限；写入/flush并完成
   CRC/size/预算检查才publish。写/复制按至多64KiB一块检查公开请求取消与预算，
   不以一个无检查的大write/copy绕过协作取消。错误/取消只清自己的temp，不删用户目标。
4. no-force以同Dir **hard_link(temp, final)** 原子发布，存在任何final项就失败，再删除temp。
   两发布者最多一个成功；不采用exists后rename。文件系统不支持该primitive时明确失败，
   不降为有覆盖竞态的实现；D070平台证据未具备仍not-run。
5. force在入场时只允许普通、非输入alias目标，然后相对持有Dir做原子rename替换目录项，
   不跟随最终symlink去写别处。它不是“只替换此前inode”的CAS接口，不承诺外部进程不能
   在检查后改名；固定输入path本身禁止作输出，源handle字节不得被写。不能满足这些safe
   primitive的平台不做弱化实现。原子可见性不冒称断电持久性。
6. 输出文件成功后stdout无binary/状态行；query/get的文本/JSON与本binary输出adapter分开。
   BrokenPipe或I/O错误停止请求，文件发布前错误目标不变、无半文件。默认get按§8.1
   生成完整有界的Java/Mixed/Smali/Unavailable展示后写出，不以JavaUnit资格挡住Mixed，
   也不把有意标记的局部失败结果等同于writer截断。

领域错误保留可区分原因：AmbiguousEntry、TargetExists、UnsafeTarget、CrcMismatch、
BudgetExceeded、Cancelled、UnsupportedCompression、EncryptedEntry及I/O。case中的snake_case是这些
公开原因的语义投影，不靠stderr猜阶段；不以任意error替代指定CRC/预算/alias拒绝。

H02/H10核心文件安全测试继续走公开领域interface与既定自制目录；上述依赖、路径包装
或命令解析不另建机械/辅助测试。原始权限/来源、跨平台not-run及release成本如实记录。

## 15. 单use整数表达式内联

inline_pure为false时直接返回已验证JavaBody；为true时执行一次有界扫描：

1. 只匹配同一原block内相邻的`t = expr; return t;`，t为普通OpResult local且仅此一次use。
   不匹配Phi、shadow、parameter、DeferredAllocation或其他control/保护域。
2. expr仅含int/long literal、local read、整数一元运算、显式整数转换、add/sub/mul、
   bitwise与shift；不含div/rem、FP、heap、ref cast、call、allocation或同步。
3. 仅对显式配置的有限配额核body副本、替换nodes、重新验证及baseline完整emit的资源；
   配额不足则Declined，默认不设估算拒绝门槛。复制一个JavaBody，按原AST顺序一次扫描、替换互不重叠的pair；不运行repeat，
   不在原body原地试改，不构建插件/选项registry。
4. 将expr树原样移入return，保留source type与两个site的Origin；允许保留无use的pure
   local declaration。重新验证type/binding/control/effects与完整emit配额。
5. 通过则Applied；普通不匹配/受限预算则Declined；Cancelled停止；内部不变量失败为BUG，
   不包装Declined。验收比较同一固定行为的on/off结果，不比较文本或local数量。

expr树次序不变，两条语句之间无其他执行，故operand读相同lambda、整数bits与原return
值不变。排除FP避免store消除涉及不同浮点执行环境；排除throwing/effectful表达式避免
新增求值/保护域义务。内联不改变CFG/SSA/CopyPlan。
