using System;
using System.Diagnostics;

namespace AppSnapshot
{
    /// <summary>
    /// Event-driven foreground application tracker based on the quick repository's
    /// TargetAppTracker design. Current is the active external app; Previous is
    /// the app used immediately before it.
    /// </summary>
    internal sealed class TargetAppTracker : IDisposable
    {
        internal sealed class TargetInfo
        {
            public IntPtr WindowHandle { get; set; }
            public uint ProcessId { get; set; }
            public string ProcessName { get; set; }
        }

        private readonly int _ownProcessId;
        private NativeMethods.WinEventDelegate _hookCallback;
        private IntPtr _hook;

        public TargetInfo Current { get; private set; }
        public TargetInfo Previous { get; private set; }

        public event EventHandler PreviousChanged;

        public TargetAppTracker()
        {
            _ownProcessId = Process.GetCurrentProcess().Id;
        }

        public void Start()
        {
            _hookCallback = OnForegroundChanged;
            _hook = NativeMethods.SetWinEventHook(
                NativeMethods.EventSystemForeground,
                NativeMethods.EventSystemForeground,
                IntPtr.Zero,
                _hookCallback,
                0,
                0,
                NativeMethods.WineventOutOfContext | NativeMethods.WineventSkipOwnProcess);

            RecordForegroundWindow();
        }

        private void OnForegroundChanged(
            IntPtr eventHook,
            uint eventType,
            IntPtr hWnd,
            int objectId,
            int childId,
            uint eventThread,
            uint eventTime)
        {
            if (hWnd != IntPtr.Zero)
            {
                RecordForegroundWindow();
            }
        }

        private void RecordForegroundWindow()
        {
            IntPtr window = NativeMethods.GetForegroundWindow();
            if (window == IntPtr.Zero || !NativeMethods.IsWindowVisible(window))
            {
                return;
            }

            uint processId;
            NativeMethods.GetWindowThreadProcessId(window, out processId);
            if (processId == 0 || processId == _ownProcessId)
            {
                return;
            }

            var next = new TargetInfo
            {
                WindowHandle = window,
                ProcessId = processId,
                ProcessName = GetProcessName(processId)
            };

            if (Current == null)
            {
                // A useful initial state while waiting for the first app switch.
                Current = next;
                Previous = next;
                RaisePreviousChanged();
                return;
            }

            if (Current.ProcessId == next.ProcessId)
            {
                // Refresh the handle when an app switches between its own windows.
                Current = next;
                if (Previous != null && Previous.ProcessId == next.ProcessId)
                {
                    Previous = next;
                    RaisePreviousChanged();
                }
                return;
            }

            Previous = Current;
            Current = next;
            RaisePreviousChanged();
        }

        private void RaisePreviousChanged()
        {
            EventHandler handler = PreviousChanged;
            if (handler != null)
            {
                handler(this, EventArgs.Empty);
            }
        }

        private static string GetProcessName(uint processId)
        {
            try
            {
                using (Process process = Process.GetProcessById((int)processId))
                {
                    return process.ProcessName;
                }
            }
            catch
            {
                return "App";
            }
        }

        public void Dispose()
        {
            if (_hook != IntPtr.Zero)
            {
                NativeMethods.UnhookWinEvent(_hook);
                _hook = IntPtr.Zero;
            }

            _hookCallback = null;
        }
    }
}
