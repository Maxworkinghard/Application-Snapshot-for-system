using System;
using System.Drawing;
using System.Text;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>全局装配：各模块通过它互相调用，避免构造顺序耦合。</summary>
    internal static class App
    {
        internal static SnapshotBubbleForm MainForm;
        internal static ToastController Toast;
        internal static TargetAppTracker Tracker;
        internal static RecordingService Recording;
        internal static PolishController Polish;
        internal static ActionPanelController Panels;
        internal static HotKeyManager HotKeys;
        internal static TrayController Tray;

        /// <summary>托盘/热键/面板共用的「当前应用窗口」：前台外部窗口，否则回退到上一个应用。</summary>
        internal static IntPtr CurrentTargetWindow()
        {
            IntPtr foreground = NativeMethods.GetForegroundWindow();
            if (foreground != IntPtr.Zero)
            {
                uint processId;
                NativeMethods.GetWindowThreadProcessId(foreground, out processId);
                if (processId != 0 && processId != CurrentProcessId())
                {
                    return foreground;
                }
            }

            TargetAppTracker.TargetInfo target = Tracker != null ? Tracker.Current : null;
            if (target == null)
            {
                target = Tracker != null ? Tracker.Previous : null;
            }
            return target != null ? target.WindowHandle : IntPtr.Zero;
        }

        internal static string WindowTitle(IntPtr window)
        {
            if (window == IntPtr.Zero)
            {
                return "";
            }
            int length = NativeMethods.GetWindowTextLength(window);
            if (length <= 0)
            {
                return "";
            }
            var text = new StringBuilder(length + 1);
            NativeMethods.GetWindowText(window, text, text.Capacity);
            return text.ToString();
        }

        internal static string WindowProcessName(IntPtr window)
        {
            uint processId;
            NativeMethods.GetWindowThreadProcessId(window, out processId);
            try
            {
                using (var process = System.Diagnostics.Process.GetProcessById((int)processId))
                {
                    return process.ProcessName;
                }
            }
            catch
            {
                return "应用";
            }
        }

        /// <summary>截取指定窗口（悬浮窗隐藏期间执行），对应 macOS 端的 captureWindow。</summary>
        internal static void CaptureWindow(IntPtr window)
        {
            Form mainForm = MainForm;
            if (mainForm == null)
            {
                return;
            }
            mainForm.BeginInvoke(new MethodInvoker(delegate
            {
                if (window == IntPtr.Zero || !NativeMethods.IsWindow(window))
                {
                    Toast.Show("没有找到可截取的窗口", ToastKind.Warning);
                    return;
                }
                ((SnapshotBubbleForm)mainForm).CaptureWindow(window);
            }));
        }

        /// <summary>录制开关：面板/托盘/热键共用。</summary>
        internal static void ToggleRecording()
        {
            RecordingService.State state = Recording.CurrentState;
            if (state == RecordingService.State.Idle)
            {
                IntPtr window = CurrentTargetWindow();
                if (window == IntPtr.Zero)
                {
                    Toast.Show("没有找到可录制的应用", ToastKind.Warning);
                    return;
                }
                Recording.Start(window, WindowTitle(window));
            }
            else if (state == RecordingService.State.Recording)
            {
                Recording.Stop();
            }
        }

        private static uint CurrentProcessId()
        {
            return (uint)System.Diagnostics.Process.GetCurrentProcess().Id;
        }
    }
}
