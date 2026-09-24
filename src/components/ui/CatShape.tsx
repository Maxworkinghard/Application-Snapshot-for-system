/**
 * 一只坐着的猫的剪影。没有图可显示时用它占位，主题缩略图里也用它当猫。
 * outline：虚线轮廓，表示「这里本该有只猫」（文件丢了、还没导入）。
 */
export function CatShape({ height = 16, outline = false, className = "" }: { height?: number; outline?: boolean; className?: string }) {
  return (
    <svg
      className={`cat-shape ${outline ? "is-outline" : ""} ${className}`}
      viewBox="0 0 20 24"
      width={(height * 20) / 24}
      height={height}
      aria-hidden="true"
    >
      <path d="M3.2 23.2c-1.3 0-1.9-1.4-1.6-3.4.4-2.6 1.6-4.6 2.8-6.2L3.6 3l4.2 3.6h4.4L16.4 3l-.8 10.6c1.2 1.6 2.4 3.6 2.8 6.2.3 2-.3 3.4-1.6 3.4Z" />
    </svg>
  );
}
