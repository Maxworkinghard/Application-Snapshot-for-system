//! 文字识别：Windows.Media.Ocr（WinRT，系统自带）。

use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

pub(crate) const OCR_BACKEND: &str = "Windows.Media.Ocr";

fn engine() -> Result<OcrEngine, String> {
    OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| {
        "系统没有可用的 OCR 语言包，请在「设置 → 时间和语言 → 语言」中添加".to_string()
    })
}

/// 引擎是否就绪。可用时返回识别语言的可读名称。
pub(crate) fn ocr_language() -> Result<String, String> {
    let language = engine()?
        .RecognizerLanguage()
        .map_err(|error| error.to_string())?;
    language
        .DisplayName()
        .map(|name| name.to_string_lossy())
        .map_err(|error| error.to_string())
}

pub(crate) fn ocr_recognize(png: &[u8]) -> Result<Vec<String>, String> {
    let stream = InMemoryRandomAccessStream::new().map_err(|error| error.to_string())?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(|error| error.to_string())?;
    writer.WriteBytes(png).map_err(|error| error.to_string())?;
    writer
        .StoreAsync()
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|e| e.to_string())?;
    writer
        .FlushAsync()
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|e| e.to_string())?;
    writer.DetachStream().map_err(|error| error.to_string())?;
    stream.Seek(0).map_err(|error| error.to_string())?;

    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|error| format!("解码图像失败：{error}"))?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|error| format!("读取位图失败：{error}"))?;

    let result = engine()?
        .RecognizeAsync(&bitmap)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|error| format!("识别失败：{error}"))?;

    // Text() 会把整页拼成一行，按 Lines 自己拼才能保留换行
    let lines = result.Lines().map_err(|error| error.to_string())?;
    let mut out = Vec::new();
    for line in lines {
        if let Ok(text) = line.Text() {
            out.push(text.to_string_lossy());
        }
    }
    Ok(out)
}
