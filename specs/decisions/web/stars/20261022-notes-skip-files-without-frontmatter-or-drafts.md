---
status: implemented
date: 2026-10-22
---

# 笔记文件夹中没有 frontmatter 或 `draft: true` 的 Markdown 不是笔记：cache 脚本跳过并列出，而不是报错

笔记文件夹里除了笔记，还有 README、模板这类辅助文件，它们的文件名不固定。cache 脚本以前把每个 `.md` 都当成笔记，这类文件会让整次生成失败。现在规则改为：没有 frontmatter 的文件不是笔记；frontmatter 中 `draft: true` 的文件也不是笔记，而且不再校验其他字段。两类文件都会被静默跳过，每次运行只报告跳过的数量。只要文件有 frontmatter 且不是草稿，就仍按原规则严格校验，任何一篇不合格都会让整次运行失败。

当前为 `implemented`，核心测试用例见[摘星笔记](../../../test-cases/web/stars/notes.md#cache-script-must-enumerate-the-files-search-sees-and-reject-invalid-frontmatter)。本决策基于[笔记来自挂载的 Markdown 文件夹](20261014-notes-from-mounted-folder.md)，只改动 `scripts/notes-cache.ts` 判定“哪些文件是笔记”的规则。cache 格式、服务端、接口与页面都不变。

## 问题与约束

- 面试准备之类的笔记目录会有 `README.md`，作为目录说明或索引；还会有 `case-template.md` 这样的模板，供复制后填写。它们不该出现在笔记列表中，但文件名没有固定的模式。
- 用 `.ignore` 排除需要逐个列出路径或命名模式。新增辅助文件时就得同步规则；文件改名或移动后，规则也会悄悄失效。
- 模板常带 frontmatter 骨架，例如 `title: <题目>`、`date: YYYY-MM-DD`，占位值本身通不过校验。
- 原决策要求“写错的笔记必须被发现”，提交钩子会因此拦下提交。放宽规则不能让一篇写错的笔记悄悄消失。

## 继承与修改

| 前序约定 | 本决定 |
| --- | --- |
| D4：每篇笔记的 frontmatter 必须包含 `title`、`date` | 修改：先判断文件是不是笔记，再对笔记做原有校验；没有 frontmatter 或 `draft: true` 的文件不是笔记，见 D1 |
| D5：脚本用 rg 枚举 `*.md`，`.gitignore`、`.ignore` 与隐藏文件规则只有 ripgrep 一种解释 | 继承，不变；被 ignore 的文件不读取，也不计入跳过数量 |
| D5：任何一篇不合格时报告所有问题、非零退出、不写 cache | 继承；“不合格”现在只针对笔记，以及非法的 `draft` 值，见 D1 |
| D5：cache 是 `{ version: 1, notes: [{ id, title, date }] }` | 继承，不变；被跳过的文件不进入 cache，cache 不记录跳过信息 |
| D6：搜索结果只保留当前索引中存在的笔记 | 继承，不变；rg 会扫到被跳过的文件，但它们不在索引中，不会出现在结果里 |
| D7：笔记页只读取索引中存在的 ID | 继承，不变；被跳过的文件无法通过 `/notes/<id>` 打开 |

## D1：哪些 Markdown 是笔记

| 文件 | 结果 |
| --- | --- |
| 第一行不是 `---`（没有 frontmatter） | 跳过 |
| frontmatter 中 `draft: true` | 跳过，`title`、`date` 不校验 |
| frontmatter 中 `draft` 是 `false` 或缺省 | 笔记，按原 D4 校验 `title`、`date` |
| frontmatter 中 `draft` 是其他值（如 `yes`、`"true"`） | 不合格：`draft 必须是 true 或 false` |
| 以 `---` 开头但没有结束行，或 YAML 无效、不是键值对 | 不合格，与原规则相同 |

- `draft` 按 YAML core schema 解析，只有布尔值 `true` 表示草稿。`yes`、`on` 与带引号的 `"true"` 都是字符串，会被报错而不是被猜测，避免一篇笔记因为写法不同被意外隐藏或意外发布。
- 草稿不校验其他字段：模板里的占位值就是它不是笔记的原因，校验只会制造噪音。
- 一个文件只要以 `---` 开头，就被当作“打算写 frontmatter”，后续格式错误都会报错。只有完全没有 frontmatter 的文件才被视为“不是笔记”。

**为什么按内容而不是按文件名**：是否发布由文件自身决定，改名、移动后依然成立，新增 README 或模板不需要维护任何规则。没有 frontmatter 是 README 一类文件的自然状态；`draft: true` 是常见静态站点生成器的通用写法，也适用于写了一半、暂不发布的笔记。

## D2：只报告跳过的数量

- 成功时，脚本只输出一行 `已写入 N 篇笔记到 .palace-cache.json，跳过 M 个文件`，不逐个列出被跳过的文件。
- 失败时只报告问题，与原规则相同。

**为什么**：README 与模板会随着笔记目录增多而越来越多，每次运行都逐个列出，只会把真正需要关注的输出淹没。代价是，忘了写 frontmatter 的新笔记不会拦下提交，只会在 Palace 中缺席，输出里的跳过数量会多 1；Eric 接受这一点，发现笔记缺席时补上 frontmatter 即可。

## 不变量

1. cache 恰好包含 rg 枚举到、有 frontmatter 且 `draft` 不为 `true` 的 Markdown 文件。
2. 没有 frontmatter 或 `draft: true` 的文件永远不会让脚本失败，也永远不会进入 cache。
3. 成功输出报告的跳过数量等于被跳过的文件数，不逐个列出文件。
4. `draft` 为布尔值以外的任何值时，脚本失败且不改动原有 cache。

## 为什么不是这些替代方案

| 替代方案 | 优势 | 取舍 |
| --- | --- | --- |
| 只用 `.ignore` 排除 | 不改代码；rg 搜索也不再扫描这些文件 | 文件名不固定，需要逐个列出；规则与文件分离，改名或移动后会失效 |
| 加 `--skip-invalid` 参数，容忍不合格的笔记 | 一次解决所有“跑不过”的情况 | 无法区分“不是笔记”和“写错的笔记”，写错的笔记会悄悄消失，违背原 D5 的严格校验 |
| 只认 `draft: true`，没有 frontmatter 仍报错 | 忘写 frontmatter 时会拦下提交 | 每个 README 都要为了“不是笔记”而加一段 frontmatter，与辅助文件的自然形态相悖 |
| 用 Palace 专用字段 `palace: false` | 语义只针对 Palace | `draft` 更通用，同时覆盖模板和未完成的笔记 |
| 在 cache 中记录被跳过的文件 | 服务端可以展示或诊断 | 服务端不需要；cache 格式与服务端校验都要改动，没有使用场景 |

## 落地与验收

- `scripts/notes-cache.ts` 按 D1 判定，并按 D2 只输出跳过数量。`scripts/notes-cache.test.ts` 用真实 ripgrep 覆盖 README、带占位值的草稿模板、`draft: false` 的笔记，以及非法的 `draft` 值。
- `docs/features/notes/README.md` 与 `docs/server/环境变量.md` 同步说明。

服务端、接口、cache 格式与页面行为保持不变。
