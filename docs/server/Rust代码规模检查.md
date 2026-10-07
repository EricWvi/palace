# Rust 代码规模检查

`task lint:crates` 会执行 `task check:rust-size`，因此 `task lint`、`task test:crates` 和
`task test` 都会阻止未登记的超限模块。`task build:test` 也执行同一检查。
GitHub Actions 只做构建与发布，不执行代码规模检查。

## 限制与计数

每个生产 Rust 文件以 **500 行为建议目标、800 行为硬上限**；恰好 800 行允许通过。
超过 500 行会计入检查摘要，超过 800 行且没有合规例外时检查失败。

计数是排除测试专属语法后的非空物理行，注释计入。检查使用 `syn` 解析源码，通过 Cargo
metadata 确定 workspace 成员、所属 crate 和全部 target，再跟随模块引用区分生产与测试代码。
同一物理行同时包含生产和测试语法时，只移除测试部分；测试声明之后的生产代码仍然计数。

- `#[cfg(test)]`、`#[test]`、`#[bench]` 及确定只在测试中存在的复合条件语法不计入生产行数。
- 集成测试、benchmark target 及测试模块引用的辅助文件不计入；被生产代码同时引用的文件按生产计数。
- 未知平台和 feature 条件保守保留，不依据当前机器的编译结果排除其他平台代码。
- 支持独立模块、内联模块、`#[path]`、`cfg_attr` 路径替换及既有的 `mod.rs` 路径解析。
- Git 中已跟踪或未跟踪、未忽略的 Rust 文件均检查；没有被模块引用的文件也保守按生产计数。
- 以 `// @generated` 开头的非根文件排除，生成内容应由相应生成一致性检查约束；Cargo target
  根文件必须纳入计数。已从工作区删除的文件不再读取。
- 宏不展开，宏定义和调用的源码保守计数。源码或条件编译语法解析失败会使检查失败。
- 规范化后的源文件与 target 路径必须位于 workspace 内，软链接不能绕过此边界。

## 查询与处理超限

```bash
task check:rust-size
task report:rust-size
```

等价 Cargo 入口是 `cargo xtask check-rust-size` 和 `cargo xtask report-rust-size`。
报告输出 JSON，逐文件记录所属 crate、生产行数和是否测试专属；报告与检查都不会重写基线。

超限时先按职责拆分模块，把相关测试和模块文档移到新实现附近。历史例外维护在
[`xtask/rust-size-baseline.json`](../../xtask/rust-size-baseline.json)，每项必须包含相对文件路径、
所属 Cargo crate（`owner`）、精确生产行数（`lines`）和具体拆分计划（`split_plan`）。
迁入时 Palace 没有超过硬上限的模块，因此基线为空；新增超限应拆分，不自动生成例外。

已登记例外必须遵循以下规则：

- 行数增长时检查失败，需抽取新职责。
- 行数减少但仍超限时，必须降低基线行数，避免已经移除的代码规模重新增长。
- 文件删除或缩至 800 行以内时，必须删除对应例外。
- `owner` 必须与 Cargo 所属 crate 一致，`split_plan` 去除首尾空白后至少 30 字节，并描述具体拆分方向。
- 基线拒绝未知字段，检查不会自动增加、放宽或批准例外。

这套措施从 desktop 的 `xtask/src/rust_architecture` 和 `crates/utils/src/rust_source` 移植。
Palace 将策略、源码解析和路径检查作为私有模块放在 `palace-xtask` 中，相关回归测试随实现移入，
通过 workspace 的 Clippy 和测试入口验证。
