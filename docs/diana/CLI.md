# Diana CLI 接口契约

状态：**目标接口，尚未改动当前程序**。用户指定的命令名、说明与列是本页的唯一公开 CLI 清单；PROJECT/TASKS/网页据此更新。Diana 是完整产品，内部反编译库为 `diana-decomp`。MCP 延期，不在本页定义协议；不提供旧 `getclass` / `findrefs` 别名、`skill` 命令或分页/数量截取参数。

## 1. 顶层帮助与命令表

目标帮助的命令部分：

```text
Commands:
  get-class   Fetch one class as Java
  entries     List APK/ZIP entries (columns: name, compressed_size, uncompressed_size, compression)
  extract     Extract original entry bytes to -o
  strings     List DEX string literals (columns: at, value, entry)
  xref        Search code references across every DEX (columns: entry, context, match)
  manifest    Extract decoded AndroidManifest.xml
  classes     List classes across every DEX (columns: at)
  get-method  Fetch one method as Java
  fields      List fields across every DEX (columns: at)
  methods     List defined methods (columns: at)
  help        Print this message or the help of the given subcommand(s)
```

下面表格用于网页生成；usage 是参数形式，不代表实现已经存在。

| Command | Summary | Columns | Usage |
|---|---|---|---|
| get-class | Fetch one class as Java | — | diana get-class <input> <class> |
| entries | List APK/ZIP entries | name, compressed_size, uncompressed_size, compression | diana entries <input> |
| extract | Extract original entry bytes to -o | — | diana extract <input> <entry-name> -o <file> |
| strings | List DEX string literals | at, value, entry | diana strings <input> |
| xref | Search code references across every DEX | entry, context, match | diana xref <input> <kind> <target> |
| manifest | Extract decoded AndroidManifest.xml | — | diana manifest <input> |
| classes | List classes across every DEX | at | diana classes <input> |
| get-method | Fetch one method as Java | — | diana get-method <input> <method> |
| fields | List fields across every DEX | at | diana fields <input> |
| methods | List defined methods | at | diana methods <input> |
| help | Print command help | — | diana help [command] |

核心形式统一为 **`diana <command> <input> [selector] [options]`**。`help`、`--help`、`--version` 不要求 input。input 是一个 APK、含 root DEX 的 ZIP 或裸 DEX；entry 命令与 manifest 的更窄输入范围见下文。不自动调用 d8，不直接处理普通 JVM `.class` / JAR 字节码。

## 2. 通用输出与选项

- 默认表格为 **UTF-8 TSV**，首行是指定列名，恰按上表列顺序；不额外加序号、索引表、颜色或调试列。记录顺序不保证，按完成/到达输出。
- `--format text|json`，默认 text；表格 JSON 为对象 `{ "items": [...] }`，字段与列名相同。Java/manifest 的 JSON 是明确 domain report，见 §5/§7；不是 MCP envelope，不设计协议工具。
- `-o, --output <file>`：文本/JSON命令可指定文件，成功时文件与 stdout 的同次 payload 字节相同；`extract` 是例外，只向文件写二进制，不向 stdout 输出副本。
- `--filter <literal>`：仅适用于 `classes` / `fields` / `methods` / `strings`；大小写不敏感的字面子串，作用于完整 `at` 表达中语义名称/描述符（不匹配 opaque e/l/c/m/f/s 索引前缀），strings 则作用于 value。不是正则，不截取记录。
- `--class <class>`：fields/methods 的定义类选择；xref 的引用方类选择。采用 §3 精确 class selector，不默认模糊匹配，不代替 xref target。
- `--jobs <N>`：对独立 DEX 扫描的并发资源配置，N 必须大于0；不承诺稳定输出，也不要求 engine 每个方法另起线程池。无相关并行工作的命令不暴露无效选项。
- `--debug`：诊断/trace 去 stderr；不污染 TSV、JSON、Java、XML 或提取文件。没有“按钮改变但实现未消费”的选项。
- **没有 `--offset` / `--limit` / cursor / preview / stable-output**。成功查询返回全部匹配项。安全 input/work/output 预算保留：超预算错误，不以只返回前N条冒充成功。内部 batch/backpressure 不是分页。

TSV 的任意字符串字段按显示转义：`\\`、`\t`、`\r`、`\n`、`\0`；孤立 UTF-16 surrogate 用 `\uXXXX`，不得改成 replacement character 或让一条记录占多行。列中包含 delimiter 的值不靠有损切割。JSON serializer 同样保留必要的 Unicode escape；控制/孤立 surrogate的具体表示须 F03/Q09 验收。`at` 自身可逐字回传 CLI，包含空格或 `$` 等时需 shell 引号。

