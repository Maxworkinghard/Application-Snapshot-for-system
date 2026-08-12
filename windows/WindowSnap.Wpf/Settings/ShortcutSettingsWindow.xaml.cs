using System;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using WindowSnap.Wpf.PInvoke;
using WindowSnap.Wpf.Services;

namespace WindowSnap.Wpf.Settings;

public partial class ShortcutSettingsWindow : Window
{
    private ShortcutConfiguration _current;
    private readonly Func<ShortcutConfiguration, bool> _onSave;

    public ShortcutSettingsWindow(ShortcutConfiguration current, Func<ShortcutConfiguration, bool> onSave)
    {
        _current = current;
        _onSave = onSave;
        InitializeComponent();
        UpdateLabel();
        Loaded += (_, _) => Keyboard.Focus(Recorder);
    }

    private void OnRecorderKeyDown(object sender, KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        if (key == Key.LeftCtrl || key == Key.RightCtrl ||
            key == Key.LeftShift || key == Key.RightShift ||
            key == Key.LeftAlt || key == Key.RightAlt ||
            key == Key.LWin || key == Key.RWin)
        {
            // 仅修饰键按下，等真正的非修饰键
            RecorderLabel.Text = "按下组合键…";
            return;
        }

        var modifiers = User32.ModifierFlags.None;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Alt)) modifiers |= User32.ModifierFlags.Alt;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Control)) modifiers |= User32.ModifierFlags.Control;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Shift)) modifiers |= User32.ModifierFlags.Shift;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Windows)) modifiers |= User32.ModifierFlags.Windows;

        if (modifiers == User32.ModifierFlags.None)
        {
            System.Media.SystemSounds.Beep.Play();
            ShowError("请使用至少一个修饰键，例如 Ctrl、Alt 或 Shift");
            return;
        }

        var vk = (uint)KeyInterop.VirtualKeyFromKey(key);
        _current = new ShortcutConfiguration(vk, modifiers);
        UpdateLabel();
        e.Handled = true;
    }

    private void OnRecorderGotFocus(object sender, RoutedEventArgs e)
    {
        Recorder.BorderBrush = SystemColors.HighlightBrush;
    }

    private void OnRecorderLostFocus(object sender, RoutedEventArgs e)
    {
        Recorder.BorderBrush = SystemColors.ControlDarkBrush;
    }

    private void OnCancelClick(object sender, RoutedEventArgs e) => Close();

    private void OnSaveClick(object sender, RoutedEventArgs e)
    {
        if (!_onSave(_current))
        {
            System.Media.SystemSounds.Beep.Play();
            ShowError("这个快捷键已被占用，请换一个组合");
            return;
        }
        Close();
    }

    private void UpdateLabel()
    {
        RecorderLabel.Text = _current.DisplayString;
        RecorderLabel.Foreground = SystemColors.ControlTextBrush;
        RecorderLabel.FontSize = 17;
    }

    private void ShowError(string message)
    {
        RecorderLabel.Text = message;
        RecorderLabel.Foreground = Brushes.OrangeRed;
        RecorderLabel.FontSize = 12;
    }
}
