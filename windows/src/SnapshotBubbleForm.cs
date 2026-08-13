using System;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Threading;
using System.Windows.Forms;

namespace AppSnapshot
{
    // Non-activating floating bubble showing the previous foreground app.
    // Current/Previous behavior follows Maxworkinghard/quick's desktop pet.
    internal sealed class SnapshotBubbleForm : Form
    {
        private const int BubbleSize = 168;
        private const int DragThreshold = 4;

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

        public SnapshotBubbleForm()
        {
            _tracker = new TargetAppTracker();
            _tracker.PreviousChanged += OnPreviousChanged;

            AutoScaleMode = AutoScaleMode.Dpi;
            BackColor = Color.White;
            ClientSize = new Size(BubbleSize, BubbleSize);
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
                bubblePath.AddEllipse(new Rectangle(8, 8, 152, 152));
                Region = new Region(bubblePath);
            }

            SetStyle(
                ControlStyles.AllPaintingInWmPaint
                | ControlStyles.OptimizedDoubleBuffer
                | ControlStyles.ResizeRedraw
                | ControlStyles.UserPaint,
                true);

            var menu = new ContextMenuStrip();
            menu.Items.Add("\u9000\u51fa", null, delegate { Close(); });
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

            Rectangle workingArea = Screen.PrimaryScreen.WorkingArea;
            Location = new Point(
                workingArea.Right - Width - 16,
                workingArea.Bottom - Height - 16);
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
                    && (Math.Abs(deltaX) >= DragThreshold || Math.Abs(deltaY) >= DragThreshold))
                {
                    _dragging = true;
                }

                if (_dragging)
                {
                    Location = new Point(_windowDown.X + deltaX, _windowDown.Y + deltaY);
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
                if (!_dragging)
                {
                    CaptureTarget();
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

            Bitmap icon = LoadWindowIcon(target.WindowHandle, (int)target.ProcessId);
            Bitmap previousIcon = _targetIcon;
            _target = target;
            _targetIcon = icon;
            _toolTip.SetToolTip(this, "\u70b9\u51fb\u622a\u53d6\u5e76\u590d\u5236 " + target.ProcessName);
            Invalidate();

            if (previousIcon != null)
            {
                previousIcon.Dispose();
            }
        }

        private static Bitmap LoadWindowIcon(IntPtr window, int processId)
        {
            // Window messages commonly return only a 16x16 or 32x32 icon. Try
            // the executable first so Windows can select its 128/256px icon
            // resource instead of stretching a tiny bitmap to 112x112.
            try
            {
                using (Process process = Process.GetProcessById(processId))
                {
                    string executablePath = process.MainModule.FileName;
                    Bitmap executableIcon = LoadHighResolutionExecutableIcon(executablePath);
                    if (executableIcon != null)
                    {
                        return executableIcon;
                    }
                }
            }
            catch
            {
            }

            IntPtr iconHandle = NativeMethods.SendMessage(
                window, NativeMethods.WmGetIcon,
                new IntPtr(NativeMethods.IconBig), IntPtr.Zero);
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.SendMessage(
                    window, NativeMethods.WmGetIcon,
                    new IntPtr(NativeMethods.IconSmall2), IntPtr.Zero);
            }
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.SendMessage(
                    window, NativeMethods.WmGetIcon,
                    new IntPtr(NativeMethods.IconSmall), IntPtr.Zero);
            }
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.GetClassLongPtr(window, NativeMethods.GclpHIconSmall);
            }
            if (iconHandle == IntPtr.Zero)
            {
                iconHandle = NativeMethods.GetClassLongPtr(window, NativeMethods.GclpHIcon);
            }
            if (iconHandle != IntPtr.Zero)
            {
                try
                {
                    return RenderIcon(iconHandle);
                }
                catch
                {
                }
            }

            try
            {
                using (Process process = Process.GetProcessById(processId))
                using (Icon icon = Icon.ExtractAssociatedIcon(process.MainModule.FileName))
                {
                    if (icon != null)
                    {
                        return RenderIcon(icon.Handle);
                    }
                }
            }
            catch
            {
            }

            return RenderIcon(SystemIcons.Application.Handle);
        }

        private static Bitmap LoadHighResolutionExecutableIcon(string executablePath)
        {
            if (string.IsNullOrEmpty(executablePath))
            {
                return null;
            }

            var iconHandles = new IntPtr[1];
            var iconIds = new uint[1];
            uint extracted = NativeMethods.PrivateExtractIcons(
                executablePath,
                0,
                112,
                112,
                iconHandles,
                iconIds,
                1,
                0);

            if (extracted == 0 || iconHandles[0] == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                return RenderIcon(iconHandles[0]);
            }
            finally
            {
                NativeMethods.DestroyIcon(iconHandles[0]);
            }
        }

        private static Bitmap RenderIcon(IntPtr iconHandle)
        {
            var bitmap = new Bitmap(112, 112, PixelFormat.Format32bppPArgb);
            using (Graphics graphics = Graphics.FromImage(bitmap))
            using (Icon icon = Icon.FromHandle(iconHandle))
            {
                graphics.Clear(Color.Transparent);
                graphics.SmoothingMode = SmoothingMode.HighQuality;
                graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
                graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
                graphics.CompositingQuality = CompositingQuality.HighQuality;
                graphics.DrawIcon(icon, new Rectangle(0, 0, 112, 112));
            }

            return bitmap;
        }

        private void CaptureTarget()
        {
            TargetAppTracker.TargetInfo target = _target;
            if (target == null
                || target.WindowHandle == IntPtr.Zero
                || !NativeMethods.IsWindow(target.WindowHandle))
            {
                return;
            }

            Hide();
            Application.DoEvents();
            Thread.Sleep(100);

            try
            {
                _snapshotClipboardSequence = CaptureService.CaptureWindowToClipboard(target.WindowHandle);
                _clipboardClearTimer.Stop();
                _clipboardClearTimer.Start();
            }
            catch
            {
            }
            finally
            {
                Show();
            }
        }
    }
}
