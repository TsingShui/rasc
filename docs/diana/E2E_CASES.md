# E2E案例设计：先冻结行为，再实现Diana

**状态：70个案例设计稿（draft-1），不是已生成DEX或已通过E2E。** 机器可读清单：[e2e_cases.json](e2e_cases.json)。准备/冻结过程：[TEST_PREPARATION.md](TEST_PREPARATION.md)。团队来源/工具：[REPRODUCIBILITY.md](REPRODUCIBILITY.md)。

每个case的recipe描述将来的自制输入，包含伪代码，不是可直接编译文件。B01先审阅规范、B02生成fixture、B03独立认证输入与原Driver、B04证明比较器会拒绝错结果；**B04通过前不开始T01/H01引擎或产品改造**。不CLI黑盒，不私有phase/callcount，不变量名/排版golden。

## 1. 固定测试合同

- core-java必须得到实际Java并通过指定行为Driver；未实现不能改为Smali成功或从required集合排除。默认MCP/CLIschema不参与测试。
- input-safety必须按明确Malformed/Unavailable/Resource/Cancelled语义拒绝，不能any-error或panic算过。
- raw-integrity必须保留原instruction/ref/target/payload/handler/string_units，assemblerexit0不够；未知width不能fake标准smali。
- resource/extract/identity/domain测试跨公开module库interface，抓复杂内容/安全，不测参数/列名/简单路由。
- 独立方法/report顺序不保证；**程序event顺序仍严格**。原始ID/实际span关联准确，变量名/源码形态可不同。
- 所有依赖源、expected、recipe和hash签入；工具锁可公开获取，不依赖作者home/AndroidSDK/兄弟仓库/cache。
- 原始基准源码/规范workedexample与回归expected先独立核验；Diana/dexdec/radguard输出不作oracle，expected不自动refresh。

## 2. 70个案例摘要

详尽inputRecipe、invocations、独立expected、mutation-killer、oracle与dependency在JSON；下表提供阅读索引。示例索引/源码均自制，不含真实corpus标识。

