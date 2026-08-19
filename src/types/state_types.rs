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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedGameRecord {
    pub display_name: String,
    pub xboxgames_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ImportState {
    pub imported: Vec<ImportedGameRecord>,
}

impl ImportState {
    pub fn is_imported(&self, xboxgames_path: &str) -> bool {
        self.imported
            .iter()
            .any(|r| r.xboxgames_path == xboxgames_path)
    }

    pub fn display_name_for(&self, xboxgames_path: &str) -> Option<&str> {
        self.imported
            .iter()
            .find(|r| r.xboxgames_path == xboxgames_path)
            .map(|r| r.display_name.as_str())
    }

    pub fn upsert(&mut self, display_name: String, xboxgames_path: String) {
        match self
            .imported
            .iter_mut()
            .find(|r| r.xboxgames_path == xboxgames_path)
        {
            Some(existing) => existing.display_name = display_name,
            None => self.imported.push(ImportedGameRecord {
                display_name,
                xboxgames_path,
            }),
        }
    }
}
