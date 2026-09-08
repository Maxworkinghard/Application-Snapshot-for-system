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

                NativeMethods.SetProcessDPIAware();
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

                    var mainForm = new SnapshotBubbleForm();
                    App.MainForm = mainForm;
                    App.Tracker = mainForm.Tracker;

                    App.HotKeys.RegisterAll();

                    Application.Run(mainForm);

                    App.MainForm = null;
                }
            }
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
            if (recordingOrPending && mainForm.Visible)
            {
                App.Panels.Close();
                mainForm.Hide();
            }
            else if (!recordingOrPending && !mainForm.Visible)
            {
                mainForm.Show();
            }
        }
    }
}
