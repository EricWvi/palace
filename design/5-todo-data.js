/*
 * Sample lists and tasks for the 5.x 行事 prototypes, drawn from the legacy
 * Dashboard's todo module.
 *
 * Schedules are placed relative to the day the page opens, so 今日 always has
 * tasks. `schedule` is null (unscheduled), "shelved" (set aside: never
 * offered when planning a day) or a local "YYYY-MM-DD". Shelved is a state
 * of its own, not the far-future date (2096-10-02) the legacy Dashboard used:
 * a sentinel date would sort and compare as a real plan wherever dates are
 * read. A past date counts as unscheduled. `count` is how many days the task was marked done. List 0
 * is the inbox, the buffer for tasks whose list is not decided yet; it is
 * never planned from. Rows carry no order: the page gives each list
 * fractional-index keys in the order the rows appear here.
 */
(() => {
  const pad = (n) => String(n).padStart(2, "0");
  const day = (offset) => {
    const date = new Date();
    date.setDate(date.getDate() + offset);
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  };

  const lists = [
    { id: 0, name: "收件箱" },
    { id: 1, name: "工作" },
    { id: 2, name: "个人" },
    { id: 3, name: "甜点清单" },
    { id: 4, name: "恢复精力" },
    { id: 5, name: "英语" },
    { id: 6, name: "清理" },
    { id: 7, name: "通勤饭后" },
    { id: 8, name: "娱乐" },
    { id: 9, name: "未来" },
  ];

  let id = 0;
  const task = (list, title, extra = {}) => ({
    id: ++id,
    list,
    title,
    schedule: null,
    done: false,
    count: 0,
    link: "",
    draft: "",
    completedAt: null,
    ...extra,
  });

  const todos = [
    task(0, "看一下 Tailscale 的 ACL 写法"),
    task(0, "回复房东关于暖气的消息"),
    task(0, "整理下载文件夹"),

    task(1, "ai infra", { schedule: day(0), done: true, count: 31, draft: "本周：\n- 调度器的抢占策略\n- 推理服务的冷启动" }),
    task(1, "ora", { schedule: day(0), done: true, count: 24, draft: "Controller 与 Node 的结果接管边界。" }),
    task(1, "工作勤喝水，同时思考当前事项提效", { count: 12 }),
    task(1, "rust 博客", { count: 3, link: "https://blog.rust-lang.org/" }),
    task(1, "季度复盘材料", { schedule: day(2), count: 1 }),

    task(2, "arcade", { count: 6, draft: "像素风小游戏合集。" }),
    task(2, "研究 matt skill", { schedule: day(0), count: 4 }),
    task(2, "openai claude 博客 一篇一篇往前读，在 ai 帮助下", { count: 9 }),
    task(2, "（占位）", { schedule: "shelved" }),
    task(2, "肚子痉挛，之前冬天早上起床刷牙有时候胸部肌肉痉挛，背部肌肉痉挛，都是遇冷", { count: 1 }),
    task(2, "不要毫无规划直接进入大段时间", { count: 7 }),
    task(2, "ai 方法论", { schedule: "shelved", draft: "先问清楚问题，再让 AI 动手。" }),
    task(2, "博客汇总", { schedule: day(0), count: 15, draft: "按主题汇总读过的博客。" }),
    task(2, "palace", { schedule: day(0), count: 40, draft: "行事页原型。" }),
    task(2, "2245 准备睡觉，2300 睡觉", { count: 18 }),
    task(2, "看看拉伸视频，研究大腿肌肉不舒服的地方，两大腿内侧", { count: 2, link: "https://www.youtube.com/" }),
    task(2, "时间片使得计算机伟大；人亦然；把任务拆细；剩下的交给调度", { count: 5 }),

    task(3, "Manga: 游戏王", { count: 2 }),
    task(3, "《葬送的芙莉莲》第二季", { schedule: day(4) }),

    task(4, "通勤路上可以按摩脖子 斜方", { count: 11 }),
    task(4, "问问 ai 劳损 怎么才是真正的放松", { count: 1 }),

    task(5, "每日一篇 The Economist 短文", { count: 20, link: "https://www.economist.com/" }),

    task(6, "清理旧的 Docker 镜像", { schedule: "shelved" }),

    task(7, "饭后散步十五分钟", { count: 14 }),

    task(9, "学一门乐器"),

    task(2, "subhub", { count: 5, completedAt: "2026-09-02T22:52", draft: "订阅聚合。" }),
    task(2, "洗澡后养成看漫画看书的习惯", { count: 0, completedAt: "2026-08-09T11:48" }),
    task(2, "程序员的自我修养 链接 装载 与库", { count: 0, completedAt: "2026-08-09T11:46" }),
    task(2, "rustlings", { count: 0, completedAt: "2026-08-09T11:42", link: "https://github.com/rust-lang/rustlings" }),
    task(2, "agent 包装系统", { count: 0, completedAt: "2026-06-07T16:55", draft: "……" }),
    task(2, "wireguard", { count: 0, completedAt: "2026-04-09T22:58" }),
    task(2, "journal", { count: 28, completedAt: "2026-01-02T12:19", draft: "……" }),
    task(1, "迁移监控告警", { count: 3, completedAt: "2026-09-18T18:30" }),
  ];

  globalThis.todoData = { lists, todos };
})();
