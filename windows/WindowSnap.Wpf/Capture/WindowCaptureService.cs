using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Media.Imaging;
using Windows.Foundation;
using Windows.Graphics;
using Windows.Graphics.Capture;
using Windows.Graphics.DirectX;
using Windows.Graphics.DirectX.Direct3D11;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;
using WinRT;
using WinRTBitmapEncoder = Windows.Graphics.Imaging.BitmapEncoder;
using WpfBitmapFrame = System.Windows.Media.Imaging.BitmapFrame;

namespace WindowSnap.Wpf.Capture;

/// <summary>
/// 用 Windows.Graphics.Capture 截取指定窗口的最新帧。
/// 编码走系统自带的 SoftwareBitmap + BitmapEncoder，不依赖 Win2D。
/// </summary>
public sealed class WindowCaptureService
{
    public record Result(string ApplicationName, string Title);

    public async Task<Result> CaptureAsync(IntPtr hwnd, string appName, string title)
    {
        if (hwnd == IntPtr.Zero) throw new ArgumentException("hwnd 为空");

        var item = CaptureItemFromWindow(hwnd)
            ?? throw new InvalidOperationException("无法为目标窗口创建 CaptureItem");

        var device = Direct3D11Helper.CreateDevice();

        // CreateFreeThreaded：WPF 线程没有 WinRT DispatcherQueue，普通 Create 的 FrameArrived 不会触发
        var framePool = Direct3D11CaptureFramePool.CreateFreeThreaded(
            device, DirectXPixelFormat.B8G8R8A8UIntNormalized, 2,
            new SizeInt32 { Width = Math.Max(1, item.Size.Width), Height = Math.Max(1, item.Size.Height) });

        var tcs = new TaskCompletionSource<Direct3D11CaptureFrame?>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        TypedEventHandler<Direct3D11CaptureFramePool, object> arrived = (pool, _) =>
        {
            // 帧要传给 await 方使用，这里不能 Dispose
            try { tcs.TrySetResult(pool.TryGetNextFrame()); }
            catch { tcs.TrySetResult(null); }
        };
        framePool.FrameArrived += arrived;

        var session = framePool.CreateCaptureSession(item);
        try { session.IsCursorCaptureEnabled = false; } catch { }
        try { session.IsBorderRequired = false; } catch { } // Win11 起才有，旧系统会抛
        session.StartCapture();

        using var cts = new System.Threading.CancellationTokenSource(TimeSpan.FromSeconds(2));
        using var reg = cts.Token.Register(() => tcs.TrySetResult(null));

        var frame = await tcs.Task;
        session.Dispose();
        framePool.FrameArrived -= arrived;
        framePool.Dispose();

        if (frame == null) throw new InvalidOperationException("未能获取到窗口帧（超时）");

        byte[] pngBytes;
        using (frame)
        {
            pngBytes = await EncodePngAsync(frame.Surface);
        }

        WriteToClipboard(pngBytes);
        App.Current.AutoClearService.OnWrite();
        return new Result(appName, title);
    }

    private static async Task<byte[]> EncodePngAsync(IDirect3DSurface surface)
    {
        using var softwareBitmap = await SoftwareBitmap.CreateCopyFromSurfaceAsync(surface);
        using var converted = SoftwareBitmap.Convert(
            softwareBitmap, BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied);
        using var stream = new InMemoryRandomAccessStream();
        var encoder = await WinRTBitmapEncoder.CreateAsync(WinRTBitmapEncoder.PngEncoderId, stream);
        encoder.SetSoftwareBitmap(converted);
        await encoder.FlushAsync();

        var bytes = new byte[(int)stream.Size];
        using var reader = new DataReader(stream.GetInputStreamAt(0));
        await reader.LoadAsync((uint)bytes.Length);
        reader.ReadBytes(bytes);
        return bytes;
    }

