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
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::logging::log;
use crate::types::shortcut_types::{ArtworkSlot, ShortcutEntry, VdfValue};

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("listing userdata dirs under {root:?}: {source}")]
    ListUserdata {
        root: String,
        #[source]
        source: std::io::Error,
    },
    #[error("checking whether steam is running: {source}")]
    CheckRunning {
        #[source]
        source: std::io::Error,
    },
    #[error("reading shortcuts file {path:?}: {source}")]
    ReadShortcuts {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing shortcuts file {path:?}: {reason}")]
    ParseShortcuts { path: String, reason: String },
    #[error("backing up shortcuts file {path:?}: {source}")]
    Backup {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("writing shortcuts file {path:?}: {source}")]
    WriteShortcuts {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("placing grid image into {grid_dir:?}: {source}")]
    PlaceGridImage {
        grid_dir: String,
        #[source]
        source: std::io::Error,
    },
    #[error("converting {source_path:?} to fit {target_w}x{target_h}: {reason}")]
    ConvertImage {
        source_path: String,
        target_w: u32,
        target_h: u32,
        reason: String,
    },
}

#[cfg_attr(test, mockall::automock)]
pub trait SteamShortcuts: Send + Sync {
    fn find_userdata_dirs(&self, steam_root: &str) -> Result<Vec<String>, AdapterError>;
    fn is_steam_running(&self) -> Result<bool, AdapterError>;
    fn read_shortcuts(&self, path: &str) -> Result<Vec<VdfValue>, AdapterError>;
    fn write_shortcuts(&self, path: &str, shortcuts: &[VdfValue]) -> Result<(), AdapterError>;
    fn place_grid_image(
        &self,
        grid_dir: &str,
        appid: i32,
        slot: ArtworkSlot,
        source_path: &str,
    ) -> Result<Option<String>, AdapterError>;
}

pub struct FsSteamShortcuts;

impl SteamShortcuts for FsSteamShortcuts {
    fn find_userdata_dirs(&self, steam_root: &str) -> Result<Vec<String>, AdapterError> {
        log(&format!(
            "steam_adapter: listing userdata under {steam_root:?}"
        ));
        let userdata_root = Path::new(steam_root).join("userdata");
        let entries =
            fs::read_dir(&userdata_root).map_err(|source| AdapterError::ListUserdata {
                root: userdata_root.to_string_lossy().to_string(),
                source,
            })?;
        let mut dirs: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .chars()
                    .all(|c| c.is_ascii_digit())
            })
            .map(|e| e.path().to_string_lossy().to_string())
            .collect();
        dirs.sort();
        Ok(dirs)
    }

    fn is_steam_running(&self) -> Result<bool, AdapterError> {
        log("steam_adapter: checking whether steam is running");
        let result = is_steam_running_impl();
        log(&format!("steam_adapter: is_steam_running -> {result:?}"));
        result
    }

    fn read_shortcuts(&self, path: &str) -> Result<Vec<VdfValue>, AdapterError> {
        log(&format!("steam_adapter: reading shortcuts {path:?}"));
        if !Path::new(path).exists() {
            log(&format!("steam_adapter: {path:?} does not exist yet"));
            return Ok(Vec::new());
        }
        let bytes = fs::read(path).map_err(|source| AdapterError::ReadShortcuts {
            path: path.to_string(),
            source,
        })?;
        let root = parse_root(&bytes).map_err(|reason| AdapterError::ParseShortcuts {
            path: path.to_string(),
            reason,
        })?;
        let shortcuts_value = root
            .into_iter()
            .find(|(k, _)| k == "shortcuts")
            .map(|(_, v)| v);
        let result = match shortcuts_value {
            Some(VdfValue::Object(children)) => children.into_iter().map(|(_, v)| v).collect(),
            _ => Vec::new(),
        };
        log(&format!(
            "steam_adapter: {path:?} has {} existing shortcut(s)",
            result.len()
        ));
        Ok(result)
    }

    fn write_shortcuts(&self, path: &str, shortcuts: &[VdfValue]) -> Result<(), AdapterError> {
        log(&format!(
            "steam_adapter: writing {} shortcut(s) to {path:?}",
            shortcuts.len()
        ));
        if Path::new(path).exists() {
            backup(path)?;
        }
        let indexed: Vec<(String, VdfValue)> = shortcuts
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v.clone()))
            .collect();
        let root = vec![("shortcuts".to_string(), VdfValue::Object(indexed))];
        let bytes = serialize_root(&root);
        fs::write(path, bytes).map_err(|source| AdapterError::WriteShortcuts {
            path: path.to_string(),
            source,
        })?;
        log(&format!("steam_adapter: wrote {path:?}"));
        Ok(())
    }

    fn place_grid_image(
        &self,
        grid_dir: &str,
        appid: i32,
        slot: ArtworkSlot,
        source_path: &str,
    ) -> Result<Option<String>, AdapterError> {
        log(&format!(
            "steam_adapter: placing grid image for slot {slot:?} from {source_path:?}"
        ));
        let Some(suffix) = slot.grid_suffix() else {
            return Ok(None);
        };
        fs::create_dir_all(grid_dir).map_err(|source| AdapterError::PlaceGridImage {
            grid_dir: grid_dir.to_string(),
            source,
        })?;
        let grid_id = compute_grid_id(appid);
        let dest = Path::new(grid_dir).join(format!("{grid_id}{suffix}.png"));
        let (target_w, target_h) = slot.ideal_size();
        convert_and_fit(source_path, &dest, target_w, target_h)?;
        log(&format!("steam_adapter: wrote grid image {dest:?}"));
        Ok(Some(dest.to_string_lossy().to_string()))
    }
}

