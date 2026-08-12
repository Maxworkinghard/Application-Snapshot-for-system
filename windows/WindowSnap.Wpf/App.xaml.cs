using System;
using System.Threading;
using System.Windows;

namespace WindowSnap.Wpf;

public partial class App : Application
{
    public static new App Current => (App)Application.Current;

    private AppDelegate? _delegate;
    private Mutex? _instanceMutex;

    public Services.ClipboardAutoClearService AutoClearService =>
        _delegate?.AutoClearService ?? throw new InvalidOperationException("App not started yet");

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        // 单实例：双开会导致热键注册冲突 + 双托盘
        _instanceMutex = new Mutex(true, @"Local\WindowSnap.SingleInstance", out var createdNew);
        if (!createdNew)
        {
            MessageBox.Show("应用快照已在运行。", "应用快照",
                MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown();
            return;
        }

        _delegate = new AppDelegate();
        _delegate.Start();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _delegate?.Shutdown();
        _instanceMutex?.Dispose();
        base.OnExit(e);
    }
}
