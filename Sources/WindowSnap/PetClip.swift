import AppKit
import ImageIO

/// 一段解码好的 GIF 动画：整张画布的逐帧位图 + 每帧延迟（毫秒）。
/// 引用类型——idle 缺失时会被别的动作别名共享，值类型会复制解码结果。
final class PetClip {
    let frames: [CGImage]
    let delays: [Int]
    let loops: Bool

    private init(frames: [CGImage], delays: [Int], loops: Bool) {
        self.frames = frames
        self.delays = delays
        self.loops = loops
    }

    /// 文件缺失或解码失败返回 nil（该动作视为不存在，SetPose 静默忽略）。
    /// ImageIO 已处理 GIF 的 disposal 与子区域合成，逐帧直接是整张画布。
    static func load(url: URL, loops: Bool) -> PetClip? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        let count = CGImageSourceGetCount(source)
        guard count > 0 else { return nil }

        var frames: [CGImage] = []
        var delays: [Int] = []
        for index in 0..<count {
            guard let frame = CGImageSourceCreateImageAtIndex(source, index, nil) else { return nil }
            frames.append(frame)
            delays.append(delayMs(source: source, index: index))
        }
        return PetClip(frames: frames, delays: delays, loops: loops)
    }

    /// 与 Windows 同规则：下限 20ms，读不到延迟按 10 厘秒（100ms）算。
    private static func delayMs(source: CGImageSource, index: Int) -> Int {
        let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any]
        let gif = properties?[kCGImagePropertyGIFDictionary] as? [CFString: Any]
        let seconds = (gif?[kCGImagePropertyGIFUnclampedDelayTime] as? Double)
            ?? (gif?[kCGImagePropertyGIFDelayTime] as? Double)
            ?? 0.1
        return max(20, Int((seconds * 1000).rounded()))
    }
}