流式 text 查询可以在失败前已写部分记录，必须非零退出并在stderr明确错误；这种部分输出不是完整成功。JSON报告、Java获取、manifest和extract文件按完整产物构建并原子提交，失败不留假完整文件。大表格JSON受完整结果预算；用户可改用流式text与外部shell过滤，不通过新增分页解决。

## 3. `at`：原始身份定位符，不是结果行号

列表必须具备可回传的精确定位，不依赖结果顺序。**索引均为原输入**，不是排序后的行号、合成 LocalId 或 Java 名称。

### 3.1 规范格式

```text
class-at  = e<physical-entry>:l<logical-member>:c<class-def>::<class-descriptor>
method-at = e<physical-entry>:l<logical-member>:c<class-def>:m<method-id>::<owner>-><name><proto>
field-at  = e<physical-entry>:l<logical-member>:c<class-def>:f<field-id>::<owner>-><name>:<type>
string-at = e<physical-entry>:l<logical-member>:s<string-id>
```

数字为非负十进制，`e`是物理 ZIP central-directory record index，`l`是该entry内部 logical DEX序号（普通DEX为0），`c`为class_def index，`m/f/s`为对应DEX原id表index。裸DEX视为单entry `e0:l0`。规范class descriptor如`Lcom/example/App;`；method proto必须包含参数与返回type，如`(ILjava/lang/String;)Z`。

例子（示例索引，不是真实 corpus）：

```text
e0:l0:c7::Lcom/example/App;
e0:l0:c7:m23::Lcom/example/App;->check(I)Z
e0:l0:c7:f9::Lcom/example/App;->count:I
e0:l0:s15
```

规范格式中的 descriptor/name/proto 与索引双向核对：修改输入后原定位符可能失效，返回错误，不能索引指向别的对象时忽略名称检查。不同输入/DEX相同名字不合并。所谓“不重不漏”指每个原身份记录一次，不是按文本name去重。

### 3.2 可读短选择器

- get-class / `--class`接受 class-at、`Lowner;` 或 dotted `com.example.App`；点仅按明确class名称归一化，不做模糊搜索。
- get-method接受 method-at 或完整 `Lowner;->name(args)return`；可读owner也可dotted，但proto中的描述符不能随便点斜杠替换。**没有裸方法名或单独 `--method` / `--proto` 组合**，一个完整selector已表达身份。
- xref field/method target接受原始完整引用 descriptor；也可接受规范field-at/method-at，但先校验定位并解析为reference，再跨全部DEX查引用。目标选择definition不自动限制引用方entry。
- 短名称若有多定义，返回 `AMBIGUOUS_SELECTION`（exit4）并提供规范at定位信息；不按最先完成的worker任意挑一个。exact at可以区分同DEX同descriptor的重复class-def。
- 查询空集合正常 exit0；唯一对象获取/提取缺失exit3。非法at或proto语法是用法错误exit2；范围失效/名称索引不一致是输入/selector错误exit1，不发错对象。

## 4. 列表命令

### entries

`diana entries <apk-or-zip> [--format text|json] [-o file]`。

列恰为 `name, compressed_size, uncompressed_size, compression`，sizes是非负字节数，compression用数值ZIP method（例如0 stored、8 deflate），不是随语言变化的标签。不解压所有entries来列目录；不按name去重或排序。所有physicalrecords都列出，未知压缩方法也可作为metadata列出，不意味着extract支持。

指定列不含额外entry_index。重复name的精确提取通过 `extract --entry-index`，详情与歧义诊断可带index；不擅自在entries默认列里加第五列。裸DEX不当ZIP目录，明确输入类型错误。

### classes / fields / methods

```sh
diana classes sample.apk --filter 'example'
diana fields sample.apk --class 'com.example.App'
diana methods sample.apk --class 'com.example.App' --filter 'check'
```

每命令只有 `at` 一列。classes枚举class_defs定义；fields枚举各class_data中实际定义的static/instance fields；methods枚举direct/virtual **defined methods**，含abstract/native/constructor/clinit声明，不把所有method_ids里的外部reference当defined method。

使用原始DEXname/proto/index，包括synthetic/bridge flag涉及的原方法（flag如需查看属于JSON后续明确扩展，不擅自增列）；不为只列符号运行Diana SSA/Java后端。duplicate定义各有独立at，CLI只列其原identity，非法结构仍错误。

