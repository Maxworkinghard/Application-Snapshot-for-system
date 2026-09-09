using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Windows.Forms;

namespace AppSnapshot
{
    // Non-activating floating bubble showing the previous foreground app.
    // Current/Previous behavior follows Maxworkinghard/quick's desktop pet.
    internal sealed class SnapshotBubbleForm : Form
    {
        // 绘制坐标系的设计尺寸：OnPaint 里所有常量都基于 168x168 的设计稿，
        // 通过 ScaleTransform 等比缩放到实际大小，调整外观尺寸只改 BubbleSize。
        private const int DesignSize = 168;
        // 逻辑尺寸（96 DPI 设计值，UiScale 换算成物理像素）。
        // 168 在 200% 缩放屏幕上视觉接近 4.5cm，用户反馈太大；96 约 2.5cm。
        private const int BubbleSize = 96;
        private const int DragThreshold = 4;
        private const int PositionMargin = 8;

        private readonly System.Windows.Forms.Timer _clipboardClearTimer;
        private readonly ToolTip _toolTip;
        private readonly TargetAppTracker _tracker;

        private TargetAppTracker.TargetInfo _target;
        private Bitmap _targetIcon;
        private Point _pointerDown;
        private Point _windowDown;
        private bool _pointerPressed;
        private bool _dragging;
        private uint _snapshotClipboardSequence;

        internal TargetAppTracker Tracker
        {
            get { return _tracker; }
        }

        public SnapshotBubbleForm()
        {
            _tracker = new TargetAppTracker();
            _tracker.PreviousChanged += OnPreviousChanged;

            AutoScaleMode = AutoScaleMode.Dpi;
            BackColor = Color.White;
            ClientSize = new Size(UiScale.Px(BubbleSize), UiScale.Px(BubbleSize));
            FormBorderStyle = FormBorderStyle.None;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowIcon = false;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            Text = "AppSnapshot";
            TopMost = true;

            // Clip the actual window to the white circle instead of using a
            // color-key transparency. Color-keyed anti-aliasing mixed magenta
            // into the edge and produced the colored halo seen on screen.
            using (var bubblePath = new GraphicsPath())
            {
                // 与设计稿 8px 边距等比换算
                int margin = ClientSize.Width * 8 / DesignSize;
                int diameter = ClientSize.Width - margin * 2;
                bubblePath.AddEllipse(new Rectangle(margin, margin, diameter, diameter));
                Region = new Region(bubblePath);
            }

            SetStyle(
                ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.ResizeRedraw
                | ControlStyles.UserPaint,
                true);

            var menu = new ContextMenuStrip();
            var settingsItem = new ToolStripMenuItem("设置…", null, delegate
            {
                using (var form = new SettingsForm())
                {
                    form.ShowDialog();
                }
            });
            menu.Items.Add(settingsItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add("\u9000\u51FA", null, delegate { Application.Exit(); });
            ContextMenuStrip = menu;

            _toolTip = new ToolTip
            {
                InitialDelay = 350,
                ReshowDelay = 150,
                AutoPopDelay = 3000,
                ShowAlways = true
            };

            _clipboardClearTimer = new System.Windows.Forms.Timer { Interval = 60000 };
            _clipboardClearTimer.Tick += delegate
            {
                _clipboardClearTimer.Stop();
                if (_snapshotClipboardSequence != 0
                    && NativeMethods.GetClipboardSequenceNumber() == _snapshotClipboardSequence)
                {
                    try
                    {
                        Clipboard.Clear();
                    }
                    catch
                    {
                        // A busy clipboard is harmless here; never disturb newer content.
                    }
                }
                _snapshotClipboardSequence = 0;
            };

            Location = RestoreOrClampPosition();
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
            _tracker.Start();
            UpdateTarget(_tracker.Previous);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            e.Graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            e.Graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            e.Graphics.CompositingQuality = CompositingQuality.HighQuality;

            // 把 168x168 设计坐标系等比缩放到实际尺寸，
            // 下面的常量全部是设计稿像素，不随 BubbleSize 改动。
            float scale = ClientSize.Width / (float)DesignSize;
            e.Graphics.ScaleTransform(scale, scale);

            Rectangle body = new Rectangle(8, 8, 152, 152);
            using (var fill = new SolidBrush(Color.White))
            {
                e.Graphics.FillEllipse(fill, body);
            }

            if (_targetIcon != null)
            {
                e.Graphics.DrawImage(_targetIcon, new Rectangle(28, 28, 112, 112));
            }
            else
            {
                using (var placeholder = new Pen(Color.FromArgb(215, 210, 214, 220), 4f))
                {
                    e.Graphics.DrawEllipse(placeholder, new Rectangle(62, 62, 44, 44));
                    e.Graphics.DrawLine(placeholder, 84, 52, 84, 116);
                    e.Graphics.DrawLine(placeholder, 52, 84, 116, 84);
                }
            }
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            if (e.Button == MouseButtons.Left)
            {
                _pointerPressed = true;
                _dragging = false;
                _pointerDown = Cursor.Position;
                _windowDown = Location;
                Capture = true;
            }

            base.OnMouseDown(e);
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            if (_pointerPressed && (Control.MouseButtons & MouseButtons.Left) != 0)
            {
                Point current = Cursor.Position;
                int deltaX = current.X - _pointerDown.X;
                int deltaY = current.Y - _pointerDown.Y;
                if (!_dragging
                    && (Math.Abs(deltaX) >= UiScale.Px(DragThreshold) || Math.Abs(deltaY) >= UiScale.Px(DragThreshold)))
                {
                    _dragging = true;
                }

                if (_dragging)
                {
                    Location = ClampToWorkingArea(
                        new Point(_windowDown.X + deltaX, _windowDown.Y + deltaY),
                        Size,
                        current);
                    if (App.Panels != null)
                    {
                        App.Panels.Reposition();
                    }
                }
            }

            base.OnMouseMove(e);
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            if (e.Button == MouseButtons.Left && _pointerPressed)
            {
                _pointerPressed = false;
                Capture = false;
                if (_dragging)
                {
                    SavePosition(Location);
                }
                else
                {
                    if (App.Panels != null)
                    {
                        App.Panels.Toggle();
                    }
                }
            }

            base.OnMouseUp(e);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                _tracker.PreviousChanged -= OnPreviousChanged;
                _tracker.Dispose();
                _clipboardClearTimer.Dispose();
                _toolTip.Dispose();
                if (_targetIcon != null)
                {
                    _targetIcon.Dispose();
                }
            }

            base.Dispose(disposing);
        }