fn convert_and_fit(
    source_path: &str,
    dest: &Path,
    target_w: u32,
    target_h: u32,
) -> Result<(), AdapterError> {
    let convert_err = |reason: String| AdapterError::ConvertImage {
        source_path: source_path.to_string(),
        target_w,
        target_h,
        reason,
    };

    let bytes = fs::read(source_path).map_err(|e| convert_err(e.to_string()))?;
    let source = image::load_from_memory(&bytes).map_err(|e| convert_err(e.to_string()))?;
    let fitted = crop_to_aspect_and_resize(&source, target_w, target_h);
    fitted
        .save_with_format(dest, image::ImageFormat::Png)
        .map_err(|e| convert_err(e.to_string()))
}

fn crop_to_aspect_and_resize(
    source: &image::DynamicImage,
    target_w: u32,
    target_h: u32,
) -> image::DynamicImage {
    use image::GenericImageView;

    let (w, h) = source.dimensions();
    let target_ratio = target_w as f64 / target_h as f64;
    let source_ratio = w as f64 / h as f64;

    let (crop_w, crop_h) = if source_ratio > target_ratio {
        (((h as f64) * target_ratio).round() as u32, h)
    } else {
        (w, ((w as f64) / target_ratio).round() as u32)
    };
    let crop_w = crop_w.clamp(1, w);
    let crop_h = crop_h.clamp(1, h);
    let x = (w - crop_w) / 2;
    let y = (h - crop_h) / 2;

    source.crop_imm(x, y, crop_w, crop_h).resize_exact(
        target_w,
        target_h,
        image::imageops::FilterType::Lanczos3,
    )
}

