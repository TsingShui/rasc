# Diana 活动工程与 ref-fork 参考隔离

用户最新决定：**Diana 分支不再构建、测试 rasc/dexdec；旧实现降级到 ref-fork。**
这覆盖此前“根 rasc 默认、同 workspace 双项目”的临时方案。主 workspace 只含
Diana 包，rasc 整体保留为独立参考，不删除源码或测试，不把它当生产依赖。
本次仍只做目录/包/构建范围，不实现 Engine 或产品功能，B01–B04 仍先于 H/T。
用户另要求预留 GUI 与 Web UI：保留 apps/，新增两个 README-only 目录，不实现或选型。

## 1. 当前实际目录

```text
Cargo.toml                      # Diana-only virtual workspace，没有 root rasc package
Cargo.lock                      # 只锁 Diana 活动包
apps/README.md                  # CLI / GUI / Web UI 应用入口索引
apps/diana-gui/README.md         # GUI 占位，无 manifest/source/build
apps/diana-web-ui/README.md      # Web UI 占位，无 manifest/source/build
apps/diana/
  Cargo.toml                    # package diana，publish=false，0.0.0 骨架
  src/main.rs                   # 只报告未实现并失败退出，不转发旧 CLI
  README.md
  AGENTS.md
crates/diana-decomp/
  Cargo.toml                    # package diana-decomp，无外部依赖
  src/lib.rs                    # 文档级空库；没有 Engine/阶段实现
  README.md
  AGENTS.md
docs/diana/                     # 架构、任务、70 案例设计与离线看板
tools/audit/                    # 继续服务整个仓库的敏感信息检查
.github/workflows/diana.yml      # 只做 Diana 格式/lint/build，无发布
ref-fork/
  README.md                     # 参考范围与公开源码版本定位
  rasc/
    Cargo.toml                  # 独立旧 workspace，主 workspace explicitly excluded
    Cargo.lock                  # 保留旧依赖锁；不由主工程更新
    src/                        # 全部 rasc CLI/library/analysis/MCP
    tests/                      # 原集成测试完整保留，不纳入 Diana 测试
    skill/                      # rasc 旧内嵌 skill
    crates/dexdec/              # 旧反编译器与 license
    crates/rusty-dex/           # 旧解析器与 license
    vendor/axmldecoder/          # 原补丁依赖与 license/PATCHES
    tools/release/              # 旧发布脚本/测试归档，不自动运行
    tools/verify_codemode.mjs    # 旧 MCP code-mode 验证工具归档
    docs/                       # 旧 headless-MCP 文档与 release 记录
    .github/workflows/release.yml # 嵌套归档，GitHub 不发现/执行
    README.md
    AGENTS.md
    CHANGELOG.md
```

所有路径相对仓库；旧实现相对 rasc 工程内部的布局未变，因此原路径依赖与
`CARGO_MANIFEST_DIR` 关联仍在参考工程内，不需要生产代码改写。
参考目录保存源内容，不代表它是可发布的第二产品。历史发布 workflow 为旧
根 checkout 编写，现仅归档，不作为这里可直接运行的发布命令。
根 NOTICE 按用户要求删除；不新增许可证文件或 bundle，已有 crate license 不动。

当前没有创建 `tests/e2e/`、`apps/diana/tests/` 或 Engine 具体 phase 目录。
以后 B02/B03 生成共享 E2E sources/expected/inputs/tool lock，实际产品测试在
`apps/diana/tests/product_contracts.rs`，内部引擎测试仅加公开库 adapter。
不铺满空文件、getter 测试或 `.gitkeep` 来伪装功能/准备完成。

保留 `apps/` 是为了区分多个未来产品入口与可复用引擎。
[应用索引](../../apps/README.md)列出 CLI、[GUI](../../apps/diana-gui/README.md)、
[Web UI](../../apps/diana-web-ui/README.md)：后两者当前只有 README，没有 Cargo/前端
manifest、源码、依赖、构建目标或 CI job，不加入 workspace。GUI 框架、Web UI
运行环境、WASM 绑定或浏览器界面、引擎可移植性均未选型或验证，不纳入现有
H/T/E2E 分母。

## 2. Workspace、依赖与锁文件隔离

```text
活动 root workspace
  diana（apps/diana/） ──→ diana-decomp（当前无依赖）

excluded reference workspace（ref-fork/rasc/）
  rasc ──→ dexdec ──→ rusty-dex
       └→ 原有 ZIP/manifest/session/MCP 依赖
```

