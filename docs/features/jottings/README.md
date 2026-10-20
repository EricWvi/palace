# 札记

札记是标题可选的短文，以带 frontmatter 的 Markdown 文件保存在一个 Git 仓库中，主要由本地 Agent 用文件
工具维护。服务端按 commit 把札记索引进 PostgreSQL，供移动端离线同步；移动端可以离线新建札记，但不能
修改或删除已有札记。决策见[札记根决策](../../../specs/decisions/server/jotting/0-jottings-indexed-from-git-commits.md)。
Web 回响页实时读取札记并可以新建，决策见[回响页根决策](../../../specs/decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md)。

## 文件格式

`PALACE_JOTTINGS_DIR` 下（含子目录）的每个 `.md` 文件都是一张札记：

```markdown
---
id: 0199f2a4-7c1e-7b3a-9d2e-4f6a8b0c1d2e
created: 2026-10-19T08:30:00+08:00
title: 可选标题
tags: [阅读, rust]
---

正文 Markdown。
```

| 字段 | 规则 |
| --- | --- |
| `id` | 必填，小写连字符形式的 UUIDv7；是札记的唯一身份，移动或改名文件不改变它 |
| `created` | 必填，带时区偏移的 RFC 3339 时间，精度不超过毫秒 |
| `title` | 可选；出现时是非空字符串 |
| `tags` | 可选；不重复的非空字符串数组 |

其他 frontmatter 字段保留在文件中，服务端不读取，修改它们也不会同步给客户端。正文是 frontmatter 之后
去掉开头空行的全部内容。

## Agent 的工作方式

- 写完再 commit。服务端只读取当前分支 HEAD 所指 commit 的树，工作区中未 commit 的修改不会被同步，写到
  一半的文件也不会被看到。
- 一个 commit 里只要有一个文件不合格（frontmatter 解析失败、字段不合规则、两个文件 `id` 相同），整个
  commit 都不索引，索引停在上一个合格的 commit，服务端日志列出全部问题。修好后再 commit 一次即可追上。
- commit 之前可以运行 `pal jottings check <札记目录>`，它与服务端使用同一份校验代码，适合作为
  pre-commit hook：

  ```sh
  #!/bin/sh
  exec pal jottings check jottings
  ```

- 移动端上传的札记由服务端写到 `inbox/<id>.md`，服务端不 commit。Agent 整理（移动、改写）后照常
  commit；原样 commit 不产生新的同步版本。上传札记在 commit 之前被删除的，下一次 commit 时变为删除。

## 同步接口

| 接口 | 用途 |
| --- | --- |
| `GET /api/jottings/changes?since=<版本>&limit=<n>` | 按版本升序返回 `since` 之后变化的札记的当前状态，`limit` 默认 200、上限 1000 |
| `POST /api/jottings` | 上传离线新建的札记 `{ id, created_at, title?, tags?, body }` |

- 首次同步用 `since=0`，这时不返回删除记录；之后按响应中的 `next` 继续，并在同一个本地事务里应用本页
  和推进游标。`has_more` 为 `true` 时继续拉取。
- 上传以 `id` 幂等：首次接受返回 `201`；同一内容再次上传返回 `200` 和该札记当前的版本，即使它已被
  Agent 修改或删除；`id` 已属于另一张札记时返回 `409 jotting_id_conflict`。正文上限 1 MiB。
- 未配置 `PALACE_JOTTINGS_DIR` 时，`changes` 返回空页，上传返回 `503 jottings_unavailable`。
- 札记不属于某个 Owner，所有已认证请求（浏览器 Session 或 Access Token）看到同一份数据。

## Web 读取接口

回响页不使用同步接口，而是按页面需要实时查询索引。三个接口都只返回未删除的札记，每张札记带 `pending`：
文件位于札记目录下第一级的 `inbox/` 时为 `true`，与是否已经 commit 无关。

| 接口 | 用途 |
| --- | --- |
| `GET /api/jottings?tag=&q=&cursor=&limit=` | 按 `created_at`、`id` 降序分页，`limit` 默认 60、上限 200；`tag` 精确匹配，`q` 在标题、正文和标签中做不区分大小写的子串匹配，两者取交集；`total` 是符合条件的总数 |
| `GET /api/jottings/overview?from=YYYY-MM-DD` | 札记总数、有札记的天数、全部标签及数量（按数量降序、名称字节序），以及从 `from` 到今天每天的数量（含 0）；`from` 最多一年前、不能晚于今天 |
| `GET /api/jottings/recall` | 今天的回顾卡片组：今天零点以前创建的札记按 `SHA-256("YYYY-MM-DD" ‖ id)` 排序取前 5 张；不保存任何状态，候选不变时同一天结果不变 |

日期都按 `PALACE_TIMEZONE` 计算。未配置 `PALACE_JOTTINGS_DIR` 时三个接口都返回 `503 jottings_unavailable`。

## Web 回响页

`/jottings` 是加宽的页面：左列是今日回响、统计、12 周热力图和前 8 个标签，右侧是瀑布流。

- 点卡片组把它平铺到瀑布流上方，点别处收起；平铺后点一张卡片放大阅读，关闭即算读过，离开卡片组。已读只记在页面里，刷新后整组恢复。
- 有标签筛选或搜索时卡片组收起。点任意位置的标签按它筛选，再点一次清除；“全部标签”把右侧换成全部标签。
- `n` 新建札记：正文里空白或行首之后的 `#词` 提取为标签并从正文移除，代码块里的不算；Ctrl/Cmd + Enter 保存，
  写入 `inbox/`，所以新卡片带待整理圆点。有内容的草稿不会因 Escape 或失焦丢失。
- `/` 搜索，只在按 Enter 时执行；Escape 先清空再收起。页面上的 Escape 依次退出放大的卡片、平铺的卡片组、全部标签、标签筛选。

## 运行条件

见[环境变量](../../server/环境变量.md#札记)。