| ID | Suite / 语义族 | 案例 | 独立预期摘要 |
|---|---|---|---|
| D001 | core-java / control | 分支合流不覆盖未选路径 | `[{"return":10},{"return":20}]` |
| D002 | core-java / control | 并行拷贝交换循环 | `[{"return":1020},{"return":2010},{"return":1020}]` |
| D003 | core-java / control | 三个循环携带值同步旋转 | `[{"return":231},{"return":312},{"return":123}]` |
| D004 | core-java / control | continue之前更新和carry值 | `[{"return":0},{"return":9}]` |
| D005 | core-java / control | 嵌套循环具名退出 | `[{"return":6}]` |
| D006 | core-java / control | 条件含副作用只执行实际分支 | `[{"return":false,"events":["A"]},{"return":true,"events":["A","B"]}]` |
| D007 | core-java / control | switch共享case与fallthrough | `[{"return":12},{"return":12},{"return":2},{"return":7}]` |
| D008 | core-java / control | 循环多出口保留最后值 | `[{"return":0},{"return":12},{"return":33}]` |
| D009 | core-java / numeric | 整型溢出保持位宽 | `[{"return":-2147483648},{"return":2147483647}]` |
| D010 | core-java / numeric | 整型除零异常与MIN除负一 | `[{"return":-2147483648},{"exception":"java.lang.ArithmeticException"}]` |
| D011 | core-java / numeric | 有符号和无符号移位mask | `[{"return":-1},{"return":1},{"returnLong":"1"}]` |
| D012 | core-java / numeric | 窄化byte/char保留符号 | `[{"return":-1},{"return":65535}]` |
| D013 | core-java / numeric | boolean位运算不是int表达式 | `[{"return":false},{"return":true},{"return":false},{"return":false}]` |
| D014 | core-java / numeric | 零值在ref/boolean/int上下文不混用 | `[{"return":true},{"return":false},{"return":false},{"return":0}]` |
| D015 | core-java / numeric | cmpl/cmpg NaN bias | `[{"lt":false,"le":false,"gt":false,"ge":false},{"lt":false,"le":false,"gt":false,"ge":false}]` |
| D016 | core-java / numeric | 负零与浮点除零 | `[{"returnFloatBits":"80000000"},{"returnFloatBits":"7f800000"}]` |
| D017 | core-java / numeric | FP转int的NaN/无穷和截断 | `[{"return":0},{"return":2147483647},{"return":-2147483648},{"return":3},{"return":-3}]` |
| D018 | core-java / calls | argument从左到右且receiver先求值 | `[{"return":3,"events":["R","A1","A2","SUM"]}]` |
| D019 | core-java / calls | 静态重载按descriptor选择 | `[{"returnStringUnits":[79],"events":["pick:Object"]}]` |
| D020 | core-java / calls | 虚分派保留override | `[{"return":2}]` |
| D021 | core-java / calls | invoke-super不变成virtual | `[{"return":2,"events":["C","B"]}]` |
| D022 | core-java / calls | unused调用仍抛异常和执行effect | `[{"exception":"fixtures.ExpectedError","events":["B"],"heap":{"counter":1}}]` |
| D023 | core-java / calls | interface分派保留实现 | `[{"return":9},{"exception":"java.lang.NullPointerException"}]` |
| D024 | core-java / heap | 数组越界保留效果prefix | `[{"exception":"java.lang.ArrayIndexOutOfBoundsException","events":["IDX"]},{"exception":"java.lang.NullPointerException","events":["IDX"]}]` |
| D025 | core-java / heap | unused check-cast仍执行检查 | `[{"returnVoid":true},{"exception":"java.lang.ClassCastException"},{"returnVoid":true}]` |
| D026 | core-java / heap | array-store运行类型检查 | `[{"exception":"java.lang.ArrayStoreException"},{"returnVoid":true,"heap":{"firstString":"ok"}},{"exception":"java.lang.NullPointerException"}]` |
| D027 | core-java / heap | 数组literal元素位模式 | `[{"arrayInt":[-1,0,2147483647]},{"arrayFloatBits":["80000000","3f800000"]}]` |
| D028 | core-java / heap | 字段读取和写入顺序 | `[{"return":4,"heap":{"n":7}},{"exception":"java.lang.NullPointerException"}]` |
| D029 | core-java / exceptions | 赋值call失败catch观察旧值 | `[{"return":7,"events":["CALL","OLD:7"]}]` |
| D030 | core-java / exceptions | 两个throw site选择对应shadow旧值 | `[{"return":7,"events":["A","OLD:7"]},{"return":9,"events":["B","OLD:9"]}]` |
| D031 | core-java / exceptions | catch子类型优先而非类型排序 | `[{"return":1},{"return":2},{"return":0}]` |
| D032 | core-java / exceptions | try外cleanup异常不可被旧handler捕获 | `[{"exception":"fixtures.ExpectedError","events":["OK","CLEANUP"]}]` |
| D033 | core-java / exceptions | finally在return前执行但返回值先取 | `[{"return":3,"events":["FINAL:8"]}]` |
| D034 | core-java / exceptions | finally抛新异常覆盖return | `[{"exception":"fixtures.FinalError","events":["FINAL"]}]` |
| D035 | core-java / exceptions | catch中抛出被外层捕获 | `[{"return":4,"events":["A","B","OUTER"]}]` |
| D036 | core-java / monitor | monitor作用域内调用确实持锁 | `[{"return":true},{"exception":"java.lang.NullPointerException"}]` |
| D037 | core-java / monitor | 异常退出确实释放monitor | `[{"exception":"fixtures.ExpectedError","events":["RAISE"],"monitorAfter":false}]` |
| D038 | core-java / monitor | 嵌套同锁重入与退出 | `[{"return":true,"events":["IN"],"monitorAfter":false}]` |
| D039 | core-java / monitor | 锁表达式只求值一次 | `[{"returnVoid":true,"events":["LOCK","BODY"],"heap":{"lockCalls":1}}]` |
| D040 | core-java / monitor | 声明synchronized的实例与静态锁 | `[{"return":true},{"return":true}]` |
| D041 | core-java / construction | 构造器super先于body | `[{"events":["BASE","BODY"]}]` |
| D042 | core-java / construction | this构造链不重复字段初始化 | `[{"heap":{"n":4},"events":["INT:4","ZERO"]}]` |
| D043 | core-java / construction | 邻接new/init的参数effect只一次 | `[{"heap":{"boxN":5},"events":["ARG","CTOR:5"]}]` |
| D044 | core-java / initialization | static初始化触发与单次执行 | `[{"returns":[7,7],"events":["INIT"]}]` |
| D045 | core-java / initialization | blank final分别构造赋值一次 | `[{"heap":{"x":1}},{"heap":{"x":2}}]` |
| D046 | core-java / initialization | encoded null/false与空串区分 | `[{"heap":{"s":null,"b":false,"i":0}}]` |
| D047 | core-java / initialization | 静态初始化失败保持异常类型 | `[{"exception":"java.lang.ExceptionInInitializerError","events":["INIT"]}]` |
| D048 | core-java / declarations | 协变bridge的调用行为 | `[{"returnStringUnits":[99,104,105,108,100]}]` |
| D049 | core-java / declarations | inner capture保持outer this身份 | `[{"return":7}]` |
| D050 | core-java / declarations | enum名字序数与values副本 | `[{"enumNames":["A","B"],"enumOrdinals":[0,1],"valuesFresh":true}]` |
| D051 | core-java / declarations | runtime annotation值没有静默丢失 | `[{"annotation":{"valueUnits":[120],"nums":[1,2]}}]` |
| D052 | core-java / strings | NUL/孤立surrogate/反斜线u保真 | `[{"returnStringUnits":[0,55296,65,56320,92,117,48,48,48,97]}]` |
| D053 | core-java / strings | 空串与实际换行/制表符 | `[{"returnStringUnits":[]},{"returnStringUnits":[65,10,9,34,92,66]}]` |
| D054 | core-java / strings | 非BMP由UTF16两个单元组成 | `[{"returnStringUnits":[55357,56898]}]` |
| D055 | input-safety / input | 超大count先校验不分配 | `[{"errorKind":"malformed","noJava":true,"noTrustedSmali":true}]` |
| D056 | input-safety / input | branch到指令中间拒绝 | `[{"errorKind":"malformed","noJava":true,"noTrustedSmali":true}]` |
| D057 | input-safety / input | 实际read-before-def不是默认0 | `[{"errorKind":"malformed","noJava":true,"noTrustedSmali":true}]` |
| D058 | input-safety / input | 宽寄存器被部分覆盖后不能读取 | `[{"errorKind":"malformed","noJava":true,"noTrustedSmali":true}]` |
| D059 | raw-integrity / raw | 未知opcode宽度不可猜NOP | `[{"outcome":"unavailable","java":false,"standardSmali":false,"rawExcerpt":true}]` |
| D060 | raw-integrity / raw | 合法return-only重载保留raw而非改public名字 | `[{"outcome":"mixed","rawMethodsPreserved":2,"noFakeJavaUnit":true}]` |
| D061 | raw-integrity / raw | 标准smali回环完整覆盖branch/payload/handler | `[{"outcome":"smali","allInstructionSites":true,"payloadBitsExact":true,"handlerPriorityExact":true,"stringUnitsExact":true,"memberRefsExact":true}]` |
| D062 | resource-safety / resources | 预算耗尽有独立raw预算 | `[{"outcome":"smali","reason":"budget_exceeded","rawExact":true},{"outcome":"unavailable","reason":"budget_exceeded"}]` |
| D063 | resource-safety / resources | 取消停止请求不转fallback | `[{"errorKind":"cancelled","noFallbackReport":true}]` |
| D064 | resource-safety / resources | 大量转义value保持实际输出预算 | `[{"resourceFailureExplicit":true,"noTruncatedSuccess":true}]` |
| D065 | identity-semantics / identity | 041相同数字index不同logicalmember不能串定义 | `[{"return":11},{"return":22}]` |
| D066 | domain-integrity / identity | 定义方法与外部method-id引用分开 | `[{"definedMethods":["Own.f"],"mustNotInclude":["External.g"]}]` |
| D067 | domain-integrity / identity | 同方法两个引用site不能合并 | `[{"siteCount":2,"referencingMethod":"Case.target","allCertifiedSites":true}]` |
| D068 | extract-safety / extract | stored/deflate原payload逐字相等 | `[{"fileBytes":[0,255,65,0,128,10]},{"fileBytes":[0,255,65,0,128,10]}]` |
| D069 | extract-safety / extract | CRC坏与资源失败目标不变 | `[{"explicitError":true,"targetBytes":[75,69,69,80],"tempFilesRemaining":0},{"explicitError":true,"targetBytes":[75,69,69,80],"tempFilesRemaining":0}]` |
| D070 | extract-safety / extract | no-clobber与输入alias/ZipSlip安全 | `[{"explicitRejection":true,"originalFilesUnchanged":true},{"outputOnlyExplicitPath":true},{"exactlyOnePublish":true,"otherNoClobberError":true,"noPartialFile":true}]` |

