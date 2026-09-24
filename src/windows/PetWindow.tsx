import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
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
import type { PreviousApp, Settings } from "../types";

export function PetWindow() {
  const [app, setApp] = useState<PreviousApp>({ id: null, pid: null, name: "", title: "" });
  const [appearanceId, setAppearanceId] = useState("app-icon");
  const [animations, setAnimations] = useState<string[]>([]);
  const [animationIndex, setAnimationIndex] = useState(0);
  const [petSize, setPetSize] = useState(60);
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
    const petWindow = getCurrentWindow();
    let active = true;

    const fitToScreen = async () => {
      const shortEdge = Math.min(window.screen.availWidth, window.screen.availHeight);
      const nextSize = Math.round(Math.max(58, Math.min(72, shortEdge * 0.06)));
      if (active) setPetSize(nextSize);
      await petWindow.setSize(new LogicalSize(nextSize, nextSize));
    };

    void fitToScreen();
    const movedListener = petWindow.onMoved(() => void fitToScreen());
    const scaleListener = petWindow.onScaleChanged(() => void fitToScreen());
    return () => {
      active = false;
      void movedListener.then((unlisten) => unlisten());
      void scaleListener.then((unlisten) => unlisten());
    };
  }, []);

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

  const petSrc = appearanceId === "app-icon" ? null : petUrl(appearanceId, animations[animationIndex] ?? null);

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
