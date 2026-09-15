using System;
using System.Collections.Generic;
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
            internal static PetController Pet;

            /// <summary>
            /// 桌面呈现形式:悬浮球(默认)或桌宠,持久化于 ui.mode。
            /// 两者互斥——pet 为真时悬浮球保持隐藏(SnapshotBubbleForm.SetVisibleCore),
            /// 悬浮球显示时桌宠不存在实例。
            /// </summary>
            internal static bool IsPetMode;

            /// <summary>桌宠形象:pet 素材目录下的子目录名,持久化于 pet.skin。</summary>
            internal static string CurrentPetSkin = "shuangyan-crate-duo";

            /// <summary>可用形象中解析当前选择;没有任何素材时返回 null(桌宠不可用)。</summary>
            internal static string ResolvePetSkin()
            {
                List<string> skins = PetAssets.AvailableSkins();
                if (skins.Contains(CurrentPetSkin))
                {
                    return CurrentPetSkin;
                }
                return skins.Count > 0 ? skins[0] : null;
            }

            /// <summary>切换桌宠形象;桌宠显示中时立即换装(位置沿用 desktoppet.x/y)。</summary>
            internal static void SwitchPetSkin(string skin)
            {
                if (skin == CurrentPetSkin)
                {
                    return;
                }
                CurrentPetSkin = skin;
                AppSettings.Write("pet.skin", skin);

                if (IsPetMode && Pet != null)
                {
                    Pet.Dispose();
                    Pet = new PetController(skin);
                    Pet.Start();
                }
            }

            /// <summary>切换悬浮球/桌宠形式,设置窗口保存时调用;无素材可切桌宠时拒绝并提示。</summary>
            internal static void SwitchUiMode(bool petMode)
            {
                if (petMode == IsPetMode)
                {
                    return;
                }
                if (petMode && ResolvePetSkin() == null)
                {
                    if (Toast != null)
                    {
                        Toast.Show("未找到桌宠素材(" + PetAssets.Root + "\\<形象>\\*.gif),无法启用桌宠", ToastKind.Warning);
                    }
                    return;
                }
                IsPetMode = petMode;
                AppSettings.Write("ui.mode", petMode ? "pet" : "bubble");

                if (petMode)
                {
                    if (Pet == null)
                    {
                        Pet = new PetController(CurrentPetSkin);
                    }
                    Pet.Start();
                    Form mainForm = MainForm;
                    if (mainForm != null && !mainForm.IsDisposed)
                    {
                        mainForm.Hide();
                    }
                }
                else
                {
                    if (Pet != null)
                    {
                        Pet.Dispose();
                        Pet = null;
                    }
                    Form mainForm = MainForm;
                    if (mainForm != null && !mainForm.IsDisposed)
                    {
                        mainForm.Show();
                    }
                }

                if (Toast != null)
                {
                    Toast.Show(petMode ? "已切换到桌宠模式" : "已切换到悬浮球模式", ToastKind.Success);
                }
            }

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

        /// <summary>截取「上一个前台应用」窗口（悬浮球当前显示图标的目标应用）。</summary>
        internal static void CapturePreviousApp()
        {
            TargetAppTracker.TargetInfo target = Tracker != null ? Tracker.Previous : null;
            if (target == null || target.WindowHandle == IntPtr.Zero)
            {
                Toast.Show("还没有上一个应用可截取", ToastKind.Warning);
                return;
            }
            CaptureWindow(target.WindowHandle);
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
