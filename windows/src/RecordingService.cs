using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace AppSnapshot
{
    /// <summary>
    /// 窗口录制：通过 ffmpeg 的 gdigrab 管道捕获（title 模式，窗口移动时跟随），
    /// 输出 H.264 MP4 到用户设置的保存目录。对应 macOS 端的 WindowRecordingService。
    /// </summary>
    internal sealed class RecordingService
    {
        internal enum State
        {
            Idle,
            Starting,
            Recording,
            Stopping
        }

        private readonly object syncRoot = new object();
        private State state = State.Idle;
        private Process ffmpeg;
        private string outputPath;
        private RecordingControlsForm controlsForm;
        private DateTime startedAt;
        private string ffmpegPath;
        private bool ffmpegChecked;
        private readonly StringBuilder stderrTail = new StringBuilder();

        internal event Action StateChanged;

        internal State CurrentState
        {
            get
            {
                lock (syncRoot)
                {
                    return state;
                }
            }
        }

        /// <summary>面板/托盘上的录制按钮文案，与 macOS 端 configureRecordButton 对齐。</summary>
        internal string RecordButtonTitle
        {
            get
            {
                switch (CurrentState)
                {
                    case State.Recording:
                        return "停止录制";
                    case State.Starting:
                        return "录制\n正在开始录制…";
                    case State.Stopping:
                        return "录制\n正在保存录制…";
                    default:
                        return "录制";
                }
            }
        }

        internal string RecordButtonSubtitle
        {
            get
            {
                switch (CurrentState)
                {
                    case State.Recording:
                        return "结束当前录制";
                    case State.Starting:
                    case State.Stopping:
                        return null;
                    default:
                        return Tracker != null && Tracker.Current != null
                            ? Tracker.Current.ProcessName + " 的窗口"
                            : "未找到可录制的应用";
                }
            }
        }

        internal bool RecordButtonEnabled
        {
            get
            {
                switch (CurrentState)
                {
                    case State.Idle:
                        return Tracker != null && Tracker.Current != null;
                    case State.Recording:
                        return true;
                    default:
                        return false;
                }
            }
        }

        private static TargetAppTracker Tracker
        {
            get { return App.Tracker; }
        }

        /// <summary>启动录制：window 为目标窗口句柄。内部在后台线程执行。</summary>
        internal void Start(IntPtr window, string windowTitle)
        {
            if (CurrentState != State.Idle)
            {
                return;
            }
            if (window == IntPtr.Zero || !NativeMethods.IsWindow(window))
            {
                App.Toast.Show("没有找到可录制的应用", ToastKind.Warning);
                return;
            }
            if (NativeMethods.IsIconic(window))
            {
                App.Toast.Show("目标窗口已最小化，请先还原后再录制", ToastKind.Warning);
                return;
            }
            if (!ResolveFfmpeg())
            {
                App.Toast.Show("未找到 ffmpeg，请安装后将其加入 PATH 再试", ToastKind.Warning);
                return;
            }
            if (string.IsNullOrEmpty(windowTitle))
            {
                App.Toast.Show("目标窗口没有标题，无法录制", ToastKind.Warning);
                return;
            }

            SetState(State.Starting);
            ThreadPool.QueueUserWorkItem(delegate { StartCore(windowTitle); });
        }

        private void StartCore(string windowTitle)
        {
            outputPath = MakeOutputPath();
            stderrTail.Length = 0;

            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(outputPath));
            }
            catch
            {
                OnStartFailed("无法创建录制保存目录：" + outputPath);
                return;
            }

            var process = new Process
            {
                StartInfo =
                {
                    FileName = ffmpegPath,
                    UseShellExecute = false,
                    CreateNoWindow = true,
                    RedirectStandardInput = true,
                    RedirectStandardError = true,
                    RedirectStandardOutput = true
                },
                EnableRaisingEvents = true
            };
            process.Exited += delegate
            {
                // 录制中 ffmpeg 意外退出（如目标窗口被关闭）
                if (CurrentState == State.Recording)
                {
                    CleanupAfterUnexpectedExit();
                }
            };
            AppendArgument(process, "-y");
            AppendArgument(process, "-f");
            AppendArgument(process, "gdigrab");
            AppendArgument(process, "-framerate");
            AppendArgument(process, "30");
            AppendArgument(process, "-i");
            AppendArgument(process, "title=" + windowTitle);
            AppendArgument(process, "-c:v");
            AppendArgument(process, "libx264");
            AppendArgument(process, "-preset");
            AppendArgument(process, "veryfast");
            AppendArgument(process, "-pix_fmt");
            AppendArgument(process, "yuv420p");
            AppendArgument(process, "-crf");
            AppendArgument(process, "23");
            AppendArgument(process, outputPath);

            try
            {
                process.Start();
            }
            catch (Exception error)
            {
                OnStartFailed("启动 ffmpeg 失败：" + error.Message);
                return;
            }

            ffmpeg = process;
            process.ErrorDataReceived += delegate(object sender, DataReceivedEventArgs e)
            {
                if (e.Data != null)
                {
                    lock (stderrTail)
                    {
                        if (stderrTail.Length > 4000)
                        {
                            stderrTail.Remove(0, 2000);
                        }
                        stderrTail.AppendLine(e.Data);
                    }
                }
            };
            process.OutputDataReceived += delegate { };
            process.BeginErrorReadLine();
            process.BeginOutputReadLine();

            // 给 ffmpeg 一点初始化时间；立刻退出说明参数或环境有问题
            Thread.Sleep(1200);
            if (process.HasExited)
            {
                string tail = ReadStderrTail();
                ffmpeg = null;
                OnStartFailed("录制启动失败" + (tail.Length == 0 ? "" : "：" + tail));
                return;
            }

            startedAt = DateTime.Now;
            SetState(State.Recording);
            RunOnUi(delegate
            {
                if (CurrentState != State.Recording)
                {
                    return;
                }
                controlsForm = new RecordingControlsForm(startedAt, Stop);
                controlsForm.ShowControls();
            });
        }

        private void OnStartFailed(string message)
        {
            SetState(State.Idle);
            RunOnUi(delegate { App.Toast.Show(message, ToastKind.Error); });
        }

        private void CleanupAfterUnexpectedExit()
        {
            ffmpeg = null;
            outputPath = null;
            SetState(State.Idle);
            RunOnUi(delegate
            {
                if (controlsForm != null)
                {
                    controlsForm.Close();
                    controlsForm.Dispose();
                    controlsForm = null;
                }
                App.Toast.Show("录制已意外结束", ToastKind.Warning);
            });
        }

        /// <summary>停止录制：向 ffmpeg stdin 发送 q 优雅收尾，等待 MP4 完成封装。</summary>
        internal void Stop()
        {
            if (CurrentState != State.Recording)
            {
                return;
            }
            SetState(State.Stopping);

            Process process = ffmpeg;
            ThreadPool.QueueUserWorkItem(delegate { StopCore(process); });
        }

        private void StopCore(Process process)
        {
            RunOnUi(delegate
            {
                if (controlsForm != null)
                {
                    controlsForm.Close();
                    controlsForm.Dispose();
                    controlsForm = null;
                }
            });

            bool finishedCleanly = false;
            try
            {
                if (process != null && !process.HasExited)
                {
                    process.StandardInput.WriteLine("q");
                    finishedCleanly = process.WaitForExit(8000);
                    if (!finishedCleanly)
                    {
                        try
                        {
                            process.Kill();
                        }
                        catch
                        {
                        }
                        process.WaitForExit(3000);
                    }
                    else
                    {
                        finishedCleanly = process.ExitCode == 0;
                    }
                }
            }
            catch
            {
            }

            ffmpeg = null;
            string path = outputPath;
            outputPath = null;

            bool fileValid = false;
            try
            {
                fileValid = finishedCleanly
                    && File.Exists(path)
                    && new FileInfo(path).Length > 0;
            }
            catch
            {
            }

            SetState(State.Idle);
            RunOnUi(delegate
            {
                if (fileValid)
                {
                    App.Toast.Show("已保存到 " + Path.GetFileName(path), ToastKind.Success);
                }
                else
                {
                    App.Toast.Show("录制保存失败，文件未生成", ToastKind.Error);
                }
            });
        }

        /// <summary>应用退出时调用：尽力优雅收尾 ffmpeg，超时强杀，避免孤儿进程。</summary>
        internal void ShutdownOnAppExit()
        {
            Process process = ffmpeg;
            if (process == null)
            {
                return;
            }
            try
            {
                if (!process.HasExited)
                {
                    try
                    {
                        process.StandardInput.WriteLine("q");
                    }
                    catch
                    {
                    }
                    if (!process.WaitForExit(2500))
                    {
                        process.Kill();
                        process.WaitForExit(2000);
                    }
                }
            }
            catch
            {
            }
            ffmpeg = null;
        }

        private bool ResolveFfmpeg()
        {
            lock (syncRoot)
            {
                if (ffmpegChecked)
                {
                    return ffmpegPath != null;
                }
                ffmpegChecked = true;

                try
                {
                    var probe = new Process
                    {
                        StartInfo =
                        {
                            FileName = "ffmpeg",
                            Arguments = "-version",
                            UseShellExecute = false,
                            CreateNoWindow = true,
                            RedirectStandardOutput = true,
                            RedirectStandardError = true
                        }
                    };
                    probe.Start();
                    probe.StandardOutput.ReadToEnd();
                    probe.WaitForExit(5000);
                    if (probe.ExitCode == 0)
                    {
                        ffmpegPath = "ffmpeg";
                        return true;
                    }
                }
                catch
                {
                }
                ffmpegPath = null;
                return false;
            }
        }

        private static string MakeOutputPath()
        {
            string directory = AppSettings.SaveDirectory;
            try
            {
                Directory.CreateDirectory(directory);
            }
            catch
            {
                directory = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
            }
            string stamp = DateTime.Now.ToString("yyyy-MM-dd_HH-mm-ss");
            return Path.Combine(directory, "应用快照-" + stamp + ".mp4");
        }

        private string ReadStderrTail()
        {
            lock (stderrTail)
            {
                string tail = stderrTail.ToString();
                tail = tail == null ? "" : tail.Trim();
                if (tail.Length > 300)
                {
                    tail = tail.Substring(tail.Length - 300);
                }
                return tail.Replace("\r", " ").Replace("\n", " ");
            }
        }

        private void SetState(State next)
        {
            State previous;
            lock (syncRoot)
            {
                previous = state;
                state = next;
            }
            if (previous != next)
            {
                RunOnUi(delegate
                {
                    Action handler = StateChanged;
                    if (handler != null)
                    {
                        handler();
                    }
                });
            }
        }

        private static void RunOnUi(MethodInvoker action)
        {
            Form mainForm = App.MainForm;
            if (mainForm != null && !mainForm.IsDisposed && mainForm.IsHandleCreated)
            {
                try
                {
                    mainForm.BeginInvoke(action);
                    return;
                }
                catch
                {
                }
            }
        }

        /// <summary>.NET 4 没有 ArgumentList，手工转义并拼接命令行。</summary>
        private static void AppendArgument(Process process, string argument)
        {
            if (argument == null)
            {
                return;
            }
            bool needsQuotes = argument.Length == 0
                || argument.IndexOfAny(new[] { ' ', '\t', '"' }) >= 0;
            string escaped = argument.Replace("\\\"", "\\\\\"").Replace("\"", "\\\"");
            process.StartInfo.Arguments += (process.StartInfo.Arguments.Length == 0 ? "" : " ")
                + (needsQuotes ? "\"" + escaped + "\"" : escaped);
        }
    }
}
