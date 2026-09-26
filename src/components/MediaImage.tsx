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
  hidden,
}: {
  src: string | null;
  fallback: ReactNode;
  className?: string;
  alt?: string;
  /** 先藏着但保持加载（桌宠拖动时换成走路图，松手再露出来，不用重新取图） */
  hidden?: boolean;
}) {
  const [failed, setFailed] = useState(false);
  if (!src || failed) return <>{fallback}</>;
  return <img className={className} src={src} alt={alt} hidden={hidden} draggable={false} onError={() => setFailed(true)} />;
}
