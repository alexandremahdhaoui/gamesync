// Copyright 2026 Alexandre Mahdhaoui
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::logging::log;
use crate::types::shortcut_types::ArtworkCandidate;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("walking artwork candidates under {dir:?}: {source}")]
    Walk {
        dir: String,
        #[source]
        source: std::io::Error,
    },
}

const MAX_DEPTH: u32 = 4;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CANDIDATES: usize = 2000;
const MAX_WALK_DURATION: Duration = Duration::from_secs(4);

const EXCLUDED_DIR_NAMES: [&str; 9] = [
    "media", "physics", "shaders", "audio", "sound", "fonts", "scripts", "cache", "logs",
];

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff", "ico"];

#[cfg_attr(test, mockall::automock)]
pub trait ArtworkFinder: Send + Sync {
    fn find_candidates(&self, content_dir: &str) -> Result<Vec<ArtworkCandidate>, AdapterError>;
}

pub struct FsArtworkFinder;

impl ArtworkFinder for FsArtworkFinder {
    fn find_candidates(&self, content_dir: &str) -> Result<Vec<ArtworkCandidate>, AdapterError> {
        log(&format!("artwork_adapter: walking {content_dir:?}"));
        let deadline = Instant::now() + MAX_WALK_DURATION;
        let mut candidates = Vec::new();
        walk(Path::new(content_dir), 0, deadline, &mut candidates).map_err(|source| {
            AdapterError::Walk {
                dir: content_dir.to_string(),
                source,
            }
        })?;
        log(&format!(
            "artwork_adapter: {content_dir:?} yielded {} candidate(s)",
            candidates.len()
        ));
        Ok(candidates)
    }
}

fn walk(
    dir: &Path,
    depth: u32,
    deadline: Instant,
    out: &mut Vec<ArtworkCandidate>,
) -> std::io::Result<()> {
    if depth > MAX_DEPTH || out.len() >= MAX_CANDIDATES {
        return Ok(());
    }
    if deadline_passed(dir, deadline) {
        return Ok(());
    }
    log(&format!(
        "artwork_adapter: reading dir {dir:?} (depth {depth})"
    ));
    let entries: Vec<_> = fs::read_dir(dir)?.flatten().collect();

    for entry in &entries {
        if out.len() >= MAX_CANDIDATES || deadline_passed(dir, deadline) {
            return Ok(());
        }
        let path = entry.path();
        if path.is_dir() || !has_image_extension(&path) {
            continue;
        }
        add_candidate_if_image(&path, entry, out);
    }

    for entry in &entries {
        if out.len() >= MAX_CANDIDATES || deadline_passed(dir, deadline) {
            return Ok(());
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name_lower = entry.file_name().to_string_lossy().to_lowercase();
        if EXCLUDED_DIR_NAMES.contains(&name_lower.as_str()) {
            continue;
        }
        walk(&path, depth + 1, deadline, out)?;
    }
    Ok(())
}

fn deadline_passed(dir: &Path, deadline: Instant) -> bool {
    if Instant::now() < deadline {
        return false;
    }
    log(&format!(
        "artwork_adapter: hit {MAX_WALK_DURATION:?} time budget at {dir:?}, stopping"
    ));
    true
}

fn add_candidate_if_image(path: &Path, entry: &fs::DirEntry, out: &mut Vec<ArtworkCandidate>) {
    let Ok(metadata) = entry.metadata() else {
        return;
    };
    if metadata.len() > MAX_FILE_BYTES {
        log(&format!(
            "artwork_adapter: skipping {path:?}, {} bytes exceeds MAX_FILE_BYTES",
            metadata.len()
        ));
        return;
    }
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        return;
    };
    out.push(ArtworkCandidate {
        path: path.to_string_lossy().to_string(),
        width: decoded.width(),
        height: decoded.height(),
        has_alpha: decoded.color().has_alpha(),
    });
}

fn has_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "gamesync-artwork-test-{}-{}",
                std::process::id(),
                id
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn png_bytes(width: u32, height: u32, with_alpha: bool) -> Vec<u8> {
        let img = if with_alpha {
            image::DynamicImage::ImageRgba8(image::RgbaImage::new(width, height))
        } else {
            image::DynamicImage::ImageRgb8(image::RgbImage::new(width, height))
        };
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        buf
    }

    #[test]
    fn finds_png_candidates_with_dimensions() {
        let tmp = TempDir::new();
        fs::write(tmp.path.join("StoreLogo.png"), png_bytes(100, 100, false)).unwrap();

        let finder = FsArtworkFinder;
        let candidates = finder.find_candidates(tmp.path.to_str().unwrap()).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].width, 100);
        assert_eq!(candidates[0].height, 100);
    }

    #[test]
    fn excludes_media_subfolder() {
        let tmp = TempDir::new();
        let junk_dir = tmp.path.join("media").join("physics").join("suspension");
        fs::create_dir_all(&junk_dir).unwrap();
        fs::write(junk_dir.join("5link_AWD.jpg"), b"not a real jpeg").unwrap();
        fs::write(
            tmp.path.join("SplashScreen.png"),
            png_bytes(1920, 1080, false),
        )
        .unwrap();

        let finder = FsArtworkFinder;
        let candidates = finder.find_candidates(tmp.path.to_str().unwrap()).unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].width, 1920);
    }

    #[test]
    fn skips_non_image_files() {
        let tmp = TempDir::new();
        fs::write(tmp.path.join("appxmanifest.xml"), b"<Package/>").unwrap();

        let finder = FsArtworkFinder;
        let candidates = finder.find_candidates(tmp.path.to_str().unwrap()).unwrap();

        assert!(candidates.is_empty());
    }

    #[test]
    fn finds_candidates_nested_within_depth_cap() {
        let tmp = TempDir::new();
        let nested = tmp.path.join("sys_resource");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("WideLogo.png"), png_bytes(480, 480, true)).unwrap();

        let finder = FsArtworkFinder;
        let candidates = finder.find_candidates(tmp.path.to_str().unwrap()).unwrap();

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].has_alpha);
    }

    #[test]
    fn errors_on_missing_dir() {
        let finder = FsArtworkFinder;
        let result = finder.find_candidates("/nonexistent/gamesync-content");
        assert!(result.is_err());
    }
}
