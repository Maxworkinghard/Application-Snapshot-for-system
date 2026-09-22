//! Windows 窗口录制：Windows.Graphics.Capture 取帧 + Media Foundation 编码。
//!
//! 取代此前的 ffmpeg `gdigrab`。gdigrab 走 GDI `BitBlt`，抓的是屏幕上那块像素，
//! 抓不到 DWM 的合成层——硬件加速的窗口（Chrome、Electron、游戏）录出来是黑的；
//! 窗口被挡住就录到遮挡物；而且 ffmpeg 只认窗口标题，同名窗口会选错。
//!
//! WGC 直接从 DWM 拿合成后的帧，按 HWND 定位，这三件事都不存在。编码交给
//! Media Foundation 的 SinkWriter，有硬件编码器时自动走硬件，顺带把 ffmpeg
//! 这个外部依赖从录制链路上摘掉。

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use parking_lot::Mutex;
use windows::{
    core::{Interface, HSTRING},
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        SizeInt32,
    },
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D11::{
                D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
                D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
                D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
            },
            Dxgi::IDXGIDevice,
        },
        Media::MediaFoundation::{
            IMFAttributes, IMFSinkWriter, MFCreateAttributes, MFCreateMediaType,
            MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Video,
            MFStartup, MFVideoFormat_H264,
            MFVideoFormat_RGB32, MFVideoInterlace_Progressive, MFSTARTUP_NOSOCKET,
            MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE,
            MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
            MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING, MF_VERSION,
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
};

const FPS: u32 = 30;
/// MF 用一个 UINT64 表示「宽高」或「分子分母」：高 32 位在前，低 32 位在后。
/// MFSetAttributeSize / MFSetAttributeRatio 是 C++ 头文件里的内联函数，
/// Rust 绑定里没有，只能自己打包。
fn packed(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

/// 100 纳秒为单位，MF 的时间基准
const HNS_PER_SECOND: i64 = 10_000_000;

fn err(context: &str, error: windows::core::Error) -> String {
    format!("{context}：{}", error.message())
}

/// 码率按面积估，1080p 约 12Mbps。给高了浪费，给低了文字发糊。
fn bitrate_for(width: u32, height: u32) -> u32 {
    let pixels = u64::from(width) * u64::from(height);
    let scaled = pixels * 12_000_000 / (1920 * 1080);
    scaled.clamp(2_000_000, 40_000_000) as u32
}

struct Encoder {
    writer: IMFSinkWriter,
    stream: u32,
    width: u32,
    height: u32,
    /// 第一帧的 SystemRelativeTime，后续帧减它得到相对时间轴
    base_time: Option<i64>,
}

impl Encoder {
    fn new(output: &Path, width: u32, height: u32) -> Result<Self, String> {
        let attributes: IMFAttributes = unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 2).map_err(|e| err("创建编码器属性失败", e))?;
            attributes.ok_or_else(|| "创建编码器属性失败".to_string())?
        };
        unsafe {
            // 有硬件编码器就用，没有回落到软件实现
            attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(|e| err("启用硬件编码失败", e))?;
            // 采集帧是实时来的，不需要 sink 再限速
            attributes
                .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
                .map_err(|e| err("配置编码器失败", e))?;
        }

        let writer = unsafe {
            MFCreateSinkWriterFromURL(
                &HSTRING::from(output.as_os_str()),
                None,
                &attributes,
            )
            .map_err(|e| err("无法创建录制文件", e))?
        };

        let out_type = unsafe { MFCreateMediaType().map_err(|e| err("创建输出格式失败", e))? };
        unsafe {
            out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
            out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).ok();
            out_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate_for(width, height)).ok();
            out_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .ok();
            out_type.SetUINT64(&MF_MT_FRAME_SIZE, packed(width, height)).ok();
            out_type.SetUINT64(&MF_MT_FRAME_RATE, packed(FPS, 1)).ok();
            out_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, packed(1, 1)).ok();
        }
        let stream = unsafe {
            writer.AddStream(&out_type).map_err(|e| err("添加视频轨失败", e))?
        };

        let in_type = unsafe { MFCreateMediaType().map_err(|e| err("创建输入格式失败", e))? };
        unsafe {
            in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
            // WGC 给的是 BGRA8，对应 MF 的 RGB32
            in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).ok();
            in_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .ok();
            in_type.SetUINT64(&MF_MT_FRAME_SIZE, packed(width, height)).ok();
            in_type.SetUINT64(&MF_MT_FRAME_RATE, packed(FPS, 1)).ok();
            in_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, packed(1, 1)).ok();
            writer
                .SetInputMediaType(stream, &in_type, None)
                .map_err(|e| err("设置输入格式失败", e))?;
            writer.BeginWriting().map_err(|e| err("开始录制失败", e))?;
        }

        Ok(Self { writer, stream, width, height, base_time: None })
    }

    /// `pixels` 是映射出来的 BGRA 数据，`stride` 是它的行距（字节）。
    fn write(&mut self, pixels: &[u8], stride: usize, timestamp: i64) -> Result<(), String> {
        let row_bytes = self.width as usize * 4;
        let total = row_bytes * self.height as usize;

        let buffer = unsafe {
            MFCreateMemoryBuffer(total as u32).map_err(|e| err("分配帧缓冲失败", e))?
        };
        unsafe {
            let mut target: *mut u8 = std::ptr::null_mut();
            buffer
                .Lock(&mut target, None, None)
                .map_err(|e| err("锁定帧缓冲失败", e))?;
            // RGB32 在 MF 里按正 stride 解释是自底向上的，而 D3D 纹理是自顶向下，
            // 所以逐行倒着拷——不这样录出来整个画面是上下颠倒的。
            for row in 0..self.height as usize {
                let src_offset = (self.height as usize - 1 - row) * stride;
                let dst_offset = row * row_bytes;
                if src_offset + row_bytes > pixels.len() {
                    break;
                }
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().add(src_offset),
                    target.add(dst_offset),
                    row_bytes,
                );
            }
            buffer.SetCurrentLength(total as u32).ok();
            buffer.Unlock().ok();
        }

        let base = *self.base_time.get_or_insert(timestamp);
        let sample = unsafe { MFCreateSample().map_err(|e| err("创建帧样本失败", e))? };
        unsafe {
            sample.AddBuffer(&buffer).ok();
            sample.SetSampleTime(timestamp - base).ok();
            sample.SetSampleDuration(HNS_PER_SECOND / i64::from(FPS)).ok();
            self.writer
                .WriteSample(self.stream, &sample)
                .map_err(|e| err("写入帧失败", e))?;
        }
        Ok(())
    }

    fn finish(self) {
        unsafe {
            let _ = self.writer.Finalize();
        }
    }
}

