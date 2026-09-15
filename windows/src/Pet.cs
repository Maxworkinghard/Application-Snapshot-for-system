using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>桌宠动画状态;running / review 两个动作预留,待接入更多事件钩子。</summary>
    internal enum PetPose
    {
        Idle,
        Waving,
        Jumping,
        Failed,
        Waiting,
        RunningLeft,
        RunningRight
    }

    /// <summary>
    /// 桌宠素材目录:%APPDATA%\AppSnapshot\pet\<形象>\idle.gif 等。
    /// 素材不随应用内置或分发(版权考虑),由用户自行放入;目录为空时桌宠模式不可用。
    /// </summary>
    internal static class PetAssets
    {
        internal static string Root
        {
            get { return Path.Combine(AppSettings.DataDirectory, "pet"); }
        }

        /// <summary>可用形象 = pet 目录下含至少一个 gif 的子目录,按名称排序。</summary>
        internal static List<string> AvailableSkins()
        {
            var skins = new List<string>();
            try
            {
                if (!Directory.Exists(Root))
                {
                    return skins;
                }
                foreach (string dir in Directory.GetDirectories(Root))
                {
                    if (Directory.GetFiles(dir, "*.gif").Length > 0)
                    {
                        skins.Add(Path.GetFileName(dir));
                    }
                }
                skins.Sort(StringComparer.OrdinalIgnoreCase);
            }
            catch
            {
            }
            return skins;
        }

        internal static bool SkinExists(string skin)
        {
            if (string.IsNullOrEmpty(skin))
            {
                return false;
            }
            try
            {
                return Directory.Exists(Path.Combine(Root, skin))
                    && Directory.GetFiles(Path.Combine(Root, skin), "*.gif").Length > 0;
            }
            catch
            {
                return false;
            }
        }

        internal static string PosePath(string skin, string pose)
        {
            return Path.Combine(Root, skin, pose + ".gif");
        }
    }

    /// <summary>
    /// 桌宠模式控制器:持有 PetForm,把应用事件翻译成动画。
    /// 与悬浮球互斥显示——App.IsPetMode 为真时悬浮球保持隐藏
    /// (见 SnapshotBubbleForm.SetVisibleCore),反之桌宠由 SwitchUiMode 收起。
    /// 形象(skin)对应 assets/pet 下的一组 GIF 前缀,可运行时切换。
    /// </summary>
    internal sealed class PetController : IDisposable
    {
        private readonly string skin;
        private PetForm form;
        private bool hiddenForRecording;
        private bool hiddenTemporarily;

        internal PetController(string skin)
        {
            this.skin = skin;
            // 所有成败结果都汇入 Toast,这里一处订阅即可覆盖截图/录制/润色
            App.Toast.Notified += OnToastNotified;
        }

        internal void Start()
        {
            if (form == null || form.IsDisposed)
            {
                string skin = App.ResolvePetSkin();
                if (skin == null)
                {
                    return;
                }
                form = new PetForm(skin);
            }
            form.Show();
        }

        internal void Stop()
        {
            if (form != null)
            {
                form.Dispose();
                form = null;
            }
        }

        internal Rectangle CurrentBounds
        {
            get { return form != null && !form.IsDisposed ? form.Bounds : Rectangle.Empty; }
        }

        /// <summary>截图流程期间短暂隐藏(与悬浮球同规则,CopyFromScreen 回退会拍屏幕)。</summary>
        internal void HideTemporarily()
        {
            if (form != null && !form.IsDisposed && form.Visible)
            {
                hiddenTemporarily = true;
                form.Hide();
            }
        }

        internal void RestoreTemporarily()
        {
            if (hiddenTemporarily)
            {
                hiddenTemporarily = false;
                if (!hiddenForRecording && form != null && !form.IsDisposed)
                {
                    form.Show();
                }
            }
        }

        /// <summary>gdigrab 录制的是屏幕像素,录制期间猫必须离场,结束后再回来。</summary>
        internal void SetHiddenForRecording(bool hidden)
        {
            hiddenForRecording = hidden;
            if (form == null || form.IsDisposed)
            {
                return;
            }
            if (hidden)
            {
                form.Hide();
            }
            else if (!hiddenTemporarily)
            {
                form.Show();
            }
        }

        private void OnToastNotified(ToastKind kind)
        {
            switch (kind)
            {
                case ToastKind.Success:
                    SetPose(PetPose.Jumping);
                    break;
                case ToastKind.Error:
                    SetPose(PetPose.Failed);
                    break;
                default:
                    SetPose(PetPose.Waiting);
                    break;
            }
        }

        internal void SetPose(PetPose pose)
        {
            if (form != null && !form.IsDisposed)
            {
                form.SetPose(pose);
            }
        }

        public void Dispose()
        {
            App.Toast.Notified -= OnToastNotified;
            Stop();
        }
    }

    /// <summary>
    /// 分层窗口桌宠:UpdateLayeredWindow 逐像素 alpha 渲染 GIF 帧,
    /// 猫毛边缘无色键光晕,透明区域自动穿透鼠标。
    /// 帧按 GIF 自带延迟推进;单次动画(挥手/起跳/失败/等待)播完回 idle,
    /// 循环动画(idle/跑动)持续循环。全部尺寸走 UiScale.Px(系统级 DPI 感知)。
    /// </summary>
    internal sealed class PetForm : Form
    {
        // 96 DPI 设计值;GIF 原始 192x208,等比缩到 155x168 展示
        private const int PetWidth = 155;
        private const int PetHeight = 168;
        private const int DragThreshold = 4;

        private class PoseClip
        {
            public Image Gif;
            public Stream Source;   // GDI+ 要求流在 Image 存活期内保持打开
            public FrameDimension Dimension;
            public int FrameCount;
            public int[] DelaysMs;
            public bool Loop;
        }

        private readonly PoseClip[] clips;
        private readonly System.Windows.Forms.Timer frameTimer;
        private readonly ToolTip toolTip;

        private PetPose currentPose = PetPose.Idle;
        private int frameIndex;
        private Bitmap scratch;
        private Point pointerDown;
        private Point windowDown;
        private bool pointerPressed;
        private bool dragging;

        internal PetForm(string skin)
        {
            AutoScaleMode = AutoScaleMode.Dpi;
            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            TopMost = true;
            ClientSize = UiScale.Px(PetWidth, PetHeight);
            Text = "AppSnapshot 桌宠";

            clips = new PoseClip[7];
            clips[(int)PetPose.Idle] = LoadPose(skin, "idle", true);
            clips[(int)PetPose.Waving] = LoadPose(skin, "waving", false);
            clips[(int)PetPose.Jumping] = LoadPose(skin, "jumping", false);
            clips[(int)PetPose.Failed] = LoadPose(skin, "failed", false);
            clips[(int)PetPose.Waiting] = LoadPose(skin, "waiting", false);
            clips[(int)PetPose.RunningLeft] = LoadPose(skin, "running-left", true);
            clips[(int)PetPose.RunningRight] = LoadPose(skin, "running-right", true);
            // idle 缺失(素材不完整)时退化到任意可用的动画,避免空窗口
            if (clips[(int)PetPose.Idle] == null)
            {
                for (int i = 0; i < clips.Length; i++)
                {
                    if (clips[i] != null)
                    {
                        clips[(int)PetPose.Idle] = clips[i];
                        break;
                    }
                }
            }

            frameTimer = new System.Windows.Forms.Timer();
            frameTimer.Tick += OnFrameTick;

            var menu = new ContextMenuStrip();
            menu.Items.Add("设置…", null, delegate
            {
                using (var settingsForm = new SettingsForm())
                {
                    settingsForm.ShowDialog();
                }
            });
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add("退出应用快照", null, delegate { Application.Exit(); });
            ContextMenuStrip = menu;

            toolTip = new ToolTip
            {
                InitialDelay = 350,
                ReshowDelay = 150,
                AutoPopDelay = 3000,
                ShowAlways = true
            };
            toolTip.SetToolTip(this, "点击打开功能菜单，拖动移动");

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
                parameters.ExStyle |= NativeMethods.WsExToolWindow
                    | NativeMethods.WsExNoActivate
                    | NativeMethods.WsExLayered;
                return parameters;
            }
        }

        // ULW 窗口的显示完全由位图决定,WinForms 的 GDI 绘制管线一律留空
        protected override void OnPaintBackground(PaintEventArgs e)
        {
        }

        protected override void OnPaint(PaintEventArgs e)
        {
        }

        protected override void OnShown(EventArgs e)
        {
            base.OnShown(e);
            PlayPose(PetPose.Idle);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                if (frameTimer != null)
                {
                    frameTimer.Stop();
                    frameTimer.Dispose();
                }
                if (clips != null)
                {
                    for (int i = 0; i < clips.Length; i++)
                    {
                        if (clips[i] != null)
                        {
                            clips[i].Gif.Dispose();
                            clips[i].Source.Dispose();
                        }
                    }
                }
                if (scratch != null)
                {
                    scratch.Dispose();
                }
                if (toolTip != null)
                {
                    toolTip.Dispose();
                }
            }

            base.Dispose(disposing);
        }

        /// <summary>切换动画。循环态重复调用不重置帧(拖动中持续跑动);一次性动画重播。</summary>
        internal void SetPose(PetPose pose)
        {
            if (InvokeRequired)
            {
                BeginInvoke(new MethodInvoker(delegate { SetPose(pose); }));
                return;
            }
            if (IsDisposed || Disposing)
            {
                return;
            }

            PoseClip clip = clips[(int)pose];
            if (clip == null)
            {
                return;
            }
            if (currentPose == pose && clip.Loop)
            {
                return;
            }
            PlayPose(pose);
        }

        private void PlayPose(PetPose pose)
        {
            PoseClip clip = clips[(int)pose];
            if (clip == null)
            {
                return;
            }
            currentPose = pose;
            frameIndex = 0;
            RenderCurrentFrame();
            frameTimer.Stop();
            frameTimer.Interval = clip.DelaysMs[0];
            frameTimer.Start();
        }

        private void OnFrameTick(object sender, EventArgs e)
        {
            frameTimer.Stop();
            PoseClip clip = clips[(int)currentPose];
            if (clip == null)
            {
                return;
            }

            frameIndex++;
            if (frameIndex >= clip.FrameCount)
            {
                if (clip.Loop)
                {
                    frameIndex = 0;
                }
                else
                {
                    // 单次动画播完回待机
                    PlayPose(PetPose.Idle);
                    return;
                }
            }
            RenderCurrentFrame();
            frameTimer.Interval = clip.DelaysMs[frameIndex];
            frameTimer.Start();
        }

        private PoseClip LoadPose(string skin, string name, bool loop)
        {
            // GDI+ 要求流在 Image 存活期内保持打开,交给 PoseClip 后随 Dispose 一起释放。
            // 素材由用户自行放入,损坏/非 GIF/被占用都会让解析抛异常,此时必须就地关流,
            // 否则句柄一直挂着,用户覆盖或删除这个 gif 会被 Windows 拒绝(文件正在使用)。
            Stream source = null;
            Image gif = null;
            try
            {
                string path = PetAssets.PosePath(skin, name);
                if (!File.Exists(path))
                {
                    return null;
                }
                source = new FileStream(path, FileMode.Open, FileAccess.Read);
                gif = Image.FromStream(source);
                var dimension = new FrameDimension(gif.FrameDimensionsList[0]);
                int frameCount = gif.GetFrameCount(dimension);
                var delays = new int[frameCount];
                bool delaysLoaded = false;
                try
                {
                    // PropertyTagFrameDelay:每帧延迟,单位 1/100 秒
                    PropertyItem delayItem = gif.GetPropertyItem(0x5100);
                    for (int i = 0; i < frameCount; i++)
                    {
                        int centiseconds = i * 4 + 4 <= delayItem.Value.Length
                            ? BitConverter.ToInt32(delayItem.Value, i * 4)
                            : 10;
                        delays[i] = Math.Max(20, centiseconds * 10);
                    }
                    delaysLoaded = true;
                }
                catch
                {
                }
                if (!delaysLoaded)
                {
                    for (int i = 0; i < frameCount; i++)
                    {
                        delays[i] = 100;
                    }
                }
                var clip = new PoseClip
                {
                    Gif = gif,
                    Source = source,
                    Dimension = dimension,
                    FrameCount = frameCount,
                    DelaysMs = delays,
                    Loop = loop
                };
                // 所有权交给 clip,置空以免 finally 把正在用的资源关掉
                source = null;
                gif = null;
                return clip;
            }
            catch
            {
                return null;
            }
            finally
            {
                if (gif != null)
                {
                    gif.Dispose();
                }
                if (source != null)
                {
                    source.Dispose();
                }
            }
        }

        private void RenderCurrentFrame()
        {
            PoseClip clip = clips[(int)currentPose];
            if (clip == null || frameIndex >= clip.FrameCount)
            {
                return;
            }

            if (scratch == null)
            {
                scratch = new Bitmap(ClientSize.Width, ClientSize.Height, PixelFormat.Format32bppPArgb);
            }

            clip.Gif.SelectActiveFrame(clip.Dimension, frameIndex);
            using (Graphics graphics = Graphics.FromImage(scratch))
            {
                graphics.Clear(Color.Transparent);
                graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
                graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
                graphics.DrawImage(clip.Gif, new Rectangle(0, 0, ClientSize.Width, ClientSize.Height));
            }
            PushToLayeredWindow(scratch);
        }

        private void PushToLayeredWindow(Bitmap bitmap)
        {
            IntPtr screenDc = NativeMethods.GetDC(IntPtr.Zero);
            IntPtr memoryDc = NativeMethods.CreateCompatibleDC(screenDc);
            IntPtr bitmapHandle = IntPtr.Zero;
            IntPtr previousHandle = IntPtr.Zero;
            try
            {
                bitmapHandle = bitmap.GetHbitmap(Color.FromArgb(0));
                previousHandle = NativeMethods.SelectObject(memoryDc, bitmapHandle);

                var blend = new NativeMethods.BlendFunction();
                blend.BlendOp = NativeMethods.AcSrcOver;
                blend.SourceConstantAlpha = 255;
                blend.AlphaFormat = NativeMethods.AcSrcAlpha;

                Point destination = Location;
                Size size = new Size(ClientSize.Width, ClientSize.Height);
                Point sourcePoint = new Point(0, 0);
                NativeMethods.UpdateLayeredWindow(
                    Handle,
                    screenDc,
                    ref destination,
                    ref size,
                    memoryDc,
                    ref sourcePoint,
                    0,
                    ref blend,
                    NativeMethods.UlwAlpha);
            }
            finally
            {
                if (previousHandle != IntPtr.Zero)
                {
                    NativeMethods.SelectObject(memoryDc, previousHandle);
                }
                if (bitmapHandle != IntPtr.Zero)
                {
                    NativeMethods.DeleteObject(bitmapHandle);
                }
                NativeMethods.DeleteDC(memoryDc);
                NativeMethods.ReleaseDC(IntPtr.Zero, screenDc);
            }
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            if (e.Button == MouseButtons.Left)
            {
                pointerPressed = true;
                dragging = false;
                pointerDown = Cursor.Position;
                windowDown = Location;
                Capture = true;
            }

            base.OnMouseDown(e);
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            if (pointerPressed && (Control.MouseButtons & MouseButtons.Left) != 0)
            {
                Point current = Cursor.Position;
                int deltaX = current.X - pointerDown.X;
                int deltaY = current.Y - pointerDown.Y;
                if (!dragging
                    && (Math.Abs(deltaX) >= UiScale.Px(DragThreshold) || Math.Abs(deltaY) >= UiScale.Px(DragThreshold)))
                {
                    dragging = true;
                }

                if (dragging)
                {
                    Location = SnapshotBubbleForm.ClampToWorkingArea(
                        new Point(windowDown.X + deltaX, windowDown.Y + deltaY),
                        Size,
                        current);
                    SetPose(deltaX >= 0 ? PetPose.RunningRight : PetPose.RunningLeft);
                    if (App.Panels != null)
                    {
                        // 面板开着时跟随猫移动,与悬浮球拖动行为一致
                        App.Panels.Reposition();
                    }
                }
            }

            base.OnMouseMove(e);
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            if (e.Button == MouseButtons.Left && pointerPressed)
            {
                pointerPressed = false;
                Capture = false;
                if (dragging)
                {
                    dragging = false;
                    SavePosition(Location);
                    SetPose(PetPose.Idle);
                }
                else if (App.Panels != null)
                {
                    // 与悬浮球单击行为一致:开关功能面板
                    App.Panels.Toggle();
                }
            }

            base.OnMouseUp(e);
        }

        // ---------- 位置持久化与钳制(与悬浮球同规则,键名独立) ----------

        private Point RestoreOrClampPosition()
        {
            string savedX = AppSettings.Read("desktoppet.x");
            string savedY = AppSettings.Read("desktoppet.y");
            int x;
            int y;
            if (int.TryParse(savedX, out x) && int.TryParse(savedY, out y)
                && SnapshotBubbleForm.IsPointOnAnyScreen(new Point(x, y)))
            {
                return SnapshotBubbleForm.ClampToWorkingArea(new Point(x, y), Size, new Point(x, y));
            }

            Rectangle workingArea = Screen.PrimaryScreen.WorkingArea;
            return new Point(
                workingArea.Right - Width - UiScale.Px(16),
                workingArea.Bottom - Height - UiScale.Px(16));
        }

        private static void SavePosition(Point location)
        {
            AppSettings.Write("desktoppet.x", location.X.ToString());
            AppSettings.Write("desktoppet.y", location.Y.ToString());
        }
    }
}