- 主 `Cargo.toml` 只有 `[workspace]`，成员/default-members 均是 Diana 两包，
  `exclude=["ref-fork/rasc"]`；没有旧 rasc package/deps/axmldecoder patch。
- 参考有独立 `[workspace]`、旧三包和自己的 patch/lock，不加入主 members。
  `cargo --workspace` 不递归进入 ref-fork，也不会解析/获取其 crates。
- 根 `Cargo.lock` 仅 Diana 本地包；旧锁保留在 `ref-fork/rasc/Cargo.lock`，
  不以根 lock 合并维持旧依赖，不更新旧版本/工具。
- Diana 不依赖旧包、不转发到旧 CLI/engine。当前 diana→diana-decomp 仅包关系，
  主程序没有 Engine 调用或实现。
- 默认 Cargo 构建/测试/格式/lint 只选 Diana。旧工程仅源阅读，不为 Diana
  开发 gate 构建、测试、格式或 lint 它；不要显式逐 ref manifest 再跑一圈。
- 旧 Rust 源码/测试、版权与许可保持原内容，除明确审阅的路径/说明更新。
  不移动个人 cache/旧 target 到参考目录或签入忽略的 APK/DEX/JAR。
- 以后复用输入/ZIP 思路要按公开来源与语义/安全测试聚焦提取或独立实现，
  不创建跨产品机械转发或从 ref-fork 引入整个依赖树。

两个活动包是 Rust 2024 / 最低 1.93，unsafe forbid，publish=false；0.0.0
仅未发布骨架。根 release profile 为活动工程所用，参考保留自己的 profile。
旧 1.0.1 版本未改；版本/源码存在不是它仍需参加当前 CI 的理由。

## 3. 活动工程构建/测试范围

在仓库根运行：

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
# 后续实际 Diana 测试存在时才作为能力 gate；当前零测试不算通过
cargo test --workspace --all-features --locked
```

这些命令只覆盖 Diana。单包可 `-p diana` 或 `-p diana-decomp`，
`cargo run` 的唯一 binary 是 `diana`；当前只 stderr 报未实现并失败。
不运行旧 tests 或发布准备测试来给本分支“增加通过数”。
当前 `.github/workflows/diana.yml` 仅格式/lint/release build，不自动跑
空测试，不上传 artifact，不创建 tag/release。

已存在的根 `target/` 可能有旧 rasc 编译产物，它们不是当前 build 输出证据；
不擅自清理用户 cache。验证活动包选择用 `cargo metadata` / `cargo tree` 与
编译命令，不能根据 target 里残留 binary 宣称仍在构建旧工程。
`.gitignore` 排除参考 workspace 的 target/cache，避免未来显式维护产生文件入库。

## 4. 参考与历史来源定位

固定提交的源码锚点见 [SOURCE_MAP.md](SOURCE_MAP.md)；这些路径仍相对原公开
rasc 提交，不能把历史 URL 改成 ref-fork 路径。当前 checkout 读取时：

| 原提交路径 | 当前参考路径 |
|---|---|
| `src/analysis/...`、`src/cli/...` | `ref-fork/rasc/src/...` |
| `crates/dexdec/...` | `ref-fork/rasc/crates/dexdec/...` |
| `crates/rusty-dex/...` | `ref-fork/rasc/crates/rusty-dex/...` |
| `vendor/axmldecoder/...` | `ref-fork/rasc/vendor/axmldecoder/...` |
| `tests/...`、`skill/...` | `ref-fork/rasc/tests/...`、`ref-fork/rasc/skill/...` |

MCP 完整旧实现随 rasc 归档，不在 Diana 主工作区继续接线，也不因此设计新协议。
被移动的源码不成为行为真值；70 案例 expected 不从参考输出生成。
参考归档不阻止以后单独明确授权维护，但本分支默认不构建/测试/发布它。

## 5. 功能任务与状态

早期任务中的“root 产品/src”仍解释为 **Diana 应用根 `apps/diana/`**；
固定旧提交的 src 引用现在在 `ref-fork/rasc/`。共享 `tests/e2e/`、`docs/diana/`、
tools/audit 和活动 workspace Cargo/lock 仍相对仓库根。

H01/T01 后续实现产品合同与真实 Engine/预算/调度，不是只检查目录存在。
本轮隔离不算这些任务完成，不弱化 B04 的前置依赖。70 案例仍为 designed-not-built，
source/DEX/tool lock/runner 尚未准备，零测试不是 E2E 已绿。
不提交、推送、发布或手工改版本，除非用户明确要求。
