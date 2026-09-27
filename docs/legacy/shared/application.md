# 身份、导航与外部集成

[返回总览](../README.md)。本页记录各业务页面之上的共同能力，避免迁移只复制页面主表而遗漏日常使用入口。

## C08：账户资料与语言

后端读取反向代理传入的 `Remote-Email`，将邮箱映射到 `d_user`，不存在则创建。名为 JWT 的 middleware 在这里执行的是邮箱头映射，不是应用自己验证用户名密码的登录流程。没有该头时用户 ID 为 0，因此部署认证边界在代理层，不能仅根据 middleware 文件名推断鉴权能力。

主 Dashboard 在 `username` 为空时显示初始化资料页；随后桌面 Profile 可修改昵称、头像和 `zh-CN / en-US`。Journal 复用用户资料。迁移需要将旧 `creator_id` 映射到 Palace owner，不能把两个系统的数字 ID 直接等同。

## C09：主应用导航与独立 Journal

桌面顶部导航默认收起，悬停 / 点击区域后展开：Dashboard、Todo、Journey、Bookmark、Echoes、Blog。移动端底部导航有前五项，没有 Blog。访问过的页签保持挂载，以保留当前页面状态；Journey 与 Echoes 自己还有子页签。

Journal 使用独立 `JournalApp`；服务端另挂 `/journal/` 静态入口，桌面 Profile 也能在侧栏 iframe 打开部署的 Journal 站点。页面切换主要是组件状态，不等于每条内容都有独立可分享 URL。

主应用和 Journal 都注册 PWA service worker，新版本就绪时提示用户更新。这不证明已有可离线写作、离线队列或端到端同步能力。

## C10：返回页面时发现其他客户端修改

客户端请求使用 `Only-Session-Token`。后端在内存中保存用户写版本与客户端版本（循环保存 5 个客户端槽），成功 POST 推进版本。Dashboard 回到可见状态会查 `GetSessionStatus`，过期时重置访问页签集合，再调用 `SyncSessionStatus`。

它是缓存陈旧检测，不是持久同步日志。进程重启会丢失该状态，没有内容级合并、推送同步或离线回放。正文另有 `ts` 冲突检查；两种机制不能混为一谈。

## C11：RSS 和邮箱未读数

桌面 Profile 支持输入 Miniflux token、QQ 邮箱账户和授权 token。配置存在 `d_user.rss_token / email_token / email_feed`，token 经后端加密；接口向客户端返回是否配置等状态而非用于展示明文。

后端向 Miniflux / QQ 邮箱查询未读数量，首页显示非零角标，页面重新可见时有刷新逻辑。旧库不存邮件正文或 RSS 文章列表。凭据缺失时入口可引导配置；返回 0 也可能是连接失败后的结果，不能把角标消失一概等同于已读完。

迁移时需要原加密配置才能解读旧凭据，或让用户重新连接；数据导入不应把密文当可用 token。

## C12：打开外部工具

Journal、Beaver 和 Miniflux 提供侧栏嵌入入口，QQ Mail 新窗口打开。这些是导航与未读集成；本代码不能证明 Beaver 的其他功能或外部服务的内部模型，需要独立梳理时再查相应项目。

首页 Today 未完成事项还有 Start，复制标题并打开 TimeTagger，详见 [T15](../dashboard/todo/planning-and-execution.md)。计时数据不存于本项目，不能从 `d_count` 推算耗时。

## C13：全局外部搜索命令

在 Dashboard 主应用按 Ctrl / Cmd + J 打开命令框；输入搜索内容选择引擎，或输入引擎缩写后按 Tab 固定引擎再输入内容。

支持 Bing、Google、百度、朗文词典、翻译、GitHub、小红书、Bilibili、知乎、豆瓣。它只拼接外部搜索 URL 并打开新窗口，不查询 Todo、Journal、Bookmark 或正文表，也不保存内部搜索历史。

## 迁移核对

资料、语言、更新提示、外部入口与连接配置分别保留或明确替代方案。新平台的 owner、会话和同步沿用 Palace 自身设计，不把旧版内存版本机制直接当作统一同步规范。

源码：[用户模型](../../../third_party/dashboard/model/user.go)、[身份 middleware](../../../third_party/dashboard/middleware/jwt.go)、[会话机制](../../../third_party/dashboard/middleware/session.go)、[主导航](../../../third_party/dashboard/client/src/components/tabbed-app.tsx)、[Profile](../../../third_party/dashboard/client/src/components/profile.tsx)、[外部搜索](../../../third_party/dashboard/client/src/components/search-command.tsx)、[用户接口](../../../third_party/dashboard/handler/user)。
