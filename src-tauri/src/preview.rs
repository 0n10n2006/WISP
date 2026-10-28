//! Previews of indexed files for the bar: Windows' own thumbnail for
//! pictures, PDFs, documents and videos; the first lines of text files; a
//! folder's first entries. Only for files the index knows (by id).

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use wisp_core::files::{self, FileHit, FileKind};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    /// "image" | "text" | "folder" | "none"
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<String>>,
}

const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "log", "json", "xml", "yaml", "yml", "ini", "toml",
    "cfg", "conf", "rs", "ts", "tsx", "jsx", "css", "scss", "html", "htm", "c", "h", "cpp", "hpp",
    "cs", "java", "kt", "go", "rb", "php", "swift", "sql", "lua", "tex", "srt",
];
const MAX_TEXT_BYTES: u64 = 256 * 1024;
const TEXT_LINES: usize = 60;
const FOLDER_ENTRIES: usize = 20;

/// The first lines of a text file (lossy UTF-8, control characters dropped).
pub fn text_preview(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_TEXT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<String> = text
        .lines()
        .take(TEXT_LINES)
        .map(|l| {
            l.chars()
                .filter(|c| !c.is_control() || *c == '\t')
                .take(200)
                .collect()
        })
        .collect();
    Some(lines.join("\n"))
}

/// A folder's first entries (folders first), with the index's skip rules.
pub fn folder_preview(path: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut items: Vec<(bool, String)> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ft = e.file_type().ok()?;
            if ft.is_symlink() {
                return None;
            }
            let dir = ft.is_dir();
            if (dir && files::skip_dir(&name)) || (!dir && files::skip_file(&name)) {
                return None;
            }
            let meta = e.metadata().ok()?;
            (!files::is_hidden(&meta)).then_some((!dir, name))
        })
        .take(500)
        .collect();
    items.sort_by_key(|(file, name)| (*file, name.to_lowercase()));
    items
        .into_iter()
        .take(FOLDER_ENTRIES)
        .map(|(file, name)| if file { name } else { format!("{name}/") })
        .collect()
}

#[cfg(windows)]
fn shell_thumbnail(path: &Path) -> Option<String> {
    use base64::Engine;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_RESIZETOFIT,
    };
    unsafe {
        // Thumbnail handlers expect a single-threaded apartment.
        let inited = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| {
            let factory: IShellItemImageFactory =
                SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).ok()?;
            let bmp = factory
                .GetImage(
                    SIZE { cx: 320, cy: 320 },
                    SIIGBF_RESIZETOFIT | SIIGBF_BIGGERSIZEOK,
                )
                .ok()?;
            let mut bm = BITMAP::default();
            let got = GetObjectW(
                bmp.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut core::ffi::c_void),
            );
            let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
            let ok_size = got != 0 && w > 0 && h > 0 && w <= 1024 && h <= 1024;
            let mut buf = vec![0u8; (w.max(0) * h.max(0) * 4) as usize];
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let dc = CreateCompatibleDC(None);
            let lines = if ok_size {
                GetDIBits(
                    dc,
                    bmp,
                    0,
                    h as u32,
                    Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            } else {
                0
            };
            let _ = DeleteDC(dc);
            let _ = DeleteObject(bmp.into());
            if lines == 0 {
                return None;
            }
            // BGRA → RGBA; bitmaps without alpha come back fully transparent.
            let opaque = buf.chunks(4).all(|p| p[3] == 0);
            for p in buf.chunks_mut(4) {
                p.swap(0, 2);
                if opaque {
                    p[3] = 255;
                }
            }
            let img = image::RgbaImage::from_raw(w as u32, h as u32, buf)?;
            let mut out = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut out, image::ImageFormat::Png)
                .ok()?;
            Some(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(out.into_inner())
            ))
        })();
        if inited {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
fn shell_thumbnail(path: &Path) -> Option<String> {
    let lower = path.to_string_lossy().to_lowercase();
    let image = [".png", ".jpg", ".jpeg", ".bmp"]
        .iter()
        .any(|e| lower.ends_with(e));
    image
        .then(|| crate::wallpaper::thumbnail(&path.to_string_lossy()).ok())
        .flatten()
}

/// The preview for `hit` at `path` (already checked by the caller).
pub fn build(hit: &FileHit, path: &Path) -> FilePreview {
    if hit.kind == FileKind::Folder {
        return FilePreview {
            kind: "folder",
            entries: Some(folder_preview(path)),
            ..Default::default()
        };
    }
    let ext = files::extension(&hit.name);
    if TEXT_EXTS.contains(&ext.as_str()) {
        if let Some(text) = text_preview(path) {
            return FilePreview {
                kind: "text",
                text: Some(text),
                ..Default::default()
            };
        }
    }
    match shell_thumbnail(path) {
        Some(image) => FilePreview {
            kind: "image",
            image: Some(image),
            ..Default::default()
        },
        None => FilePreview {
            kind: "none",
            ..Default::default()
        },
    }
}

/// [`build`], remembered per file and modification time.
pub fn cached(hit: &FileHit, path: &Path) -> FilePreview {
    static CACHE: OnceLock<Mutex<HashMap<(i64, i64), FilePreview>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (hit.id, hit.modified);
    if let Some(p) = cache.lock().expect("preview cache").get(&key) {
        return p.clone();
    }
    let p = build(hit, path);
    let mut map = cache.lock().expect("preview cache");
    if map.len() >= 100 {
        map.clear();
    }
    map.insert(key, p.clone());
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str, kind: FileKind) -> FileHit {
        FileHit {
            id: 1,
            name: name.into(),
            folder: "Documents".into(),
            kind,
            size: 1,
            modified: 0,
            path: String::new(),
            root: String::new(),
            opened: 0,
            last_opened: None,
        }
    }

    #[test]
    fn previews_text_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("notes.txt");
        let long: String = (1..=100).map(|i| format!("line {i}\u{7}\n")).collect();
        std::fs::write(&notes, long).unwrap();
        let p = build(&hit("notes.txt", FileKind::Document), &notes);
        assert_eq!(p.kind, "text");
        let text = p.text.unwrap();
        assert_eq!(text.lines().count(), 60);
        assert!(text.starts_with("line 1\n") && !text.contains('\u{7}'));

        let big = dir.path().join("big.txt");
        std::fs::write(&big, "x".repeat(300 * 1024)).unwrap();
        assert_eq!(
            build(&hit("big.txt", FileKind::Document), &big).kind,
            "none"
        );

        std::fs::create_dir_all(dir.path().join("Sub")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("setup.exe"), "x").unwrap();
        std::fs::write(dir.path().join(".hidden"), "x").unwrap();
        let f = build(&hit("Documents", FileKind::Folder), dir.path());
        assert_eq!(f.kind, "folder");
        assert_eq!(f.entries.unwrap(), vec!["Sub/", "big.txt", "notes.txt"]);
        let json = serde_json::to_value(FilePreview {
            kind: "none",
            ..Default::default()
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({"kind": "none"}));
    }
}
