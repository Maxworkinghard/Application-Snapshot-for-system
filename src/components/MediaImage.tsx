import { useState, type ReactNode } from "react";

/**
 * 从 media:// 协议取的图片：取不到（文件被移走、进程已退出、没有图标）就显示 fallback，
 * 而不是一个裂开的图片。换图时调用方用 key 重建，失败状态随之复位。
 */
export function MediaImage({
  src,
  fallback,
  className,
  alt = "",
}: {
  src: string | null;
  fallback: ReactNode;
  className?: string;
  alt?: string;
}) {
  const [failed, setFailed] = useState(false);
  if (!src || failed) return <>{fallback}</>;
  return <img className={className} src={src} alt={alt} draggable={false} onError={() => setFailed(true)} />;
}
