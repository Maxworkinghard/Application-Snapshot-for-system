using System;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>窗口/进程图标加载：优先取 exe 的高分辨率图标资源。</summary>
    internal static class WindowIconLoader
    {
        internal static Bitmap LoadWindowIcon(IntPtr window, int processId, int size)
        {
            try
            {
                using (Process process = Process.GetProcessById(processId))
                {
                    string executablePath = process.MainModule.FileName;
                    Bitmap executableIcon = LoadHighResolutionExecutableIcon(executablePath, size);
                    if (executableIcon != null)
                    {
                        return executableIcon;
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