## 3. 与原F/Q族的关系

F00–F25是历史能力族索引，不是70个最终case；D001–D070是固定case身份。执行case只用D编号，F/Q映射用于定位设计，不动态决定哪些case可以被忽略。Q01/Q07仍人工CLI审阅，不属于E2E。

| 原能力族 | 固定D案例 |
|---|---|
| Phi / branch / loops / switch（F06–F08/F13–F14） | D001–D008 |
| primitive / FP / null（F05/F09） | D009–D017 |
| call / member binding（F10–F12） | D018–D023 |
| arrays / fields / casts（F15） | D024–D028 |
| throw-state / catch / cleanup（F16–F18） | D029–D035 |
| monitors / synchronized（F19） | D036–D040 |
| construction / initialization（F12/F20） | D041–D047 |
| bridge / capture / enum / annotation（F21–F23） | D048–D051 |
| UTF16 / literals（F03） | D052–D054 |
| malformed input（F01/F25） | D055–D058 |
| unknown/raw/Java-unrepresentable（F02/F25） | D059–D061 |
| budget / cancel / output（F00/F25/Q06） | D062–D064 |
| 041 / definitions / xrefsites（Q04/Q09） | D065–D067 |
| original extract / CRC / race / alias（Q10） | D068–D070 |

## 4. 实现里程碑的固定案例组

