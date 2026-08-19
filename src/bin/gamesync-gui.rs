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

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() -> eframe::Result<()> {
    use std::sync::Arc;

    use eframe::egui;
    use gamesync::adapter::artwork_adapter::FsArtworkFinder;
    use gamesync::adapter::state_adapter::FsImportStateStore;
    use gamesync::adapter::steam_adapter::FsSteamShortcuts;
    use gamesync::adapter::xboxgames_adapter::FsXboxGamesRepository;
    use gamesync::controller::import_controller::DefaultImportController;
    use gamesync::driver::gui_driver::GamesyncGuiApp;

    gamesync::logging::init("GameSync.log");
    gamesync::logging::log("gui starting up");

    let controller = Arc::new(DefaultImportController {
        xboxgames: Arc::new(FsXboxGamesRepository),
        artwork: Arc::new(FsArtworkFinder),
        steam: Arc::new(FsSteamShortcuts),
        state: Arc::new(FsImportStateStore),
    });

    let app = GamesyncGuiApp::new(
        controller,
        Arc::new(FsSteamShortcuts),
        "C:\\XboxGames".to_string(),
        "C:\\Program Files (x86)\\Steam".to_string(),
        "GameSync-state.json".to_string(),
    );

    let icon = gamesync::assets::ICON;
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([760.0, 560.0])
            .with_resizable(true)
            .with_title("GameSync")
            .with_icon(egui::IconData {
                rgba: icon.rgba.to_vec(),
                width: icon.width,
                height: icon.height,
            }),
        ..Default::default()
    };

    eframe::run_native(
        "GameSync",
        native_options,
        Box::new(|cc| {
            gamesync::driver::gui_driver::apply_theme(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(not(windows))]
fn main() {
    eprintln!("GameSync only runs on Windows");
}
