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

use thiserror::Error;

use crate::types::state_types::ImportState;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("reading state file {path:?}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing state file {path:?}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("writing state file {path:?}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("serializing state for {path:?}: {source}")]
    Serialize {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

#[cfg_attr(test, mockall::automock)]
pub trait ImportStateStore: Send + Sync {
    fn load(&self, path: &str) -> Result<ImportState, AdapterError>;
    fn save(&self, path: &str, state: &ImportState) -> Result<(), AdapterError>;
}

pub struct FsImportStateStore;

impl ImportStateStore for FsImportStateStore {
    fn load(&self, path: &str) -> Result<ImportState, AdapterError> {
        if !std::path::Path::new(path).exists() {
            return Ok(ImportState::default());
        }
        let contents = fs::read_to_string(path).map_err(|source| AdapterError::Read {
            path: path.to_string(),
            source,
        })?;
        serde_json::from_str(&contents).map_err(|source| AdapterError::Parse {
            path: path.to_string(),
            source,
        })
    }

    fn save(&self, path: &str, state: &ImportState) -> Result<(), AdapterError> {
        let contents =
            serde_json::to_string_pretty(state).map_err(|source| AdapterError::Serialize {
                path: path.to_string(),
                source,
            })?;
        fs::write(path, contents).map_err(|source| AdapterError::Write {
            path: path.to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::state_types::ImportedGameRecord;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_path() -> String {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir()
            .join(format!(
                "gamesync-state-test-{}-{}.json",
                std::process::id(),
                id
            ))
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let store = FsImportStateStore;
        let state = store.load(&temp_path()).unwrap();
        assert!(state.imported.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let path = temp_path();
        let store = FsImportStateStore;
        let mut state = ImportState::default();
        state.imported.push(ImportedGameRecord {
            display_name: "Forza Horizon 6".to_string(),
            xboxgames_path: "C:\\XboxGames\\Forza Horizon 6".to_string(),
        });

        store.save(&path, &state).unwrap();
        let loaded = store.load(&path).unwrap();

        assert_eq!(loaded, state);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_errors_on_malformed_json() {
        let path = temp_path();
        fs::write(&path, "not json").unwrap();
        let store = FsImportStateStore;
        let result = store.load(&path);
        assert!(result.is_err());
        let _ = fs::remove_file(&path);
    }
}