        private void OnPreviousChanged(object sender, EventArgs e)
        {
            if (IsDisposed || Disposing)
            {
                return;
            }

            if (InvokeRequired)
            {
                BeginInvoke(new MethodInvoker(delegate { UpdateTarget(_tracker.Previous); }));
                return;
            }

            UpdateTarget(_tracker.Previous);
        }

        private void UpdateTarget(TargetAppTracker.TargetInfo target)
        {
            if (target == null)
            {
                return;
            }

            // 图标按实际绘制大小加载：设计稿 112px 随 BubbleSize 等比换算
            Bitmap icon = WindowIconLoader.LoadWindowIcon(
                target.WindowHandle, (int)target.ProcessId, UiScale.Px(BubbleSize * 112 / DesignSize));
            Bitmap previousIcon = _targetIcon;
            _target = target;
            _targetIcon = icon;
            _toolTip.SetToolTip(this, "\u70b9\u51fb\u6253\u5f00\u529f\u80fd\u83dc\u5355 " + target.ProcessName);
            Invalidate();

            if (previousIcon != null)
            {
                previousIcon.Dispose();
            }
        }

        private bool _capturePending;

        /// <summary>
        /// 截取指定窗口并复制到剪贴板（悬浮窗短暂隐藏避免入镜）。
        /// 隐藏后用一个一次性 Timer 延迟执行截图：返回消息循环让窗口完成隐藏重绘，
        /// 而不是 DoEvents()+Sleep()——后者会重入消息泵，让热键/托盘事件穿插进截图流程。
        /// 最小化的窗口没有渲染内容（GetWindowRect 落在 -32000 且 PrintWindow 得到空白），
        /// 必须先 SW_RESTORE 还原，并把延迟拉长到还原动画和重绘完成之后再截；
        /// 截取完毕再 SW_MINIMIZE 恢复原状——最小化只是技术细节，对用户不可见。
        /// </summary>
        internal void CaptureWindow(IntPtr windowHandle)
        {
            if (windowHandle == IntPtr.Zero
                || !NativeMethods.IsWindow(windowHandle))
            {
                App.Toast.Show("没有找到可截取的窗口", ToastKind.Warning);
                return;
            }
            if (_capturePending)
            {
                return;
            }

            bool wasMinimized = NativeMethods.IsIconic(windowHandle);
            if (wasMinimized)
            {
                NativeMethods.ShowWindow(windowHandle, NativeMethods.SwRestore);
            }

            _capturePending = true;
            Hide();

            var delayTimer = new System.Windows.Forms.Timer { Interval = wasMinimized ? 450 : 150 };
            delayTimer.Tick += delegate
            {
                delayTimer.Stop();
                delayTimer.Dispose();
                if (IsDisposed || Disposing)
                {
                    return;
                }
                _capturePending = false;

                try
                {
                    _snapshotClipboardSequence = CaptureService.CaptureWindowToClipboard(windowHandle);
                    _clipboardClearTimer.Stop();
                    _clipboardClearTimer.Start();
                    string applicationName = _target != null && _target.WindowHandle == windowHandle
                        ? _target.ProcessName
                        : App.WindowProcessName(windowHandle);
                    ShutterSound.Play();
                    App.Toast.Show("已复制 " + applicationName + " 窗口，60 秒后自动清空", ToastKind.Success);
                }
                catch
                {
                    App.Toast.Show("截取失败，请稍后重试", ToastKind.Error);
                }
                finally
                {
                    // 恢复最小化：还原只是截取的技术需要，不该改变窗口的原始状态
                    if (wasMinimized && NativeMethods.IsWindow(windowHandle))
                    {
                        NativeMethods.ShowWindow(windowHandle, NativeMethods.SwMinimize);
                    }
                    if (!IsDisposed && !Disposing)
                    {
                        Show();
                    }
                }
            };
            delayTimer.Start();
        }

