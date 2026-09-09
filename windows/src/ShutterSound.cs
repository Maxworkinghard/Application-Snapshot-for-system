using System;
using System.IO;
using System.Media;

namespace AppSnapshot
{
    /// <summary>
    /// 截图快门声：代码合成的短促「咔嚓」音（两段衰减噪声脉冲 + 低频尾），
    /// 不依赖外部资源文件；对应 macOS 端播放系统截屏快门声。
    /// </summary>
    internal static class ShutterSound
    {
        private static SoundPlayer player;

        internal static void Play()
        {
            try
            {
                if (player == null)
                {
                    // 流随 player 常驻（异步播放期间不能释放）
                    player = new SoundPlayer(new MemoryStream(BuildWav()));
                }
                player.Play();
            }
            catch
            {
                // 播放失败不影响截图主流程
            }
        }

        private static byte[] BuildWav()
        {
            const int sampleRate = 44100;
            const int sampleCount = 44100 * 12 / 100; // 约 0.12 秒

            using (var memory = new MemoryStream())
            using (var writer = new BinaryWriter(memory))
            {
                writer.Write(new byte[] { (byte)'R', (byte)'I', (byte)'F', (byte)'F' });
                writer.Write(36 + sampleCount * 2);
                writer.Write(new byte[] { (byte)'W', (byte)'A', (byte)'V', (byte)'E' });
                writer.Write(new byte[] { (byte)'f', (byte)'m', (byte)'t', (byte)' ' });
                writer.Write(16);
                writer.Write((short)1);
                writer.Write((short)1);
                writer.Write(sampleRate);
                writer.Write(sampleRate * 2);
                writer.Write((short)2);
                writer.Write((short)16);
                writer.Write(new byte[] { (byte)'d', (byte)'a', (byte)'t', (byte)'a' });
                writer.Write(sampleCount * 2);

                var random = new Random(7);
                for (int i = 0; i < sampleCount; i++)
                {
                    double t = (double)i / sampleRate;
                    double envelope =
                        Burst(t, 0.000, 0.014, 280.0)
                        + Burst(t, 0.032, 0.016, 220.0)
                        + Burst(t, 0.055, 0.045, 50.0);
                    double noise = (random.NextDouble() * 2.0 - 1.0) * 0.7;
                    double tone = Math.Sin(2.0 * Math.PI * 140.0 * t) * 0.3;
                    double value = (noise + tone) * envelope * 0.6;
                    if (value > 1.0)
                    {
                        value = 1.0;
                    }
                    else if (value < -1.0)
                    {
                        value = -1.0;
                    }
                    writer.Write((short)(value * short.MaxValue));
                }

                return memory.ToArray();
            }
        }

        /// <summary>从 start 开始、长度 length 的指数衰减脉冲包络。</summary>
        private static double Burst(double t, double start, double length, double decay)
        {
            if (t < start || t >= start + length)
            {
                return 0.0;
            }
            return Math.Exp(-(t - start) * decay);
        }
    }
}
