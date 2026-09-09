using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace AppSnapshot
{
    internal enum ToastKind
    {
        Success,
        Warning,
        Error
    }

    /// <summary>
    /// 轻量 Toast 提示：屏幕右下角的小浮层，显示 1.8 秒后自动消失，
    /// 对应 macOS 端的 ToastController。
    /// </summary>
    internal sealed class ToastController : IDisposable
    {
        private ToastForm currentForm;

        public void Show(string message, ToastKind kind)
        {
            Close();

            var form = new ToastForm(message, kind);
            currentForm = form;
            form.ShowToast();
        }

        public void Close()
        {
            if (currentForm != null)
            {
                currentForm.HideToast();
                currentForm.Dispose();
                currentForm = null;
            }
        }

        public void Dispose()
        {
            Close();
        }
    }

    internal sealed class ToastForm : Form
    {
        private readonly string message;
        private readonly ToastKind kind;
        private static ToastForm visibleForm;

        internal ToastForm(string message, ToastKind kind)
        {
            this.message = message;
            this.kind = kind;

            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            TopMost = true;
            BackColor = Color.FromArgb(32, 32, 36);
            ForeColor = Color.White;
            ClientSize = new Size(340, 46);
            Font = new Font("Microsoft YaHei UI", 9F);

            SetStyle(
                ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.ResizeRedraw
                | ControlStyles.UserPaint,
                true);

            // 跟随悬浮球所在屏幕（多显示器时不再总是落到主屏）
            Rectangle workingArea = AnchorScreen().WorkingArea;
            Location = new Point(
                workingArea.Right - Width - 16,
                workingArea.Bottom - Height - 12);
        }

        internal static Screen AnchorScreen()
        {
            Form bubble = App.MainForm;
            if (bubble != null && !bubble.IsDisposed)
            {
                Rectangle bounds = bubble.Bounds;
                var center = new Point(bounds.Left + bounds.Width / 2, bounds.Top + bounds.Height / 2);
                return Screen.FromPoint(center);
            }
            return Screen.FromPoint(Cursor.Position);
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

        public void ShowToast()
        {
            if (visibleForm != null && !visibleForm.IsDisposed)
            {
                visibleForm.HideToast();
            }
            visibleForm = this;

            using (var path = new GraphicsPath())
            {
                int radius = 10;
                var rect = new Rectangle(1, 1, ClientSize.Width - 2, ClientSize.Height - 2);
                path.AddArc(rect.X, rect.Y, radius * 2, radius * 2, 180, 90);
                path.AddArc(rect.Right - radius * 2, rect.Y, radius * 2, radius * 2, 270, 90);
                path.AddArc(rect.Right - radius * 2, rect.Bottom - radius * 2, radius * 2, radius * 2, 0, 90);
                path.AddArc(rect.X, rect.Bottom - radius * 2, radius * 2, radius * 2, 90, 90);
                path.CloseFigure();
                Region = new Region(path);
            }

            Show();
            var timer = new Timer { Interval = 1800 };
            timer.Tick += delegate
            {
                timer.Stop();
                timer.Dispose();
                HideToast();
            };
            timer.Start();
        }

        public void HideToast()
        {
            if (!IsDisposed)
            {
                Hide();
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            e.Graphics.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;

            Color iconColor;
            string iconText;
            switch (kind)
            {
                case ToastKind.Success:
                    iconColor = Color.FromArgb(82, 196, 26);
                    iconText = "\u2713";
                    break;
                case ToastKind.Error:
                    iconColor = Color.FromArgb(245, 108, 108);
                    iconText = "\u2715";
                    break;
                default:
                    iconColor = Color.FromArgb(230, 162, 60);
                    iconText = "!";
                    break;
            }

            using (var iconBrush = new SolidBrush(iconColor))
            using (var iconFont = new Font(Font.FontFamily, 13F, FontStyle.Bold))
            {
                e.Graphics.FillEllipse(iconBrush, new Rectangle(14, 12, 22, 22));
                var iconSize = e.Graphics.MeasureString(iconText, iconFont);
                e.Graphics.DrawString(
                    iconText, iconFont, Brushes.White,
                    new RectangleF(14 + (22 - iconSize.Width) / 2f, 12 + (22 - iconSize.Height) / 2f, iconSize.Width, iconSize.Height));
            }

            using (var textBrush = new SolidBrush(Color.FromArgb(238, 238, 240)))
            {
                var bounds = new RectangleF(48, 0, ClientSize.Width - 48 - 14, ClientSize.Height);
                var format = new StringFormat
                {
                    Alignment = StringAlignment.Near,
                    LineAlignment = StringAlignment.Center,
                    Trimming = StringTrimming.EllipsisCharacter,
                    FormatFlags = StringFormatFlags.LineLimit
                };
                e.Graphics.DrawString(message, Font, textBrush, bounds, format);
            }
        }

        protected override void OnHandleDestroyed(EventArgs e)
        {
            if (visibleForm == this)
            {
                visibleForm = null;
            }
            base.OnHandleDestroyed(e);
        }
    }
}
