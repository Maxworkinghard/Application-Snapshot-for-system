using System;
using System.IO;
using System.Xml;
using System.Xml.Linq;

namespace WindowSnap.Wpf.Properties;

/// <summary>
/// 极简 XML 设置存储，序列化到 %APPDATA%\WindowSnap\settings.xml。
/// 避免依赖 .NET Settings 生成器与 app.config。
/// </summary>
public sealed class Settings
{
    private static Settings? _instance;
    public static Settings Default => _instance ??= new Settings();

    private readonly string _path;

    private Settings()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "WindowSnap");
        Directory.CreateDirectory(dir);
        _path = Path.Combine(dir, "settings.xml");
    }

    public string? Get(string key)
    {
        if (!File.Exists(_path)) return null;
        try
        {
            var doc = XElement.Load(_path);
            return doc.Element(key)?.Value;
        }
        catch
        {
            return null;
        }
    }

    public T Get<T>(string key, T fallback)
    {
        var raw = Get(key);
        if (raw == null) return fallback;
        try
        {
            return (T)Convert.ChangeType(raw, typeof(T));
        }
        catch
        {
            return fallback;
        }
    }

    public void Set(string key, object? value)
    {
        XElement doc;
        if (File.Exists(_path))
        {
            try { doc = XElement.Load(_path); }
            catch { doc = new XElement("settings"); }
        }
        else
        {
            doc = new XElement("settings");
        }
        var node = doc.Element(key);
        if (node == null)
        {
            node = new XElement(key);
            doc.Add(node);
        }
        node.Value = value?.ToString() ?? string.Empty;
        doc.Save(_path);
    }
}
