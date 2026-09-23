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
            MFStartup, MFVideoFormat_H264, MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
            MFSTARTUP_NOSOCKET, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
            MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
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
    /// 实际写进去多少帧。一帧没有的话收尾会得到一个播放器打不开的空 MP4
    frames: u64,
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
            MFCreateSinkWriterFromURL(&HSTRING::from(output.as_os_str()), None, &attributes)
                .map_err(|e| err("无法创建录制文件", e))?
        };

        let out_type =
            unsafe { MFCreateMediaType().map_err(|e| err("创建输出格式失败", e))? };
        unsafe {
            out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
            out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).ok();
            out_type
                .SetUINT32(&MF_MT_AVG_BITRATE, bitrate_for(width, height))
                .ok();
            out_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .ok();
            out_type
                .SetUINT64(&MF_MT_FRAME_SIZE, packed(width, height))
                .ok();
            out_type.SetUINT64(&MF_MT_FRAME_RATE, packed(FPS, 1)).ok();
            out_type
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, packed(1, 1))
                .ok();
        }
        let stream = unsafe {
            writer
                .AddStream(&out_type)
                .map_err(|e| err("添加视频轨失败", e))?
        };

        let in_type =
            unsafe { MFCreateMediaType().map_err(|e| err("创建输入格式失败", e))? };
        unsafe {
            in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
            // WGC 给的是 BGRA8，对应 MF 的 RGB32
            in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).ok();
            in_type
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .ok();
            in_type
                .SetUINT64(&MF_MT_FRAME_SIZE, packed(width, height))
                .ok();
            in_type.SetUINT64(&MF_MT_FRAME_RATE, packed(FPS, 1)).ok();
            in_type
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, packed(1, 1))
                .ok();
            writer
                .SetInputMediaType(stream, &in_type, None)
                .map_err(|e| err("设置输入格式失败", e))?;
            writer.BeginWriting().map_err(|e| err("开始录制失败", e))?;
        }

        Ok(Self {
            writer,
            stream,
            width,
            height,
            base_time: None,
            frames: 0,
        })
    }

    /// `frame` 已按 MF 的行序排好（见 pack_frame），直接整块拷进样本。
    fn write(&mut self, frame: &[u8], timestamp: i64) -> Result<(), String> {
        let total = self.width as usize * 4 * self.height as usize;
        if frame.len() < total {
            return Err("帧数据不完整".into());
        }

        let buffer = unsafe {
            MFCreateMemoryBuffer(total as u32).map_err(|e| err("分配帧缓冲失败", e))?
        };
        unsafe {
            let mut target: *mut u8 = std::ptr::null_mut();
            buffer
                .Lock(&mut target, None, None)
                .map_err(|e| err("锁定帧缓冲失败", e))?;
            std::ptr::copy_nonoverlapping(frame.as_ptr(), target, total);
            buffer.SetCurrentLength(total as u32).ok();
            buffer.Unlock().ok();
        }

        let base = *self.base_time.get_or_insert(timestamp);
        let sample = unsafe { MFCreateSample().map_err(|e| err("创建帧样本失败", e))? };
        unsafe {
            sample.AddBuffer(&buffer).ok();
            sample.SetSampleTime(timestamp - base).ok();
            sample
                .SetSampleDuration(HNS_PER_SECOND / i64::from(FPS))
                .ok();
            self.writer
                .WriteSample(self.stream, &sample)
                .map_err(|e| err("写入帧失败", e))?;
        }
        self.frames += 1;
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
/// 把映射出来的 BGRA 逐行倒序排成 MF 要的样子。
///
/// MF 对 RGB32 按正 stride 的解释是自底向上，而 D3D 纹理自顶向下，
/// 不倒过来录出的画面是上下颠倒的。顺带把 stride 的补齐去掉，
/// 排成紧凑的一整块，定时器补帧时可以直接重发。
fn pack_frame(pixels: &[u8], stride: usize, width: u32, height: u32) -> Vec<u8> {
    let row_bytes = width as usize * 4;
    let mut packed = vec![0u8; row_bytes * height as usize];
    for row in 0..height as usize {
        let src = (height as usize - 1 - row) * stride;
        let dst = row * row_bytes;
        if src + row_bytes > pixels.len() {
            break;
        }
        packed[dst..dst + row_bytes].copy_from_slice(&pixels[src..src + row_bytes]);
    }
    packed
}

struct FrameSink {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    encoder: Encoder,
    /// 最近一帧。窗口静止不重绘时 FrameArrived 根本不触发，
    /// 定时器就靠它补帧，保证时间轴连续、文件可播。
    latest: Option<Vec<u8>>,
}

unsafe impl Send for FrameSink {}

/// 录制中的句柄。停止时交还产物路径。
pub struct ActiveRecording {
    session: GraphicsCaptureSession,
    frame_pool: Direct3D11CaptureFramePool,
    sink: Arc<Mutex<Option<FrameSink>>>,
    stopped: Arc<AtomicBool>,
    /// 按固定帧率补帧的线程，停止时要先收掉它再 Finalize
    pacer: Option<std::thread::JoinHandle<()>>,
    output: PathBuf,
}

impl ActiveRecording {
    /// 返回 (产物路径, 写入帧数)
    pub fn stop(mut self) -> (PathBuf, u64) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = self.session.Close();
        let _ = self.frame_pool.Close();
        // 先等定时器退出，否则它可能在 Finalize 之后还往 writer 里塞帧
        if let Some(pacer) = self.pacer.take() {
            let _ = pacer.join();
        }
        let frames = match self.sink.lock().take() {
            Some(sink) => {
                let frames = sink.encoder.frames;
                sink.encoder.finish();
                frames
            }
            None => 0,
        };
        // 这里不调 MFShutdown：它是进程级的，会把整个进程的 Media Foundation 关掉，
        // 之后任何 MF 操作都报「已调用 Shutdown」。初始化只做一次、不再收回。
        (self.output, frames)
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

/// 窗口是否处于最小化。
///
/// 最小化后 DWM 不再为它合成画面，WGC 一帧也拿不到——实测正常状态 44 帧、
/// 最小化 0 帧。所以必须在开录前拦下来，否则只会写出一个空文件。
pub fn is_minimized(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
    unsafe { IsIconic(hwnd as usize as *mut core::ffi::c_void) != 0 }
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
        let inspectable =
            CreateDirect3D11DeviceFromDXGIDevice(&dxgi).map_err(|e| err("创建采集设备失败", e))?;
        inspectable.cast().map_err(|e| err("创建采集设备失败", e))?
    };

    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &d3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        SizeInt32 {
            Width: width as i32,
            Height: height as i32,
        },
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
    let sink = Arc::new(Mutex::new(Some(FrameSink {
        device,
        context,
        encoder,
        latest: None,
    })));
    let stopped = Arc::new(AtomicBool::new(false));

    let handler_sink = Arc::clone(&sink);
    let handler_stopped = Arc::clone(&stopped);
    frame_pool
        .FrameArrived(&TypedEventHandler::new(
            move |pool: windows::core::Ref<Direct3D11CaptureFramePool>, _| {
                if handler_stopped.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let Some(pool) = pool.as_ref() else {
                    return Ok(());
                };
                let Ok(frame) = pool.TryGetNextFrame() else {
                    return Ok(());
                };
                let Ok(surface) = frame.Surface() else {
                    return Ok(());
                };
                let Ok(access) = surface.cast::<IDirect3DDxgiInterfaceAccess>() else {
                    return Ok(());
                };
                let texture: ID3D11Texture2D = match unsafe { access.GetInterface() } {
                    Ok(texture) => texture,
                    Err(_) => return Ok(()),
                };

                let mut guard = handler_sink.lock();
                let Some(sink) = guard.as_mut() else {
                    return Ok(());
                };

                // 采集纹理在 GPU 上且不可 CPU 读，先拷进一张 staging 纹理再映射
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                unsafe { texture.GetDesc(&mut desc) };
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                desc.MiscFlags = 0;
                let mut staging: Option<ID3D11Texture2D> = None;
                if unsafe { sink.device.CreateTexture2D(&desc, None, Some(&mut staging)) }.is_err()
                {
                    return Ok(());
                }
                let Some(staging) = staging else {
                    return Ok(());
                };
                unsafe { sink.context.CopyResource(&staging, &texture) };

                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                if unsafe {
                    sink.context
                        .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
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
                // 只更新「最近一帧」，写入交给定时器——采集是事件驱动的，
                // 直接在这里写会让输出帧率随窗口重绘频率漂移
                let (width, height) = (sink.encoder.width, sink.encoder.height);
                sink.latest = Some(pack_frame(pixels, stride, width, height));
                unsafe { sink.context.Unmap(&staging, 0) };
                Ok(())
            },
        ))
        .map_err(|e| err("注册帧回调失败", e))?;

    session.StartCapture().map_err(|e| err("启动录制失败", e))?;

    // 按固定节奏往编码器里灌帧。窗口不重绘时 FrameArrived 不触发，
    // 没有这个定时器就会写出一个零帧、播放器打不开的空 MP4——
    // 这正是「把别的应用压在上面再录」会坏掉的原因。
    let pacer_sink = Arc::clone(&sink);
    let pacer_stopped = Arc::clone(&stopped);
    let pacer = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let interval = std::time::Duration::from_nanos(1_000_000_000 / u64::from(FPS));
        let mut tick = 0u64;
        while !pacer_stopped.load(Ordering::SeqCst) {
            tick += 1;
            let due = interval * tick as u32;
            let now = start.elapsed();
            if due > now {
                std::thread::sleep(due - now);
            }
            if pacer_stopped.load(Ordering::SeqCst) {
                break;
            }
            let mut guard = pacer_sink.lock();
            let Some(sink) = guard.as_mut() else { break };
            // 分开借用：latest 只读，encoder 要可变
            let FrameSink {
                encoder, latest, ..
            } = sink;
            if let Some(frame) = latest.as_deref() {
                let timestamp = (due.as_nanos() / 100) as i64;
                let _ = encoder.write(frame, timestamp);
            }
        }
    });

    Ok(ActiveRecording {
        session,
        frame_pool,
        sink,
        stopped,
        pacer: Some(pacer),
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
        let (path, frames) = active.stop();

        let bytes = std::fs::metadata(&path).expect("产物不存在").len();
        eprintln!("产物：{} 字节，{} 帧 → {}", bytes, frames, path.display());
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
        let size = current
            .GetUINT64(&MF_MT_FRAME_SIZE)
            .map_err(|e| err("读取尺寸", e))?;
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
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|e| err("取帧缓冲", e))?;
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

    /// 把一张图压成 N 段的逐行亮度曲线。比两个粗糙的平均值稳得多：
    /// 窗口内容在垂直方向偏均匀时，两段平均值的差落在噪声里，判不出方向。
    fn row_profile(
        height: u32,
        width: u32,
        buckets: usize,
        get: &dyn Fn(u32, u32) -> f64,
    ) -> Vec<f64> {
        let mut profile = vec![0.0; buckets];
        for (bucket, bucket_profile) in profile.iter_mut().enumerate() {
            let from = height as usize * bucket / buckets;
            let to = (height as usize * (bucket + 1) / buckets).max(from + 1);
            let mut sum = 0.0;
            let mut count = 0.0;
            for y in (from..to.min(height as usize)).step_by(3) {
                for x in (0..width).step_by(16) {
                    sum += get(x, y as u32);
                    count += 1.0;
                }
            }
            *bucket_profile = if count == 0.0 { 0.0 } else { sum / count };
        }
        profile
    }

    fn distance(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum()
    }

    /// 端到端验证方向：解出的首帧与实拍的逐行亮度曲线应当同向。
    /// 若上下颠倒，曲线会与实拍的倒序更接近。
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
        // 解出来的是首帧，实拍也要尽早抓，否则期间内容变了会污染比对
        std::thread::sleep(std::time::Duration::from_millis(250));
        let truth = window.capture_image().expect("实拍失败");
        std::thread::sleep(std::time::Duration::from_millis(400));
        let (path, frames) = active.stop();
        eprintln!("写入 {frames} 帧");

        let (width, height, pixels) = decode_first_frame(&path).expect("解码失败");
        eprintln!(
            "解出 {width}x{height}，实拍 {}x{}",
            truth.width(),
            truth.height()
        );

        const BUCKETS: usize = 48;
        let usable_w = width.min(truth.width());
        let decoded = row_profile(height, usable_w, BUCKETS, &|x, y| {
            let i = (y as usize * width as usize + x as usize) * 4;
            if i + 2 < pixels.len() {
                (pixels[i] as f64 + pixels[i + 1] as f64 + pixels[i + 2] as f64) / 3.0
            } else {
                0.0
            }
        });
        let actual = row_profile(truth.height().min(height), usable_w, BUCKETS, &|x, y| {
            let p = truth.get_pixel(x.min(truth.width() - 1), y.min(truth.height() - 1));
            (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0
        });

        let mut reversed = actual.clone();
        reversed.reverse();
        let aligned = distance(&decoded, &actual);
        let flipped = distance(&decoded, &reversed);
        eprintln!("逐行曲线距离：同向 {aligned:.1} / 翻转 {flipped:.1}");
        assert!(
            aligned < flipped,
            "录出来的画面上下颠倒了：同向 {aligned:.1} 应当小于翻转 {flipped:.1}"
        );
    }
}

#[cfg(test)]
mod frame_yield_tests {
    use super::*;

    /// 诊断用：对若干后台窗口各录一小段，看各自产出多少帧。
    /// 假设是 FrameArrived 只在窗口重绘时触发，静止/被遮挡的窗口会是 0 帧。
    #[test]
    #[ignore]
    fn reports_frame_yield_per_window() {
        let windows = xcap::Window::all().expect("枚举窗口失败");
        let mut checked = 0;
        for window in windows {
            let title = window.title().unwrap_or_default();
            if title.trim().is_empty() || title.contains("snapshot") {
                continue;
            }
            if window.width().unwrap_or(0) < 320 || window.height().unwrap_or(0) < 240 {
                continue;
            }
            let Ok(id) = window.id() else { continue };
            let minimized = window.is_minimized().unwrap_or(false);

            let output = std::env::temp_dir().join(format!("snapshot-yield-{id}.mp4"));
            let _ = std::fs::remove_file(&output);
            match start(id as isize, false, &output) {
                Ok(active) => {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    let (path, frames) = active.stop();
                    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    eprintln!(
                        "{:>4} 帧  {:>9} 字节  最小化={:<5}  {}",
                        frames,
                        bytes,
                        minimized,
                        title.chars().take(46).collect::<String>()
                    );
                    let _ = std::fs::remove_file(&path);
                }
                Err(error) => eprintln!("   -  启动失败：{error}  {title}"),
            }
            checked += 1;
            if checked >= 6 {
                break;
            }
        }
        if checked == 0 {
            // 桌面上没有可测窗口是环境状态，不是回归——诊断用例不该因此变红
            eprintln!("没有找到可测的窗口，跳过");
        }
    }
}

#[cfg(test)]
mod minimized_tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, ShowWindow, SW_MINIMIZE, SW_RESTORE,
    };

    /// 最小化的窗口到底能不能录？实测：把目标最小化、录一小段、立刻还原。
    #[test]
    #[ignore]
    fn measures_frame_yield_while_minimized() {
        let windows = xcap::Window::all().expect("枚举窗口失败");
        let Some(window) = windows.into_iter().find(|w| {
            let title = w.title().unwrap_or_default();
            !title.trim().is_empty()
                && !title.contains("snapshot")
                && !w.is_minimized().unwrap_or(false)
                && w.width().unwrap_or(0) >= 320
                && w.height().unwrap_or(0) >= 240
        }) else {
            eprintln!("没有可用窗口，跳过");
            return;
        };
        let id = window.id().expect("取窗口 id 失败");
        let handle = id as usize as *mut core::ffi::c_void;
        eprintln!("对象：{}", window.title().unwrap_or_default());

        // 先量一次正常状态做对照
        let baseline = record_and_count(id as isize, "baseline");

        unsafe { ShowWindow(handle, SW_MINIMIZE) };
        std::thread::sleep(std::time::Duration::from_millis(500));
        let minimized_now = unsafe { IsIconic(handle) } != 0;
        let minimized = record_and_count(id as isize, "minimized");
        // 不论上面结果如何都要还原，别把用户的窗口留在最小化状态
        unsafe { ShowWindow(handle, SW_RESTORE) };

        eprintln!("确实处于最小化：{minimized_now}");
        eprintln!("正常状态 {baseline} 帧 / 最小化 {minimized} 帧");
        assert!(minimized_now, "没能把窗口最小化，这次测量无效");
        assert!(baseline > 0, "正常状态都没帧，环境有问题");
        // 钉住这个事实：最小化就是拿不到帧，所以上层必须先还原再录
        assert_eq!(
            minimized, 0,
            "最小化窗口本不该产出帧，行为若变了要重新审视还原逻辑"
        );
        assert!(!is_minimized(id as isize), "测完应当已还原");
    }

    fn record_and_count(hwnd: isize, tag: &str) -> u64 {
        let output = std::env::temp_dir().join(format!("snapshot-min-{tag}.mp4"));
        let _ = std::fs::remove_file(&output);
        match start(hwnd, false, &output) {
            Ok(active) => {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                let (path, frames) = active.stop();
                let _ = std::fs::remove_file(&path);
                frames
            }
            Err(error) => {
                eprintln!("{tag} 启动失败：{error}");
                0
            }
        }
    }
}
