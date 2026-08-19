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

use std::sync::Arc;

use thiserror::Error;

use crate::logging::log;

use crate::adapter::artwork_adapter::{self, ArtworkFinder};
use crate::adapter::state_adapter::{self, ImportStateStore};
use crate::adapter::steam_adapter::{self, SteamShortcuts};
use crate::adapter::xboxgames_adapter::{self, XboxGamesRepository};
use crate::types::game_types::Game;
use crate::types::shortcut_types::{
    ArtworkCandidate, ArtworkSlot, BuiltGame, GameProposal, GameSelection, ScoredCandidate,
    ShortcutEntry, SlotCandidates,
};
use crate::types::state_types::ImportState;

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("scanning xboxgames root: {0}")]
    ScanGames(#[source] xboxgames_adapter::AdapterError),
    #[error("finding artwork for {game:?}: {source}")]
    FindArtwork {
        game: String,
        #[source]
        source: artwork_adapter::AdapterError,
    },
    #[error("loading import state: {0}")]
    LoadState(#[source] state_adapter::AdapterError),
    #[error("saving import state: {0}")]
    SaveState(#[source] state_adapter::AdapterError),
    #[error("reading existing steam shortcuts: {0}")]
    ReadShortcuts(#[source] steam_adapter::AdapterError),
}

const ASPECT_WEIGHT: f64 = 1000.0;
const RESOLUTION_WEIGHT: f64 = 10.0;
const ALPHA_BONUS: f64 = 1.0;
const WEAK_MATCH_THRESHOLD: f64 = ASPECT_WEIGHT * 0.5;

pub struct DefaultImportController {
    pub xboxgames: Arc<dyn XboxGamesRepository>,
    pub artwork: Arc<dyn ArtworkFinder>,
    pub steam: Arc<dyn SteamShortcuts>,
    pub state: Arc<dyn ImportStateStore>,
}

impl DefaultImportController {
    pub fn scan(
        &self,
        xboxgames_root: &str,
        shortcuts_vdf_path: &str,
        state_path: &str,
        include_already_imported: bool,
    ) -> Result<Vec<GameProposal>, ControllerError> {
        log(&format!(
            "import_controller: scan starting, xboxgames_root={xboxgames_root:?}"
        ));
        let all_games = self
            .xboxgames
            .list_games(xboxgames_root)
            .map_err(ControllerError::ScanGames)?;
        let real_games: Vec<Game> = all_games
            .into_iter()
            .filter(|g| g.has_application_element)
            .collect();
        log(&format!(
            "import_controller: {} real game(s) found",
            real_games.len()
        ));

        let mut state = self
            .state
            .load(state_path)
            .map_err(ControllerError::LoadState)?;

        let existing_shortcuts = self
            .steam
            .read_shortcuts(shortcuts_vdf_path)
            .map_err(ControllerError::ReadShortcuts)?;
        let seeded = seed_state_from_existing(&existing_shortcuts, &real_games, &mut state);
        if seeded {
            self.state
                .save(state_path, &state)
                .map_err(ControllerError::SaveState)?;
        }

        let mut proposals = Vec::new();
        for game in real_games {
            let already_imported = state.is_imported(&game.xboxgames_path);
            if already_imported && !include_already_imported {
                continue;
            }
            let suggested_name = state
                .display_name_for(&game.xboxgames_path)
                .unwrap_or(&game.name)
                .to_string();
            log(&format!(
                "import_controller: scoring artwork for {:?}",
                game.name
            ));
            let candidates = self
                .artwork
                .find_candidates(&game.content_dir)
                .map_err(|source| ControllerError::FindArtwork {
                    game: game.name.clone(),
                    source,
                })?;
            let slots = ArtworkSlot::ALL
                .iter()
                .map(|&slot| SlotCandidates {
                    slot,
                    ranked: rank_candidates(&candidates, slot),
                })
                .collect();
            proposals.push(GameProposal {
                game,
                slots,
                already_imported,
                suggested_name,
            });
        }
        log(&format!(
            "import_controller: scan finished, {} proposal(s)",
            proposals.len()
        ));
        Ok(proposals)
    }

    pub fn build_entries(
        &self,
        selections: &[GameSelection],
        existing_appid_by_app_name: &std::collections::HashMap<String, i32>,
    ) -> Vec<BuiltGame> {
        let mut used_appids: Vec<i32> = existing_appid_by_app_name.values().copied().collect();
        selections
            .iter()
            .map(|selection| {
                let appid = match selection
                    .previous_app_name
                    .as_ref()
                    .and_then(|name| existing_appid_by_app_name.get(name))
                {
                    Some(&reused) => reused,
                    None => {
                        let generated = steam_adapter::generate_unique_appid(&used_appids);
                        used_appids.push(generated);
                        generated
                    }
                };
                let icon = selection
                    .chosen_paths
                    .iter()
                    .find(|(slot, _)| *slot == ArtworkSlot::Icon)
                    .and_then(|(_, path)| path.clone())
                    .unwrap_or_default();
                let entry = ShortcutEntry {
                    appid,
                    app_name: selection.display_name.clone(),
                    exe: format!("\"{}\"", selection.game.exe_path),
                    start_dir: format!("{}\\", selection.game.content_dir),
                    icon,
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
                };
                BuiltGame {
                    entry,
                    artwork: selection.chosen_paths.clone(),
                    previous_app_name: selection.previous_app_name.clone(),
                }
            })
            .collect()
    }

    pub fn mark_imported(
        &self,
        state_path: &str,
        imported: &[(Game, String)],
    ) -> Result<(), ControllerError> {
        let mut state = self
            .state
            .load(state_path)
            .map_err(ControllerError::LoadState)?;
        for (game, display_name) in imported {
            state.upsert(display_name.clone(), game.xboxgames_path.clone());
        }
        self.state
            .save(state_path, &state)
            .map_err(ControllerError::SaveState)
    }
}

fn seed_state_from_existing(
    existing_shortcuts: &[crate::types::shortcut_types::VdfValue],
    games: &[Game],
    state: &mut ImportState,
) -> bool {
    let mut changed = false;
    for shortcut in existing_shortcuts {
        let Some(app_name) = steam_adapter::app_name_of(shortcut) else {
            continue;
        };
        let Some(game) = games.iter().find(|g| g.name == app_name) else {
            continue;
        };
        if !state.is_imported(&game.xboxgames_path) {
            state.upsert(app_name, game.xboxgames_path.clone());
            changed = true;
        }
    }
    changed
}

fn rank_candidates(candidates: &[ArtworkCandidate], slot: ArtworkSlot) -> Vec<ScoredCandidate> {
    let mut scored: Vec<ScoredCandidate> = candidates
        .iter()
        .map(|c| ScoredCandidate {
            candidate: c.clone(),
            score: score_candidate(c, slot),
        })
        .collect();
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    scored
}

fn score_candidate(candidate: &ArtworkCandidate, slot: ArtworkSlot) -> f64 {
    let (ideal_w, ideal_h) = slot.ideal_size();
    let ideal_ratio = ideal_w as f64 / ideal_h as f64;
    let candidate_ratio = candidate.width as f64 / candidate.height as f64;
    let ratio_diff = (candidate_ratio - ideal_ratio).abs() / ideal_ratio;
    let aspect_score = 1.0 / (1.0 + ratio_diff);

    let effective_w = (candidate.width as f64).min(candidate.height as f64 * ideal_ratio);
    let effective_h = (candidate.height as f64).min(candidate.width as f64 / ideal_ratio);
    let resolution_score = if effective_w >= ideal_w as f64 && effective_h >= ideal_h as f64 {
        1.0
    } else {
        let effective_area = effective_w * effective_h;
        let ideal_area = ideal_w as f64 * ideal_h as f64;
        (effective_area / ideal_area).min(1.0)
    };

    let alpha_bonus = if slot == ArtworkSlot::Logo && candidate.has_alpha {
        ALPHA_BONUS
    } else {
        0.0
    };

    aspect_score * ASPECT_WEIGHT + resolution_score * RESOLUTION_WEIGHT + alpha_bonus
}

pub fn is_weak_match(score: f64) -> bool {
    score < WEAK_MATCH_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::artwork_adapter::MockArtworkFinder;
    use crate::adapter::state_adapter::MockImportStateStore;
    use crate::adapter::steam_adapter::MockSteamShortcuts;
    use crate::adapter::xboxgames_adapter::MockXboxGamesRepository;
    use crate::types::shortcut_types::VdfValue;
    use crate::types::state_types::ImportedGameRecord;

    fn game(name: &str, has_application_element: bool) -> Game {
        Game {
            name: name.to_string(),
            xboxgames_path: format!("C:\\XboxGames\\{name}"),
            content_dir: format!("C:\\XboxGames\\{name}\\Content"),
            exe_path: format!("C:\\XboxGames\\{name}\\Content\\gamelaunchhelper.exe"),
            has_application_element,
        }
    }

    #[test]
    fn scan_drops_stubs_and_already_imported_games() {
        let mut xboxgames = MockXboxGamesRepository::new();
        xboxgames.expect_list_games().returning(|_| {
            Ok(vec![
                game("Forza Horizon 6", true),
                game("BO7 DLC01 Game Stub 01", false),
                game("Persona 5 Royal", true),
            ])
        });

        let mut state_store = MockImportStateStore::new();
        state_store.expect_load().returning(|_| {
            Ok(ImportState {
                imported: vec![ImportedGameRecord {
                    display_name: "Persona 5 Royal".to_string(),
                    xboxgames_path: "C:\\XboxGames\\Persona 5 Royal".to_string(),
                }],
            })
        });
        state_store.expect_save().returning(|_, _| Ok(()));

        let mut steam = MockSteamShortcuts::new();
        steam.expect_read_shortcuts().returning(|_| Ok(vec![]));

        let mut artwork = MockArtworkFinder::new();
        artwork.expect_find_candidates().returning(|_| Ok(vec![]));

        let controller = DefaultImportController {
            xboxgames: Arc::new(xboxgames),
            artwork: Arc::new(artwork),
            steam: Arc::new(steam),
            state: Arc::new(state_store),
        };

        let proposals = controller
            .scan("C:\\XboxGames", "shortcuts.vdf", "state.json", false)
            .unwrap();

        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].game.name, "Forza Horizon 6");
    }

    #[test]
    fn scan_seeds_state_from_existing_shortcuts_by_app_name() {
        let mut xboxgames = MockXboxGamesRepository::new();
        xboxgames
            .expect_list_games()
            .returning(|_| Ok(vec![game("Forza Horizon 6", true)]));

        let mut state_store = MockImportStateStore::new();
        state_store
            .expect_load()
            .returning(|_| Ok(ImportState::default()));
        state_store
            .expect_save()
            .withf(|_, state: &ImportState| {
                state.imported.len() == 1 && state.imported[0].display_name == "Forza Horizon 6"
            })
            .returning(|_, _| Ok(()));

        let mut steam = MockSteamShortcuts::new();
        steam.expect_read_shortcuts().returning(|_| {
            Ok(vec![VdfValue::Object(vec![
                ("appid".to_string(), VdfValue::Int(-1)),
                (
                    "AppName".to_string(),
                    VdfValue::Str("Forza Horizon 6".to_string()),
                ),
            ])])
        });

        let mut artwork = MockArtworkFinder::new();
        artwork.expect_find_candidates().times(0);

        let controller = DefaultImportController {
            xboxgames: Arc::new(xboxgames),
            artwork: Arc::new(artwork),
            steam: Arc::new(steam),
            state: Arc::new(state_store),
        };

        let proposals = controller
            .scan("C:\\XboxGames", "shortcuts.vdf", "state.json", false)
            .unwrap();

        assert!(proposals.is_empty());
    }

    #[test]
    fn ranks_wide_splash_screen_above_square_logo_for_background_slot() {
        let splash = ArtworkCandidate {
            path: "SplashScreen.png".to_string(),
            width: 1920,
            height: 1080,
            has_alpha: false,
        };
        let square_logo = ArtworkCandidate {
            path: "StoreLogo.png".to_string(),
            width: 100,
            height: 100,
            has_alpha: false,
        };
        let ranked = rank_candidates(&[square_logo, splash.clone()], ArtworkSlot::Background);
        assert_eq!(ranked[0].candidate, splash);
    }

    #[test]
    fn logo_slot_prefers_alpha_candidate_at_equal_aspect() {
        let opaque = ArtworkCandidate {
            path: "opaque.png".to_string(),
            width: 1280,
            height: 720,
            has_alpha: false,
        };
        let transparent = ArtworkCandidate {
            path: "transparent.png".to_string(),
            width: 1280,
            height: 720,
            has_alpha: true,
        };
        let ranked = rank_candidates(&[opaque, transparent.clone()], ArtworkSlot::Logo);
        assert_eq!(ranked[0].candidate, transparent);
    }

    #[test]
    fn weak_match_threshold_flags_poor_aspect_fit() {
        let wide_splash = ArtworkCandidate {
            path: "SplashScreen.png".to_string(),
            width: 1920,
            height: 1080,
            has_alpha: false,
        };
        let score = score_candidate(&wide_splash, ArtworkSlot::Cover);
        assert!(is_weak_match(score));
    }

    #[test]
    fn strong_match_is_not_flagged_as_weak() {
        let close_fit = ArtworkCandidate {
            path: "cover-ish.png".to_string(),
            width: 600,
            height: 900,
            has_alpha: false,
        };
        let score = score_candidate(&close_fit, ArtworkSlot::Cover);
        assert!(!is_weak_match(score));
    }

    #[test]
    fn build_entries_generates_distinct_appids() {
        let selections = vec![
            GameSelection {
                game: game("Forza Horizon 6", true),
                display_name: "Forza Horizon 6".to_string(),
                previous_app_name: None,
                chosen_paths: vec![(ArtworkSlot::Icon, Some("WideLogo.png".to_string()))],
            },
            GameSelection {
                game: game("Persona 5 Royal", true),
                display_name: "Persona 5 Royal".to_string(),
                previous_app_name: None,
                chosen_paths: vec![(ArtworkSlot::Icon, None)],
            },
        ];
        let controller = DefaultImportController {
            xboxgames: Arc::new(MockXboxGamesRepository::new()),
            artwork: Arc::new(MockArtworkFinder::new()),
            steam: Arc::new(MockSteamShortcuts::new()),
            state: Arc::new(MockImportStateStore::new()),
        };

        let built = controller.build_entries(&selections, &std::collections::HashMap::new());

        assert_eq!(built.len(), 2);
        assert_ne!(built[0].entry.appid, built[1].entry.appid);
        assert_eq!(built[0].entry.icon, "WideLogo.png");
        assert_eq!(built[1].entry.icon, "");
    }

    #[test]
    fn build_entries_reuses_existing_appid_when_re_syncing() {
        let selections = vec![GameSelection {
            game: game("Forza Horizon 6", true),
            display_name: "Forza Horizon 6".to_string(),
            previous_app_name: Some("Forza Horizon 6".to_string()),
            chosen_paths: vec![(ArtworkSlot::Icon, Some("NewLogo.png".to_string()))],
        }];
        let mut existing_appid_by_name = std::collections::HashMap::new();
        existing_appid_by_name.insert("Forza Horizon 6".to_string(), -2090050060);
        let controller = DefaultImportController {
            xboxgames: Arc::new(MockXboxGamesRepository::new()),
            artwork: Arc::new(MockArtworkFinder::new()),
            steam: Arc::new(MockSteamShortcuts::new()),
            state: Arc::new(MockImportStateStore::new()),
        };

        let built = controller.build_entries(&selections, &existing_appid_by_name);

        assert_eq!(built[0].entry.appid, -2090050060);
        assert_eq!(built[0].entry.icon, "NewLogo.png");
    }

    #[test]
    fn build_entries_reuses_appid_by_previous_name_when_renaming() {
        let selections = vec![GameSelection {
            game: game("Forza Horizon 6", true),
            display_name: "Forza Horizon 6 - PC Edition".to_string(),
            previous_app_name: Some("Forza Horizon 6".to_string()),
            chosen_paths: vec![],
        }];
        let mut existing_appid_by_name = std::collections::HashMap::new();
        existing_appid_by_name.insert("Forza Horizon 6".to_string(), -2090050060);
        let controller = DefaultImportController {
            xboxgames: Arc::new(MockXboxGamesRepository::new()),
            artwork: Arc::new(MockArtworkFinder::new()),
            steam: Arc::new(MockSteamShortcuts::new()),
            state: Arc::new(MockImportStateStore::new()),
        };

        let built = controller.build_entries(&selections, &existing_appid_by_name);

        assert_eq!(built[0].entry.appid, -2090050060);
        assert_eq!(built[0].entry.app_name, "Forza Horizon 6 - PC Edition");
    }

    #[test]
    fn scan_includes_already_imported_games_when_asked() {
        let mut xboxgames = MockXboxGamesRepository::new();
        xboxgames
            .expect_list_games()
            .returning(|_| Ok(vec![game("Forza Horizon 6", true)]));

        let mut state_store = MockImportStateStore::new();
        state_store.expect_load().returning(|_| {
            Ok(ImportState {
                imported: vec![ImportedGameRecord {
                    display_name: "Forza Horizon 6".to_string(),
                    xboxgames_path: "C:\\XboxGames\\Forza Horizon 6".to_string(),
                }],
            })
        });

        let mut steam = MockSteamShortcuts::new();
        steam.expect_read_shortcuts().returning(|_| Ok(vec![]));

        let mut artwork = MockArtworkFinder::new();
        artwork.expect_find_candidates().returning(|_| Ok(vec![]));

        let controller = DefaultImportController {
            xboxgames: Arc::new(xboxgames),
            artwork: Arc::new(artwork),
            steam: Arc::new(steam),
            state: Arc::new(state_store),
        };

        let proposals = controller
            .scan("C:\\XboxGames", "shortcuts.vdf", "state.json", true)
            .unwrap();

        assert_eq!(proposals.len(), 1);
        assert!(proposals[0].already_imported);
    }
}
