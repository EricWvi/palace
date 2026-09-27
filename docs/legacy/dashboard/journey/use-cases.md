# Journey 体验与回顾用例

[返回 Journey](README.md)。除特别说明外，元信息操作修改 `d_watch`，长文操作修改其引用的 `d_tiptap`。

## J01：建立待体验清单

To Watch → Add，填写名称、类型，可补年份、作者和链接。创建 `status=Plan to Watch`，`payload.epoch=1`。新增项按创建时间出现在前面。

行菜单支持编辑基础资料和封面、撰写感想、置顶、开始体验及确认删除。置顶直接重写 `created_at=now`，没有独立排序列。筛选和分页不会改变数据。

## J02：开始体验，并生成提醒

1. 待体验条目菜单选择开始。
2. 设置进度单位和总量，确认后改为 `Watching`，当前进度为 0，追加当天 `[日期, 0]` checkpoint，并改写 `created_at`。
3. 记录从 To Watch 移到 Watching 卡片。
4. 前端在成功回调中另行 `CreateTodo`，标题为本地化的“类型: 名称”，链接复制作品链接，归属 Inbox。

Watch 和 Todo 没有彼此 ID 引用，两个写入不在同一事务。后续修改作品标题 / 链接、完成或放弃体验，不会自动同步、归档或删除这条 Todo。移植时要保留这条跨功能工作流，并重新决定是否建立正式关联。

## J03：继续阅读、观看或游玩

Watching 卡片显示封面、名称、作品年份、类型、累计进度 / 总量与百分比。点击 Continue 使用 `payload.link` 打开外部内容；没有链接时阻止跳转。点击卡片编辑入口可以修改作品资料和封面。

进度完全由用户手动记录，不自动读取外部播放位置。

## J04：记录进度和查看体验时间线

1. 点击进度区域打开进度弹框，修改单位、总量和当前进度；提供加减按钮。
2. 保存 `progress`，总量至少为当前进度；Percentage 更新逻辑将总量基准设为 100。
3. 有 checkpoint 时：最后一条已经是今天，则覆盖其累计进度；否则仅在进度改变时追加今天记录。
4. 在进度弹框查看历史 checkpoint；完成后可从行菜单 Timeline 查看。

同一天多次更新一般只保留当天最后的累计进度，不是逐操作日志。首次从 To Watch 开始会建立第一条 checkpoint；对缺少 checkpoint 的旧数据，更新逻辑没有自动补建第一条。放弃事件的 -1 也在同一数组中，不能解释为负进度。

## J05：记录感想与书籍摘录

待体验、进行中和已完成都有撰写感想的入口；缺少 `payload.review` 时先创建文档再回写引用。已有感想可只读预览，再进入编辑。

书籍在进行中进度弹框和已完成菜单提供 Quotes 摘录，保存在 `payload.quotes`。感想与摘录是两份独立正文，不要合并成一个评论字段；其富文本、附件、历史沿用[共享编辑](../../shared/editor-and-history.md)。

## J06：完成体验并评分

Watching 当前进度等于总量后，卡片的继续按钮变为完成入口。选择评分并确认：设置 `status=Completed`、`rate`，将 `created_at` 更新为当前完成时间，移入 Watched。完成时有庆祝动效，根据 checkpoint 起点至完成日的天数选择效果。

后端更新没有把进度自动补到总量；到达总量是当前页面提供完成入口的条件。迁移时须保留作品日期与完成日期的区别，不能用发行年份替代个人体验时间。

## J07：补录历史完成记录

Watched → Add，可以直接录入已完成作品，填类型、名称、发行年份、作者、评分、完成日期。新建 `Completed` 记录，`payload` 初始为空，因此可能没有进度、开始时间或 checkpoint。

已有完成项可以修改这些元信息并上传封面。历史记录没有 Watching 阶段是合法来源，迁移不应为它虚构开始时间和进度轨迹。

## J08：放弃并恢复

在 Watching 编辑流程中选择放弃，将 `status=Dropped`，在 checkpoints 追加 `[当天, -1]`。To Watch 下方入口打开 Dropped 列表，显示名称、放弃日期，可查看已有感想。

恢复把同一记录改回 `Plan to Watch`，重写 `created_at`，将进度归零，保留其他 payload（含旧 checkpoints 和正文引用）。再次开始还会再创建 Inbox 提醒。放弃不是删除，不影响先前生成的 Todo。

## J09：再次体验同一作品

Watched 行菜单再次体验，确认后**新建**一条 `Plan to Watch`：复制名称、类型、年份、作者与 payload，评分归零、进度归零、checkpoints 清空、`epoch+1`。原完成记录保留。

复制 payload 时不会清空 `review`、`quotes`，所以两轮记录可能引用同一份感想 / 摘录。Palace 导入时先保留共享关系；若改成每轮独立正文，需要明确复制规则，不能默默串改或重复丢失内容。

## J10：分享作品体验

Watched 的作品展示可打开分享预览，呈现封面、作品名、作者、数值 / 星级评分、用户头像昵称、完成日期和轮次，有感想时加上正文。分享按钮将卡片渲染成 PNG 下载。

数据读取 `d_watch`、`d_user`，有感想再读 `d_tiptap`，图片依赖 `d_media` 或外部 URL。操作不新建分享表，也不改变公开状态；[截图](../../screenshots/Journey-Share.png)展示的是预览，不是一个匿名可访问的详情地址。

## J11：删除作品记录

To Watch / Watched 行菜单 Delete 经确认后软删除当前 `d_watch`。没有同时删除对应 Inbox Todo、正文或媒体，也没有作品回收站入口。

## 迁移核对

- 待体验 → 开始 → 更新进度 → 完成 → 分享形成完整闭环。
- 同日更新覆盖、跨日推进、放弃 -1、恢复后重新开始均能解释。
- 直接补录完成项与正常体验记录共存。
- 再次体验保留旧记录、轮次和共享材料，评分按正确倍率呈现。
- 完成筛选年份使用个人完成日期；书籍摘录和感想分别可读写。

源码：[Watching 操作](../../../../third_party/dashboard/client/src/components/watching-list.tsx)、[表格新增](../../../../third_party/dashboard/client/src/components/react-table/data-table-toolbar.tsx)、[行菜单](../../../../third_party/dashboard/client/src/components/react-table/data-table-row-actions.tsx)、[Dropped](../../../../third_party/dashboard/client/src/components/dropped-watch.tsx)、[分享卡](../../../../third_party/dashboard/client/src/components/watch-review.tsx)、[checkpoint 展示](../../../../third_party/dashboard/client/src/components/watch-checkpoint.tsx)。
