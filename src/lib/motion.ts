import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import { flushSync } from "react-dom";

/**
 * 动效的几条规矩：
 * - 只表达因果和位置：东西从哪来、到哪去。换页时新页从导航的方向滑进来，
 *   选中标记从旧位置滑到新位置，换颜色从你点的地方铺开，放大图从缩略图原位长出来。
 * - 进场 320ms 先快后稳（--ease-enter），退场 160ms 加速离开（--ease-exit），
 *   移动类（指示条、列表补位）用标准曲线（--ease-out）。回弹只给猫。
 * - 可打断：新操作直接打断正在播的动画，过渡进行中照样能点。
 * - 「减弱」只去掉位移和回弹，淡入淡出保留，不砍成瞬切。
 */

export const DURATION = { press: 100, fade: 180, open: 320, exit: 160, theme: 520 } as const;
export const EASE_OUT = "cubic-bezier(0.2, 0, 0, 1)";
export const EASE_ENTER = "cubic-bezier(0.05, 0.7, 0.1, 1)";
export const EASE_EXIT = "cubic-bezier(0.3, 0, 0.8, 0.15)";
export const EASE_IN = EASE_EXIT;

export function motionReduced(): boolean {
  return document.documentElement.dataset.motion === "reduced";
}

/** 按当前动效档位折算时长 */
export function duration(ms: number): number {
  return motionReduced() ? Math.min(ms, 200) : ms;
}

type ViewTransitionLike = { finished: Promise<void>; skipTransition?: () => void };
type ViewTransitionDocument = Document & {
  startViewTransition?: (update: () => void) => ViewTransitionLike;
};

/** 过渡的种类决定 CSS 里走哪套动画（见 app.css「过渡」一节） */
export type TransitionKind = "page" | "layout" | "theme";
export type TransitionDirection = "up" | "down" | "left" | "right";

// 记住最近一次按下的位置：换颜色时从这里铺开
let lastPointer: { x: number; y: number; at: number } | null = null;
if (typeof window !== "undefined") {
  window.addEventListener(
    "pointerdown",
    (event) => {
      lastPointer = { x: event.clientX, y: event.clientY, at: Date.now() };
    },
    { capture: true, passive: true },
  );
}

let transitionToken = 0;

/**
 * 用 View Transitions 做整块的过渡：浏览器先拍下旧画面，再换成新画面，两张图之间按 kind 播动画。
 * 不支持时（旧 WebView、测试环境）直接切换。
 */
/** 这次换画面会不会走 View Transitions（不会的话，新页要自己淡入） */
export function canViewTransition(): boolean {
  return Boolean((document as ViewTransitionDocument).startViewTransition) && document.visibilityState !== "hidden";
}

export function viewTransition(kind: TransitionKind, update: () => void, options: { direction?: TransitionDirection } = {}) {
  const doc = document as ViewTransitionDocument;
  if (!doc.startViewTransition || !canViewTransition()) {
    update();
    return;
  }
  const root = document.documentElement;
  const token = ++transitionToken;
  root.dataset.vt = kind;
  if (options.direction) root.dataset.vtDir = options.direction;
  else delete root.dataset.vtDir;
  if (kind === "theme") {
    const recent = lastPointer && Date.now() - lastPointer.at < 1500 ? lastPointer : null;
    const x = recent?.x ?? window.innerWidth / 2;
    const y = recent?.y ?? window.innerHeight / 2;
    const radius = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y));
    root.style.setProperty("--vt-ox", `${x}px`);
    root.style.setProperty("--vt-oy", `${y}px`);
    root.style.setProperty("--vt-r", `${Math.ceil(radius)}px`);
  }
  const transition = doc.startViewTransition(() => {
    flushSync(update);
  });
  const done = () => {
    if (token !== transitionToken) return;
    delete root.dataset.vt;
    delete root.dataset.vtDir;
  };
  transition.finished.then(done, done);
}

/** 旧名字：整窗交叉淡化 */
export function withViewTransition(update: () => void) {
  viewTransition("layout", update);
}

/**
 * 让一个值「晚一点消失」：值变成空以后，再保留 exitMs 让退场动画播完。
 * 返回要渲染的东西，以及它是不是正在退场。
 */
export function usePresence<T>(value: T | null | undefined | false, exitMs: number = DURATION.exit) {
  const live = value === false || value === undefined ? null : value;
  const isLive = live !== null;
  // 记住最后一个非空值，退场时还渲染它（对象每次渲染都是新的，所以放 ref 不放 state）
  const last = useRef<T | null>(live);
  if (isLive) last.current = live;
  const [gone, setGone] = useState(!isLive);
  useEffect(() => {
    if (isLive) {
      setGone(false);
      return;
    }
    const timer = window.setTimeout(() => setGone(true), motionReduced() ? 150 : exitMs);
    return () => window.clearTimeout(timer);
  }, [isLive, exitMs]);
  if (isLive) return { item: live as T, leaving: false };
  if (gone || last.current === null) return { item: null, leaving: false };
  return { item: last.current, leaving: true };
}

