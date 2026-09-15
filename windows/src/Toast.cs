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

        /// <summary>每次弹出 Toast 时触发;桌宠据此做成功/失败/警告反应动画。</summary>
        internal event Action<ToastKind> Notified;

        public void Show(string message, ToastKind kind)
        {
            Close();

            Action<ToastKind> notified = Notified;
            if (notified != null)
            {
                notified(kind);
            }

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
            // 常量是 96 DPI 设计值，经 UiScale 换算成当前屏幕的物理像素
            ClientSize = UiScale.Px(340, 46);
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
                workingArea.Right - Width - UiScale.Px(16),
                workingArea.Bottom - Height - UiScale.Px(12));
        }

        internal static Screen AnchorScreen()
        {
            // 桌宠模式下锚定到猫的位置;悬浮球隐藏时其 Bounds 已不代表屏幕锚点
            if (App.IsPetMode && App.Pet != null)
            {
                Rectangle petBounds = App.Pet.CurrentBounds;
                if (!petBounds.IsEmpty)
                {
                    var petCenter = new Point(petBounds.Left + petBounds.Width / 2, petBounds.Top + petBounds.Height / 2);
                    return Screen.FromPoint(petCenter);
                }
            }

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
                int radius = UiScale.Px(10);
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
                int iconX = UiScale.Px(14);
                int iconY = UiScale.Px(12);
                int iconDiameter = UiScale.Px(22);
                e.Graphics.FillEllipse(iconBrush, new Rectangle(iconX, iconY, iconDiameter, iconDiameter));
                var iconSize = e.Graphics.MeasureString(iconText, iconFont);
                e.Graphics.DrawString(
                    iconText, iconFont, Brushes.White,
                    new RectangleF(iconX + (iconDiameter - iconSize.Width) / 2f, iconY + (iconDiameter - iconSize.Height) / 2f, iconSize.Width, iconSize.Height));
            }

            using (var textBrush = new SolidBrush(Color.FromArgb(238, 238, 240)))
            {
                var bounds = new RectangleF(UiScale.Px(48), 0, ClientSize.Width - UiScale.Px(48) - UiScale.Px(14), ClientSize.Height);
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
