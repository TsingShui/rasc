# 团队协作与可复现输入：不依赖作者机器

本页是所有Diana实施任务的共同合同。参考源码、测试输入、工具链与结果必须能由**干净checkout**获得，不依赖个人home、其他本地仓库、历史worktree、未签入cache、机器预装SDK或真实私有语料。本次记录公开入口与版本核查，不声称准备脚本/二进制fixture已经实现。

## 1. 四种路径分开

| 类型 | 必须使用的形式 | 禁止 |
|---|---|---|
| 源码与fixture | 仓库相对路径、公开URL+commit | 个人绝对路径、兄弟仓库目录、未签入文件当必需输入 |
| 固定工具 | artifact坐标/version、公开下载源、锁文件SHA-256 | 自动找作者SDK目录、latest、只写“装一个d8”、未锁依赖的fat jar |
| 生成/cache | 当前checkout的`.cache/e2e/`与`.cache/references/` | 把预存cache当已有前提；无hash/version验证就读cache |
| 临时运行产物 | harness调用系统temporary-directory API分配，日志按case归档到`.cache/e2e/runs/` | 写死某OS的临时目录、共享固定临时名、实际路径进golden |

`.cache/`是本仓明确的不提交生成目录，不是私有机器依赖；**任何必需内容都必须能重建**。输入source、expectation、manifest、recipe与hash记录签入；不得用“我机器上有”解决缺文件。运行command工作目录由脚本按自身/checkout定位，不要求当前shell恰好在某目录。

## 2. 参考源码可获取、固定版本

来源见[SOURCE_MAP.md](SOURCE_MAP.md)，每条锚点格式为`项目ID + commit + 相对文件 + 符号`。仅审阅需要，可显式在当前checkout准备：

```sh
# Unix shell示例；Windows使用同语义Git命令，路径仍在checkout内。
# 只在目标不存在时clone；已有工作目录不得覆盖、reset或自动stash。
mkdir -p .cache/references

git clone --no-checkout https://github.com/TsingShui/rasc.git .cache/references/dx
git -C .cache/references/dx checkout --detach ef8a4c92c6b89e8afed9ccbc1552feae67b6982d

git clone --no-checkout https://github.com/Noelo-Lab/kuna.git .cache/references/ku
git -C .cache/references/ku checkout --detach 29c44b57f8c1d9b35044aa4ee3f636c193288009

git clone --no-checkout https://github.com/MG1937/radguard.git .cache/references/rg
git -C .cache/references/rg checkout --detach 290de20c925c4a2c7642b1a63c9997185a06e130
```

每次核`git rev-parse HEAD`、工作区无source修改。未来自动准备器遇已有修改必须报错，不擅自清理；网络clone失败不改用latest/其他fork。也可在离线环境由团队批准的镜像或源码包提供相同commit与hash，URL覆盖不能覆盖内容锁。

E2E不运行任何参考引擎、不读取其fixtures/pickle，也不需要准备上述源码。radguard固定版本许可不明确，不因此copy代码或私有fixtures；自己实现与自制fixture是默认路线。

## 3. 开发工具链候选锁（B02生成可执行锁，不留到T05）

候选组合先于引擎实现验证，用于自制fixture生成/基准执行。生产二进制不依赖Java/Maven/Python。

