import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { AppWindow, PawPrint } from "lucide-react";
import {
  getPetAssetDataUrl,
  getPreviousApp,
  loadSettings,
  onPreviousAppChanged,
  onSettingsChanged,
  showQuickMenu,
} from "../lib/backend";
import type { PreviousApp, Settings } from "../types";

/** 桌宠点击/拖动起始时的提示音（与设置页试听同一套逻辑） */
function playPetClickSound(enabled: boolean, volume: number, customPath: string | null) {
  if (!enabled) return;
  try {
    const gain = Math.max(0, Math.min(1, volume / 100));
    if (customPath) {
      const inTauri = "__TAURI_INTERNALS__" in window;
      const audio = new Audio(inTauri ? convertFileSrc(customPath) : customPath);
      audio.volume = gain;
      void audio.play();
      return;
    }
    const AudioContextCtor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioContextCtor) return;
    const ctx = new AudioContextCtor();
    const osc = ctx.createOscillator();
    const amp = ctx.createGain();
    const now = ctx.currentTime;
    osc.frequency.value = 660;
    osc.type = "triangle";
    amp.gain.setValueAtTime(0.0001, now);
    amp.gain.exponentialRampToValueAtTime(Math.max(0.0001, gain * 0.35), now + 0.015);
    amp.gain.exponentialRampToValueAtTime(0.0001, now + 0.18);
    osc.connect(amp).connect(ctx.destination);
    osc.start(now);
    osc.stop(now + 0.4);
  } catch {
    // 提示音失败不影响拖动/点击
  }
}

/** 桌宠素材既可能是图片 (GIF/WebP/APNG/PNG)，也可能是视频 (MP4/WebM)——按 data URL 的 MIME 分流。 */
export function renderPetMedia(dataUrl: string, className: string) {
  if (dataUrl.startsWith("data:video/")) {
    return <video key={dataUrl} className={className} src={dataUrl} autoPlay loop muted playsInline />;
  }
  return <img key={dataUrl} className={className} src={dataUrl} alt="" draggable={false} />;
}

export function PetWindow() {
  const [app, setApp] = useState<PreviousApp>({ id: null, name: "", title: "", iconDataUrl: null });
  const [appearanceId, setAppearanceId] = useState("app-icon");
  const [animations, setAnimations] = useState<string[]>([]);
  const [animationIndex, setAnimationIndex] = useState(0);
  const [assetDataUrl, setAssetDataUrl] = useState<string | null>(null);
  const [petSize, setPetSize] = useState(60);
  const [soundEnabled, setSoundEnabled] = useState(true);
  const [soundVolume, setSoundVolume] = useState(65);
  const [soundPath, setSoundPath] = useState<string | null>(null);
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
      setSoundEnabled(settings.petSoundEnabled);
      setSoundVolume(settings.petSoundVolume);
      setSoundPath(settings.petCustomSoundPath);
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
    let active = true;
    if (appearanceId === "app-icon") {
      setAssetDataUrl(null);
      return () => { active = false; };
    }
    const entry = animations[animationIndex] ?? null;
    void getPetAssetDataUrl(appearanceId, entry)
      .then((value) => { if (active) setAssetDataUrl(value); })
      .catch(() => { if (active) setAssetDataUrl(null); });
    return () => { active = false; };
  }, [appearanceId, animationIndex, animations]);

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
    playPetClickSound(soundEnabled, soundVolume, soundPath);
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

  function onContextMenu(event: React.MouseEvent) {
    event.preventDefault();
    void showQuickMenu(event.screenX, event.screenY);
  }

  return (
    <div
      className={`pet-window ${appearanceId === "app-icon" ? "app-icon" : "custom"}`}
      onPointerDown={onPointerDown}
      onContextMenu={onContextMenu}
      title={appearanceId === "app-icon" && app.name ? `${app.name} · 右键打开功能` : "右键打开功能"}
      style={{ "--pet-icon-size": `${petSize - 4}px` } as React.CSSProperties}
    >
      <div className="pet-orb">
        {appearanceId !== "app-icon" ? (
          assetDataUrl ? renderPetMedia(assetDataUrl, "pet-fade") : <PawPrint size={petSize - 10} />
        ) : app.iconDataUrl ? (
          <img src={app.iconDataUrl} alt={app.name} draggable={false} />
        ) : (
          <AppWindow size={petSize - 12} />
        )}
      </div>
    </div>
  );
}
