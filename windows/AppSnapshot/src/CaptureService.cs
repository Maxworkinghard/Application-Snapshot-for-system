using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

namespace AppSnapshot
{
    internal static class CaptureService
    {
        public static uint CaptureWindowToClipboard(IntPtr handle)
        {
            if (handle == IntPtr.Zero || !NativeMethods.IsWindow(handle))
            {
                throw new InvalidOperationException("Target window is no longer available.");
            }

            NativeMethods.Rect rect;
            if (!NativeMethods.GetWindowRect(handle, out rect))
            {
                int result = NativeMethods.DwmGetWindowAttribute(
                    handle,
                    NativeMethods.DwmwaExtendedFrameBounds,
                    out rect,
                    System.Runtime.InteropServices.Marshal.SizeOf(typeof(NativeMethods.Rect)));
                if (result != 0)
                {
                    throw new InvalidOperationException("Unable to read the target window bounds.");
                }
            }

            if (rect.Width < 2 || rect.Height < 2)
            {
                throw new InvalidOperationException("The target window cannot be captured now.");
            }

            using (var bitmap = new Bitmap(rect.Width, rect.Height, PixelFormat.Format32bppArgb))
            using (var graphics = Graphics.FromImage(bitmap))
            {
                bool captured = false;
                IntPtr deviceContext = graphics.GetHdc();
                try
                {
                    captured = NativeMethods.PrintWindow(
                        handle,
                        deviceContext,
                        NativeMethods.PwRenderFullContent);
                }
                finally
                {
                    graphics.ReleaseHdc(deviceContext);
                }

                // GPU/browser windows may reject PrintWindow or return a blank frame.
                // Fall back to visible screen pixels for maximum compatibility.
                if (!captured || IsProbablyBlank(bitmap))
                {
                    graphics.Clear(Color.Transparent);
                    graphics.CopyFromScreen(
                        rect.Left,
                        rect.Top,
                        0,
                        0,
                        new Size(rect.Width, rect.Height),
                        CopyPixelOperation.SourceCopy);
                }

                WriteToClipboard(bitmap);
                return NativeMethods.GetClipboardSequenceNumber();
            }
        }

        private static void WriteToClipboard(Bitmap bitmap)
        {
            // The clipboard can briefly be locked by another application. Retry a
            // few times so a normal capture does not fail during that short window.
            ExternalException lastError = null;
            for (int attempt = 0; attempt < 6; attempt++)
            {
                try
                {
                    Clipboard.SetImage(bitmap);
                    return;
                }
                catch (ExternalException error)
                {
                    lastError = error;
                    Thread.Sleep(40);
                }
            }

            throw new InvalidOperationException("Unable to copy the snapshot to the clipboard.", lastError);
        }

        private static bool IsProbablyBlank(Bitmap bitmap)
        {
            const int columns = 8;
            const int rows = 8;
            int blackOrTransparent = 0;
            int sampleCount = columns * rows;

            for (int row = 0; row < rows; row++)
            {
                for (int column = 0; column < columns; column++)
                {
                    int x = Math.Min(bitmap.Width - 1, (column * bitmap.Width) / columns);
                    int y = Math.Min(bitmap.Height - 1, (row * bitmap.Height) / rows);
                    Color sample = bitmap.GetPixel(x, y);
                    if (sample.A < 8 || (sample.R < 8 && sample.G < 8 && sample.B < 8))
                    {
                        blackOrTransparent++;
                    }
                }
            }

            return blackOrTransparent >= (sampleCount * 9) / 10;
        }
    }
}
