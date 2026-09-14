# rasc

[English](README.md)（主文档） · **源仓库：<https://github.com/TsingShui/rasc>**

## rasc 

rasc 是 asc [ASC](https://github.com/MG1937/ASC) 的 Rust 实现，用于以极快的速度分析 Apk\Dex。
rasc 的实现主要由 Agent + 少量人工介入完成，在部分实现、优化上与 asc 不同。
rasc 有两个核心 Target: Cli 以及 WASM，不过会以 Cli 为主。


## 性能与取舍

在一份 343 MiB APK 的 11 个测试场景中，rasc 相对 ASC 的几何平均加速比为 **7.1×**。
测量方法与详细结果见[英文版「性能与取舍」](README.md#performance-and-trade-offs)中的折叠区。

这个结果并非没有代价。随着优化推进，rasc 的部分设计已经与 ASC 不同，不再是逐行翻译。
它更偏向吞吐性能，愿意用更多内存换取速度；例如，多线程引用搜索的峰值内存高于 ASC。
这是一种取舍，不代表每个场景都更省资源。

理解这一速度优势时，首先应考虑从 Python 转向 Rust 原生实现的差别，而不是把它当作 Rust
优于其他语言、或 Agent 优于人工开发的证明。实际结果也受算法、并行方式和内存策略影响。
换用 Zig、C++ 等语言，性能也可能进一步提升。

## 构建和安装

macOS 预编译产物（Apple Silicon 与 Intel）在
[Releases](https://github.com/TsingShui/rasc/releases) 页；从源码构建：

```sh
cargo install --path . # 构建使用 cargo build --release
rasc --help
```

## 命令行

```sh
rasc getclass app.apk com.example.Main                 # 单个类 → Java 风格源码
rasc getclass --threads 16 -o Main.java app.apk 'Lcom/example/Main;'
rasc findrefs app.apk string Authorization             # 在所有根 DEX 中查找引用
rasc findrefs app.apk method onCreate --class com.example.Main
rasc findrefs app.apk field INSTANCE --class example --fuzzy-class
rasc classes app.apk                                   # 类索引
rasc manifest app.apk                                  # 二进制 AndroidManifest.xml → XML
rasc skill                                             # 为 Agent 安装 rasc skill
```

更多用法见 `rasc --help`，具体命令的参数可通过 `rasc <命令> --help` 查看。

### 给 Agent 用

`rasc skill` 安装一个单文件 [skill](skill/SKILL.md)：说清 rasc 的适用范围，每条指令一行用法，
让 coding agent 知道什么时候该用 rasc、怎么调：

```sh
rasc skill                        # 自动识别已安装的 Agent，都没有时装到 ~/.agents/skills
rasc skill pi codex claude        # 也可以显式指定
rasc skill --dir .claude/skills   # 装到任意 skills 目录（比如项目内）
rasc skill --print                # 只把 skill 内容写到 stdout，不落盘
```

## 用了哪些库、借鉴了哪些开源项目


| 来源 | 上游 | 在这里的作用 |
|---|---|---|
| `ASC` | [MG1937/ASC](https://github.com/MG1937/ASC) （Apache-2.0）| 参考实现与对标基准（Python + Androguard）：rasc 从这里起步。 |
| `crates/dexdec` | [asLody/dexdec](https://github.com/asLody/dexdec)（Apache-2.0） | `getclass` 背后的 Java 反编译器：DEX → CFG/SSA → region → Java 源码。fork 说明见 [`FORK.md`](crates/dexdec/FORK.md)，改动清单见 [`PATCHES.md`](crates/dexdec/PATCHES.md)。 |
| `crates/rusty-dex` | [rusty-rs/rusty-dex](https://github.com/rusty-rs/rusty-dex)（Apache-2.0）+ 本仓库的扩展 | `dexdec` 读取字节用的 DEX 解析器。字符串与 id 池的按需解码在这里，可用的指令层（smali 基础）也在这里。 |
| `vendor/axmldecoder` | [axmldecoder](https://crates.io/crates/axmldecoder)（Apache-2.0 OR MIT） | `rasc manifest`：二进制 AndroidManifest.xml → 文本 XML。 |


## 许可

Apache-2.0，全文见 [LICENSE](LICENSE)；内树组件（`vendor/axmldecoder`、`crates/dexdec`、
`crates/rusty-dex`）的归属见 [NOTICE](NOTICE)。
