# Diana 设计与实施入口

当前只有Diana包骨架；Engine、可运行fixture与runner尚未实现。资料研究与设计不是能力验收。

Diana是反编译器：默认展示可恢复的代码，方法失败在原位置/身份处标Smali或原因，
不因Mixed清空整类成果。完整JavaUnit资格与正常展示分开，text/json不改变恢复能力。
资源配额默认不限，只执行调用者显式设置的上限；不按猜测的JVM费用拒绝Java。
范围/格式检查、真实分配失败、取消与背压仍保留，实际编译属于独立验收。

只维护以下文档，不再维护HTML看板、生成器或拆分的流程/审阅/任务副本：

| 文档 | 唯一职责 |
|---|---|
| [DESIGN.md](DESIGN.md) | 产品/CLI合同、Engine接口、Module/数据结构、错误与预算 |
| [ALGORITHMS.md](ALGORITHMS.md) | 解码、SSA、类型/copy、控制/异常/锁、class与应用安全的逐步算法 |
| [TASKS.md](TASKS.md) | 开发流程、B01–B04、T01–T25、H01–H10、工具与验证/完成条件 |
| [SOURCE_MAP.md](SOURCE_MAP.md) | 参考版本、来源/许可、资料获取与证据边界 |

[e2e_cases.json](e2e_cases.json)单独保存案例/独立expected，schema 2将每次调用的输入与
预期绑定；draft-5含86个required案例、177个调用场景。允许有依据的纠错与追加，不为数量
保留错误格式/漏洞；不另维护expected或网页镜像。工具锁直接指向TASKS，不留历史跳转。
尚未通过B01–B04，工具自测授权阻塞见TASKS，不能提前写功能。

实施时读DESIGN → ALGORITHMS对应节 → TASKS当前卡；只在需要核查证据时查SOURCE_MAP。
应用仅apps/diana，引擎仅crates/diana-decomp；ref-fork只读，不自动构建/测试。
GUI/Web仍README占位，MCP延期。其余硬约束见根[AGENTS.md](../../AGENTS.md)。

资料按需取：`python3 tools/fetch_references.py --list`或`--only <id> ...`。
缓存为忽略的.cache/references，论文只保TXT；参考不校checksum，不影响E2E内容锁。