### strings

`diana strings <input> [--filter literal] [--format text|json] [-o file]`。

列为 `at, value, entry`。这里默认采用原查询的**string_ids值枚举**（既包括代码literal，也包括名称/描述符/注解值），不声称仅扫描`const-string`出现的字符串。代码实际使用位置用`xref ... string ...`查询，两者不混淆。保留完整UTF-16值并按§2转义。

entry列是物理entry原name；041 logical/member身份在at里，不拼造另一个真实ZIPname。裸DEX的entry使用caller提供输入的basename作为展示标签；不用于身份判断。

## 5. get-class / get-method：默认必须是 Java

```sh
diana get-class sample.apk 'com.example.App' -o App.java
diana get-class sample.apk 'e0:l0:c7::Lcom/example/App;'
diana get-method sample.apk 'Lcom/example/App;->check(I)Z'
diana get-method sample.dex 'e0:l0:c7:m23::Lcom/example/App;->check(I)Z'
```

- 默认text：get-class只在完整`VerifiedJavaUnit`时成功输出Java；get-method输出完整方法声明/body，abstract/native输出合法DeclarationOnly。方法源码需要owner上下文，不声称它本身是一整个编译单元。
- 默认Java不可恢复时exit1，stdout不发smali、Mixed或半个Java；stderr报告分类原因/原identity。失败不要写 `.java` 文件，也不通过抛异常stub补齐。
- **引擎仍保留逐方法raw fallback**，只是CLI不把它当“Fetch as Java”的默认成功。`--format json`显式获取完整domain report，允许`java_unit/mixed`以及方法`java/declaration_only/smali/unavailable`，每项原identity与diagnostics/spans明确。完整Mixed报告本身可exit0；requesterror、BUG、Resource、Cancel仍错误，不因有raw片段变成功。
- JSON保持`schema_version:1`作为**本地CLIreport格式**（不是MCPversion），含command/selection/status/source/methods/diagnostics/spans；不serialize内部AST/arena。source_range/spans相对于对应source字段UTF8bytes，字段scope明确。top-levelreport与method-onlyprojection在接口规格/源码审阅中明确，不为这种基础包装另建测试；复杂语义/raw/identity验证走公开库interface。
- 取消此前 `getclass --members` 与 `--require-java` 提议：fields/methods是独立符号清单，get-默认就要求Java。不要同时输出索引表前缀污染source/span坐标。没有旧命令alias或自动转旧enginefallback。

Java编译成功不等于语义等价；完整classpath和语义fixture仍按VERIFICATION验收。JSON显式选择并不允许吞掉malformed/内部错误、假smali或语义不忠实Java。

## 6. xref

```sh
diana xref sample.apk string 'Authorization'
diana xref sample.apk type 'Ljava/lang/String;'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z'
diana xref sample.apk field 'Lcom/example/App;->count:I'
diana xref sample.apk method 'Lcom/example/App;->check(I)Z' --class 'com.example.Caller'
```

kind明确为`string|type|method|field`；target是字面精确值/descriptor（string为完整UTF16字符串相等，非正则；type接受class dotted归一化或标准primitive/arraydescriptor）。首版不把名相同的所有overload混为methodreference。以后substring/fuzzy搜索必须单独命名/验收，不从当前裸value猜。

默认跨输入的每个rootDEX和041logicalmember扫描代码引用，使用原始opcode/refindex；不先反编译源码再搜索。`--class`只约束引用方定义，可读短selector歧义规则同§3。

列恰为 `entry, context, match`：

- entry：引用方物理entry名，显示标签不决定identity。
- context：`<referencing-method-at>@cu:<offset>`，offset是该方法原始code units，显示为十六进制如`@cu:0x0012`；不是输出行号。
- match：实际被引用的字符串值、TypeRef、MethodRef或FieldRef，按§2转义；不得只回显用户target但忽略真实引用。

**每个实际引用site一行**；相同方法多个site各列，不能像旧findrefs方法级结果那样无声合并。查询相同site可能重复引用参数时按已定义opcodeoperand-site身份表达，不以名称去重。这是有意精化，H09/Q09验收。空结果header/emptyitems exit0；成功全部匹配，不分页、不保证顺序。

## 7. extract / manifest

### extract：原始 entry payload，不是重建 DEX

