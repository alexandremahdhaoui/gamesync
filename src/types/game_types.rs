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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFacts {
    pub has_application_element: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub name: String,
    pub xboxgames_path: String,
    pub content_dir: String,
    pub exe_path: String,
    pub has_application_element: bool,
}
