#!/usr/bin/env bash
# 纯重构的契约快照：拆分前后各跑一次，两份输出必须逐字相同。
# 这里列的东西改了名，cargo check 和 tsc 都不会报错，只会在运行时或用户机器上坏掉。
# 不用 -e：某一节没有匹配（grep 返回 1）是正常结果，不该中断后面各节。
set -u
cd "$(git rev-parse --show-toplevel)"

rs_all() { find src-tauri/src -name '*.rs' -print0 | xargs -0 cat; }
ts_all() { find src -name '*.ts' -o -name '*.tsx' | tr '\n' '\0' | xargs -0 cat; }
unquote() { grep -oE '"[^"]+"' | tr -d '"'; }

echo "## 1. 注册的 Tauri 命令（前端按字符串调用，改名只会在运行时报错）"
rs_all | awk '/generate_handler!\[/,/\]\)/' \
  | grep -oE '[A-Za-z_][A-Za-z0-9_:]*,' | sed -E 's/.*:://; s/,$//' | sort

echo "## 2. 前端 invoke 的命令名"
ts_all | grep -oE 'invoke(<[^>]*>)?\("[^"]+"' | unquote | sort -u

echo "## 3. 事件名（跨窗口传递；含经常量引用的，如 THEME_EVENT）"
{
  rs_all | grep -oE 'emit(_to)?\([^")]*"[a-z0-9-]+"' | unquote
  rs_all | grep -oE 'const [A-Z_]*EVENT[A-Z_]*: *&str *= *"[^"]+"' | unquote
  ts_all | grep -oE '(emit|listen|once)(<[^>]*>)?\("[a-z0-9-]+"' | unquote
  ts_all | grep -oE 'const [A-Z_]*EVENT[A-Z_]* *= *"[^"]+"' | unquote
} | sort -u

echo "## 4. Settings 字段（序列化进用户的 settings.json，改名 = 该设置对老用户静默重置）"
rs_all | awk '/^(pub(\(crate\))? )?struct Settings \{/,/^\}/' \
  | grep -oE '^ +(pub(\(crate\))? )?[a-z_]+:' | sed -E 's/^ +(pub(\(crate\))? )?//; s/:$//' | sort

echo "## 5. 窗口 label（tauri.conf.json、capabilities、main.tsx 路由三处互相引用）"
grep -oE '"label": *"[^"]+"' src-tauri/tauri.conf.json | sed -E 's/.*: *//' | tr -d '"' | sort

echo "## 6. localStorage 键（含经常量引用的）"
{
  ts_all | grep -oE 'localStorage\.(get|set|remove)Item\("[^"]+"' | unquote
  ts_all | grep -oE 'STORAGE_KEY *= *"[^"]+"' | unquote
} | sort -u
