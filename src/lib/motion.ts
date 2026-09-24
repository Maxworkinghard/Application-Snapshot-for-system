import { useLayoutEffect, useRef, type RefObject } from "react";
import { flushSync } from "react-dom";

/**
 * 动效的几条规矩（和设计稿「动效」画板一致）：
 * - 只表达因果和位置：东西从哪来、到哪去。
 * - 时长跟移动距离走：90 按下 / 160 淡入与所有退场 / 220 展开放大入列 / 280 换主题。
 * - 进场 ease-out，退场 ease-in 且快三分之一；回弹只给猫；倒计时是唯一的 linear。
 * - 可打断：新操作直接打断正在播的动画，不排队。
 * - 系统开了「减少动态效果」时，全部退成 120ms 淡入淡出（见 prefs.ts 与 variables.css）。
 */

export const DURATION = { press: 90, fade: 160, open: 220, theme: 280 } as const;
export const EASE_OUT = "cubic-bezier(0.2, 0, 0, 1)";
export const EASE_IN = "cubic-bezier(0.4, 0, 1, 1)";

export function motionReduced(): boolean {
  return document.documentElement.dataset.motion === "reduced";
}

/** 按当前动效档位折算时长：减弱时统一 120ms，且不做位移（调用方自己判断是否跳过位移） */
export function duration(ms: number): number {
  return motionReduced() ? Math.min(ms, 120) : ms;
}

type ViewTransitionDocument = Document & {
  startViewTransition?: (update: () => void) => { finished: Promise<void> };
};

/**
 * 整窗交叉淡化（换布局主题用）：结构变了就当翻一页，不让每个零件各自飞。
 * WebView 不支持 View Transitions 时直接切换。
 */
export function withViewTransition(update: () => void) {
  const doc = document as ViewTransitionDocument;
  if (!doc.startViewTransition) {
    update();
    return;
  }
  doc.startViewTransition(() => {
    flushSync(update);
  });
}

/**
 * 共享元素：让 element 看起来是从 from 这块区域长出来的（FLIP）。
 * 放大预览用它——图从缩略图原位长出来，关掉时再缩回去。
 */
export function growFrom(element: HTMLElement, from: DOMRect, ms = DURATION.open) {
  const to = element.getBoundingClientRect();
  if (!to.width || !to.height || typeof element.animate !== "function") return;
  if (motionReduced()) {
    element.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 120 });
    return;
  }
  const dx = from.left - to.left;
  const dy = from.top - to.top;
  const sx = from.width / to.width;
  const sy = from.height / to.height;
  element.animate(
    [
      { transformOrigin: "0 0", transform: `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})` },
      { transformOrigin: "0 0", transform: "none" },
    ],
    { duration: ms, easing: EASE_OUT },
  );
}

/** growFrom 的反向：缩回 to 这块区域，结束后 resolve */
export function shrinkTo(element: HTMLElement, to: DOMRect | null, ms = DURATION.fade): Promise<void> {
  const from = element.getBoundingClientRect();
  if (typeof element.animate !== "function") return Promise.resolve();
  if (!to || motionReduced() || !from.width || !from.height) {
    return element.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 120, fill: "forwards" }).finished.then(
      () => undefined,
      () => undefined,
    );
  }
  const dx = to.left - from.left;
  const dy = to.top - from.top;
  const animation = element.animate(
    [
      { transformOrigin: "0 0", transform: "none" },
      { transformOrigin: "0 0", transform: `translate(${dx}px, ${dy}px) scale(${to.width / from.width}, ${to.height / from.height})` },
    ],
    { duration: ms, easing: EASE_IN, fill: "forwards" },
  );
  return animation.finished.then(
    () => undefined,
    () => undefined,
  );
}

/**
 * 列表重排时让其余元素滑到新位置，而不是瞬移（删掉几张快照后，后面的补上来）。
 * 用法：改数据前调 capture()，渲染后自动播放。子元素要带 data-flip-key。
 */
export function useFlip(container: RefObject<HTMLElement | null>) {
  const before = useRef<Map<string, DOMRect> | null>(null);

  useLayoutEffect(() => {
    const snapshot = before.current;
    const root = container.current;
    before.current = null;
    if (!snapshot || !root || motionReduced()) return;
    root.querySelectorAll<HTMLElement>("[data-flip-key]").forEach((node) => {
      const previous = snapshot.get(node.dataset.flipKey!);
      if (!previous || typeof node.animate !== "function") return;
      const now = node.getBoundingClientRect();
      const dx = previous.left - now.left;
      const dy = previous.top - now.top;
      if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return;
      node.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "none" }], {
        duration: DURATION.open,
        easing: EASE_OUT,
      });
    });
  });

  return {
    capture() {
      const root = container.current;
      if (!root) return;
      const map = new Map<string, DOMRect>();
      root.querySelectorAll<HTMLElement>("[data-flip-key]").forEach((node) => {
        map.set(node.dataset.flipKey!, node.getBoundingClientRect());
      });
      before.current = map;
    },
  };
}

export const wait = (ms: number) => new Promise<void>((resolve) => window.setTimeout(resolve, ms));
