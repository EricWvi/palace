# Palace 前端工程

前端承载 Palace 的整体产品体验。产品方向是按日期浏览时间线、通过 Moment 卡片进入类型化详情，
见[跨端形态](../product/跨端形态.md)。这些产品约定包含后续建设目标。

当前 Web 应用位于 `apps/palace-web/`，视觉以 [`design`](../../design/README.md) 的原型为准。已实现：
顶部五个文字导航（只有「时刻」可点击）、按日期浏览的时刻时间线、摘星中的对话阅读页及对话的导入与管理；
其他 Moment 类型和摘星入口页尚未实现。对话的业务规则放在[对话功能](../features/conversations/README.md)，
本目录记录各功能共同使用的工程能力。

## 文档导航

- [开发与验证](开发与验证.md)：本地联调、构建、部署边界和测试入口。
- [API 客户端](API客户端.md)：生成类型、同源凭证、错误处理及文件上传。
- [组件与样式](组件与样式.md)：UI 基础组件和主题。

## 当前技术边界

- React 19 + Vite + TypeScript，React Router 管理页面路由。
- React Query 管理服务端数据与 mutation；正在浏览的日期、当前 Path 等页面状态保存在地址中，不另设客户端状态库。
- 时刻页在没有当天缓存时，同时请求时间线和轮廓（`/api/timeline/outline`）：轮廓先到就按每项的 `kind` 画出骨架，
  卡片到达后每个骨架在同一个列表项里按 `id` 过渡为自己的卡片（`DayMoments`）。系统要求减少动态效果时不做动画。
  规则见 `specs/decisions/server/moment/20261005-day-outline-and-skeleton-to-card.md`。
- 阅读页加载对话详情时，页首用同样的骨架画出标题与元信息行两行，逐行与真实文字同高，不需要额外请求；返回链接只依赖地址，加载期间照常显示。
- 通用组件和主题由前端工程维护，Moment 类型的摘要、详情与专属操作归各功能文档。
- 认证与 Session 的服务端行为见[认证系统](../server/认证系统.md)。

新增 Moment 类型时，在 `docs/features/` 下记录该类型的发生时间、摘要、详情与专属操作，
并在 `DayMoments` 的骨架映射中给出它的骨架形状（缺少时类型检查失败）；
跨 Moment 类型的产品约定继续维护在 `docs/product/`。
文档目录用于组织实现知识，不要求每种 Moment 拥有独立一级页面。