    private static void WriteToClipboard(byte[] pngBytes)
    {
        WpfBitmapFrame image;
        try
        {
            image = WpfBitmapFrame.Create(
                new MemoryStream(pngBytes), BitmapCreateOptions.None, BitmapCacheOption.OnLoad);
        }
        catch (Exception)
        {
            throw new InvalidOperationException("PNG 解码失败");
        }

        // SetImage 写 CF_BITMAP/CF_DIB，几乎所有应用都认；
        // "PNG" 自定义格式保留透明度，浏览器/聊天工具会优先取用。
        // MemoryStream 会按原始字节写入，byte[] 会被序列化污染，不能直接 SetData byte[]。
        var data = new DataObject();
        data.SetImage(image);
        data.SetData("PNG", new MemoryStream(pngBytes));

        try
        {
            Clipboard.SetDataObject(data, copy: true);
        }
        catch (Exception)
        {
            throw new InvalidOperationException("写入剪贴板失败");
        }
    }

    // IGraphicsCaptureItem 的接口 IID（公开文档值）
    private static readonly Guid GraphicsCaptureItemIid = new("79C3F95B-31F7-4EC2-A464-632EF5D30760");

    private static GraphicsCaptureItem? CaptureItemFromWindow(IntPtr hwnd)
    {
        var interop = GraphicsCaptureItem.As<IGraphicsCaptureItemInterop>();
        var iid = GraphicsCaptureItemIid;
        var abi = interop.CreateForWindow(hwnd, ref iid);
        if (abi == IntPtr.Zero) return null;
        try
        {
            return GraphicsCaptureItem.FromAbi(abi);
        }
        finally
        {
            Marshal.Release(abi);
        }
    }
}

[ComImport]
[Guid("3628E81B-3CAC-4C60-B7F4-23CE0E0C3356")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IGraphicsCaptureItemInterop
{
    IntPtr CreateForWindow([In] IntPtr window, [In] ref Guid iid);
    IntPtr CreateForMonitor([In] IntPtr hmon, [In] ref Guid iid);
}

/// <summary>
/// 创建 WinRT IDirect3DDevice，替代 Win2D 的 CanvasDevice。
/// 硬件设备失败时回退 WARP 软渲染（远程桌面/无 GPU 环境）。
/// </summary>
internal static class Direct3D11Helper
{
    private const uint D3D11SdkVersion = 7;
    private const uint D3D11CreateDeviceBgraSupport = 0x20;
    private const uint DriverTypeHardware = 1;
    private const uint DriverTypeWarp = 5;
    private static readonly Guid DxgiDeviceIid = new("54EC77FA-1377-44E6-8C32-88FD5F44C84C");

    [DllImport("d3d11.dll", ExactSpelling = true)]
    private static extern int D3D11CreateDevice(
        IntPtr adapter, uint driverType, IntPtr software, uint flags,
        IntPtr featureLevels, uint featureLevelCount, uint sdkVersion,
        out IntPtr device, out uint featureLevel, out IntPtr immediateContext);

    [DllImport("d3d11.dll", ExactSpelling = true)]
    private static extern int CreateDirect3D11DeviceFromDXGIDevice(
        IntPtr dxgiDevice, out IntPtr graphicsDevice);

    public static IDirect3DDevice CreateDevice()
    {
        var hr = D3D11CreateDevice(
            IntPtr.Zero, DriverTypeHardware, IntPtr.Zero, D3D11CreateDeviceBgraSupport,
            IntPtr.Zero, 0, D3D11SdkVersion, out var d3dDevice, out _, out var context);
        if (hr < 0)
        {
            hr = D3D11CreateDevice(
                IntPtr.Zero, DriverTypeWarp, IntPtr.Zero, D3D11CreateDeviceBgraSupport,
                IntPtr.Zero, 0, D3D11SdkVersion, out d3dDevice, out _, out context);
        }
        Marshal.ThrowExceptionForHR(hr);
        if (context != IntPtr.Zero) Marshal.Release(context);

        try
        {
            var iid = DxgiDeviceIid;
            Marshal.ThrowExceptionForHR(Marshal.QueryInterface(d3dDevice, ref iid, out var dxgiDevice));
            try
            {
                Marshal.ThrowExceptionForHR(
                    CreateDirect3D11DeviceFromDXGIDevice(dxgiDevice, out var inspectable));
                try
                {
                    return MarshalInterface<IDirect3DDevice>.FromAbi(inspectable);
                }
                finally
                {
                    Marshal.Release(inspectable);
                }
            }
            finally
            {
                Marshal.Release(dxgiDevice);
            }
        }
        finally
        {
            Marshal.Release(d3dDevice);
        }
    }
}