        // ---------- 位置持久化与钳制（对应 macOS 端 clampToVisibleArea） ----------

        private static Point ClampToWorkingArea(Point origin, Size size, Point anchor)
        {
            Screen screen = Screen.FromPoint(anchor);
            Rectangle working = screen.WorkingArea;
            int margin = UiScale.Px(PositionMargin);
            int x = Math.Max(working.Left + margin,
                Math.Min(origin.X, working.Right - size.Width - margin));
            int y = Math.Max(working.Top + margin,
                Math.Min(origin.Y, working.Bottom - size.Height - margin));
            return new Point(x, y);
        }

        private static bool IsPointOnAnyScreen(Point point)
        {
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle bounds = screen.Bounds;
                bounds.Inflate(200, 200);
                if (bounds.Contains(point))
                {
                    return true;
                }
            }
            return false;
        }

        private Point RestoreOrClampPosition()
        {
            string savedX = AppSettings.Read("pet.position.x");
            string savedY = AppSettings.Read("pet.position.y");
            int x;
            int y;
            if (int.TryParse(savedX, out x) && int.TryParse(savedY, out y))
            {
                var point = new Point(x, y);
                if (IsPointOnAnyScreen(point))
                {
                    return ClampToWorkingArea(point, Size, point);
                }
            }

            Rectangle workingArea = Screen.PrimaryScreen.WorkingArea;
            return new Point(
                workingArea.Right - Width - UiScale.Px(16),
                workingArea.Bottom - Height - UiScale.Px(16));
        }

        private static void SavePosition(Point location)
        {
            AppSettings.Write("pet.position.x", location.X.ToString());
            AppSettings.Write("pet.position.y", location.Y.ToString());
        }
    }
}
