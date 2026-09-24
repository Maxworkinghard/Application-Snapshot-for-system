/** 常见动作名的中文对照；素材包命名五花八门，命中不了就退回原名 */
const ACTION_NAMES: Record<string, string> = {
  idle: "待机",
  sleep: "打盹",
  sleeping: "打盹",
  dance: "跳舞",
  dancing: "跳舞",
  think: "思考",
  thinking: "思考",
  type: "敲键盘",
  typing: "敲键盘",
  "type-keyboard": "敲键盘",
  keyboard: "敲键盘",
  build: "搭建",
  carry: "搬运",
  cheer: "欢呼",
  crabwalk: "横着走",
  error: "报错",
  jump: "跳跃",
  "jump-happy": "开心跳",
  point: "指路",
  sweep: "打扫",
  listening: "聆听",
  listen: "聆听",
  "xmas-idle": "圣诞待机",
  xmas: "圣诞",
  greeting: "挥爪问候",
  greet: "挥爪问候",
  wave: "招手",
  snuggle: "撒娇",
  walk: "走动",
  run: "跑动",
  eat: "进食",
  work: "工作",
  happy: "开心",
  sad: "难过",
  angry: "生气",
  surprised: "惊讶",
  love: "卖萌",
  hello: "打招呼",
  bye: "告别",
  "type-laptop": "敲笔记本",
  laptop: "笔记本",
  read: "阅读",
  write: "书写",
  code: "写代码",
  coding: "写代码",
  search: "搜索",
  wait: "等待",
  waiting: "等待",
  alert: "警告",
  warning: "警告",
  success: "成功",
  done: "完成",
  loading: "加载中",
  drag: "拖拽",
  click: "点击",
  sit: "坐下",
  stand: "站立",
};

function stemOf(entry: string) {
  return (entry.split("/").pop() ?? entry).replace(/\.[^.]+$/, "");
}

/** 整包共有的前缀（如 clawd-）对用户没有信息量，逐条截掉 */
export function commonPrefix(entries: string[]): string {
  const stems = entries.map(stemOf);
  if (stems.length < 2) return "";
  let prefix = stems[0];
  for (const stem of stems.slice(1)) {
    while (prefix && !stem.startsWith(prefix)) prefix = prefix.slice(0, -1);
    if (!prefix) break;
  }
  // 只在分隔符处截断，免得把 "sleep" 砍成 "sle"
  const cut = Math.max(prefix.lastIndexOf("-"), prefix.lastIndexOf("_"));
  return cut > 0 ? prefix.slice(0, cut + 1) : "";
}

/** cat/clawd-type-keyboard.gif → 敲键盘 */
export function entryLabel(entry: string, prefix = ""): string {
  const stem = stemOf(entry);
  const key = (prefix && stem.startsWith(prefix) ? stem.slice(prefix.length) : stem).toLowerCase();
  if (ACTION_NAMES[key]) return ACTION_NAMES[key];
  // 整体没命中就按分隔符逐段翻，但要求每段都能译——只译出一半会得到「敲键盘laptop」，不如保留原名
  const parts = key.split(/[-_]/).filter(Boolean);
  if (parts.length > 1 && parts.every((part) => ACTION_NAMES[part])) {
    return parts.map((part) => ACTION_NAMES[part]).join("");
  }
  return key || stem;
}

/** 输入框润色时，猫换成敲键盘的那个动作（素材包里有的话） */
export function typingEntry(entries: string[]): string | null {
  return (
    entries.find((entry) => /type|typing|keyboard|laptop|code|coding|敲键盘|打字/i.test(stemOf(entry))) ?? null
  );
}
