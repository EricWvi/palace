# Palace 文档

Palace 的产品方向是按发生时间组织个人的 Moment（时刻），通过时间线卡片打开各类 Moment 的详情。
当前工程已实现对话收藏及其服务端能力；产品规划与已实现功能分别记录。

## 文档导航

| 目录 | 内容 |
| --- | --- |
| [产品设计](product/README.md) | 产品方向、领域模型及 Web/移动端形态 |
| [设计系统](design/README.md) | 视觉语言：颜色、字体、间距、交互与组件规范 |
| [前端工程](frontend/README.md) | Web 开发、API 客户端、通用组件与样式 |
| [功能实现](features/README.md) | 各 Moment 类型的业务规则和专属交互，目前包含对话收藏 |
| [服务端](server/README.md) | API 契约、数据库、同步、认证及运行方式 |

## 常用入口

- [Web 本地开发与验证](frontend/开发与验证.md)
- [对话收藏](features/conversations/README.md)与[分支管理](features/conversations/分支与路径.md)
- [认证系统](server/认证系统.md)与[环境变量](server/环境变量.md)
- [API 契约生成与检查](server/README.md#api-契约)
- [正式 OpenAPI](../contracts/openapi.json)

运行 `task --list` 查看工程任务。CI 检查由 Taskfile 编排，GitHub Actions 只做构建。
已批准的设计决策与核心测试义务保存在 `specs/`；本目录解释产品方向与当前实现。