以下组在B01审阅时固定，解决原F族过宽导致首批要跑未来功能的问题；T/H卡的B(F)/S(F)是历史定位，实际运行用本表D编号，不在开发时重新选expected或发明新case。分组只是当前里程碑运行选择，不删全suite required成员。

| 实现任务 | 该里程碑公开interface的实际D案例 | 说明 |
|---|---|---|
| T01/T02/T03/T04/T05 | D055/D056/D059输入/raw、D061 raw认证及D052–D054输入值认证；D062/D063/D064在真实interface接线后 | P1/Rust读取/rawadapter测试不声称这些core-java字符串已恢复成功；B01–B04已认证全部输入，T05只接库 |
| T06 | D009 | 首个真实scalar Java方法为int加法/溢出；旧answer7例子只是实现解释，不是要开发时新造fixture |
| T07/T08 | D001；D002/D003的固定输入/平行copy义务在T09公开Java回环验收 | T07单独没有全部Java结构可用时保持整体requiredcase未通过，不改expected |
| T09 | D002/D003/D005/D008 | 普通无call循环/多出口，dispatcher证据；D004的bit操作到T10 |
| T10 | D004、D010–D017、D052–D054 | primitive/FP/null/stringliteral完整值，不涉及新增目标库call；Driver可使用JDK作为观察者 |
| T11 | D006、D018/D019/D022/D023、D061 | caller effects/receiver/args/descriptor/原始raw；D020/D021完整class在后续class能力接线 |
| T12 | D043 | 目标method使用已知自制Box依赖；target参数已先准备，邻接allocation/init |
| T13/T14 | D001–D005/D008（T13），D007（T14） | natural与dispatcher按相同case行为比较；不新增私有structure golden |
| T15 | D024–D028 | heap/arrays/casts的真实异常与输出值，输入是否保留unusedcast在B03已认证 |
| T16/T17/T18 | D029–D035 | T16/T17接口准备不代表Java行为已通过；T18才跑公开恢复+JVM行为，不把旧状态反例提前改成raw成功 |
| T19 | D036–D040 | 同scope/声明monitor语义、holdsLock/异常释放；JVM-only证据显式 |
| T20 | D020/D021、D041/D042/D044–D047 | full-unit class/constructor/this/super/static/final支持接线后验收 |
| T21/T22/T23 | D048/D060（T21）、D049（T22）、D050/D051（T23） | bridge/不可Java声明raw、inner捕获、enum/annotation，full-unit与method范围明确 |
| H04/H09/H10/H06 | D065（H04）、D066/D067（H09）、D068–D070（H10）、D062–D064（H06） | 公开领域/资源interface，不启动CLI测试参数/表头 |
| T25/H08 | 所有D001–D070按各suite固定期望 | 核心54Java + 16输入/raw/资源/身份/提取case；Q01/Q07人工CLI清单不纳入E2E |

