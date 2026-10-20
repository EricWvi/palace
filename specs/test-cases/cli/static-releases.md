# pal 静态发布核心测试用例

决策依据：[musl 静态发布 ADR](../../decisions/cli/20261020-static-linux-releases-for-distroless.md)。

## Linux releases must run without a dynamic loader

- 风险：GNU 包或错误架构包进入 distroless，复制成功却无法启动。
- 前置状态：amd64、arm64 发布构建，及含解释器、动态依赖或其他 CPU 的 ELF。
- 触发：构建发布包、选择自更新平台、安装到 posts 镜像。
- 必须成立：Linux 选择 musl 目标；没有 `PT_INTERP`、`DT_NEEDED`，CPU 与目标一致；非 root 运行帮助命令成功。
- 禁止结果：自动回退 GNU 包、依赖宿主 glibc、构建阶段执行异架构文件。

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| Linux 自更新两种 CPU 都选择 musl | Covered | `apps/pal/src/update/tests.rs::linux_updates_select_static_musl_releases`，纯函数；发布清单脚本测试包含两种 Linux 附件 |
| posts 安装器拒绝动态加载器、共享库依赖、错误架构、损坏文件 | Covered | posts `scripts/install-pal.test.mjs` 的 `dynamic loaders, shared dependencies, truncated files and wrong CPUs are refused`，真实 ELF 字节解析 |
| 发布包在 distroless 非 root 运行，两种最终 posts 镜像均可启动 pal | Partial | 本地真实 x86_64 musl 构建、实际归档安装与 ELF 验证，并在现成 `distroless/static-debian12:nonroot` 镜像中运行成功；两仓库 CI 包含原生 amd64/arm64 容器检查，尚待发布运行 |

## Container builds must install one verified release for both architectures

- 风险：latest 漂移导致不同版本、缓存保留旧版、下载损坏却进入镜像。
- 前置状态：包含两种静态附件的清单，错误摘要、缺失或重复平台、外部 URL、固定版本不匹配。
- 触发：posts CI 解析 latest，安装脚本按 `PAL_VERSION` 下载。
- 必须成立：固定版本和官方附件 URL，摘要与文件匹配才安装，正确执行权限；错误使构建失败，已有目标不被替换。
- 禁止结果：摘要失败仍安装，GNU 包回退，运行中自更新。

| 验证义务 | 状态 | 直接证据 |
| --- | --- | --- |
| 两种架构下载固定版本且校验摘要、设置权限，摘要失败保留原文件 | Covered | posts `scripts/install-pal.test.mjs` 的 `both architectures install only the pinned, checksum-verified static binary`，真实归档、文件系统和注入下载源 |
| 缺失、重复、GNU 目标、外部 URL 或错误版本被拒绝 | Covered | 同文件 `GNU-only, ambiguous, foreign and mismatched release metadata is refused` |
| 发布 CI 将同一个解析出的版本传给两种镜像架构 | Partial | posts `.github/workflows/release.yml` 配置完成，实际 GitHub 发布尚未运行 |
