# 设计系统

这里存放 Palace 的设计 token 和 HTML 原型。原型直接在浏览器里打开（`file://`），不需要构建，也不需要起本地服务。

## 原型目录

| 文件 | 内容 |
| --- | --- |
| [0-palette.html](0-palette.html) | 色板：浅色与深色主题的 token、对比度和界面样例 |
| [1-prototype.html](1-prototype.html) | 时间线：按天浏览 Moment，卡片原地展开详情 |
| [2.1-moment-conversation.html](2.1-moment-conversation.html) | 摘星中的对话阅读页：从时间线的对话标题进入，含分支切换与返回当天 |

## 共享代码

页面依次引入以下文件，后面的文件依赖前面的：

| 文件 | 内容 |
| --- | --- |
| [tokens.css](tokens.css) | 颜色、字体、字号、圆角等变量，以及浅色/深色主题 |
| [base.css](base.css) | reset 和 `body` 默认样式，所有页面都引入 |
| [components.css](components.css) | 多个原型共用的应用组件：页面容器 `.sheet`、页头、导航、元信息行 `.meta` |
| [ornament.js](ornament.js) | 原型和 React 共用的装饰 SVG 路径与 viewBox；[ornament.d.ts](ornament.d.ts) 提供 TypeScript 类型 |
| [shared.js](shared.js) | 共用的 HTML 结构，目前提供 `<palace-header>` |

新原型的 `<head>` 按这个顺序引入（只用到 token 的页面，比如色板，可以只引入前两个）：

```html
<link rel="stylesheet" href="tokens.css">
<link rel="stylesheet" href="base.css">
<link rel="stylesheet" href="components.css">
<script src="ornament.js"></script>
<script src="shared.js"></script>
```

页头用一个标签，`active` 填当前导航项：

```html
<palace-header active="时刻"></palace-header>
```

## 约定

- **命名**：文件名以编号开头，同一主题的多个原型用小数编号，例如 `2.1-moment-conversation.html`、`2.2-…`。
- **只用 token**：颜色、字体、字号、圆角一律用 `tokens.css` 里的变量，不写字面值。确实缺一档时，先在 `tokens.css` 里加 token。
- **token 也供应用使用**：`apps/palace-web` 直接导入这份 `tokens.css`（见[组件与样式](../docs/frontend/组件与样式.md#token-只有一份)），改动它会同时影响线上页面，提交前要一并检查应用。
- **晚一点再共享**：样式或结构先写在原型自己的页面里，等第二个原型也需要时，再挪进 `components.css` 或 `shared.js`。改共享文件前，先确认引用它的原型都没被改坏。
- **不用 ES module 和 `fetch`**：浏览器在 `file://` 下会拦截它们。共享脚本用普通 `<script src>`，共享结构用自定义元素。
- **装饰只有一份**：路径和 viewBox 只维护 `ornament.js` 中的 `globalThis.palaceOrnament`，原型先加载它再加载 `shared.js`，React 通过副作用导入读取同一份只读数据。
- **SVG 内联**：`shared.js` 和 React 使用共享数据渲染内联 SVG，不要用 `<img>` 或外部 `<use href>` 引用，否则无法通过 CSS 变量控制颜色，`file://` 下外部引用也会被拦截。
- **更新目录**：新增或删除原型时，同步修改上面的原型目录。
