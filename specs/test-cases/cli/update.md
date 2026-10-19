# pal 自动更新核心测试用例

依据本次直接开发要求及[CLI 使用约定](../../../docs/cli/README.md)，跟踪更新的校验边界和替换副作用。
本功能未新增 ADR。

## Updates must verify the release before replacing the executable

### 风险

更新替换用户下一次运行的程序。版本、平台或摘要不匹配仍继续安装，会使原 CLI 不可用。

### 前置状态与触发

准备正常的 `latest.json` 和对应 TAR.GZ / ZIP 压缩包，以及错误版本、平台、下载地址、重复目标、
错误摘要、特殊文件和损坏压缩包。通过更新流水线触发下载与安装。

### 必须成立与禁止结果

只下载清单指向的同一版本、当前目标平台的官方附件。摘要和压缩包均通过验证后，安装器才收到
临时目录中的普通可执行文件。任何前置步骤失败都禁止调用安装器。

### 验证义务与证据

| 验证义务                                                    | 状态    | 直接证据                                                                                                                                   |
| ----------------------------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 平台、版本、名称和官方 URL 一致，重复目标和不完整清单被拒绝 | Covered | `apps/pal/src/update/tests.rs::rejects_untrusted_or_ambiguous_manifest_assets`，注入下载源，观察安装边界及请求记录                         |
| 摘要不符或不合法的压缩包不进入安装步骤                      | Covered | 同文件 `checksum_mismatch_never_reaches_the_installer`、`invalid_archives_never_reach_the_installer`，真实 SHA-256 与 TAR.GZ / ZIP 解析    |
| 两种格式的正常附件交付完整内容，完成后清理暂存文件          | Covered | 同文件 `verifies_and_stages_both_release_formats_before_installing`，真实压缩包、文件系统及注入安装器                                      |
| HTTP 错误和超限响应不作为下载结果返回                       | Covered | 同文件 `download_checks_status_and_size`，真实 reqwest 与本机 HTTP 服务器                                                                  |
| 清单记录实际发布包的摘要、平台及版本固定 URL                | Covered | `scripts/pal-release-manifest.test.mjs` 的 `manifest records the actual archives with version-pinned URLs and SHA-256`，真实文件与生成脚本 |

## Self replacement must preserve executable permissions and clean up staging

### 风险

直接截断当前文件或丢失执行权限会破坏 CLI；安装失败遗留临时文件会污染安装目录。

### 前置状态与触发

复制测试可执行文件作为独立运行进程；准备已验证的更新包。另以注入安装器模拟目录不可写。

### 必须成立与禁止结果

安装成功后，原路径内容为新程序且保留执行权限。安装失败需报告替换错误；无论结果如何均清理
下载暂存目录。禁止对开发者实际使用的 CLI 或原测试运行器执行替换。

### 验证义务与证据

| 验证义务                                                | 状态    | 直接证据                                                                                                                          |
| ------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Unix 上运行中程序经完整验证流程替换，内容及执行权限正确 | Covered | `apps/pal/src/update/tests.rs::replaces_a_running_executable_after_verification`，复制出的真实子进程、真实文件系统与 self-replace |
| 替换错误能报告且清理暂存目录                            | Covered | 同文件 `replacement_errors_are_reported_and_staging_is_removed`，注入 PermissionDenied 的安装器与真实暂存目录                     |
| Windows 运行中文件替换                                  | Missing | 生产使用 self-replace 的 Windows 实现；尚无该平台真实子进程证据                                                                   |
| GitHub 上完整发布和下载链路                             | Partial | v0.4.1 五平台正式 Release 已发布，Linux 初次下载、摘要校验与安装已完成；完整 `pal update` 链路由 Eric 验证                        |

## Updates must honor standard proxy environment variables

### 风险、前置状态与触发

用户通过常见代理访问 GitHub。分别仅在独立子进程设置 `HTTPS_PROXY`、`https_proxy`、`ALL_PROXY`、
`all_proxy`，指向本机 HTTP 代理；再设置 `NO_PROXY` 或 `no_proxy` 排除目标主机。使用生产下载客户端请求
解析到回环地址的测试 HTTPS 主机，代理拒绝 CONNECT，目标端口不可连接，禁止访问 GitHub 或替换程序。

### 必须成立与禁止结果

未排除目标时，代理收到该主机的 CONNECT；排除后，代理不收到连接。禁止修改父进程环境或忽略标准
代理配置。

### 验证义务与证据

| 验证义务                                                        | 状态    | 直接证据                                                                                                                                               |
| --------------------------------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 大小写 HTTPS/ALL 代理环境变量生效，大小写 NO_PROXY 均能绕过代理 | Covered | `apps/pal/src/update/proxy_tests.rs::standard_proxy_environment_is_used_and_no_proxy_is_respected`，独立测试子进程、生产客户端及本机 HTTP CONNECT 代理 |

### 决策依据

Eric 要求自动更新采用环境变量中的常见代理设置。