fn backup(path: &str) -> Result<(), AdapterError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_path = format!("{path}.bak.{timestamp}");
    fs::copy(path, &backup_path).map_err(|source| AdapterError::Backup {
        path: path.to_string(),
        source,
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_steam_running_impl() -> Result<bool, AdapterError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq steam.exe", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|source| AdapterError::CheckRunning { source })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    Ok(stdout.contains("steam.exe"))
}

#[cfg(not(target_os = "windows"))]
fn is_steam_running_impl() -> Result<bool, AdapterError> {
    Ok(false)
}

pub fn compute_grid_id(appid: i32) -> u32 {
    (appid as u32) | 0x8000_0000
}

static APPID_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn generate_unique_appid(existing: &[i32]) -> i32 {
    loop {
        let candidate = next_pseudo_random_appid();
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
}

fn next_pseudo_random_appid() -> i32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let counter = APPID_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mixed = nanos
        .wrapping_mul(2_654_435_761)
        .wrapping_add(counter.wrapping_mul(40_503))
        .wrapping_add(std::process::id());
    (mixed | 0x8000_0000) as i32
}

pub fn shortcut_entry_to_vdf(entry: &ShortcutEntry) -> VdfValue {
    VdfValue::Object(vec![
        ("appid".to_string(), VdfValue::Int(entry.appid)),
        ("AppName".to_string(), VdfValue::Str(entry.app_name.clone())),
        ("Exe".to_string(), VdfValue::Str(entry.exe.clone())),
        (
            "StartDir".to_string(),
            VdfValue::Str(entry.start_dir.clone()),
        ),
        ("icon".to_string(), VdfValue::Str(entry.icon.clone())),
        (
            "ShortcutPath".to_string(),
            VdfValue::Str(entry.shortcut_path.clone()),
        ),
        (
            "LaunchOptions".to_string(),
            VdfValue::Str(entry.launch_options.clone()),
        ),
        (
            "IsHidden".to_string(),
            VdfValue::Int(entry.is_hidden as i32),
        ),
        (
            "AllowDesktopConfig".to_string(),
            VdfValue::Int(entry.allow_desktop_config as i32),
        ),
        (
            "AllowOverlay".to_string(),
            VdfValue::Int(entry.allow_overlay as i32),
        ),
        ("OpenVR".to_string(), VdfValue::Int(entry.open_vr as i32)),
        ("Devkit".to_string(), VdfValue::Int(entry.devkit)),
        (
            "DevkitGameID".to_string(),
            VdfValue::Str(entry.devkit_game_id.clone()),
        ),
        (
            "DevkitOverrideAppID".to_string(),
            VdfValue::Int(entry.devkit_override_app_id),
        ),
        (
            "LastPlayTime".to_string(),
            VdfValue::Int(entry.last_play_time),
        ),
        (
            "FlatpakAppID".to_string(),
            VdfValue::Str(entry.flatpak_app_id.clone()),
        ),
        ("sortas".to_string(), VdfValue::Str(entry.sortas.clone())),
        ("tags".to_string(), VdfValue::Object(Vec::new())),
    ])
}

pub fn appid_of(shortcut: &VdfValue) -> Option<i32> {
    field_of(shortcut, "appid").and_then(|v| match v {
        VdfValue::Int(i) => Some(*i),
        _ => None,
    })
}

pub fn app_name_of(shortcut: &VdfValue) -> Option<String> {
    field_of(shortcut, "AppName").and_then(|v| match v {
        VdfValue::Str(s) => Some(s.clone()),
        _ => None,
    })
}

fn field_of<'a>(shortcut: &'a VdfValue, key: &str) -> Option<&'a VdfValue> {
    match shortcut {
        VdfValue::Object(children) => children.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn read_cstr(bytes: &[u8], offset: usize) -> Result<(String, usize), String> {
    let end = bytes[offset..]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| "unterminated string".to_string())?
        + offset;
    let s = String::from_utf8_lossy(&bytes[offset..end]).to_string();
    Ok((s, end + 1))
}

fn parse_entries(
    bytes: &[u8],
    mut offset: usize,
    stop_at_end_marker: bool,
) -> Result<(Vec<(String, VdfValue)>, usize), String> {
    let mut entries = Vec::new();
    loop {
        if offset >= bytes.len() {
            if stop_at_end_marker {
                return Err("unexpected end of file inside object".to_string());
            }
            return Ok((entries, offset));
        }
        let type_byte = bytes[offset];
        offset += 1;
        if type_byte == 0x08 {
            if stop_at_end_marker {
                return Ok((entries, offset));
            }
            return Err("unexpected end-of-object marker at root".to_string());
        }
        let (key, next_offset) = read_cstr(bytes, offset)?;
        offset = next_offset;
        match type_byte {
            0x00 => {
                let (children, next_offset) = parse_entries(bytes, offset, true)?;
                offset = next_offset;
                entries.push((key, VdfValue::Object(children)));
            }
            0x01 => {
                let (value, next_offset) = read_cstr(bytes, offset)?;
                offset = next_offset;
                entries.push((key, VdfValue::Str(value)));
            }
            0x02 => {
                if offset + 4 > bytes.len() {
                    return Err("truncated int32 value".to_string());
                }
                let value = i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                offset += 4;
                entries.push((key, VdfValue::Int(value)));
            }
            other => return Err(format!("unknown vdf type byte {other:#04x}")),
        }
    }
}