/// 帧回调跑在 WGC 的线程池上，而 D3D 设备、上下文、SinkWriter 都是裸 COM 指针，
/// 编译器认定它们不是 Send。事实上 D3D11 设备本身是线程安全的（没开
/// SINGLETHREADED），设备上下文与 SinkWriter 不是——所以把三者锁进同一个
/// Mutex，保证任何时刻只有一个线程在用，这个 Send 才是成立的。
struct FrameSink {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    encoder: Encoder,
}

unsafe impl Send for FrameSink {}

/// 录制中的句柄。停止时交还产物路径。
pub struct ActiveRecording {
    session: GraphicsCaptureSession,
    frame_pool: Direct3D11CaptureFramePool,
    sink: Arc<Mutex<Option<FrameSink>>>,
    stopped: Arc<AtomicBool>,
    output: PathBuf,
}

impl ActiveRecording {
    pub fn stop(self) -> PathBuf {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = self.session.Close();
        let _ = self.frame_pool.Close();
        if let Some(sink) = self.sink.lock().take() {
            sink.encoder.finish();
        }
        // 这里不调 MFShutdown：它是进程级的，会把整个进程的 Media Foundation 关掉，
        // 之后任何 MF 操作都报「已调用 Shutdown」。初始化只做一次、不再收回。
        self.output
    }
}

fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    // WGC 的帧池要求设备支持 BGRA
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let mut context = None;
        let result = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        if result.is_ok() {
            if let (Some(device), Some(context)) = (device, context) {
                return Ok((device, context));
            }
        }
    }
    Err("无法创建 Direct3D 设备".into())
}

