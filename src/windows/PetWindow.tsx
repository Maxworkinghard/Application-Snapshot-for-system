import { useEffect, useRef, useState } from "react";
import { currentMonitor, getCurrentWindow, LogicalSize, PhysicalPosition, type Window } from "@tauri-apps/api/window";
import { AppWindow, PawPrint } from "lucide-react";
import {
  getPreviousApp,
  loadSettings,
  onPreviousAppChanged,
  onSettingsChanged,
  showQuickMenu,
} from "../lib/backend";
import { iconUrl, petUrl } from "../lib/media";
import { MediaImage } from "../components/MediaImage";
import { broadcastPetAnimation, onPetAnimationAsked } from "../lib/petSync";
import { PET_SCALE, petWindowSize } from "../lib/petSize";
import type { PreviousApp, Settings } from "../types";

/** 放大后超出了所在屏幕的可用区域，就挪回来 */
async function keepOnScreen(petWindow: Window) {
  const monitor = await currentMonitor();
  if (!monitor) return;
  const [position, size] = await Promise.all([petWindow.outerPosition(), petWindow.outerSize()]);
  const { position: origin, size: area } = monitor.workArea;
  const x = Math.max(origin.x, Math.min(position.x, origin.x + area.width - size.width));
  const y = Math.max(origin.y, Math.min(position.y, origin.y + area.height - size.height));
  if (x !== position.x || y !== position.y) await petWindow.setPosition(new PhysicalPosition(x, y));
}

export function PetWindow() {
  const [app, setApp] = useState<PreviousApp>({ id: null, pid: null, name: "", title: "" });
  const [appearanceId, setAppearanceId] = useState("app-icon");
  const [animations, setAnimations] = useState<string[]>([]);
  const [animationIndex, setAnimationIndex] = useState(0);
  const [petSize, setPetSize] = useState(60);
  // 读到设置前不定大小，免得先按默认缩放一次再跳到用户设的大小
  const [petScale, setPetScale] = useState<number | null>(null);
  const dragged = useRef(false);

  useEffect(() => {
    const applyAppearance = (settings: Settings) => {
      const id = settings.selectedAppearanceId;
      const asset = settings.petAssets.find((item) => item.id === id);
      const entries = asset?.animations ?? [];
      const initialIndex = asset ? Math.max(0, entries.indexOf(asset.entry)) : 0;
      setAppearanceId(id);
      setAnimations(entries);
      setAnimationIndex(initialIndex);
      setPetScale(settings.petScale ?? PET_SCALE.default);
    };
    void getPreviousApp().then(setApp);
    void loadSettings().then(applyAppearance);
    const appListener = onPreviousAppChanged(setApp);
    const settingsListener = onSettingsChanged(applyAppearance);
    return () => {
      void appListener.then((unlisten) => unlisten());
      void settingsListener.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (appearanceId === "app-icon" || animations.length < 2) return;
    const timer = window.setInterval(() => {
      setAnimationIndex((index) => (index + 1) % animations.length);
    }, 8000);
    return () => window.clearInterval(timer);
  }, [appearanceId, animations]);

  useEffect(() => {
    if (petScale === null) return;
    const petWindow = getCurrentWindow();
    let active = true;
    let lastSize = 0;

    const fitToScreen = async () => {
      const shortEdge = Math.min(window.screen.availWidth, window.screen.availHeight);
      const nextSize = petWindowSize(shortEdge, petScale);
      if (active) setPetSize(nextSize);
      if (nextSize === lastSize) return;
      lastSize = nextSize;
      await petWindow.setSize(new LogicalSize(nextSize, nextSize));
      // 只在大小变了时才挪：平常拖动桌宠不去抢它的位置
      await keepOnScreen(petWindow);
    };

    void fitToScreen();
    const movedListener = petWindow.onMoved(() => void fitToScreen());
    const scaleListener = petWindow.onScaleChanged(() => void fitToScreen());
    return () => {
      active = false;
      void movedListener.then((unlisten) => unlisten());
      void scaleListener.then((unlisten) => unlisten());
    };
  }, [petScale]);

  function onPointerDown(event: React.PointerEvent) {
    if (event.button !== 0) return;
    dragged.current = false;
    const startX = event.clientX;
    const startY = event.clientY;
    const finishClick = () => {
      cleanup();
      if (!dragged.current && appearanceId !== "app-icon" && animations.length > 1) {
        setAnimationIndex((index) => (index + 1) % animations.length);
      }
    };
    const markDrag = (move: PointerEvent) => {
      if (Math.abs(move.clientX - startX) + Math.abs(move.clientY - startY) <= 4) return;
      dragged.current = true;
      cleanup();
      void getCurrentWindow().startDragging();
    };
    const cleanup = () => {
      window.removeEventListener("pointermove", markDrag);
      window.removeEventListener("pointerup", finishClick);
    };
    window.addEventListener("pointermove", markDrag);
    window.addEventListener("pointerup", finishClick);
  }

  // 换了动作就告诉主窗口，侧栏的猫跟着换；主窗口刚打开来问时也答一声
  const currentEntry = animations[animationIndex] ?? null;
  const current = useRef({ assetId: appearanceId, entry: currentEntry });
  current.current = { assetId: appearanceId, entry: currentEntry };
  useEffect(() => {
    if (appearanceId !== "app-icon") broadcastPetAnimation({ assetId: appearanceId, entry: currentEntry });
  }, [appearanceId, currentEntry]);
  useEffect(
    () =>
      onPetAnimationAsked(() => {
        if (current.current.assetId !== "app-icon") broadcastPetAnimation(current.current);
      }),
    [],
  );

  const petSrc = appearanceId === "app-icon" ? null : petUrl(appearanceId, currentEntry);

  function onContextMenu(event: React.MouseEvent) {
    event.preventDefault();
    void showQuickMenu(event.screenX, event.screenY);
  }

  return (
    <div
      className={`pet-window ${appearanceId === "app-icon" ? "app-icon" : "custom"}`}
      onPointerDown={onPointerDown}
      onContextMenu={onContextMenu}
      title={appearanceId === "app-icon" && app.name ? app.name : undefined}
      style={{ "--pet-icon-size": `${petSize - 4}px` } as React.CSSProperties}
    >
      <div className="pet-orb">
        {appearanceId !== "app-icon" ? (
          <MediaImage
            key={petSrc ?? "none"}
            className="pet-fade"
            src={petSrc}
            fallback={<PawPrint size={petSize - 10} />}
          />
        ) : (
          <MediaImage
            key={app.pid ?? "none"}
            src={app.pid === null ? null : iconUrl(app.pid, petSize)}
            alt={app.name}
            fallback={<AppWindow size={petSize - 12} />}
          />
        )}
      </div>
    </div>
  );
}