fn parse_root(bytes: &[u8]) -> Result<Vec<(String, VdfValue)>, String> {
    let (entries, offset) = parse_entries(bytes, 0, true)?;
    if offset != bytes.len() {
        return Err(format!(
            "trailing bytes after root object: consumed {offset} of {}",
            bytes.len()
        ));
    }
    Ok(entries)
}

fn write_cstr(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

fn serialize_entries(entries: &[(String, VdfValue)], out: &mut Vec<u8>, write_end_marker: bool) {
    for (key, value) in entries {
        match value {
            VdfValue::Object(children) => {
                out.push(0x00);
                write_cstr(out, key);
                serialize_entries(children, out, true);
            }
            VdfValue::Str(s) => {
                out.push(0x01);
                write_cstr(out, key);
                write_cstr(out, s);
            }
            VdfValue::Int(i) => {
                out.push(0x02);
                write_cstr(out, key);
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
    }
    if write_end_marker {
        out.push(0x08);
    }
}

fn serialize_root(entries: &[(String, VdfValue)]) -> Vec<u8> {
    let mut out = Vec::new();
    serialize_entries(entries, &mut out, true);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32 as TestCounter, Ordering as TestOrdering};

    static TMP_COUNTER: TestCounter = TestCounter::new(0);

    fn temp_path(name: &str) -> String {
        let id = TMP_COUNTER.fetch_add(1, TestOrdering::SeqCst);
        std::env::temp_dir()
            .join(format!(
                "gamesync-steam-test-{}-{}-{}",
                std::process::id(),
                id,
                name
            ))
            .to_string_lossy()
            .to_string()
    }

    fn sample_entry(appid: i32, name: &str) -> ShortcutEntry {
        ShortcutEntry {
            appid,
            app_name: name.to_string(),
            exe: format!("\"C:\\XboxGames\\{name}\\Content\\gamelaunchhelper.exe\""),
            start_dir: format!("C:\\XboxGames\\{name}\\Content\\"),
            icon: format!("C:\\XboxGames\\{name}\\Content\\StoreLogo.png"),
            shortcut_path: String::new(),
            launch_options: String::new(),
            is_hidden: false,
            allow_desktop_config: true,
            allow_overlay: true,
            open_vr: false,
            devkit: 0,
            devkit_game_id: String::new(),
            devkit_override_app_id: 0,
            last_play_time: 0,
            flatpak_app_id: String::new(),
            sortas: String::new(),
        }
    }

    #[test]
    fn root_object_ends_with_its_own_closing_marker() {
        let mut expected = vec![0x02];
        expected.extend_from_slice(b"shortcuts\0");
        expected.extend_from_slice(&7i32.to_le_bytes());
        expected.push(0x08);

        let root = vec![("shortcuts".to_string(), VdfValue::Int(7))];
        let serialized = serialize_root(&root);
        assert_eq!(serialized, expected);

        let parsed = parse_root(&serialized).unwrap();
        assert_eq!(parsed, root);
    }

    #[test]
    fn parse_root_rejects_trailing_garbage_after_the_closing_marker() {
        let mut bytes = serialize_root(&[("shortcuts".to_string(), VdfValue::Int(1))]);
        bytes.push(0xFF);
        assert!(parse_root(&bytes).is_err());
    }

    #[test]
    fn round_trips_a_single_shortcut_through_the_codec() {
        let entry = sample_entry(-2090050060, "Forza Horizon 6");
        let vdf = shortcut_entry_to_vdf(&entry);
        let root = vec![(
            "shortcuts".to_string(),
            VdfValue::Object(vec![("0".to_string(), vdf.clone())]),
        )];
        let bytes = serialize_root(&root);
        let parsed = parse_root(&bytes).unwrap();

        assert_eq!(parsed, root);
        assert_eq!(appid_of(&vdf), Some(-2090050060));
        assert_eq!(app_name_of(&vdf), Some("Forza Horizon 6".to_string()));
    }

    #[test]
    fn write_then_read_preserves_untouched_entries() {
        let path = temp_path("shortcuts.vdf");
        let steam = FsSteamShortcuts;

        let existing = shortcut_entry_to_vdf(&sample_entry(-2090050060, "Forza Horizon 6"));
        steam
            .write_shortcuts(&path, std::slice::from_ref(&existing))
            .unwrap();

        let new_entry = shortcut_entry_to_vdf(&sample_entry(-448463598, "Persona 5 Royal"));
        let mut all = steam.read_shortcuts(&path).unwrap();
        all.push(new_entry.clone());
        steam.write_shortcuts(&path, &all).unwrap();

        let round_tripped = steam.read_shortcuts(&path).unwrap();
        assert_eq!(round_tripped.len(), 2);
        assert_eq!(round_tripped[0], existing);
        assert_eq!(round_tripped[1], new_entry);

        let _ = fs::remove_file(&path);
        for entry in fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(
                &Path::new(&path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            ) && name.contains(".bak.")
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    #[test]
    fn write_backs_up_existing_file_before_overwrite() {
        let path = temp_path("shortcuts2.vdf");
        let steam = FsSteamShortcuts;
        steam
            .write_shortcuts(&path, &[shortcut_entry_to_vdf(&sample_entry(-1, "A"))])
            .unwrap();
        steam
            .write_shortcuts(&path, &[shortcut_entry_to_vdf(&sample_entry(-2, "B"))])
            .unwrap();

        let has_backup = fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let base = Path::new(&path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                name.starts_with(&base) && name.contains(".bak.")
            });
        assert!(has_backup);

        let _ = fs::remove_file(&path);
        for entry in fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = Path::new(&path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            if name.starts_with(&base) && name.contains(".bak.") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    #[test]
    fn read_returns_empty_when_file_missing() {
        let steam = FsSteamShortcuts;
        let result = steam.read_shortcuts(&temp_path("missing.vdf")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn generate_unique_appid_avoids_collisions() {
        let existing = vec![1i32, 2, 3];
        let first = generate_unique_appid(&existing);
        assert!(!existing.contains(&first));
        assert_ne!(first & 0x8000_0000u32 as i32, 0);
    }

    #[test]
    fn compute_grid_id_matches_sgdboop_formula() {
        assert_eq!(compute_grid_id(-1), 0xFFFF_FFFF);
        assert_eq!(compute_grid_id(-2090050060), 2204917236);
    }

    #[test]
    fn place_grid_image_copies_and_names_by_slot() {
        let source = temp_path("source.png");
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::new(1920, 1080));
        img.save_with_format(&source, image::ImageFormat::Png)
            .unwrap();
        let grid_dir = temp_path("grid-dir");

        let steam = FsSteamShortcuts;
        let dest = steam
            .place_grid_image(&grid_dir, -1, ArtworkSlot::Background, &source)
            .unwrap()
            .unwrap();

        let grid_id = compute_grid_id(-1);
        assert!(dest.ends_with(&format!("{grid_id}_hero.png")));
        assert!(Path::new(&dest).exists());

        let fitted = image::open(&dest).unwrap();
        let (ideal_w, ideal_h) = ArtworkSlot::Background.ideal_size();
        assert_eq!((fitted.width(), fitted.height()), (ideal_w, ideal_h));

        let _ = fs::remove_file(&source);
        let _ = fs::remove_dir_all(&grid_dir);
    }

    #[test]
    fn place_grid_image_returns_none_for_icon_slot() {
        let steam = FsSteamShortcuts;
        let result = steam
            .place_grid_image("/tmp/whatever", -1, ArtworkSlot::Icon, "/tmp/source.png")
            .unwrap();
        assert!(result.is_none());
    }
}
