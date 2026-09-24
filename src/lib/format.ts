export function fileNameOf(path: string | null): string {
  if (!path) return "";
  return path.split(/[\\/]/).pop() ?? path;
}

export function normalizeKey(key: string) {
  if (key === " ") return "Space";
  if (key === "ArrowUp") return "Up";
  if (key === "ArrowDown") return "Down";
  if (key === "ArrowLeft") return "Left";
  if (key === "ArrowRight") return "Right";
  return key.length === 1 ? key.toUpperCase() : key;
}

const isMac = () => typeof navigator !== "undefined" && navigator.platform.includes("Mac");

/** "CommandOrControl+Shift+A" → ["Ctrl", "Shift", "A"]（mac 上是 ⌘） */
export function shortcutKeys(value: string | null): string[] {
  if (!value) return [];
  return value.split("+").map((key) => {
    const lower = key.trim().toLowerCase();
    if (lower === "commandorcontrol" || lower === "cmdorctrl") return isMac() ? "⌘" : "Ctrl";
    if (lower === "control" || lower === "ctrl") return "Ctrl";
    if (lower === "alt" || lower === "option") return isMac() ? "⌥" : "Alt";
    if (lower === "shift") return "Shift";
    return key.trim();
  });
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export function formatWhen(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return new Date(timestamp).toLocaleString();
}

const pad = (value: number) => String(value).padStart(2, "0");

/** 14:32 */
export function formatClock(timestamp: number): string {
  const date = new Date(timestamp);
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function startOfDay(timestamp: number): number {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

/** 今天 / 昨天 / 9 月 21 日 / 2025 年 12 月 3 日 */
export function dayLabel(timestamp: number, now = Date.now()): string {
  const days = Math.round((startOfDay(now) - startOfDay(timestamp)) / 86_400_000);
  if (days === 0) return "今天";
  if (days === 1) return "昨天";
  const date = new Date(timestamp);
  const sameYear = date.getFullYear() === new Date(now).getFullYear();
  return `${sameYear ? "" : `${date.getFullYear()} 年 `}${date.getMonth() + 1} 月 ${date.getDate()} 日`;
}

/** 按天分组，保持原顺序（调用方先按时间倒序排好） */
export function groupByDay<T>(items: T[], at: (item: T) => number, now = Date.now()) {
  const groups: Array<{ key: string; label: string; items: T[] }> = [];
  for (const item of items) {
    const key = String(startOfDay(at(item)));
    const last = groups[groups.length - 1];
    if (last && last.key === key) last.items.push(item);
    else groups.push({ key, label: dayLabel(at(item), now), items: [item] });
  }
  return groups;
}

/** 02:13、1:02:03 */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor(total / 60) % 60;
  const seconds = total % 60;
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${pad(minutes)}:${pad(seconds)}`;
}

/** 长路径只留头尾：D:\Dropbox\工作\…\应用快照\原始文件 */
export function middleTruncatePath(path: string, max = 34): string {
  if (path.length <= max) return path;
  const separator = path.includes("\\") ? "\\" : "/";
  const parts = path.split(separator);
  if (parts.length <= 3) return `${path.slice(0, Math.ceil(max / 2) - 1)}…${path.slice(-Math.floor(max / 2))}`;
  let head = parts[0];
  let index = 1;
  const tail: string[] = [];
  let tailIndex = parts.length - 1;
  // 尾部尽量多留（文件夹名在尾巴上），头部留盘符和第一层
  while (tailIndex > index && [head, "…", ...tail, parts[tailIndex]].join(separator).length <= max) {
    tail.unshift(parts[tailIndex]);
    tailIndex -= 1;
  }
  if (index < tailIndex && [head, parts[index], "…", ...tail].join(separator).length <= max) {
    head = `${head}${separator}${parts[index]}`;
    index += 1;
  }
  return [head, "…", ...tail].join(separator);
}
