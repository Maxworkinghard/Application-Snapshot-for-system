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
        private readonly ToolStripMenuItem captureItem;
        private readonly ToolStripMenuItem recordItem;

        internal TrayController()
        {
            captureItem = new ToolStripMenuItem(
                "截取当前应用窗口", null,
                delegate { App.CaptureWindow(App.CurrentTargetWindow()); });

            recordItem = new ToolStripMenuItem(
                "录制当前应用窗口", null,
                delegate { App.ToggleRecording(); });

            var chooseItem = new ToolStripMenuItem(
                "选择其他窗口…", null,
                delegate
                {
                    App.Panels.ShowList();
                });

            var modeItem = new ToolStripMenuItem(
                "切换到桌宠模式", null,
                delegate { App.SwitchUiMode(!App.IsPetMode); });

            var skinDefaultItem = new ToolStripMenuItem(
                "默认形象", null,
                delegate { App.SwitchPetSkin(""); });
            var skinAltItem = new ToolStripMenuItem(
                "备用形象", null,
                delegate { App.SwitchPetSkin(""); });
            var skinItem = new ToolStripMenuItem("桌宠形象");
            skinItem.DropDownItems.Add(skinDefaultItem);
            skinItem.DropDownItems.Add(skinAltItem);

            var saveDirectoryItem = new ToolStripMenuItem(
                "设置保存目录…", null,
                delegate { ChooseSaveDirectory(); });

            var settingsItem = new ToolStripMenuItem(
                "设置…", null,
                delegate { OpenSettings(); });

            var quitItem = new ToolStripMenuItem(
                "退出应用快照", null,
                delegate
                {
                    // 录制中退出需先收尾 ffmpeg，否则子进程成为孤儿继续写盘
                    if (App.Recording != null)
                    {
                        App.Recording.ShutdownOnAppExit();
                    }
                    Application.Exit();
                });

            var menu = new ContextMenuStrip();
            menu.Items.Add(captureItem);
            menu.Items.Add(recordItem);
            menu.Items.Add(chooseItem);
            menu.Items.Add(modeItem);
            menu.Items.Add(skinItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(saveDirectoryItem);
            menu.Items.Add(settingsItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(quitItem);
            menu.Opening += delegate
            {
                modeItem.Text = App.IsPetMode ? "切换到悬浮球模式" : "切换到桌宠模式";
                skinDefaultItem.Checked = App.CurrentPetSkin == "";
                skinAltItem.Checked = App.CurrentPetSkin == "";
            };

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

            RefreshShortcuts();
            App.Recording.StateChanged += UpdateRecordItem;
        }

        /// <summary>快捷键配置变化后刷新菜单文字（无绑定的项不显示组合键）。</summary>
        internal void RefreshShortcuts()
        {
            HotKeySpec capture = HotKeyPreferences.Capture;
            captureItem.Text = "截取当前应用窗口"
                + (capture == null ? "" : "（" + capture.DisplayName + "）");
            UpdateRecordItem();
        }

        internal void UpdateRecordItem()
        {
            HotKeySpec record = HotKeyPreferences.Record;
            string recordSuffix = record == null ? "" : "（" + record.DisplayName + "）";
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
                    recordItem.Text = "录制当前应用窗口" + recordSuffix;
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

        private static void OpenSettings()
        {
            using (var form = new SettingsForm())
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
