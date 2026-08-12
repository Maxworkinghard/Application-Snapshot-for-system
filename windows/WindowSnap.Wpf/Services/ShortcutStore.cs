using System;
using WindowSnap.Wpf.PInvoke;
using WindowSnap.Wpf.Properties;

namespace WindowSnap.Wpf.Services;

/// <summary>
/// 持久化的快捷键配置。和 macOS 版保持语义一致。
/// </summary>
public readonly struct ShortcutConfiguration : IEquatable<ShortcutConfiguration>
{
    public uint VirtualKey { get; }
    public User32.ModifierFlags Modifiers { get; }

    public ShortcutConfiguration(uint virtualKey, User32.ModifierFlags modifiers)
    {
        VirtualKey = virtualKey;
        Modifiers = modifiers;
    }

    // VK_OEM_3 = ` 反引号；VK_2 = '2'。默认绑定 Alt+Shift+2
    public static readonly ShortcutConfiguration Default = new(0x32 /* VK_2 */, User32.ModifierFlags.Alt | User32.ModifierFlags.Shift);

    public string DisplayString
    {
        get
        {
            var sb = new System.Text.StringBuilder();
            if (Modifiers.HasFlag(User32.ModifierFlags.Windows)) sb.Append("Win+");
            if (Modifiers.HasFlag(User32.ModifierFlags.Control)) sb.Append("Ctrl+");
            if (Modifiers.HasFlag(User32.ModifierFlags.Alt)) sb.Append("Alt+");
            if (Modifiers.HasFlag(User32.ModifierFlags.Shift)) sb.Append("Shift+");
            sb.Append(KeyName(VirtualKey));
            return sb.ToString();
        }
    }

    private static string KeyName(uint vk) => vk switch
    {
        >= 0x30 and <= 0x39 => ((char)vk).ToString(), // 0-9
        >= 0x41 and <= 0x5A => ((char)vk).ToString(), // A-Z
        0x20 => "Space",
        0x0D => "Enter",
        0x1B => "Esc",
        0x08 => "Backspace",
        0x09 => "Tab",
        0xBD => "-",
        0xBB => "=",
        0xDB => "[",
        0xDD => "]",
        0xBA => ";",
        0xDE => "'",
        0xBC => ",",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDC => "\\",
        >= 0x70 and <= 0x87 => "F" + (vk - 0x6F),
        _ => $"VK 0x{vk:X2}",
    };

    public bool Equals(ShortcutConfiguration other) =>
        VirtualKey == other.VirtualKey && Modifiers == other.Modifiers;

    public override bool Equals(object? obj) => obj is ShortcutConfiguration other && Equals(other);
    public override int GetHashCode() => HashCode.Combine(VirtualKey, (uint)Modifiers);
    public static bool operator ==(ShortcutConfiguration a, ShortcutConfiguration b) => a.Equals(b);
    public static bool operator !=(ShortcutConfiguration a, ShortcutConfiguration b) => !a.Equals(b);
}

public sealed class ShortcutStore
{
    private const string VkKey = "shortcut.vk";
    private const string ModKey = "shortcut.modifiers";

    public ShortcutConfiguration Load()
    {
        var props = Properties.Settings.Default;
        var mod = props.Get("shortcut.modifiers", uint.MinValue);
        var vk = props.Get("shortcut.vk", uint.MinValue);
        if (mod == uint.MinValue || vk == uint.MinValue) return ShortcutConfiguration.Default;
        return new ShortcutConfiguration(vk, (User32.ModifierFlags)mod);
    }

    public void Save(ShortcutConfiguration configuration)
    {
        var props = Properties.Settings.Default;
        props.Set("shortcut.vk", configuration.VirtualKey);
        props.Set("shortcut.modifiers", (uint)configuration.Modifiers);
    }
}