/// 开始录制 `hwnd` 指向的窗口。
pub fn start(hwnd: isize, include_cursor: bool, output: &Path) -> Result<ActiveRecording, String> {
    if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
        return Err("当前系统不支持窗口录制（需要 Windows 10 1903 或更高）".into());
    }
    // Media Foundation 按进程初始化一次即可；MFShutdown 是进程级的，不在停止录制时调用
    static MF_INIT: std::sync::Once = std::sync::Once::new();
    let mut init_error = None;
    MF_INIT.call_once(|| unsafe {
        if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) {
            init_error = Some(err("初始化编码器失败", error));
        }
    });
    if let Some(error) = init_error {
        return Err(error);
    }

    let item: GraphicsCaptureItem = unsafe {
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                .map_err(|e| err("无法访问窗口采集接口", e))?;
        interop
            .CreateForWindow(HWND(hwnd as *mut _))
            .map_err(|e| err("无法采集该窗口", e))?
    };

    let size: SizeInt32 = item.Size().map_err(|e| err("读取窗口尺寸失败", e))?;
    // H.264 要求宽高为偶数，奇数会被编码器拒绝
    let width = (size.Width.max(2) as u32) & !1;
    let height = (size.Height.max(2) as u32) & !1;

    let (device, context) = create_d3d_device()?;
    let dxgi: IDXGIDevice = device.cast().map_err(|e| err("获取 DXGI 设备失败", e))?;
    let d3d_device: IDirect3DDevice = unsafe {
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
            .map_err(|e| err("创建采集设备失败", e))?;
        inspectable.cast().map_err(|e| err("创建采集设备失败", e))?
    };

    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &d3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        SizeInt32 { Width: width as i32, Height: height as i32 },
    )
    .map_err(|e| err("创建帧池失败", e))?;

    let session = frame_pool
        .CreateCaptureSession(&item)
        .map_err(|e| err("创建采集会话失败", e))?;
    // 光标是否入画跟随设置，与其它平台一致
    let _ = session.SetIsCursorCaptureEnabled(include_cursor);
    // Win11 起可以去掉采集时那圈黄框；老系统上这个属性不存在，忽略失败即可
    let _ = session.SetIsBorderRequired(false);

    let encoder = Encoder::new(output, width, height)?;
    let sink = Arc::new(Mutex::new(Some(FrameSink { device, context, encoder })));
    let stopped = Arc::new(AtomicBool::new(false));

    let handler_sink = Arc::clone(&sink);
    let handler_stopped = Arc::clone(&stopped);
    frame_pool
        .FrameArrived(&TypedEventHandler::new(
            move |pool: windows::core::Ref<Direct3D11CaptureFramePool>, _| {
                if handler_stopped.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let Some(pool) = pool.as_ref() else { return Ok(()) };
                let Ok(frame) = pool.TryGetNextFrame() else { return Ok(()) };
                let Ok(timestamp) = frame.SystemRelativeTime() else { return Ok(()) };
                let Ok(surface) = frame.Surface() else { return Ok(()) };
                let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() else {
                    return Ok(());
                };
                let texture: ID3D11Texture2D = match unsafe { access.GetInterface() } {
                    Ok(texture) => texture,
                    Err(_) => return Ok(()),
                };

                let mut guard = handler_sink.lock();
                let Some(sink) = guard.as_mut() else { return Ok(()) };

                // 采集纹理在 GPU 上且不可 CPU 读，先拷进一张 staging 纹理再映射
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                unsafe { texture.GetDesc(&mut desc) };
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                desc.MiscFlags = 0;
                let mut staging: Option<ID3D11Texture2D> = None;
                if unsafe { sink.device.CreateTexture2D(&desc, None, Some(&mut staging)) }
                    .is_err()
                {
                    return Ok(());
                }
                let Some(staging) = staging else { return Ok(()) };
                unsafe { sink.context.CopyResource(&staging, &texture) };

                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                if unsafe {
                    sink.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                }
                .is_err()
                {
                    return Ok(());
                }
                let stride = mapped.RowPitch as usize;
                let pixels = unsafe {
                    std::slice::from_raw_parts(
                        mapped.pData as *const u8,
                        stride * desc.Height as usize,
                    )
                };
                let _ = sink.encoder.write(pixels, stride, timestamp.Duration);
                unsafe { sink.context.Unmap(&staging, 0) };
                Ok(())
            },
        ))
        .map_err(|e| err("注册帧回调失败", e))?;

    session.StartCapture().map_err(|e| err("启动录制失败", e))?;

    Ok(ActiveRecording {
        session,
        frame_pool,
        sink,
        stopped,
        output: output.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 找一个可见的、不是本应用自己的窗口来做被摄对象
    fn some_window() -> Option<(isize, String)> {
        let windows = xcap::Window::all().ok()?;
        for window in windows {
            let title = window.title().unwrap_or_default();
            if title.trim().is_empty() || title.contains("snapshot") {
                continue;
            }
            if window.width().unwrap_or(0) < 320 || window.height().unwrap_or(0) < 240 {
                continue;
            }
            if let Ok(id) = window.id() {
                return Some((id as isize, title));
            }
        }
        None
    }

    /// 真机录一段。默认 ignore——它会在临时目录留下一个 MP4，且依赖当前有可见窗口。
    #[test]
    #[ignore]
    fn records_a_real_window_to_mp4() {
        let Some((hwnd, title)) = some_window() else {
            eprintln!("没有找到可用的窗口，跳过");
            return;
        };
        let output = std::env::temp_dir().join("snapshot-wgc-test.mp4");
        let _ = std::fs::remove_file(&output);

        eprintln!("录制目标：{title}");
        let active = start(hwnd, true, &output).expect("启动录制失败");
        std::thread::sleep(std::time::Duration::from_secs(3));
        let path = active.stop();

        let bytes = std::fs::metadata(&path).expect("产物不存在").len();
        eprintln!("产物：{} 字节 → {}", bytes, path.display());
        assert!(bytes > 20_000, "文件太小，多半没写进帧：{bytes} 字节");

        // MP4 的 moov 要在 Finalize 时补上，没有它播放器读不了
        let head = std::fs::read(&path).expect("读取产物失败");
        let has_moov = head.windows(4).any(|w| w == b"moov");
        assert!(has_moov, "缺少 moov box，文件没有正常收尾");
    }

    /// 单独验采集那一环：抓一帧存成 PNG，肉眼确认不是黑屏
    #[test]
    #[ignore]
    fn captures_one_visible_frame() {
        let Some((hwnd, title)) = some_window() else {
            eprintln!("没有找到可用的窗口，跳过");
            return;
        };
        eprintln!("采集目标：{title}");

        let output = std::env::temp_dir().join("snapshot-wgc-probe.mp4");
        let _ = std::fs::remove_file(&output);
        let active = start(hwnd, false, &output).expect("启动采集失败");
        std::thread::sleep(std::time::Duration::from_millis(600));

        // 借录制链路里那张 staging 纹理之外，单独再抓一次当前窗口内容做对照
        let png = std::env::temp_dir().join("snapshot-wgc-probe.png");
        let window = xcap::Window::all()
            .ok()
            .and_then(|list| list.into_iter().find(|w| w.id().ok() == Some(hwnd as u32)));
        if let Some(window) = window {
            if let Ok(image) = window.capture_image() {
                let _ = image.save(&png);
                eprintln!("对照图：{}", png.display());
            }
        }
        let _ = active.stop();
    }
}

/// 解一帧出来做验证用。只给测试调用：把录好的 MP4 第一帧解成 BGRA。
#[cfg(test)]
fn decode_first_frame(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    use windows::Win32::Media::MediaFoundation::{
        MFCreateSourceReaderFromURL, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
        MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    };
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|e| err("解码属性", e))?;
        let attributes = attributes.ok_or("解码属性")?;
        attributes
            .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
            .ok();
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)
            .map_err(|e| err("打开录制文件", e))?;

        let want = MFCreateMediaType().map_err(|e| err("解码格式", e))?;
        want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
        want.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).ok();
        reader
            .SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &want)
            .map_err(|e| err("设置解码格式", e))?;

        let current = reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .map_err(|e| err("读取解码格式", e))?;
        let size = current.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|e| err("读取尺寸", e))?;
        let width = (size >> 32) as u32;
        let height = (size & 0xFFFF_FFFF) as u32;

        let mut flags = 0u32;
        let mut timestamp = 0i64;
        let mut sample = None;
        reader
            .ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )
            .map_err(|e| err("读取帧", e))?;
        let sample = sample.ok_or("没有解出帧")?;
        let buffer = sample.ConvertToContiguousBuffer().map_err(|e| err("取帧缓冲", e))?;
        let mut data: *mut u8 = std::ptr::null_mut();
        let mut length = 0u32;
        buffer
            .Lock(&mut data, None, Some(&mut length))
            .map_err(|e| err("锁定帧缓冲", e))?;
        let pixels = std::slice::from_raw_parts(data, length as usize).to_vec();
        buffer.Unlock().ok();
        Ok((width, height, pixels))
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    /// 端到端验证方向：解出来的第一帧应当与 xcap 的实拍同向。
    /// 若上下颠倒，说明写帧时的行序与 MF 对 RGB32 的 stride 约定对不上。
    #[test]
    #[ignore]
    fn recorded_frame_is_not_upside_down() {
        let windows = xcap::Window::all().expect("枚举窗口失败");
        let Some(window) = windows.into_iter().find(|w| {
            let title = w.title().unwrap_or_default();
            !title.trim().is_empty()
                && !title.contains("snapshot")
                && w.width().unwrap_or(0) >= 320
                && w.height().unwrap_or(0) >= 240
        }) else {
            eprintln!("没有可用窗口，跳过");
            return;
        };
        let hwnd = window.id().expect("取窗口 id 失败") as isize;
        eprintln!("对象：{}", window.title().unwrap_or_default());

        let output = std::env::temp_dir().join("snapshot-wgc-orient.mp4");
        let _ = std::fs::remove_file(&output);
        let active = start(hwnd, false, &output).expect("启动录制失败");
        std::thread::sleep(std::time::Duration::from_millis(1200));
        // 录制过程中抓一张实拍做基准
        let truth = window.capture_image().expect("实拍失败");
        let path = active.stop();

        let (width, height, pixels) = decode_first_frame(&path).expect("解码失败");
        eprintln!("解出 {width}x{height}，实拍 {}x{}", truth.width(), truth.height());

        // 比较上半区与下半区的平均亮度：方向错了两者会对调
        let brightness = |rows: std::ops::Range<u32>, get: &dyn Fn(u32, u32) -> [u8; 3]| -> f64 {
            let mut sum = 0f64;
            let mut count = 0f64;
            for y in rows.clone().step_by(8) {
                for x in (0..width.min(truth.width())).step_by(16) {
                    let p = get(x, y);
                    sum += (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0;
                    count += 1.0;
                }
            }
            if count == 0.0 { 0.0 } else { sum / count }
        };
        let usable = height.min(truth.height());
        let decoded_px = |x: u32, y: u32| -> [u8; 3] {
            let i = (y as usize * width as usize + x as usize) * 4;
            if i + 2 < pixels.len() { [pixels[i + 2], pixels[i + 1], pixels[i]] } else { [0, 0, 0] }
        };
        let truth_px = |x: u32, y: u32| -> [u8; 3] {
            let p = truth.get_pixel(x, y);
            [p[0], p[1], p[2]]
        };

        let d_top = brightness(0..usable / 3, &decoded_px);
        let d_bottom = brightness(usable * 2 / 3..usable, &decoded_px);
        let t_top = brightness(0..usable / 3, &truth_px);
        let t_bottom = brightness(usable * 2 / 3..usable, &truth_px);
        eprintln!("解码 上{d_top:.1} 下{d_bottom:.1} / 实拍 上{t_top:.1} 下{t_bottom:.1}");

        let same_orientation = (d_top - t_top).abs() + (d_bottom - t_bottom).abs();
        let flipped = (d_top - t_bottom).abs() + (d_bottom - t_top).abs();
        eprintln!("同向差 {same_orientation:.1} / 翻转差 {flipped:.1}");
        assert!(
            same_orientation <= flipped,
            "录出来的画面上下颠倒了：同向差 {same_orientation:.1} > 翻转差 {flipped:.1}"
        );
    }
}
