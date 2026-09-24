import { shortcutKeys } from "../../lib/format";

/**
 * 快捷键的键帽。整个界面里唯一「立起来」的元素——这是个快捷键工具，键帽是它自己的语言。
 * quiet：列表里只写成一行等宽灰字，不画键帽框（输入框里一行三个框太吵）。
 * struck：注册失败的键，划掉并标红。
 */
export function Keys({
  value,
  quiet = false,
  struck = false,
  className = "",
}: {
  value: string | null;
  quiet?: boolean;
  struck?: boolean;
  className?: string;
}) {
  const keys = shortcutKeys(value);
  if (keys.length === 0) return null;
  if (quiet) {
    return <span className={`keys-quiet ${struck ? "is-struck" : ""} ${className}`}>{keys.join(" ")}</span>;
  }
  return (
    <span className={`keys ${struck ? "is-struck" : ""} ${className}`} aria-label={keys.join(" + ")}>
      {keys.map((key, index) => (
        <kbd key={index} className="kbd">{key}</kbd>
      ))}
    </span>
  );
}