/**
 * 一组互斥选项下面那道墨线：选中项换了，线从旧位置滑过去，而不是这边消失那边出现。
 * 容器要 position: relative，并带 has-mark 类；选中的直接子元素带 is-on。
 */
export function useSlidingMark<T extends HTMLElement>(key: unknown) {
  const ref = useRef<T>(null);
  useLayoutEffect(() => {
    const root = ref.current;
    if (!root) return;
    const place = () => {
      const on = root.querySelector<HTMLElement>(":scope > .is-on");
      if (!on) {
        root.style.setProperty("--mark-w", "0px");
        return;
      }
      root.style.setProperty("--mark-x", `${on.offsetLeft}px`);
      root.style.setProperty("--mark-y", `${on.offsetTop + on.offsetHeight}px`);
      root.style.setProperty("--mark-w", `${on.offsetWidth}px`);
    };
    place();
    // 第一次落位不播动画，之后才滑
    const frame = window.requestAnimationFrame(() => root.classList.add("mark-ready"));
    if (typeof ResizeObserver === "undefined") return () => window.cancelAnimationFrame(frame);
    const observer = new ResizeObserver(place);
    observer.observe(root);
    return () => {
      window.cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [key]);
  return ref;
}

/** 列表逐个入场：第 index 个晚一点点，最多晚到第 12 个为止 */
export function stagger(index: number): CSSProperties {
  return { ["--i" as string]: Math.min(index, 12) } as CSSProperties;
}

/**
 * 共享元素：让 element 看起来是从 from 这块区域长出来的（FLIP）。
 * 放大预览用它——图从缩略图原位长出来，关掉时再缩回去。
 */
export function growFrom(element: HTMLElement, from: DOMRect, ms: number = DURATION.open) {
  const to = element.getBoundingClientRect();
  if (!to.width || !to.height || typeof element.animate !== "function") return;
  if (motionReduced()) {
    element.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 200 });
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
    { duration: ms, easing: EASE_ENTER },
  );
}

/** growFrom 的反向：缩回 to 这块区域，结束后 resolve */
export function shrinkTo(element: HTMLElement, to: DOMRect | null, ms: number = DURATION.exit + 60): Promise<void> {
  const from = element.getBoundingClientRect();
  if (typeof element.animate !== "function") return Promise.resolve();
  if (!to || motionReduced() || !from.width || !from.height) {
    return element.animate([{ opacity: 1 }, { opacity: 0 }], { duration: DURATION.exit, fill: "forwards" }).finished.then(
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
 * 把一张图从 from 飞到 to 这块区域（按比例放进去、底边对齐），落地后淡掉。
 * 换伴侣用：点架子上的一只，它的第一帧飞上舞台，真正的动图在底下接住。
 */
export function flyImage(
  src: string,
  from: DOMRect,
  to: DOMRect,
  { onLand, ms = DURATION.open + 120 }: { onLand?: () => void; ms?: number } = {},
): Promise<void> {
  if (motionReduced() || !from.width || !from.height || typeof document.body.animate !== "function") {
    onLand?.();
    return Promise.resolve();
  }
  const ghost = document.createElement("img");
  ghost.src = src;
  ghost.alt = "";
  ghost.setAttribute("aria-hidden", "true");
  Object.assign(ghost.style, {
    position: "fixed",
    left: `${from.left}px`,
    top: `${from.top}px`,
    width: `${from.width}px`,
    height: `${from.height}px`,
    zIndex: "60",
    pointerEvents: "none",
    transformOrigin: "0 0",
  });
  document.body.appendChild(ghost);
  const scale = Math.min(to.width / from.width, to.height / from.height);
  const x = to.left + (to.width - from.width * scale) / 2 - from.left;
  const y = to.bottom - from.height * scale - from.top;
  return ghost
    .animate([{ transform: "none" }, { transform: `translate(${x}px, ${y}px) scale(${scale})` }], {
      duration: ms,
      easing: EASE_ENTER,
      fill: "forwards",
    })
    .finished.then(() => {
      // 落地：真图在底下现身，替身淡掉
      onLand?.();
      return ghost.animate([{ opacity: 1 }, { opacity: 0 }], { duration: DURATION.exit + 80, fill: "forwards" }).finished;
    })
    .then(
      () => ghost.remove(),
      () => {
        onLand?.();
        ghost.remove();
      },
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
