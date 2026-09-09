using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 录制中浮动控制条：屏幕底部中央的小药丸，显示已录时长 + 停止按钮。
    /// 对应 macOS 端的 RecordingControlsPanel。
    /// </summary>
    internal sealed class RecordingControlsForm : Form
    {
        private readonly Timer tickTimer;
        private readonly DateTime startTime;
        private readonly Label timeLabel;
        private readonly Action stopHandler;

        internal RecordingControlsForm(DateTime startTime, Action onStop)
        {
            this.startTime = startTime;
            this.stopHandler = onStop;

            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            TopMost = true;
            BackColor = Color.FromArgb(32, 32, 36);
            // 常量是 96 DPI 设计值，经 UiScale 换算成当前屏幕的物理像素
            ClientSize = UiScale.Px(228, 42);
            Font = new Font("Microsoft YaHei UI", 9F);

            SetStyle(
                ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.ResizeRedraw
                | ControlStyles.UserPaint,
                true);

            var stopButton = new Button
            {
                Text = "停止",
                AutoSize = false,
                Size = UiScale.Px(64, 28),
                Location = new Point(ClientSize.Width - UiScale.Px(72), UiScale.Px(7)),
                FlatStyle = FlatStyle.Flat,
                BackColor = Color.FromArgb(196, 43, 28),
                ForeColor = Color.White,
                Font = new Font("Microsoft YaHei UI", 9F, FontStyle.Bold),
                Cursor = Cursors.Hand
            };
            stopButton.FlatAppearance.BorderSize = 0;
            stopButton.Click += delegate
            {
                Action handler = stopHandler;
                if (handler != null)
                {
                    handler();
                }
            };

            timeLabel = new Label
            {
                Text = "00:00",
                AutoSize = false,
                Size = UiScale.Px(60, 42),
                Location = new Point(UiScale.Px(96), 0),
                ForeColor = Color.White,
                TextAlign = ContentAlignment.MiddleCenter,
                Font = new Font("Consolas", 11F, FontStyle.Bold)
            };

            Controls.Add(stopButton);
            Controls.Add(timeLabel);

            // 跟随悬浮球所在屏幕（多显示器时不再总是落到主屏）
            Rectangle workingArea = ToastForm.AnchorScreen().WorkingArea;
            Location = new Point(
                workingArea.Left + (workingArea.Width - Width) / 2,
                workingArea.Bottom - Height - UiScale.Px(48));

            tickTimer = new Timer { Interval = 500 };
            tickTimer.Tick += delegate { UpdateElapsed(); };
            tickTimer.Start();
            UpdateElapsed();
        }

        protected override bool ShowWithoutActivation
        {
            get { return true; }
        }

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams parameters = base.CreateParams;
                parameters.ExStyle |= NativeMethods.WsExToolWindow | NativeMethods.WsExNoActivate;
                return parameters;
            }
        }

        protected override void OnShown(EventArgs e)
        {
            base.OnShown(e);
            using (var path = new GraphicsPath())
            {
                int radius = UiScale.Px(21);
                var rect = new Rectangle(1, 1, ClientSize.Width - 2, ClientSize.Height - 2);
                path.AddArc(rect.X, rect.Y, radius * 2, radius * 2, 180, 90);
                path.AddArc(rect.Right - radius * 2, rect.Y, radius * 2, radius * 2, 270, 90);
                path.AddArc(rect.Right - radius * 2, rect.Bottom - radius * 2, radius * 2, radius * 2, 0, 90);
                path.AddArc(rect.X, rect.Bottom - radius * 2, radius * 2, radius * 2, 90, 90);
                path.CloseFigure();
                Region = new Region(path);
            }
        }

        private void UpdateElapsed()
        {
            if (IsDisposed)
            {
                return;
            }
            int seconds = (int)Math.Max(0, (DateTime.Now - startTime).TotalSeconds);
            timeLabel.Text = string.Format("{0:00}:{1:00}", seconds / 60, seconds % 60);
        }

        public void ShowControls()
        {
            Show();
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                tickTimer.Stop();
                tickTimer.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
