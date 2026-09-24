import { soundUrl } from "./media";

/** 试听提示音：有自定义文件就播文件，否则用 WebAudio 合成一声短促提示 */
/**
 * 一记机械快门的「咔」。
 *
 * 机械声是宽频瞬态、没有音高，所以用白噪声过带通再配极快的衰减包络；
 * 用振荡器无论怎么调，出来的都是电子提示音而不是快门声。
 */
export function shutterClick(ctx: AudioContext, at: number, level: number, bright: boolean) {
  const duration = 0.05;
  const buffer = ctx.createBuffer(1, Math.max(1, Math.ceil(ctx.sampleRate * duration)), ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < data.length; i += 1) {
    data[i] = Math.random() * 2 - 1;
  }

  const source = ctx.createBufferSource();
  source.buffer = buffer;

  const filter = ctx.createBiquadFilter();
  filter.type = "bandpass";
  // 高一点像金属脆响，低一点像闷响，两声一高一低才有先后层次
  filter.frequency.value = bright ? 3200 : 1700;
  filter.Q.value = 0.9;

  const amp = ctx.createGain();
  amp.gain.setValueAtTime(Math.max(0.0001, level), at);
  amp.gain.exponentialRampToValueAtTime(0.0001, at + duration);

  source.connect(filter).connect(amp).connect(ctx.destination);
  source.start(at);
  source.stop(at + duration);
}

export function previewHintSound(kind: "crisp" | "soft", customPath: string | null, volume: number) {
  const gain = Math.min(1, Math.max(0, volume / 100));
  try {
    if (customPath) {
      // 原先用 asset 协议，但项目从没开启它，自定义音效其实一直放不出来
      const audio = new Audio(soundUrl(customPath));
      audio.volume = gain;
      // 文件被移走等情况下播不出来就算了，不打断截图流程
      audio.play().catch(() => {});
      return;
    }
    const AudioContextCtor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioContextCtor) return;
    const ctx = new AudioContextCtor();
    const now = ctx.currentTime;

    // 快门声：一声「咔」（反光板抬起）接一声稍闷的「嚓」（快门闭合）。
    // 两声间隔 70ms——再近会糊成一声，再远就听成两次独立的响动。
    const level = gain * (kind === "soft" ? 0.2 : 0.38);
    shutterClick(ctx, now, level, kind === "crisp");
    shutterClick(ctx, now + 0.07, level * 0.7, false);
  } catch {
    // 试听失败不打断设置流程
  }
}
