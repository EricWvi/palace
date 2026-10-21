# 标注

在文章正文和对话消息上留下高亮、划线和批注。决策见[摘星标注 ADR](../../../specs/decisions/web/stars/20261021-annotations-on-articles-and-messages.md)，交互原型见 [`design/3.3-stars-annotation.html`](../../../design/3.3-stars-annotation.html)（变体 A）。笔记不能标注：它在 Palace 之外被改写，锚点会失效。

## 一处标注

- **锚点**：渲染后文字中的 `[start, end)`（UTF-16 下标）和当时选中的原文 `quote`。坐标系是 `MarkdownContent` 渲染出的全部文本节点首尾相接；表格结构元素之间的空白、复制按钮、图片占位和折叠行不计入。文章取去掉开头标题后的正文，对话每条消息各自从 0 算起。
- **样式**：`yellow`、`green`、`blue`、`rose` 四种高亮，`underline` 划线，`wavy` 波浪线，或没有。
- **批注**：一段纯文本，或没有。样式和批注至少有一个；只有批注时显示为淡强调色底加细线。
- 展示时，`[start, end)` 处的文字等于 `quote` 就画在原处；否则画在离 `start` 最近的 `quote` 出现处；都找不到就不画，数据仍然保留。
- 文章、对话、Path 删除时，标注在同一事务里随被删的文字一起删除。

## 接口

| 方法与路径 | 作用 |
| --- | --- |
| `GET /api/articles/{id}/annotations` | 文章的全部标注，按创建顺序 |
| `POST /api/articles/{id}/annotations` | 请求体 `{ start, end, quote, style, note }`，返回 201 |
| `GET /api/conversations/{id}/annotations` | 对话所有消息上的标注，不论属于哪个 Path |
| `POST /api/conversations/{id}/annotations` | 请求体多一个 `message_id` |
| `PUT /api/annotations/{id}` | 请求体 `{ style, note }`，一起替换；两项都为 `null` 返回 400 |
| `DELETE /api/annotations/{id}` | 删除，返回 204 |

`quote` 的 UTF-16 长度必须等于 `end - start`。其他 Owner 的文章、对话、消息或标注都返回 404。字段的完整定义见 [OpenAPI](../../../contracts/openapi.json)。

## 阅读页

- 选中文字后，选区上方出现小条：四个色点、划线、波浪线、「批注」。快捷键 `1`–`4`（`h` 同 `1`）、`u`、`w`、`c`，只在小条显示时有效。
- 点击标记弹出同一个小条，可以换样式、写或改批注、删除（`Delete`）。再点一次当前样式会去掉它；没有批注时整处删除。
- 窗口至少 1180px 宽、且有带批注的标注时，页面像回响页那样加宽，批注卡片排在正文右侧，与原文对齐；没有批注时页面与其他阅读页同宽。窗口更窄时，悬停标记，批注浮在原文下方。
- 悬停标记或批注卡片，两边一起加深。点击卡片原地编辑，离开、`Esc` 或 `⌘/Ctrl+Enter` 保存；清空批注等于去掉批注。
- 标题旁 `···` 菜单的「只看标注」（快捷键 `a`）把没有标注的段落、列表项和对话消息折叠成「省略 N 段」一类的行，点击可单独展开；meta 行的「显示全文」、`a` 或 `Esc` 退出。

## 工程入口

- 数据库：`crates/db/migrations/0018_annotations.sql`、`crates/db/src/annotation.rs`；校验：`crates/domain/src/annotation.rs`。
- 前端：`apps/palace-web/src/lib/annotation-paint.ts`（rehype 绘制与折叠）、`lib/annotation-dom.ts`（选区换算锚点）、`components/annotation-layer.tsx`（数据与状态）、`components/annotated-area.tsx`（小条、页边栏、浮层、快捷键）、`annotations.css`。
