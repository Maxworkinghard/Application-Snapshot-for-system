using System;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 托盘图标与菜单：截取 / 录制 / 选择窗口 / 设置保存目录 / 润色设置 / 退出。
    /// 对应 macOS 端的菜单栏状态项。
    /// </summary>
    internal sealed class TrayController : IDisposable
    {
        private readonly NotifyIcon trayIcon;
        private readonly ToolStripMenuItem recordItem;

        internal TrayController()
        {
            var captureItem = new ToolStripMenuItem(
                "截取当前应用窗口（Alt+Shift+2）", null,
                delegate { App.CaptureWindow(App.CurrentTargetWindow()); });

            recordItem = new ToolStripMenuItem(
                "录制当前应用窗口（Alt+Shift+R）", null,
                delegate { App.ToggleRecording(); });

            var chooseItem = new ToolStripMenuItem(
                "选择其他窗口…", null,
                delegate
                {
                    App.Panels.ShowList();
                });

            var saveDirectoryItem = new ToolStripMenuItem(
                "设置保存目录…", null,
                delegate { ChooseSaveDirectory(); });

            var polishSettingsItem = new ToolStripMenuItem(
                "润色设置…", null,
                delegate { OpenPolishSettings(); });

            var quitItem = new ToolStripMenuItem(
                "退出应用快照", null,
                delegate { Application.Exit(); });

            var menu = new ContextMenuStrip();
            menu.Items.Add(captureItem);
            menu.Items.Add(recordItem);
            menu.Items.Add(chooseItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(saveDirectoryItem);
            menu.Items.Add(polishSettingsItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(quitItem);

            trayIcon = new NotifyIcon
            {
                Icon = System.Drawing.SystemIcons.Application,
                Text = "应用快照",
                ContextMenuStrip = menu,
                Visible = true
            };
            trayIcon.DoubleClick += delegate
            {
                App.CaptureWindow(App.CurrentTargetWindow());
            };

            App.Recording.StateChanged += UpdateRecordItem;
        }

        internal void UpdateRecordItem()
        {
            switch (App.Recording.CurrentState)
            {
                case RecordingService.State.Recording:
                    recordItem.Text = "停止录制";
                    recordItem.Enabled = true;
                    break;
                case RecordingService.State.Starting:
                    recordItem.Text = "正在开始录制…";
                    recordItem.Enabled = false;
                    break;
                case RecordingService.State.Stopping:
                    recordItem.Text = "正在保存录制…";
                    recordItem.Enabled = false;
                    break;
                default:
                    recordItem.Text = "录制当前应用窗口（Alt+Shift+R）";
                    recordItem.Enabled = true;
                    break;
            }
        }

        private static void ChooseSaveDirectory()
        {
            using (var dialog = new FolderBrowserDialog
            {
                Description = "选择录制文件的保存目录（截图只进剪贴板，不产生文件）",
                ShowNewFolderButton = true
            })
            {
                try
                {
                    dialog.SelectedPath = AppSettings.SaveDirectory;
                }
                catch
                {
                }
                if (dialog.ShowDialog() == DialogResult.OK && !string.IsNullOrEmpty(dialog.SelectedPath))
                {
                    AppSettings.SaveDirectory = dialog.SelectedPath;
                    App.Toast.Show("录制将保存到 " + dialog.SelectedPath, ToastKind.Success);
                }
            }
        }

        private static void OpenPolishSettings()
        {
            using (var form = new PolishSettingsForm())
            {
                form.ShowDialog();
            }
        }

        public void Dispose()
        {
            if (App.Recording != null)
            {
                App.Recording.StateChanged -= UpdateRecordItem;
            }
            trayIcon.Visible = false;
            trayIcon.Dispose();
        }
    }
}
