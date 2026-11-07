---
status: implemented
date: 2026-11-07
---

# inbox 里的札记不参与回顾

文件还在 `inbox/` 的札记没有整理过，不算回顾的内容：不进卡片组，放大后也没有「束阁」「启封」。整理出 `inbox/` 时才算进入回顾。

当前为 `implemented`，核心测试用例见[札记读取接口](../../../test-cases/web/echo/jotting-reading.md#inbox-jottings-must-take-no-part-in-recall)和[回响页](../../../test-cases/web/echo/echo-page.md#a-zoomed-jotting-must-say-whether-it-takes-part-in-recall)。继承[回顾按完成次数最少者优先抽取](20261101-recall-least-completed-first-from-a-stored-daily-pile.md)和[放大卡片里原地编辑和删除札记](20261101-edit-and-delete-a-zoomed-jotting-with-title-slips.md)，修改下表所列部分。

## 继承与修改

| 前序约定 | 本决定 |
| --- | --- |
| 回顾决策：候选是今天零点以前创建、参与回顾、不是上一组未完成的札记 | 修改：另须不在 `inbox/`，见 D1 |
| 回顾决策 D2：札记出现时次数为当前最小值，最小值取所有“计数”行 | 修改：最小值不计 `inbox/`，移出 `inbox/` 也算出现，见 D2 |
| 回顾决策：列表的 `recall` 为 `included` 或 `excluded` | 修改：增加 `inbox`，见 D3 |
| 编辑决策：瀑布流题签是「修订」「束阁 / 启封」「削简」 | 修改：`inbox/` 里的札记只有「修订」「削简」 |

## D1：抽卡与卡片组

抽卡的候选不含路径在 `inbox/` 下的札记。已保存的卡片组也不再显示移入 `inbox/` 的札记，所以卡片组上不会出现未整理的札记，张数可能因此少于抽取时。

## D2：完成次数

`inbox/` 里的札记不被抽到，次数不会增长；如果仍计入最小值，只要一张札记留在 `inbox/`，最小值就不再上升，此后的新札记全都从过低的次数开始。所以最小值只取已整理的札记。

移出 `inbox/` 时，次数重设为当前最小值，与新札记相同，否则在 `inbox/` 里等得久的札记整理后会连续几天占据卡片组。束阁的札记保持束阁。规则放在 `jotting` 表的触发器里，理由与回顾决策 D2 相同：对账和上传都不需要记得维护它。

## D3：判定用路径，不用 `pending`

`pending` 同时表示“在 `inbox/`”和“有网页修改还没 commit”，后者是已经整理过的札记，仍然参与回顾。所以服务端按路径判定，在 `recall` 里返回 `inbox`，前端不需要了解 `pending` 的构成。

`PUT /api/jottings/{id}/recall` 和完成接口的取值仍然只有 `included` 与 `excluded`：`inbox` 由文件位置决定，不能设置。

## 不变量

1. 抽卡结果和接口返回的卡片组都不含路径在 `inbox/` 下的札记。
2. `recall` 为 `inbox` 的札记放大后没有「束阁」「启封」。
3. 当前最小值不受 `inbox/` 里的札记影响。
4. 札记移出 `inbox/` 后处于完成次数最少的那一层，除非它已被束阁。