B04固定core suite成员，首批实现可显式运行上述小组D编号，未实现的全core cases仍列未通过分母。任务phase只作为开发归属，不写“phase尚未支持就自动把这个requiredcase skip”。T05只接薄public库adapter，不再决定input/expected；T06可以先让一组scalar/control转绿，然后按T07–T23扩大。所有R8/JDK/assembler工具与fixturehash必须在B02/B03准备并固定；当前未生成，不宣称已完成。

Optional美化测试另外按[RECOVERY_EXTENSIONS.md](RECOVERY_EXTENSIONS.md)添加；core语义case在美化off/on均有效，不为了更美源码删counterexample。原程序JVM与ART证据不同，core source-origin只标JVM-only；人工smali无独立执行oracle时只能通过raw/invariantlane，不能谎称Java行为pass。

## 5. 必须先处理的规范/输入风险

1. D015必须确认生成DEX有两种cmp bias义务；不能只看Java source文字。
2. D025 unusedcheck-cast可能被前端编译优化，需B03raw验证或独立手写同义务raw lane，不换成更简单cast已使用的case掩盖。
3. D040/D041/D044–D051是class-context/full-unit，不能methodsplice宣称全unit。
4. D060人工return-onlyoverload是raw/Java不可表达合同，不假装source-originJava可编译。
5. D061是复杂raw/handler回环，不用scalar interpreter忽略异常来作behaviororacle。
6. D065的041builder要公开格式和独立projection检验，禁止个人SDK/真实APK作为隐藏前提。
7. D070平台symlink/hardlink/atomicno-clobber能力差异要记录；缺环境不算平台通过。
8. B01可能发现设计稿本身错误，**必须先于引擎实现修正规范**；冻结后修正只接受独立证据/单独review，不由引擎actual刷新。

文档设计完成与spec-approved/inputs-certified/harness-verified是不同状态。本轮没有实现准备器或生成70份binaryfixture，准备阶段完成后才允许进入生产引擎开发。
