using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>窗口/进程图标加载：优先取 exe 的高分辨率图标资源。</summary>
    internal static class WindowIconLoader
    {
        // exe 图标提取开销大（跨进程读资源），按进程缓存；key 含进程启动时间避免 PID 复用后图标错乱。
        // 缓存的是共享副本，对外一律返回克隆，调用方负责 Dispose 返回值。
        private const int CacheLimit = 64;
        private static readonly object CacheSync = new object();
        private static readonly Dictionary<string, Bitmap> IconCache = new Dictionary<string, Bitmap>();

        internal static Bitmap LoadWindowIcon(IntPtr window, int processId, int size)
        {
            string cacheKey = null;
            try
            {
                using (Process process = Process.GetProcessById(processId))
                {
                    string executablePath = process.MainModule.FileName;
                    try
                    {
                        cacheKey = process.StartTime.Ticks + ":" + processId + ":" + size;
                    }
                    catch
                    {
                        cacheKey = null;
                    }

                    Bitmap cached = GetCached(cacheKey);
                    if (cached != null)
                    {
                        return cached;
                    }

                    Bitmap executableIcon = LoadHighResolutionExecutableIcon(executablePath, size);
                    if (executableIcon != null)
                    {
                        return StoreAndClone(cacheKey, executableIcon);
                    }
                }
            }
            catch
            {
            }

            IntPtr iconHandle = NativeMethods.SendMessage(
                window, NativeMethods.WmGetIcon,
                new IntPtr(NativeMethods.IconBig), IntPtr.Zero);
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.SendMessage(
                    window, NativeMethods.WmGetIcon,
                    new IntPtr(NativeMethods.IconSmall2), IntPtr.Zero);
            }
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.SendMessage(
                    window, NativeMethods.WmGetIcon,
                    new IntPtr(NativeMethods.IconSmall), IntPtr.Zero);
            }
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.GetClassLongPtr(window, NativeMethods.GclpHIconSmall);
            }
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.GetClassLongPtr(window, NativeMethods.GclpHIcon);
            }
            if (iconHandle != IntPtr.Zero)
            {
                try
                {
                    return RenderIcon(iconHandle, size);
                }
                catch
                {
                }
            }

            try
            {
                using (Process process = Process.GetProcessById(processId))
                using (Icon icon = Icon.ExtractAssociatedIcon(process.MainModule.FileName))
                {
                    if (icon != null)
                    {
                        return RenderIcon(icon.Handle, size);
                    }
                }
            }
            catch
            {
            }

            return RenderIcon(SystemIcons.Application.Handle, size);
        }

        private static Bitmap GetCached(string cacheKey)
        {
            if (cacheKey == null)
            {
                return null;
            }
            lock (CacheSync)
            {
                Bitmap cached;
                if (IconCache.TryGetValue(cacheKey, out cached))
                {
                    return new Bitmap(cached);
                }
            }
            return null;
        }

        private static Bitmap StoreAndClone(string cacheKey, Bitmap icon)
        {
            if (cacheKey == null)
            {
                return icon;
            }
            lock (CacheSync)
            {
                if (IconCache.Count >= CacheLimit)
                {
                    foreach (Bitmap entry in IconCache.Values)
                    {
                        entry.Dispose();
                    }
                    IconCache.Clear();
                }
                IconCache[cacheKey] = icon;
                return new Bitmap(icon);
            }
        }

        private static Bitmap LoadHighResolutionExecutableIcon(string executablePath, int size)
        {
            if (string.IsNullOrEmpty(executablePath))
            {
                return null;
            }

            var iconHandles = new IntPtr[1];
            var iconIds = new uint[1];
            uint extracted = NativeMethods.PrivateExtractIcons(
                executablePath,
                0,
                size,
                size,
                iconHandles,
                iconIds,
                1,
                0);

            if (extracted == 0 || iconHandles[0] == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                return RenderIcon(iconHandles[0], size);
            }
            finally
            {
                NativeMethods.DestroyIcon(iconHandles[0]);
            }
        }

        private static Bitmap RenderIcon(IntPtr iconHandle, int size)
        {
            var bitmap = new Bitmap(size, size, PixelFormat.Format32bppPArgb);
            using (Graphics graphics = Graphics.FromImage(bitmap))
            using (Icon icon = Icon.FromHandle(iconHandle))
            {
                graphics.Clear(Color.Transparent);
                graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.HighQuality;
                graphics.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic;
                graphics.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.HighQuality;
                graphics.CompositingQuality = System.Drawing.Drawing2D.CompositingQuality.HighQuality;
                graphics.DrawIcon(icon, new Rectangle(0, 0, size, size));
            }

            return bitmap;
        }
    }
}
