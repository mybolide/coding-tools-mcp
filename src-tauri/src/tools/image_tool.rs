use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD, Engine};
use image::codecs::{jpeg::JpegEncoder, png::PngEncoder};
use image::{DynamicImage, ImageFormat, ImageReader};
use serde_json::{json, Value};

use crate::tools::workspace::{tool_ok, Workspace, WorkspaceError};

const DEFAULT_MAX_BYTES: usize = 5_242_880;
const DEFAULT_MAX_DIMENSION: u32 = 2000;

pub fn view_image(ws: &Workspace, args: &Value) -> Result<Value, WorkspaceError> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| WorkspaceError::invalid_argument("path is required"))?;
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_MAX_BYTES as u64) as usize;
    let max_width = args
        .get("max_width")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_MAX_DIMENSION as u64) as u32;
    let max_height = args
        .get("max_height")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_MAX_DIMENSION as u64) as u32;
    let auto_resize = args.get("auto_resize").and_then(Value::as_bool).unwrap_or(true);

    let resolved = ws.resolve_read_path(path)?;
    let mut data = std::fs::read(&resolved.path).map_err(|e| WorkspaceError::Tool {
        code: "IO_ERROR",
        message: format!("Failed to read image: {e}"),
        category: "runtime",
        retryable: false,
    })?;

    let (mut mime_type, mut width, mut height) = identify_image(&data)?;
    let original = json!({
        "bytes": data.len(),
        "width": width,
        "height": height,
        "mime_type": mime_type
    });

    let mut resized = false;
    let mut warnings: Vec<String> = Vec::new();

    if auto_resize && should_resize(data.len(), width, height, max_bytes, max_width, max_height) {
        match resize_image(&data, &mime_type, max_width, max_height, max_bytes) {
            Ok(Some((new_data, _new_mime))) => {
                data = new_data;
                (mime_type, width, height) = identify_image(&data)?;
                resized = true;
            }
            Ok(None) => warnings.push(
                "auto_resize requested but image resize failed or format unsupported".into(),
            ),
            Err(err) => warnings.push(format!("auto_resize failed: {err}")),
        }
    }

    if data.len() > max_bytes {
        return Err(WorkspaceError::Tool {
            code: "OUTPUT_TOO_LARGE",
            message: "Image exceeds max_bytes.".into(),
            category: "validation",
            retryable: false,
        });
    }

    let encoded = STANDARD.encode(&data);
    Ok(tool_ok(json!({
        "path": resolved.display,
        "mime_type": mime_type,
        "bytes": data.len(),
        "width": width,
        "height": height,
        "resized": resized,
        "original": original,
        "base64": encoded,
        "data_url": format!("data:{mime_type};base64,{encoded}"),
        "warnings": warnings
    })))
}

fn identify_image(data: &[u8]) -> Result<(String, u32, u32), WorkspaceError> {
    let format = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| binary_file_error())?
        .format()
        .ok_or_else(binary_file_error)?;
    let img = image::load_from_memory(data).map_err(|_| binary_file_error())?;
    let mime = mime_for_format(format);
    Ok((mime.to_string(), img.width(), img.height()))
}

fn binary_file_error() -> WorkspaceError {
    WorkspaceError::Tool {
        code: "BINARY_FILE",
        message: "File is not a supported image.".into(),
        category: "validation",
        retryable: false,
    }
}

fn mime_for_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => "application/octet-stream",
    }
}

fn should_resize(
    bytes: usize,
    width: u32,
    height: u32,
    max_bytes: usize,
    max_width: u32,
    max_height: u32,
) -> bool {
    bytes > max_bytes || width > max_width || height > max_height
}

fn resize_image(
    data: &[u8],
    mime_type: &str,
    max_width: u32,
    max_height: u32,
    max_bytes: usize,
) -> Result<Option<(Vec<u8>, String)>, String> {
    let img = image::load_from_memory(data).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(max_width, max_height);
    let mut out = Vec::new();
    match mime_type {
        "image/png" => {
            let enc = PngEncoder::new(&mut out);
            thumb
                .write_with_encoder(enc)
                .map_err(|e| e.to_string())?;
            if out.len() > max_bytes {
                return encode_jpeg(&thumb, max_bytes);
            }
            Ok(Some((out, "image/png".into())))
        }
        "image/jpeg" => encode_jpeg(&thumb, max_bytes),
        "image/webp" => encode_jpeg(&thumb, max_bytes),
        _ => encode_jpeg(&thumb, max_bytes),
    }
}

fn encode_jpeg(img: &DynamicImage, max_bytes: usize) -> Result<Option<(Vec<u8>, String)>, String> {
    for quality in [85u8, 70, 55, 40] {
        let mut out = Vec::new();
        let enc = JpegEncoder::new_with_quality(&mut out, quality);
        img.write_with_encoder(enc)
            .map_err(|e| e.to_string())?;
        if out.len() <= max_bytes {
            return Ok(Some((out, "image/jpeg".into())));
        }
    }
    Ok(None)
}