| 工具 | 固定版本与公开来源 | 使用与验证 |
|---|---|---|
| Rust | 根manifest最低`1.93`；runner实现时固定patch版 | 普通native构建；锁文件记录`rustc -Vv`，不手工改现有工程toolchain |
| Python | harness准备器最低3.11（若采用Python） | 只作开发工具；不得进入生产runtime，脚本应跨平台 |
| JDK | Eclipse Temurin **21.0.8+9**，源码目标`--release 8` | [官方release](https://github.com/adoptium/temurin21-binaries/releases/tag/jdk-21.0.8%2B9)，选择对应OS/arch的JDK资产，SHA-256用资产官方记录核；不是只下载JRE |
| R8/D8 | `com.android.tools:r8:8.3.37` | [Google Maven POM](https://dl.google.com/dl/android/maven2/com/android/tools/r8/8.3.37/r8-8.3.37.pom)，主类`com.android.tools.r8.D8`；从固定JAR构造参数数组，不依赖AndroidSDK包装脚本 |
| smali / dexlib2 | `org.smali:smali:2.5.2`、`org.smali:dexlib2:2.5.2` | [Maven Central POM](https://repo.maven.apache.org/maven2/org/smali/smali/2.5.2/smali-2.5.2.pom)，主类`org.jf.smali.Main`；POM有transitive依赖，**不是假设任意smali.jar可`java -jar`执行** |
| 测试依赖classpath | 自制Java源码或JDK既定声明 | 全部source签入，classpath从recipe得到；核心suite不依赖Android platform jar |
| ART增强验证 | 暂不规定设备/镜像 | 与JVM-only报告分开；没有已授权可复现环境不将ART算通过，不成为核心prep隐含本地SDK依赖 |

这些公开入口本轮作存在性核查；实际组合能否完整生成全部fixture须B02/B03验证，不能仅POM有HTTP200就标工具可用。B02产出`tests/e2e/toolchain.lock.json`：每个必需artifact的完整URL、version、SHA-256、size、依赖及mainclass；JDK每目标平台需asset/hash。**任何未填checksum/占位值、混入未锁transitive版本均阻止prep通过**。校验锁由官方checksum/可信获取的实测digest审阅；不能伪造SHA。

不要将R8/smali的JVM依赖搬到Rust生产或build.rs。可用受控Maven步骤产生classpath，但必须锁住传递依赖与artifacthash后离线运行；锁不能只写两个顶层POM。核心suite的checked-in artifact manifest与source recipe可供其他开发者复验，无需外部参考仓库。

## 4. 必需设置与执行方式

未来工具准备器在B02创建，统一interface：

```text
python3 tools/e2e.py fetch-tools    # 显式网络获取；验证lock/hash
python3 tools/e2e.py prepare        # 只读已锁工具与签入source，生成fixtures和基准
python3 tools/e2e.py verify-inputs  # 验证hash、raw结构、参考Driver结果，不调用Diana
python3 tools/e2e.py verify-harness # 验证错误结果会被拒绝，不实现引擎
```

以上是**B02/B03将来实现的命令合同，脚本目前不存在**。工具全部在checkout `.cache/e2e/tools/<artifact-id>/`按hash保存；无参数运行不扫描home或自动下载，fetch是唯一联网动作。CI也必须同一路径/锁/recipe，不单独使用另一版SDK。

可提供自定义工具root或下载镜像，但显式参数必须相对checkout并通过同内容锁；不把`JAVA_HOME`/个人绝对SDK位置视必需。OS可执行suffix与classpath由代码处理；Rust`join_paths`或相应Python跨平台路径API，不硬写冒号classpath供Windows用。

核心fixture全自制、只需要锁JDK/R8/smali，不需要`android.jar`或用户APK。工具缺失、hash错、网络未执行、零cases、运行超时明确为**prep未完成**，不能skip=pass。B01/B02/B03在引擎实现之前完成，不把“等T05再填工具版本/expected”当可接受实施顺序。

## 5. 测试资产与冻结目录

设计时就创建并签入的规范：`docs/diana/e2e_cases.json`、[E2E_CASES.md](E2E_CASES.md)、[TEST_PREPARATION.md](TEST_PREPARATION.md)。实际B02产出：

```text
tests/e2e/
  sources/<case-id>/        # 自制Java/smali/driver/依赖；无机器路径
  expected/<case-id>.json   # 独立返回/轨迹/状态，不由引擎输出生成
  inputs/                  # 小型自制DEX/ZIP/AXML/badbytes；或审阅过的生成recipe
  manifest.json            # 来源、recipe/toolchain lock、artifacthash、suite固定成员
  toolchain.lock.json      # 完整artifact/hash/传递依赖/主类
  cases.json               # 从审阅过设计清单生成，签入执行版本
  runner/                  # 公开interface的薄adapter；无私有phase依赖
```

执行workspace在`.cache/e2e/`，其他目录的source/expected不可自动write-back。若签入binary fixture，需局部白名单.gitignore而非取消全局DEX/APK忽略；来源/hash/小size审阅。可以只签recipe，但prep须验证每个开发者生成结果受同hash/语义字段锁约束，不假定作者cache存在。

inputs编译/拼接可改变符号池offset时，recipe的语义检查要逐field比较；artifacthash变更需要独立fixture rebuild记录，不能当“反编译失败，重新生成一个更简单DEX”。返回/事件期望永远源于规范 worked example或独立基准，不copy Diana/dexdec/radguard打印文本。

## 6. 无路径依赖验收

B03通过干净checkout、全新cache实际执行：显式fetch→prepare→verify-inputs→verify-harness；构建和参考Driver都成功且与签入预期一致，没有作者机器目录。至少覆盖主要平台路径语义（Linux的reference lane；Windows/macOS的路径/classpath规则），未跑平台明确not-verified，不能泛称全平台完成。

检查文档/recipe中是否残留home、OS绝对临时路径、兄弟仓库、已删worktree、工具/fixturechecksum占位值、“从未签入sample读取”以及未经确认的fixtures。正文提到路径政策不是允许出现真实私有路径。没有依赖源/许可/锁文件的case处于preparation blocked，不自动从required移到optional。

这一合同只要求团队可复制准备/验证；生成源码text、并行结果和合成ID不要求稳定。锁fixture/toolchain不是恢复旧stable-output模式，原始身份/程序语义仍精确。
