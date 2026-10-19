/*
 * Sample 札记 for the 4.x 回响 prototypes.
 *
 * Recent entries are placed relative to the moment the page opens, so the
 * "within 7 days shows relative time" rule always has cards to act on.
 * Older entries carry fixed local times. `pending` marks an upload still
 * sitting in inbox/, waiting for the agent to sort it.
 */
(() => {
  const pad = (n, width = 2) => String(n).padStart(width, "0");

  // Local "YYYY-MM-DDTHH:MM" string, the same shape the fixed entries use.
  const local = (date) =>
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;

  const minutesAgo = (minutes) => local(new Date(Date.now() - minutes * 60_000));

  // A clock time some days back; never lands in the future for days = 0.
  const daysAgo = (days, time) => {
    const [h, m] = time.split(":").map(Number);
    const date = new Date();
    date.setDate(date.getDate() - days);
    date.setHours(h, m, 0, 0);
    return local(date > new Date() ? new Date(Date.now() - 90 * 60_000) : date);
  };

  const entries = [
    { created: minutesAgo(12), tags: ["想法", "产品", "设计", "札记"], pending: true, body: "地铁上突然想到：回响页的回顾区，像是从抽屉里随手摸出几张旧卡片。" },
    { created: minutesAgo(140), tags: ["生活"], pending: true, body: "楼下的桂花开到尾声了，香味淡了很多，反而更好闻。" },
    { created: daysAgo(1, "22:40"), title: "睡前的三件事", tags: ["生活", "工作流", "睡眠", "习惯", "阅读"], pending: true, body: "- 把明天最重要的一件事写在纸上\n- 手机放到客厅充电\n- 读十页纸质书\n\n坚持了一周，入睡明显快了。" },
    { created: daysAgo(1, "09:15"), tags: ["rust", "并发", "所有权"], body: "`Arc<Mutex<T>>` 写多了要警惕：多半是所有权没想清楚，把问题推给了运行时。" },
    { created: daysAgo(2, "21:05"), title: "commit 是“写完了”的边界", tags: ["札记", "工作流"], body: "札记写到一半也没关系，只要不 commit，就不会被同步出去。\n\n把 commit 当作“这一张我认可了”的仪式，比自动保存更让人安心。" },
    { created: daysAgo(3, "23:02"), tags: ["阅读", "卡片笔记"], body: "> 每张卡片只写一个想法。\n\n短卡片的价值不在单张，而在它们之间的连接。今天写下的零碎念头，可能在三个月后和另一张卡片撞出新东西。" },
    { created: daysAgo(4, "19:30"), title: "夜跑后的一点念头", tags: ["跑步", "生活"], body: "秋天的夜风已经有点凉了。跑完步脑子格外清楚，适合把白天没想通的事情重新捋一遍。" },
    { created: daysAgo(6, "14:20"), tags: ["设计", "产品", "极简", "想法"], body: "界面上每多一个按钮，都是在替用户做一次“你可能需要它”的假设。自己用的工具，可以大胆地少。" },
    { created: daysAgo(6, "08:05"), tags: ["音乐"], body: "早上循环了一遍坂本龙一的 *Aqua*，钢琴声像冷水洗脸。" },

    { created: "2026-10-09T22:18", title: "重读《禅与摩托车维修艺术》", tags: ["阅读", "哲学", "写代码", "良质", "重读"], body: "第二次读，注意力完全不同了。第一次被公路旅行和父子关系吸引，这次停在“良质”那几章反复看。\n\n作者说，良质不是主观的也不是客观的，它是主客体相遇的那个瞬间。修摩托车时，如果你只把它当成一堆零件，就永远修不好；你得先“在乎”它。\n\n这让我想到写代码：同一个需求，带着在乎去写和带着应付去写，结果看起来差不多，可半年后再读，差别一目了然。\n\n- 卡住的时候不要硬推，先回到“我在乎什么”\n- 枯燥是警报，说明你和手上的事已经脱节\n- 工具的状态反映人的状态" },
    { created: "2026-10-06T07:40", tags: ["跑步", "早起", "生活"], body: "第一次在 5 点 50 分起床跑步。天还没亮，路灯下只有扫地的阿姨和我。" },
    { created: "2026-10-03T16:12", tags: ["家人"], body: "妈妈学会了用语音转文字，今天给我发了一大段，标点全是逗号，读起来像一口气说完的。" },
    { created: "2026-09-28T21:44", title: "关于“够用就好”", tags: ["想法", "产品"], body: "很多软件死于功能太多，而不是太少。\n\nflomo 最打动我的不是某个功能，而是它拒绝了很多功能：没有文件夹，没有富文本，没有协作。它把“随手记”这一件事做得几乎没有阻力。" },
    { created: "2026-09-25T12:30", tags: ["rust", "摘抄"], body: "> Make illegal states unrepresentable.\n\n用枚举描述状态机，比一堆 `Option` 字段诚实得多。" },
    { created: "2026-09-20T10:05", tags: ["生活"], body: "阳台的薄荷又活过来了。原来它不是死了，只是冬天在休息。" },
    { created: "2026-09-16T23:30", tags: ["写作"], body: "写不出来的时候，先写“我现在写不出来，因为……”。通常写到第三行，真正想说的话就出来了。" },
    { created: "2026-09-12T18:48", title: "装修邻居", tags: ["生活"], body: "楼上装修第三周。电钻声准时在早上八点响起，比闹钟还准。我开始用它当番茄钟：钻一段，停一段，我也写一段，停一段。" },
    { created: "2026-09-07T15:22", tags: ["设计", "阅读"], body: "《设计心理学》里说，好的设计让人不需要说明书。可我觉得更高一层是：让人用完之后忘记自己用过它。" },
    { created: "2026-09-01T09:00", tags: ["工作流"], body: "九月的第一天，清空了浏览器里所有的标签页。一百多个“以后再看”，其实以后永远不会看。" },
    { created: "2026-08-27T20:15", title: "Git 作为笔记的底座", tags: ["工作流", "札记", "Git", "Palace"], body: "把札记放进 Git 仓库的好处，比我想的多：\n\n1. 任何编辑器、任何 Agent 都能直接改文件\n2. 历史天然存在，不需要额外的版本功能\n3. commit 就是“写完了”的信号\n4. 备份就是 push\n\n代价是手机上没法直接写，只好交给 App 离线上传，再由 Agent 整理进仓库。这个分工我挺满意：手机负责捕捉，电脑负责整理。\n\n唯一的担心是 frontmatter 写错导致整个 commit 被拒，所以 pre-commit hook 必须有。" },
    { created: "2026-08-21T22:50", tags: ["音乐", "雨天", "钢琴", "夜晚", "Nils Frahm"], body: "雨夜听 Nils Frahm 的 *Says*，八分多钟，最后那段合成器升起来的时候，窗外刚好打了个雷。" },
    { created: "2026-08-15T07:10", tags: ["跑步"], body: "半马 1 小时 52 分，比去年快了六分钟。最后两公里没有看表，只听自己的呼吸。" },
    { created: "2026-08-09T13:35", tags: ["想法"], body: "“忙”是一种很方便的借口，因为它不需要解释具体忙什么。" },
    { created: "2026-08-02T19:20", title: "外婆的菜谱", tags: ["家人", "生活"], body: "整理旧物翻出外婆手写的菜谱本。红烧肉那一页写着“糖多一点，火小一点，心静一点”。\n\n前两条是做菜，第三条大概是她自己的人生。" },
    { created: "2026-07-26T11:11", tags: ["rust"], body: "生命周期标注的报错，十次有八次是在提醒：这个引用活得比你以为的长。" },
    { created: "2026-07-19T21:00", tags: ["阅读", "摘抄"], body: "> 我们读书，是为了知道自己并不孤独。\n\n——《影子大地》" },
    { created: "2026-07-12T16:40", tags: ["设计"], body: "暖灰比冷灰更适合长时间阅读的界面。所有中性色都放在同一个偏黄的色相上，浅色和深色就不会打架。" },
    { created: "2026-07-05T08:25", tags: ["生活"], body: "今天第一次自己磨咖啡豆。磨得太细，萃出来又苦又涩，但闻起来是真的香。" },
    { created: "2026-06-28T23:15", title: "关于记录的焦虑", tags: ["写作", "想法"], body: "有段时间我什么都想记：读过的每篇文章、听过的每首歌、每天的步数。结果是记录本身变成了负担，真正有意思的念头反而被淹没了。\n\n现在只记两种东西：\n\n- 让我停下来想了一下的\n- 想在一年后再看到的\n\n其他的，让它们自然流走。忘掉也是一种整理。" },
    { created: "2026-06-21T17:30", tags: ["跑步", "生活"], body: "夏至。傍晚七点半天还亮着，沿河跑了十公里，看着太阳一点点落进楼群里。" },
    { created: "2026-06-14T10:45", tags: ["产品"], body: "做工具最大的陷阱：把自己想象成别人。我是唯一的用户，那就只为我做。" },
    { created: "2026-06-07T20:05", tags: ["音乐"], body: "久石让《Summer》的前奏一响，夏天就正式开始了。" },
    { created: "2026-05-31T14:00", tags: ["阅读"], body: "读完《置身事内》。地方政府的账本比我想象的复杂得多，很多新闻突然都看懂了。" },
    { created: "2026-05-24T09:30", title: "早晨的第一个小时", tags: ["工作流"], body: "试了一个月：起床后的第一个小时不看手机。\n\n变化最大的不是效率，而是情绪。以前一睁眼就被消息推着走，一整天都像在追赶；现在早上先写一页字、喝一杯水、看看窗外，再去面对世界，整个人从容很多。" },
    { created: "2026-05-17T22:22", tags: ["想法"], body: "好的问题比好的答案更稀缺。" },
    { created: "2026-05-10T15:15", tags: ["家人"], body: "母亲节，给妈妈打了视频。她举着手机给我看阳台上开的月季，镜头一直在晃，可我看得很开心。" },
    { created: "2026-05-02T18:00", tags: ["生活"], body: "五一没出门。把书架从头到尾整理了一遍，送走了三十多本不会再读的书。" },
    { created: "2026-04-25T21:35", tags: ["rust", "工作流"], body: "`cargo clippy` 的建议十条里有九条是对的，剩下一条是我还没理解它为什么对。" },
    { created: "2026-04-18T07:50", tags: ["跑步"], body: "跑步时数自己的呼吸：吸两步，呼两步。数着数着，脑子里的杂音就没了。" },
    { created: "2026-04-11T13:20", title: "春天的清单", tags: ["生活"], body: "- 去植物园看樱花\n- 换掉冬天的被子\n- 给自行车打气\n- 学会做一道新菜" },
    { created: "2026-04-04T16:30", tags: ["阅读", "写作"], body: "村上春树说他每天写十页稿纸，不多也不少。写得顺的时候也停下来，留一点给明天。" },
    { created: "2026-03-28T22:00", tags: ["设计"], body: "动画的意义不是好看，是告诉人“东西从哪里来、到哪里去”。没有方向的动画都是噪音。" },
    { created: "2026-03-21T10:10", tags: ["想法"], body: "春分。昼夜一样长的这一天，适合想想自己在做的事是不是还值得。" },
    { created: "2026-03-14T09:20", title: "第一次修自行车", tags: ["生活"], body: "链条掉了，对着视频折腾了四十分钟。手全是黑油，最后装上的那一刻，莫名有种成就感。" },
    { created: "2026-03-07T19:45", tags: ["音乐", "摘抄"], body: "> Music is the space between the notes.\n\n——德彪西" },
    { created: "2026-02-28T23:10", title: "二月的总结", tags: ["写作", "工作流"], body: "这个月写了 31 张札记，比一月多了一倍。原因很简单：输入框打开得更快了。\n\n阻力每少一秒，记录就多一点。这比任何“养成习惯”的方法都有效。\n\n下个月想试试每周日晚上把一周的札记过一遍，挑出值得展开的写成长文。不求多，一篇就好。" },
    { created: "2026-02-17T12:00", tags: ["家人", "生活"], body: "除夕。一家人包饺子，爸爸包的饺子永远立不起来，被大家笑了十几年。" },
    { created: "2026-02-08T21:30", tags: ["阅读"], body: "《小王子》每隔几年读一次，每次都在不同的地方哭。" },
    { created: "2026-01-30T08:40", tags: ["跑步"], body: "零下三度晨跑，睫毛上结了一层霜。" },
    { created: "2026-01-21T20:20", tags: ["rust"], body: "第一次写出能编译通过的异步 trait。等了好几年的语言特性，用起来却平淡得像它本来就该在那里。" },
    { created: "2026-01-12T15:50", tags: ["产品", "想法"], body: "离线优先不是技术选择，是对用户的尊重：你的东西，在没网的时候也是你的。" },
    { created: "2026-01-01T00:15", title: "新年第一张", tags: ["想法"], body: "不定目标了。只想做三件事：多走路，多读纸质书，多和家人说话。" },
    { created: "2025-12-24T22:00", tags: ["音乐"], body: "平安夜，单曲循环 *Merry Christmas Mr. Lawrence*。" },
    { created: "2025-12-15T18:30", tags: ["生活"], body: "第一场雪。路上的人都放慢了脚步，好像怕把雪踩疼。" },
    { created: "2025-12-06T10:25", title: "为什么要自己做笔记软件", tags: ["产品", "工作流", "Palace", "工具"], body: "试过 Notion、Obsidian、flomo、Bear，每一个都很好，但每一个都在某个地方让我别扭。\n\nNotion 太重，打开一个页面要等；Obsidian 插件太多，折腾比记录还多；flomo 数据不在自己手里；Bear 只在苹果设备上。\n\n最后发现，我要的其实很简单：纯文本、在自己的 Git 仓库里、手机能离线记、电脑能用任何工具改。既然没有现成的，那就自己做一个。\n\n这大概就是 Palace 的起点。" },
    { created: "2025-11-27T21:40", tags: ["阅读", "摘抄"], body: "> 慢慢来，比较快。" },
    { created: "2025-11-18T07:30", tags: ["跑步"], body: "换了新跑鞋，脚感软得像踩在面包上。" },
    { created: "2025-11-09T16:00", tags: ["设计"], body: "宋体在小字号下笔画会碎，所以界面控件用黑体，只有展示性的大字用宋体。" },
    { created: "2025-11-02T11:45", tags: ["生活", "家人"], body: "陪爸爸去钓鱼。一下午一条没钓到，他说钓鱼本来就不是为了鱼。" },
  ];

  // Stand-in UUIDv7-shaped ids: unique and stable across reloads, which is all the prototype needs.
  globalThis.echoData = {
    jottings: entries.map((entry, index) => ({
      id: `019a${pad(index, 4)}-0000-7000-8000-${pad(index, 12)}`,
      title: null,
      pending: false,
      ...entry,
    })),
  };
})();
