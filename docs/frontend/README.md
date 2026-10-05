# Palace 前端工程

前端承载 Palace 的整体产品体验。产品方向是按日期浏览时间线、通过 Moment 卡片进入类型化详情，
见[跨端形态](../product/跨端形态.md)。这些产品约定包含后续建设目标。

当前 Web 应用位于 `apps/palace-web/`，视觉以 [`docs/design`](../design/README.md) 的原型为准。已实现：
顶部五个文字导航（只有「时刻」可点击）、按日期浏览的时刻时间线、摘星中的对话阅读页及对话的导入与管理；
其他 Moment 类型和摘星入口页尚未实现。对话的业务规则放在[对话功能](../features/conversations/README.md)，
本目录记录各功能共同使用的工程能力。

## 文档导航

- [开发与验证](开发与验证.md)：本地联调、构建、部署边界和测试入口。
- [API 客户端](API客户端.md)：生成类型、同源凭证、错误处理及文件上传。
- [组件与样式](组件与样式.md)：UI 基础组件、主题和 shadcn registry。

## 当前技术边界

- React 19 + Vite + TypeScript，React Router 管理页面路由。
- React Query 管理服务端数据与 mutation；正在浏览的日期、当前 Path 等页面状态保存在地址中，不另设客户端状态库。
- 通用组件和主题由前端工程维护，Moment 类型的摘要、详情与专属操作归各功能文档。
- 认证与 Session 的服务端行为见[认证系统](../server/认证系统.md)。

新增 Moment 类型时，在 `docs/features/` 下记录该类型的发生时间、摘要、详情与专属操作；
跨 Moment 类型的产品约定继续维护在 `docs/product/`。
文档目录用于组织实现知识，不要求每种 Moment 拥有独立一级页面。