```sh
diana extract sample.apk 'classes.dex' -o original.dex
diana extract sample.apk 'classes.dex' --entry-index 4 -o original.dex
```

`extract`只处理physicalAPK/ZIPentry。原始字节指**解压后的完整entry payload**：method0直接原payload、method8原deflate解压；不是ZIPlocalheader或压缩流，不改DEXheader/checksum、不切041logicalview、不做Java/smali转换。文件内容须与原entry解压bytes逐字相等。

`<entry-name>`精确匹配，重复name未指定`--entry-index`则exit4并列出候选physicalindex；有index时须核name与index一致。只提取一个entry，`-o`必需且是一个文件路径，不提供批量目录解包。这样ZIP中的`../`等name不成为输出路径，避免ZipSlip。

默认不覆盖已存在输出；`--force`显式允许替换普通文件，拒绝输出目录/符号链接/输入文件自身或hardlink别名。先创建目标目录内private临时文件，有界解压/完整性校验后原子提交；取消/解压错误/预算不足清理临时文件，不输出半成品。no-force发布不得存在check-then-rename覆盖竞态；force也要按平台语义验收。不向stdout打印二进制或状态行，错误/`--debug`仅stderr。

compression不支持、加密、坏range/size/CRC必须明确错误，不能列目录成功就假定提取可靠。原ZIPCRC的存在/验证能力需H10核查现有codec，不先假设已有完整实现；预算受原input/inflate限额，不是分页。

### manifest

`diana manifest <apk-or-zip> [-o file] [--format text|json]`。

默认输出完整decoded `AndroidManifest.xml`；原binaryAXML bytes用extract获得。沿用原项目最后一个同名physicalentry的选择规则（依据目录顺序而非worker完成顺序），manifest选择不会因无序显示改变。缺失exit3，坏AXML/超预算exit1；裸DEX或普通JAR没有manifest则输入错误。

text为XML；json为`{ "xml": "..." }`，不附假Java状态。默认无自动外部工具；调用patchedvendoredAXML前检查其PATCHES及安全范围，修改须独立测试。

## 8. 帮助、错误和退出码

- `diana help`、`diana --help`、`diana help get-method` / `diana get-method --help`成功exit0。
- exit0：完整查询/提取/manifest、符合默认Java要求的get、显式JSON完整报告（含标记清楚的Mixed）。
- exit1：I/O、Malformed、UnsupportedJava默认get、预算、内部BUG、取消、失效选择器等请求失败。
- exit2：未知命令/flag、非法selector/proto、缺input、extract缺`-o`、不适用选项组合。
- exit3：唯一对象/entry/manifest查无结果。列表/xref空匹配是正常exit0。
- exit4：唯一选择有歧义，如重复class/method/entry名，拒绝任意取第一项。

BrokenPipe按原Unixfilter行为正常结束，不打印panic。项目名称统一Diana；不提供`getclass`、`findrefs`、`skill`、`--members`、`--require-java`、`--class-id`、`--offset`、`--limit`等旧提议接口；canonical at已解决精确identity。

`mcp`延期，不因本清单直接删除当前实现或设计新协议；旧agent skill安装逻辑也不自动执行迁移/删除用户目录。agent使用说明作为普通文档/发布资产，不添加第十二个公开`skill`命令。

## 9. 实施与验证落点

- H01冻结本页命令/flag/selector/error规则，update futureCLIargs/README，但当前生产源码未改。
- H03复用快查询并迁移指定TSV/JSONcolumns；H04 canonical at与041原始identity；H05两个get命令与默认Java/显式JSON规则。
- H09补fields/methods **definitions** 和xref **sites**；H10完成原entryextract/no-overwrite/CRC/atomicfile安全；H07统一最终help与agent说明。
- 按用户约定不新增CLI黑盒测试。Q01/Q07是基础接口人工审阅清单，不是测试族；Q05/引擎F测试走公开反编译库语义回环，Q02–Q06/Q09/Q10仅验证复杂身份、内容、安全与资源义务。具体Agent卡见[PROJECT_TASKS.md](PROJECT_TASKS.md)。

命令/flags/help/列名/默认包装/旧alias移除通过源码与本规范核对和构建确认，不为它们写CLI黑盒或golden。自动化测试只投入反编译行为、raw/smali完整性、复杂跨DEX/041身份、xrefsite/UTF16值、原entry提取CRC/竞态/alias/预算安全，跨公开库/module接口而非CLI子进程。不要求排序或跨运行同文本，不设计MCP；本次不删除现有测试。
