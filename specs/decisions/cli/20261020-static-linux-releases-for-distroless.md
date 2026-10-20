---
status: implemented
date: 2026-10-20
---

# Linux pal 改发 musl 静态包，posts 在构建镜像时安装最新正式 Release

Linux 的 x86_64、ARM64 发布目标改为 `x86_64-unknown-linux-musl`、`aarch64-unknown-linux-musl`。
posts 保持 `gcr.io/distroless/static-debian12:nonroot` 最终镜像，在独立构建阶段下载并验证 pal，再将二进制复制到
`/usr/local/bin/pal`。每次 posts 发布解析当时最新的 pal 正式 Release，两种架构固定到同一个版本；运行中不自更新。

当前为 `implemented`，由 Eric 要求重新选择构建方式、适配 posts 最终镜像而制定；代码、安装器与本地 x86_64 静态运行已验证，
实际发布仍须触发新的 Palace Release。前序为[文章上传决策](20261020-article-upload-from-directory.md)，
同时承接 [CLI 根决策](0-pal-cli-uploads-and-deletes-media-with-env-token.md)的分发与安装边界。
影响 pal 的发布矩阵、自更新平台选择，以及 posts 仓库的 Dockerfile、安装脚本和 CI；不改变服务端协议或 posts 转换业务。

## 问题与约束

posts 的最终镜像没有 shell、包管理器或 GNU 动态加载器。现有 Linux 包在 Ubuntu 24.04 上以 GNU 目标构建，
不能据此假定能在 `distroless/static` 运行。改用携带 glibc 的最终镜像仍需匹配构建机的 glibc 版本，
并扩大运行镜像的依赖范围。pal 的 HTTP 客户端已用 rustls，不需要引入 OpenSSL 动态库。

本地禁止构建 Docker 镜像或运行 BuildKit；本地验证使用原生 Rust 构建、ELF 检查，以及运行现成的
distroless 基础镜像。两种架构的发布与最终 posts 镜像启动检查放在 CI。

## 继承与修改

| 前序约定 | 本决策 |
| --- | --- |
| CLI 只使用公开 API、环境变量凭证与既有输出约定 | 继承，不变 |
| 文章从目录一次上传 | 继承，不变；容器内可执行同一命令 |
| Linux 发布采用 GNU/glibc 目标 | 修改：两种 Linux 架构只发 musl 静态包 |
| 自更新清单记录 target、固定版本附件 URL 与 SHA-256 | 继承；Linux 自更新按 OS/CPU 选择 musl 包，包括本地 GNU 构建的 updater |
| macOS 与 Windows 发布目标 | 继承，不变 |

## D1：以原生 runner 构建 musl，检查无动态依赖

amd64、arm64 分别用对应的 Ubuntu 原生 runner 安装 musl 工具链构建，不使用本地 Docker 或交叉编译容器。
发布前检查 ELF 不含 `PT_INTERP` 或 `DT_NEEDED`，运行 `--version`，并在现成的
`distroless/static-debian12:nonroot` 容器中以非 root 用户运行 `article upload --help`。
static PIE 可以包含动态重定位段；判断依据是是否需要外部加载器和共享库，不能仅因存在 `PT_DYNAMIC` 就拒绝。

**依据与代价**：静态包同时服务常见 glibc 主机、Alpine 和精简镜像，避免追踪不同发行版的 ABI 版本。
代价是二进制携带运行时，安全修复需重发 pal。删除 GNU 发布矩阵，避免同一 CPU 维护两套用途重叠的 Linux 包。

## D2：posts 下载固定版本、按架构校验，最终镜像只复制 pal

posts 发布 CI 读取最新正式 Release 的 `latest.json`，同时确认两种 musl 附件存在，再将版本通过 `PAL_VERSION`
传给 Docker 构建。安装阶段运行在构建主机上，按 `TARGETARCH` 下载固定版本清单和对应附件：

1. 检查版本、唯一 target、附件名与固定到官方仓库该版本的 URL；不能回退到 GNU 包。
2. 校验归档 SHA-256，确认包只包含普通文件 `pal`。
3. 校验 ELF 为目标 CPU 的 64 位二进制且不依赖动态加载器或共享库，设置执行权限。
4. 复制到最终镜像 `/usr/local/bin/pal`；Node、归档和解包工具只存在于构建阶段。

安装失败使镜像构建失败，不能悄悄缺少 pal。读取 `latest` 只在发布入口进行一次，后续都固定版本，
防止两个架构跨越 latest 更新窗口安装不同版本；版本也是 Docker 缓存键的一部分。
直接使用 Dockerfile 时 `PAL_VERSION` 默认 `latest`，调用方须注意构建缓存；可显式传版本或禁用 `pal` 阶段缓存。

**依据与代价**：读取已发布产物避免 posts 再拉 Palace 源码编译，两个项目发布职责分开。
代价是新流程依赖首个 musl Release 先发布；旧版只有 GNU 附件时明确失败。

## 不变量

1. Linux 发布的 pal 不需要宿主机的动态加载器、glibc 或 OpenSSL。
2. 每次 posts 发布的两种架构都安装同一个明确的 pal 版本。
3. 未通过摘要、架构和静态链接检查的包不进入最终镜像。
4. 最终镜像保持非 root 运行；安装和更新不发生在容器启动阶段。

## 为什么不是这些替代方案

| 方案 | 优势 | 取舍 |
| --- | --- | --- |
| 改 posts 最终镜像为带 glibc 的发行版 | 不必改变 pal 的 GNU 构建 | 需匹配 glibc 版本并携带额外动态依赖；静态包更适合既有运行镜像 |
| 在 posts 镜像构建中拉源码编译 pal | 不依赖 pal Release 附件 | 让消费者承担 Palace 工作区、Rust 与 C 工具链构建，职责和缓存成本扩大 |
| 容器启动时运行 `pal update` | 总能获取最新版本 | 需要安装目录可写和启动网络，重启会改变部署内容且难以复现 |
| 每种架构单独下载 latest | Dockerfile 简单 | latest 在两次下载之间变化时版本不一致，违反不变量 2 |

## 落地与验证边界

核心用例见[静态发布](../../test-cases/cli/static-releases.md)。本地验证实际 x86_64 静态构建、安装器和现成基础镜像；
CI 在发布包生成前检查两种原生架构，posts CI 发布后在两种原生架构运行最终镜像的 `pal --version` 和
`article upload --help`。CI 配置已写入不等于 CI 已执行；没有真实运行的义务保留验证缺口。

安全落地顺序是先发布带两种 musl 包的新 pal Release，再发布 posts 镜像。不需要迁移文章或数据库。
posts 生成 `article.json`、调用上传和保存远端结果仍属于后续业务接入。

依据：[distroless 官方说明](https://github.com/GoogleContainerTools/distroless)、
[Rust ARM64 musl 目标](https://doc.rust-lang.org/rustc/platform-support/aarch64-unknown-linux-musl.html)。
