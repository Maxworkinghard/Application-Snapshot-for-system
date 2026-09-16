using System;
using System.Threading;
using System.Windows.Forms;

namespace AppSnapshot
{
    internal static class Program
    {
        [STAThread]
        private static void Main()
        {
            bool created;
            using (var singleInstance = new Mutex(true, "Local\\AppSnapshot.SingleInstance", out created))
            {
                if (!created)
                {
                    return;
                }

                Application.SetHighDpiMode(HighDpiMode.SystemAware);
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);

                App.Toast = new ToastController();
                App.Polish = new PolishController();
                App.Recording = new RecordingService();
                App.Panels = new ActionPanelController();
                App.Recording.StateChanged += OnRecordingStateChanged;

                using (App.HotKeys = new HotKeyManager())
                using (App.Tray = new TrayController())
                {
                    App.HotKeys.CapturePressed += delegate
                    {
                        App.CaptureWindow(App.CurrentTargetWindow());
                    };
                    App.HotKeys.RecordPressed += delegate
                    {
                        App.ToggleRecording();
                    };
                    App.HotKeys.PreviousAppPressed += delegate
                    {
                        App.CapturePreviousApp();
                    };
                    App.HotKeys.PolishPressed += delegate
                    {
                        App.Polish.ToggleFromMenu();
                    };

                    var mainForm = new SnapshotBubbleForm();
                    App.MainForm = mainForm;
                    App.Tracker = mainForm.Tracker;

                    // 桌面呈现形式:默认悬浮窗;仅当设置选择了桌宠且本地素材可用时才启用桌宠,
                    // 悬浮窗只保留截图宿主/Tracker 职能,桌宠模式下常驻隐藏。
                    App.IsPetMode = AppSettings.Read("ui.mode") == "pet" && App.ResolvePetSkin() != null;
                    string savedSkin = AppSettings.Read("pet.skin");
                    if (!string.IsNullOrEmpty(savedSkin))
                    {
                        App.CurrentPetSkin = savedSkin;
                    }
                    if (App.IsPetMode)
                    {
                        // 隐藏态也要先建好句柄:App.CaptureWindow 的 BeginInvoke 依赖它
                        NativeMethods.IsWindow(mainForm.Handle);
                        mainForm.StartRuntime();
                        App.Pet = new PetController(App.CurrentPetSkin);
                        App.Pet.Start();
                    }

                    ApplyStartupShortcuts();

                    Application.Run(mainForm);

                    App.MainForm = null;
                }
            }
        }

        /// <summary>
        /// 启动时应用已保存的快捷键（可选绑定，未绑定项直接跳过）；
        /// 被占用的组合降级为不启用并提示，其余快捷键照常生效。
        /// </summary>
        private static void ApplyStartupShortcuts()
        {
            var specs = new[]
            {
                HotKeyPreferences.Capture,
                HotKeyPreferences.Record,
                HotKeyPreferences.PreviousApp,
                HotKeyPreferences.Polish
            };
            int[] failures = App.HotKeys.Apply(specs[0], specs[1], specs[2], specs[3]);
            if (failures == null)
            {
                return;
            }
            foreach (int index in failures)
            {
                specs[index] = null;
            }
            App.HotKeys.Apply(specs[0], specs[1], specs[2], specs[3]);

            var names = new[] { "截取当前应用", "录制", "截取上一个应用", "润色" };
            var occupied = new System.Collections.Generic.List<string>();
            foreach (int index in failures)
            {
                occupied.Add(names[index]);
            }
            App.Toast.Show(
                string.Join("、", occupied.ToArray()) + " 的快捷键被占用，已暂时停用，可在设置中更换",
                ToastKind.Warning);
        }

        /// <summary>
        /// gdigrab 录制的是屏幕像素区域，悬浮窗若与目标窗口重叠会入镜；
        /// 录制期间隐藏悬浮窗（macOS 端为窗口级捕获，无需隐藏）。
        /// </summary>
        private static void OnRecordingStateChanged()
        {
            Form mainForm = App.MainForm;
            if (mainForm == null || mainForm.IsDisposed)
            {
                return;
            }

            RecordingService.State state = App.Recording.CurrentState;
            bool recordingOrPending = state == RecordingService.State.Starting
                || state == RecordingService.State.Recording;
            if (App.Pet != null)
            {
                // 猫与悬浮球一样不能入镜(gdigrab 录屏幕像素)
                App.Pet.SetHiddenForRecording(recordingOrPending);
            }
            if (recordingOrPending && mainForm.Visible)
            {
                App.Panels.Close();
                mainForm.Hide();
            }
            else if (!recordingOrPending && !mainForm.Visible)
            {
                // 桌宠模式下 SetVisibleCore 会拦截,悬浮球保持隐藏
                mainForm.Show();
            }
        }
    }
}
