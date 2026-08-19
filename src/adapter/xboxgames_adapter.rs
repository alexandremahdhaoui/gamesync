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
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::logging::log;
use crate::types::game_types::Game;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("listing games under {root:?}: {source}")]
    ListRoot {
        root: String,
        #[source]
        source: std::io::Error,
    },
}

#[cfg_attr(test, mockall::automock)]
pub trait XboxGamesRepository: Send + Sync {
    fn list_games(&self, root: &str) -> Result<Vec<Game>, AdapterError>;
}

pub struct FsXboxGamesRepository;

impl XboxGamesRepository for FsXboxGamesRepository {
    fn list_games(&self, root: &str) -> Result<Vec<Game>, AdapterError> {
        log(&format!("xboxgames_adapter: reading dir {root:?}"));
        let entries = fs::read_dir(root).map_err(|source| AdapterError::ListRoot {
            root: root.to_string(),
            source,
        })?;
        log(&format!("xboxgames_adapter: reading dir {root:?} done"));

        let mut games = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            log(&format!("xboxgames_adapter: checking folder {path:?}"));
            if let Some(game) = game_from_folder(&path) {
                games.push(game);
            }
        }
        games.sort_by(|a, b| a.name.cmp(&b.name));
        log(&format!(
            "xboxgames_adapter: {root:?} yielded {} folder(s) with a manifest",
            games.len()
        ));
        Ok(games)
    }
}

fn game_from_folder(folder: &Path) -> Option<Game> {
    let content_dir = folder.join("Content");
    let manifest_path = content_dir.join("appxmanifest.xml");
    log(&format!(
        "xboxgames_adapter: reading manifest {manifest_path:?}"
    ));
    let manifest = fs::read_to_string(&manifest_path).ok()?;
    log(&format!(
        "xboxgames_adapter: manifest read, looking for gamelaunchhelper.exe under {content_dir:?}"
    ));
    let exe_path = find_case_insensitive(&content_dir, "gamelaunchhelper.exe")?;
    let name = folder.file_name()?.to_string_lossy().to_string();
    log(&format!(
        "xboxgames_adapter: {name:?} resolved, exe={exe_path:?}"
    ));
    Some(Game {
        name,
        xboxgames_path: folder.to_string_lossy().to_string(),
        content_dir: content_dir.to_string_lossy().to_string(),
        exe_path: exe_path.to_string_lossy().to_string(),
        has_application_element: manifest_has_application_element(&manifest),
    })
}

fn manifest_has_application_element(manifest: &str) -> bool {
    manifest.contains("<Application") && manifest.contains("Executable=")
}

fn find_case_insensitive(dir: &Path, target_lower: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if file_name.to_string_lossy().to_lowercase() == target_lower {
            return Some(entry.path());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("gamesync-test-{}-{}", std::process::id(), id));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_manifest(content_dir: &Path, has_application: bool) {
        fs::create_dir_all(content_dir).unwrap();
        let manifest = if has_application {
            r#"<Package><Applications><Application Id="X" Executable="GameLaunchHelper.exe"></Application></Applications></Package>"#
        } else {
            r#"<Package><Properties><DisplayName>Stub</DisplayName></Properties></Package>"#
        };
        fs::write(content_dir.join("appxmanifest.xml"), manifest).unwrap();
    }

    #[test]
    fn finds_real_game_with_application_element() {
        let tmp = TempDir::new();
        let game_dir = tmp.path.join("Forza Horizon 6");
        let content_dir = game_dir.join("Content");
        write_manifest(&content_dir, true);
        fs::write(content_dir.join("gamelaunchhelper.exe"), b"stub").unwrap();

        let repo = FsXboxGamesRepository;
        let games = repo.list_games(tmp.path.to_str().unwrap()).unwrap();

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Forza Horizon 6");
        assert!(games[0].exe_path.ends_with("gamelaunchhelper.exe"));
    }

    #[test]
    fn surfaces_has_application_element_fact_without_deciding() {
        let tmp = TempDir::new();
        let game_dir = tmp.path.join("BO7 DLC01 Game Stub 01");
        let content_dir = game_dir.join("Content");
        write_manifest(&content_dir, false);
        fs::write(content_dir.join("gamelaunchhelper.exe"), b"stub").unwrap();

        let repo = FsXboxGamesRepository;
        let games = repo.list_games(tmp.path.to_str().unwrap()).unwrap();

        assert_eq!(games.len(), 1);
        assert!(!games[0].has_application_element);
    }

    #[test]
    fn drops_folder_without_manifest() {
        let tmp = TempDir::new();
        fs::create_dir_all(tmp.path.join("GameSave")).unwrap();

        let repo = FsXboxGamesRepository;
        let games = repo.list_games(tmp.path.to_str().unwrap()).unwrap();

        assert!(games.is_empty());
    }

    #[test]
    fn errors_on_missing_root() {
        let repo = FsXboxGamesRepository;
        let result = repo.list_games("/nonexistent/gamesync-root");
        assert!(result.is_err());
    }
}
