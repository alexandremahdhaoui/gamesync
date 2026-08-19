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
pub enum VdfValue {
    Object(Vec<(String, VdfValue)>),
    Str(String),
    Int(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEntry {
    pub appid: i32,
    pub app_name: String,
    pub exe: String,
    pub start_dir: String,
    pub icon: String,
    pub shortcut_path: String,
    pub launch_options: String,
    pub is_hidden: bool,
    pub allow_desktop_config: bool,
    pub allow_overlay: bool,
    pub open_vr: bool,
    pub devkit: i32,
    pub devkit_game_id: String,
    pub devkit_override_app_id: i32,
    pub last_play_time: i32,
    pub flatpak_app_id: String,
    pub sortas: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtworkSlot {
    Icon,
    Cover,
    WideCover,
    Background,
    Logo,
}

impl ArtworkSlot {
    pub const ALL: [ArtworkSlot; 5] = [
        ArtworkSlot::Icon,
        ArtworkSlot::Cover,
        ArtworkSlot::WideCover,
        ArtworkSlot::Background,
        ArtworkSlot::Logo,
    ];

    pub fn ideal_size(self) -> (u32, u32) {
        match self {
            ArtworkSlot::Icon => (256, 256),
            ArtworkSlot::Cover => (600, 900),
            ArtworkSlot::WideCover => (920, 430),
            ArtworkSlot::Background => (3840, 1240),
            ArtworkSlot::Logo => (1280, 720),
        }
    }

    pub fn grid_suffix(self) -> Option<&'static str> {
        match self {
            ArtworkSlot::Icon => None,
            ArtworkSlot::Cover => Some("p"),
            ArtworkSlot::WideCover => Some(""),
            ArtworkSlot::Background => Some("_hero"),
            ArtworkSlot::Logo => Some("_logo"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ArtworkSlot::Icon => "shortcut icon",
            ArtworkSlot::Cover => "cover",
            ArtworkSlot::WideCover => "wide cover",
            ArtworkSlot::Background => "background",
            ArtworkSlot::Logo => "logo",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtworkCandidate {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoredCandidate {
    pub candidate: ArtworkCandidate,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotCandidates {
    pub slot: ArtworkSlot,
    pub ranked: Vec<ScoredCandidate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameProposal {
    pub game: crate::types::game_types::Game,
    pub slots: Vec<SlotCandidates>,
    pub already_imported: bool,
    pub suggested_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSelection {
    pub game: crate::types::game_types::Game,
    pub display_name: String,
    pub previous_app_name: Option<String>,
    pub chosen_paths: Vec<(ArtworkSlot, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltGame {
    pub entry: ShortcutEntry,
    pub artwork: Vec<(ArtworkSlot, Option<String>)>,
    pub previous_app_name: Option<String>,
}
