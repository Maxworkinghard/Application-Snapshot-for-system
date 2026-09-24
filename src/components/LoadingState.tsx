/** 读设置通常几十毫秒就完；只有慢的时候才让这行字露面（CSS 里延迟 300ms 淡入） */
export function LoadingState({ text = "正在读取设置…" }: { text?: string }) {
  return (
    <div className="loading-state" role="status">
      {text}
    </div>
  );
}
