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
| GitHub 上完整发布和下载链路                             | Missing | 本地只运行清单生成脚本和注入下载源，未发布 Release                                                                                |
