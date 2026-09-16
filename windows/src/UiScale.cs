using System;
using System.Drawing;

namespace AppSnapshot
{
    /// <summary>
    /// 高 DPI 换算：进程通过 HighDpiMode.SystemAware 声明系统级 DPI 感知，
    /// 拿到的坐标全部是物理像素，而布局常量是按 96 DPI 设计的。
    /// 字体按磅值定义会随 DPI 自动放大，像素常量必须手动乘以该系数，
    /// 否则在 150%/200% 缩放的屏幕上所有面板只剩一半有效宽度。
    /// AutoScaleMode.Dpi 对纯代码构建的窗体不生效（没有设计器序列化的
    /// AutoScaleDimensions 基准），所以统一走这里显式换算。
    /// </summary>
    internal static class UiScale
    {
        private static float factor = -1f;

        internal static float Factor
        {
            get
            {
                if (factor <= 0f)
                {
                    using (Graphics graphics = Graphics.FromHwnd(IntPtr.Zero))
                    {
                        factor = graphics.DpiX / 96f;
                    }
                    if (factor <= 0f)
                    {
                        factor = 1f;
                    }
                }
                return factor;
            }
        }

        internal static int Px(int value)
        {
            return (int)Math.Round(value * Factor, MidpointRounding.AwayFromZero);
        }

        internal static Size Px(int width, int height)
        {
            return new Size(Px(width), Px(height));
        }
    }
}
