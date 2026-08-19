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

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;

use crate::adapter::steam_adapter::{self, SteamShortcuts};
use crate::controller::import_controller::DefaultImportController;
use crate::logging::log;
use crate::types::game_types::Game;
use crate::types::shortcut_types::{
    ArtworkSlot, GameProposal, GameSelection, ScoredCandidate, VdfValue,
};

const BG_DEEP: egui::Color32 = egui::Color32::from_rgb(16, 19, 28);
const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(24, 29, 42);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(30, 36, 52);
const LINE: egui::Color32 = egui::Color32::from_rgb(46, 53, 71);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(231, 234, 242);
const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(140, 149, 168);
const TEAL: egui::Color32 = egui::Color32::from_rgb(67, 230, 210);
const TEAL_FILL: egui::Color32 = egui::Color32::from_rgb(56, 190, 174);
const MAGENTA: egui::Color32 = egui::Color32::from_rgb(226, 79, 192);
const AMBER: egui::Color32 = egui::Color32::from_rgb(227, 168, 77);
const GREEN: egui::Color32 = egui::Color32::from_rgb(79, 203, 134);
const ERR: egui::Color32 = egui::Color32::from_rgb(226, 89, 107);
const THUMB_SIZE: f32 = 76.0;
const SLOT_COLUMN_WIDTH: f32 = 160.0;
const WIDGET_RADIUS: u8 = 8;
const CARD_RADIUS: u8 = 12;

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_fill = BG_DEEP;
    visuals.panel_fill = BG_DEEP;
    visuals.faint_bg_color = BG_PANEL;
    visuals.extreme_bg_color = BG_PANEL;
    visuals.selection.bg_fill = TEAL.gamma_multiply(0.55);
    visuals.selection.stroke = egui::Stroke::new(1.0f32, TEAL);
    visuals.hyperlink_color = MAGENTA;
    visuals.window_corner_radius = egui::CornerRadius::same(CARD_RADIUS);
    visuals.menu_corner_radius = egui::CornerRadius::same(WIDGET_RADIUS);

    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0f32, LINE);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(WIDGET_RADIUS);
    visuals.widgets.inactive.bg_fill = BG_PANEL;
    visuals.widgets.inactive.weak_bg_fill = BG_PANEL;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0f32, LINE);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(WIDGET_RADIUS);
    visuals.widgets.hovered.bg_fill = BG_PANEL.gamma_multiply(1.15);
    visuals.widgets.hovered.weak_bg_fill = BG_PANEL.gamma_multiply(1.15);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5f32, TEAL);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(WIDGET_RADIUS);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5f32, MAGENTA);
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(WIDGET_RADIUS);
    visuals.widgets.open.corner_radius = egui::CornerRadius::same(WIDGET_RADIUS);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 9.0);
    style.spacing.window_margin = egui::Margin::same(14);
    style.spacing.indent = 18.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(21.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(11.5, egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}

fn primary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let font = egui::FontId::proportional(15.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, BG_DEEP);
    let size = galley.size() + egui::vec2(40.0, 22.0);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    let fill = if !enabled {
        BG_PANEL
    } else if response.is_pointer_button_down_on() {
        TEAL_FILL.gamma_multiply(0.82)
    } else if response.hovered() || response.has_focus() {
        TEAL_FILL.gamma_multiply(1.1)
    } else {
        TEAL_FILL
    };
    let text_color = if enabled { BG_DEEP } else { TEXT_MUTED };

    let painter = ui.painter();
    painter.rect_filled(rect, CARD_RADIUS, fill);
    if !enabled {
        painter.rect_stroke(
            rect,
            CARD_RADIUS,
            egui::Stroke::new(1.0f32, LINE),
            egui::StrokeKind::Inside,
        );
    }
    if enabled && response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            CARD_RADIUS,
            egui::Stroke::new(2.0f32, TEAL),
            egui::StrokeKind::Outside,
        );
    }
    let text_pos = rect.center() - galley.size() / 2.0;
    painter.galley(text_pos, galley, text_color);

    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

fn activated(response: &egui::Response, ui: &egui::Ui) -> bool {
    response.clicked()
        || (response.has_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)))
}

fn toggle_switch(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let size = egui::vec2(38.0, 21.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if activated(&response, ui) {
        *value = !*value;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool(response.id, *value);
    let track_color = lerp_color(LINE, TEAL_FILL, t);
    let painter = ui.painter();
    painter.rect_filled(rect, rect.height() / 2.0, track_color);
    let knob_x = egui::lerp(
        (rect.left() + rect.height() / 2.0)..=(rect.right() - rect.height() / 2.0),
        t,
    );
    let knob_center = egui::pos2(knob_x, rect.center().y);
    painter.circle_filled(knob_center, rect.height() / 2.0 - 3.0, TEXT_PRIMARY);
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            rect.height() / 2.0 + 2.0,
            egui::Stroke::new(2.0f32, TEAL),
            egui::StrokeKind::Outside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

fn duotone_bar(ui: &mut egui::Ui, height: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let steps = 48;
    let step_w = rect.width() / steps as f32;
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let color = lerp_color(TEAL, MAGENTA, t);
        let x0 = rect.left() + step_w * i as f32;
        let strip = egui::Rect::from_min_size(
            egui::pos2(x0, rect.top()),
            egui::vec2(step_w + 1.0, rect.height()),
        );
        ui.painter().rect_filled(strip, 0, color);
    }
}

fn render_breadcrumb(
    ui: &mut egui::Ui,
    current_step: usize,
    clickable: [bool; 3],
) -> Option<usize> {
    let steps = ["Set up", "Scan", "Review & import"];
    let hints = [
        "Go back to Setup",
        "Scanning runs in the background",
        "Back to reviewing your last scan",
    ];
    let mut clicked_step = None;
    ui.horizontal(|ui| {
        for (i, label) in steps.iter().enumerate() {
            let is_clickable = clickable[i] && i != current_step;
            let (fill, text_color, stroke) = if i == current_step {
                (TEAL_FILL, BG_DEEP, TEAL_FILL)
            } else if i < current_step {
                (BG_PANEL, GREEN, GREEN)
            } else {
                (BG_PANEL, TEXT_MUTED, LINE)
            };
            let font = egui::FontId::proportional(12.5);
            let text = format!("{}  {label}", i + 1);
            let galley = ui.painter().layout_no_wrap(text, font, text_color);
            let size = galley.size() + egui::vec2(20.0, 10.0);
            let sense = if is_clickable {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let (rect, response) = ui.allocate_exact_size(size, sense);
            let painter = ui.painter();
            let fill = if is_clickable && response.hovered() {
                fill.gamma_multiply(1.2)
            } else {
                fill
            };
            painter.rect_filled(rect, rect.height() / 2.0, fill);
            painter.rect_stroke(
                rect,
                rect.height() / 2.0,
                egui::Stroke::new(1.0f32, stroke),
                egui::StrokeKind::Inside,
            );
            if is_clickable && response.has_focus() {
                painter.rect_stroke(
                    rect.expand(2.0),
                    rect.height() / 2.0 + 2.0,
                    egui::Stroke::new(2.0f32, TEAL),
                    egui::StrokeKind::Outside,
                );
            }
            painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);
            let response = if is_clickable {
                response
                    .on_hover_text(hints[i])
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
            } else {
                response
            };
            if is_clickable && activated(&response, ui) {
                clicked_step = Some(i);
            }
            if i + 1 < steps.len() {
                ui.add_space(2.0);
                let (line_rect, _) =
                    ui.allocate_exact_size(egui::vec2(18.0, 2.0), egui::Sense::hover());
                let line_color = if i < current_step { GREEN } else { LINE };
                ui.painter().rect_filled(line_rect, 1, line_color);
                ui.add_space(2.0);
            }
        }
    });
    clicked_step
}

fn slot_help(slot: ArtworkSlot) -> &'static str {
    match slot {
        ArtworkSlot::Icon => {
            "The small icon shown next to this game in Steam's shortcut list and the taskbar. Roughly square works best."
        }
        ArtworkSlot::Cover => {
            "Tall portrait box art (600x900) shown in Steam's library grid view. Most Xbox games don't ship a portrait image, so this one is often a weak match."
        }
        ArtworkSlot::WideCover => {
            "Wide banner (920x430) shown at the front of carousels like Recent Games and in Big Picture mode."
        }
        ArtworkSlot::Background => {
            "Large hero image (3840x1240) shown across the top of the game's details page."
        }
        ArtworkSlot::Logo => {
            "Transparent logo overlaid on the background image on the details page. A logo with transparency works best."
        }
    }
}

#[derive(Clone)]
struct SlotChoice {
    slot: ArtworkSlot,
    ranked: Vec<ScoredCandidate>,
    selected: Option<usize>,
    custom_path: Option<String>,
}

impl SlotChoice {
    fn current_path(&self) -> Option<String> {
        self.custom_path.clone().or_else(|| {
            self.selected
                .and_then(|i| self.ranked.get(i))
                .map(|c| c.candidate.path.clone())
        })
    }
}

#[derive(Clone)]
struct GameRow {
    game: Game,
    accepted: bool,
    already_imported: bool,
    display_name: String,
    previous_app_name: Option<String>,
    slot_choices: Vec<SlotChoice>,
}

enum Screen {
    Setup {
        userdata_candidates: Vec<String>,
    },
    Scanning {
        receiver: Receiver<Result<Vec<GameProposal>, String>>,
    },
    Reviewing {
        rows: Vec<GameRow>,
    },
    Importing {
        receiver: Receiver<Result<String, String>>,
        rows: Vec<GameRow>,
    },
    Done {
        message: String,
    },
}

pub struct GamesyncGuiApp {
    pub controller: Arc<DefaultImportController>,
    pub steam: Arc<dyn SteamShortcuts>,
    pub xboxgames_root: String,
    pub steam_root: String,
    pub userdata_dir: Option<String>,
    pub state_path: String,
    screen: Screen,
    error: Option<String>,
    textures: HashMap<String, egui::TextureHandle>,
    show_help: bool,
    include_already_imported: bool,
    cached_rows: Option<Vec<GameRow>>,
}

impl GamesyncGuiApp {
    pub fn new(
        controller: Arc<DefaultImportController>,
        steam: Arc<dyn SteamShortcuts>,
        xboxgames_root: String,
        steam_root: String,
        state_path: String,
    ) -> Self {
        let mut app = GamesyncGuiApp {
            controller,
            steam,
            xboxgames_root,
            steam_root,
            userdata_dir: None,
            state_path,
            screen: Screen::Setup {
                userdata_candidates: Vec::new(),
            },
            error: None,
            textures: HashMap::new(),
            show_help: true,
            include_already_imported: false,
            cached_rows: None,
        };
        app.find_userdata();
        app
    }

    fn screen_step(&self) -> usize {
        match &self.screen {
            Screen::Setup { .. } => 0,
            Screen::Scanning { .. } => 1,
            Screen::Reviewing { .. } | Screen::Importing { .. } | Screen::Done { .. } => 2,
        }
    }

    fn navigate_to_setup(&mut self) {
        if let Screen::Reviewing { rows } = &self.screen {
            self.cached_rows = Some(rows.clone());
        }
        self.error = None;
        self.screen = Screen::Setup {
            userdata_candidates: Vec::new(),
        };
    }

    fn navigate_to_review(&mut self) {
        if let Some(rows) = self.cached_rows.clone() {
            self.error = None;
            self.screen = Screen::Reviewing { rows };
        }
    }

    fn shortcuts_vdf_path(&self) -> Option<String> {
        self.userdata_dir
            .as_ref()
            .map(|dir| format!("{dir}/config/shortcuts.vdf"))
    }

    fn grid_dir(&self) -> Option<String> {
        self.userdata_dir
            .as_ref()
            .map(|dir| format!("{dir}/config/grid"))
    }

    fn find_userdata(&mut self) {
        log("gui_driver: find_userdata clicked");
        match self.steam.find_userdata_dirs(&self.steam_root) {
            Ok(dirs) if dirs.len() == 1 => {
                self.userdata_dir = Some(dirs[0].clone());
                self.error = None;
                self.screen = Screen::Setup {
                    userdata_candidates: Vec::new(),
                };
            }
            Ok(dirs) if dirs.is_empty() => {
                self.error = Some(format!(
                    "no Steam userdata folders found under {}",
                    self.steam_root
                ));
                self.screen = Screen::Setup {
                    userdata_candidates: Vec::new(),
                };
            }
            Ok(dirs) => {
                self.error = None;
                self.screen = Screen::Setup {
                    userdata_candidates: dirs,
                };
            }
            Err(e) => {
                self.error = Some(format!("finding steam userdata: {e}"));
                self.screen = Screen::Setup {
                    userdata_candidates: Vec::new(),
                };
            }
        }
    }

    fn texture_for(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.textures.get(path) {
            return Some(tex.clone());
        }
        let bytes = std::fs::read(path).ok()?;
        let decoded = image::load_from_memory(&bytes).ok()?;
        let rgba = decoded.to_rgba8();
        let (w, h) = (rgba.width() as usize, rgba.height() as usize);
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
        let tex = ctx.load_texture(path, color_image, egui::TextureOptions::default());
        self.textures.insert(path.to_string(), tex.clone());
        Some(tex)
    }

    fn spawn_scan(&mut self, ctx: egui::Context) {
        let Some(shortcuts_vdf_path) = self.shortcuts_vdf_path() else {
            self.error = Some("pick a Steam userdata folder first".to_string());
            return;
        };
        self.error = None;
        log("gui_driver: scan requested");

        let controller = Arc::clone(&self.controller);
        let xboxgames_root = self.xboxgames_root.clone();
        let state_path = self.state_path.clone();
        let include_already_imported = self.include_already_imported;
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = controller
                .scan(
                    &xboxgames_root,
                    &shortcuts_vdf_path,
                    &state_path,
                    include_already_imported,
                )
                .map_err(|e| e.to_string());
            log(&format!(
                "gui_driver: scan thread finished, ok={}",
                result.is_ok()
            ));
            let _ = tx.send(result);
            ctx.request_repaint();
        });

        self.screen = Screen::Scanning { receiver: rx };
    }

    fn spawn_import(&mut self, ctx: egui::Context, rows: Vec<GameRow>) {
        let Some(shortcuts_vdf_path) = self.shortcuts_vdf_path() else {
            self.error = Some("pick a Steam userdata folder first".to_string());
            self.screen = Screen::Reviewing { rows };
            return;
        };
        let Some(grid_dir) = self.grid_dir() else {
            self.error = Some("pick a Steam userdata folder first".to_string());
            self.screen = Screen::Reviewing { rows };
            return;
        };

        let selections: Vec<GameSelection> = rows
            .iter()
            .filter(|r| r.accepted)
            .map(row_to_selection)
            .collect();
        let imported: Vec<(Game, String)> = rows
            .iter()
            .filter(|r| r.accepted)
            .map(|r| (r.game.clone(), r.display_name.clone()))
            .collect();

        if selections.is_empty() {
            self.error = Some("Check at least one game before clicking Import.".to_string());
            self.screen = Screen::Reviewing { rows };
            return;
        }

        self.error = None;
        log(&format!(
            "gui_driver: import requested for {} game(s)",
            selections.len()
        ));

        let controller = Arc::clone(&self.controller);
        let steam = Arc::clone(&self.steam);
        let state_path = self.state_path.clone();
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = run_import(
                controller.as_ref(),
                steam.as_ref(),
                &shortcuts_vdf_path,
                &grid_dir,
                &state_path,
                &selections,
                &imported,
            );
            log(&format!(
                "gui_driver: import thread finished, ok={}",
                result.is_ok()
            ));
            let _ = tx.send(result);
            ctx.request_repaint();
        });

        self.screen = Screen::Importing { receiver: rx, rows };
    }
}

fn run_import(
    controller: &DefaultImportController,
    steam: &dyn SteamShortcuts,
    shortcuts_vdf_path: &str,
    grid_dir: &str,
    state_path: &str,
    selections: &[GameSelection],
    imported: &[(Game, String)],
) -> Result<String, String> {
    if steam.is_steam_running().map_err(|e| e.to_string())? {
        return Err("Steam is running. Close it, then click Import again.".to_string());
    }

    let mut existing = steam
        .read_shortcuts(shortcuts_vdf_path)
        .map_err(|e| e.to_string())?;

    let mut existing_appid_by_name = HashMap::new();
    for shortcut in &existing {
        if let (Some(name), Some(appid)) = (
            steam_adapter::app_name_of(shortcut),
            steam_adapter::appid_of(shortcut),
        ) {
            existing_appid_by_name.insert(name, appid);
        }
    }

    let built = controller.build_entries(selections, &existing_appid_by_name);

    for built_game in &built {
        upsert_shortcut(
            &mut existing,
            built_game.previous_app_name.as_deref(),
            steam_adapter::shortcut_entry_to_vdf(&built_game.entry),
        );
        for (slot, path) in &built_game.artwork {
            let Some(path) = path else { continue };
            steam
                .place_grid_image(grid_dir, built_game.entry.appid, *slot, path)
                .map_err(|e| e.to_string())?;
        }
    }
    steam
        .write_shortcuts(shortcuts_vdf_path, &existing)
        .map_err(|e| e.to_string())?;
    controller
        .mark_imported(state_path, imported)
        .map_err(|e| e.to_string())?;

    Ok(format!("Imported {} game(s) into Steam.", built.len()))
}

fn upsert_shortcut(
    existing: &mut Vec<VdfValue>,
    previous_app_name: Option<&str>,
    new_entry: VdfValue,
) {
    let position = previous_app_name.and_then(|name| {
        existing
            .iter()
            .position(|v| steam_adapter::app_name_of(v).as_deref() == Some(name))
    });
    match position {
        Some(i) => existing[i] = new_entry,
        None => existing.push(new_entry),
    }
}

fn proposal_to_row(proposal: GameProposal) -> GameRow {
    let previous_app_name = proposal
        .already_imported
        .then(|| proposal.suggested_name.clone());
    GameRow {
        game: proposal.game,
        accepted: !proposal.already_imported,
        already_imported: proposal.already_imported,
        display_name: proposal.suggested_name,
        previous_app_name,
        slot_choices: proposal
            .slots
            .into_iter()
            .map(|sc| SlotChoice {
                slot: sc.slot,
                selected: if sc.ranked.is_empty() { None } else { Some(0) },
                ranked: sc.ranked,
                custom_path: None,
            })
            .collect(),
    }
}

fn row_to_selection(row: &GameRow) -> GameSelection {
    GameSelection {
        game: row.game.clone(),
        display_name: row.display_name.clone(),
        previous_app_name: row.previous_app_name.clone(),
        chosen_paths: row
            .slot_choices
            .iter()
            .map(|sc| (sc.slot, sc.current_path()))
            .collect(),
    }
}

impl eframe::App for GamesyncGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut breadcrumb_target: Option<usize> = None;
        let is_importing = matches!(self.screen, Screen::Importing { .. });
        let has_cached_review = self.cached_rows.is_some();

        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(BG_DEEP)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Game").heading().strong().color(TEAL));
                    ui.add_space(-8.0);
                    ui.label(
                        egui::RichText::new("Sync")
                            .heading()
                            .strong()
                            .color(MAGENTA),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("brings your Xbox games into Steam").color(TEXT_MUTED),
                    );
                    if ui
                        .button(if self.show_help {
                            "Hide help"
                        } else {
                            "Show help"
                        })
                        .clicked()
                    {
                        self.show_help = !self.show_help;
                    }
                });
                ui.add_space(6.0);
                duotone_bar(ui, 3.0);
                ui.add_space(8.0);
                breadcrumb_target = render_breadcrumb(
                    ui,
                    self.screen_step(),
                    [!is_importing, false, has_cached_review && !is_importing],
                );

                if self.show_help {
                    ui.add_space(6.0);
                    egui::Frame::group(ui.style())
                        .fill(BG_PANEL)
                        .stroke(egui::Stroke::new(1.0f32, LINE))
                        .show(ui, |ui| {
                            ui.label(
                                "Point GameSync at your Xbox games folder and your Steam folder, \
                             scan for installed games, review the icon and artwork GameSync \
                             picked for each one, then click Import. GameSync writes the games \
                             into Steam and copies the chosen artwork into Steam's own config \
                             folder. Nothing changes until you click Import. Already-imported \
                             games can be re-scanned to update their artwork without creating a \
                             duplicate entry.",
                            );
                        });
                }

                if let Some(err) = &self.error {
                    ui.add_space(6.0);
                    egui::Frame::group(ui.style())
                        .fill(ERR.gamma_multiply(0.16))
                        .stroke(egui::Stroke::new(1.0f32, ERR))
                        .corner_radius(WIDGET_RADIUS)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.colored_label(ERR, err);
                        });
                }
                ui.add_space(4.0);
            });

        if let Some(target) = breadcrumb_target {
            match target {
                0 => self.navigate_to_setup(),
                2 => self.navigate_to_review(),
                _ => {}
            }
        }

        let screen = std::mem::replace(&mut self.screen, Screen::Reviewing { rows: Vec::new() });

        match screen {
            Screen::Setup {
                userdata_candidates,
            } => {
                egui::CentralPanel::default()
                    .show(ctx, |ui| self.render_setup(ui, userdata_candidates));
            }
            Screen::Scanning { receiver } => {
                egui::CentralPanel::default().show(ctx, |ui| self.render_scanning(ui, receiver));
            }
            Screen::Reviewing { rows } => self.render_reviewing_screen(ctx, rows),
            Screen::Importing { receiver, rows } => {
                egui::CentralPanel::default()
                    .show(ctx, |ui| self.render_importing(ui, receiver, rows));
            }
            Screen::Done { message } => {
                egui::CentralPanel::default().show(ctx, |ui| self.render_done(ui, message));
            }
        }
    }
}

const PATH_LABEL_WIDTH: f32 = 130.0;

fn labeled_path_field(ui: &mut egui::Ui, label: &str, value: &mut String, hover: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([PATH_LABEL_WIDTH, 20.0], egui::Label::new(label).truncate());
        let field_width = (ui.available_width() - 90.0).max(120.0);
        ui.add_sized([field_width, 20.0], egui::TextEdit::singleline(value))
            .on_hover_text(hover);
        if ui.button("Browse...").clicked() {
            if let Some(dir) = rfd::FileDialog::new()
                .set_directory(value.as_str())
                .pick_folder()
            {
                *value = dir.to_string_lossy().to_string();
            }
        }
    });
}

impl GamesyncGuiApp {
    fn render_setup(&mut self, ui: &mut egui::Ui, userdata_candidates: Vec<String>) {
        let mut find_clicked = false;
        let mut scan_clicked = false;
        let mut continue_clicked = false;
        let mut chosen_userdata: Option<String> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Set up");
                ui.label(
                    "These default to the normal install locations. Only change them if yours differ.",
                );
                ui.add_space(6.0);

                egui::Frame::group(ui.style())
                    .fill(BG_CARD)
                    .stroke(egui::Stroke::new(1.0f32, LINE))
                    .corner_radius(CARD_RADIUS)
                    .inner_margin(14.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        labeled_path_field(
                            ui,
                            "Xbox games folder",
                            &mut self.xboxgames_root,
                            "Where the Xbox app installs your GamePass games. Usually C:\\XboxGames.",
                        );
                        labeled_path_field(
                            ui,
                            "Steam folder",
                            &mut self.steam_root,
                            "Where Steam itself is installed. Usually C:\\Program Files (x86)\\Steam.",
                        );

                        ui.add_space(4.0);
                        match &self.userdata_dir {
                            Some(dir) => {
                                ui.horizontal(|ui| {
                                    ui.colored_label(GREEN, format!("Steam account found: {dir}"));
                                    if ui.small_button("change").clicked() {
                                        find_clicked = true;
                                    }
                                });
                            }
                            None => {
                                ui.horizontal(|ui| {
                                    ui.colored_label(AMBER, "No Steam account found yet.");
                                    if ui.button("Find Steam account folder").clicked() {
                                        find_clicked = true;
                                    }
                                });
                            }
                        }
                    });

                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.include_already_imported,
                    "Also show already-imported games (to update their artwork)",
                )
                .on_hover_text(
                    "Slower: also re-scans games already in Steam so you can pick new artwork for \
                     them. Off by default so a normal scan only touches new games.",
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let can_scan = self.userdata_dir.is_some();
                    let resp = primary_button(ui, "Scan for games", can_scan).on_hover_text(
                        "Looks through your Xbox games folder for real, installed games.",
                    );
                    scan_clicked = activated(&resp, ui);

                    if let Some(cached) = &self.cached_rows {
                        if ui
                            .button(format!("Continue reviewing ({} games)", cached.len()))
                            .on_hover_text("Go back to the review screen without rescanning")
                            .clicked()
                        {
                            continue_clicked = true;
                        }
                    }
                });

                if !userdata_candidates.is_empty() {
                    ui.add_space(6.0);
                    ui.label("multiple Steam accounts found, pick one:");
                    for dir in &userdata_candidates {
                        if ui.button(dir).clicked() {
                            chosen_userdata = Some(dir.clone());
                        }
                    }
                }
            });

        if find_clicked {
            self.find_userdata();
            return;
        }
        if scan_clicked {
            let ctx = ui.ctx().clone();
            self.spawn_scan(ctx);
            return;
        }
        if continue_clicked {
            self.navigate_to_review();
            return;
        }
        if let Some(dir) = chosen_userdata {
            self.userdata_dir = Some(dir);
            self.screen = Screen::Setup {
                userdata_candidates: Vec::new(),
            };
            return;
        }

        self.screen = Screen::Setup {
            userdata_candidates,
        };
    }

    fn render_scanning(
        &mut self,
        ui: &mut egui::Ui,
        receiver: Receiver<Result<Vec<GameProposal>, String>>,
    ) {
        let mut cancel_clicked = false;
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Scanning for games. This can take a few seconds.");
        });
        ui.add_space(4.0);
        if ui.button("Cancel").clicked() {
            cancel_clicked = true;
        }
        if cancel_clicked {
            log("gui_driver: scan cancelled from UI, scan thread keeps running in background");
            self.error = None;
            self.screen = Screen::Setup {
                userdata_candidates: Vec::new(),
            };
            return;
        }

        match receiver.try_recv() {
            Ok(Ok(proposals)) => {
                self.error = None;
                let rows: Vec<GameRow> = proposals.into_iter().map(proposal_to_row).collect();
                self.cached_rows = Some(rows.clone());
                self.screen = Screen::Reviewing { rows };
            }
            Ok(Err(e)) => {
                self.error = Some(format!("scanning: {e}"));
                self.screen = Screen::Setup {
                    userdata_candidates: Vec::new(),
                };
            }
            Err(TryRecvError::Empty) => {
                ui.ctx().request_repaint_after(Duration::from_millis(200));
                self.screen = Screen::Scanning { receiver };
            }
            Err(TryRecvError::Disconnected) => {
                self.error = Some("scan thread ended unexpectedly".to_string());
                self.screen = Screen::Setup {
                    userdata_candidates: Vec::new(),
                };
            }
        }
    }

    fn render_reviewing_screen(&mut self, ctx: &egui::Context, mut rows: Vec<GameRow>) {
        self.cached_rows = Some(rows.clone());
        let accepted_count = rows.iter().filter(|r| r.accepted).count();
        let mut import_clicked = false;
        let mut back_clicked = false;

        egui::TopBottomPanel::bottom("import_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let resp = primary_button(
                    ui,
                    &format!("Import {accepted_count} selected game(s)"),
                    accepted_count > 0,
                );
                import_clicked = activated(&resp, ui);
                ui.label(
                    egui::RichText::new("Checks that Steam is closed for you").color(TEXT_MUTED),
                );
            });
            ui.add_space(6.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button("\u{2190} Back to Setup")
                    .on_hover_text("Keeps this review \u{2014} you can jump back in from Setup")
                    .clicked()
                {
                    back_clicked = true;
                }
                ui.heading("Review and import");
            });
            ui.label(
                "New games are checked by default with GameSync's best pick per slot. \
                 Already-imported games (if shown) start unchecked \u{2014} check one to update \
                 its artwork without creating a duplicate Steam entry.",
            );
            ui.add_space(4.0);
            ui.separator();

            if rows.is_empty() {
                if self.include_already_imported {
                    ui.label(
                        "No games found. Every installed game is already in Steam and nothing \
                         new turned up under your Xbox games folder.",
                    );
                } else {
                    ui.label(
                        "No new games found. Every installed game is already in Steam. Turn on \
                         \"Also show already-imported games\" on the previous screen to update \
                         one's artwork instead.",
                    );
                }
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in &mut rows {
                        render_game_card(ui, ctx, self, row);
                        ui.add_space(6.0);
                    }
                });
        });

        if import_clicked {
            let ctx2 = ctx.clone();
            self.spawn_import(ctx2, rows);
        } else if back_clicked {
            self.cached_rows = Some(rows);
            self.screen = Screen::Setup {
                userdata_candidates: Vec::new(),
            };
        } else {
            self.screen = Screen::Reviewing { rows };
        }
    }

    fn render_importing(
        &mut self,
        ui: &mut egui::Ui,
        receiver: Receiver<Result<String, String>>,
        rows: Vec<GameRow>,
    ) {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Importing...");
        });

        match receiver.try_recv() {
            Ok(Ok(message)) => {
                self.error = None;
                self.screen = Screen::Done { message };
            }
            Ok(Err(e)) => {
                self.error = Some(format!("importing: {e}"));
                self.screen = Screen::Reviewing { rows };
            }
            Err(TryRecvError::Empty) => {
                ui.ctx().request_repaint_after(Duration::from_millis(200));
                self.screen = Screen::Importing { receiver, rows };
            }
            Err(TryRecvError::Disconnected) => {
                self.error = Some("import thread ended unexpectedly".to_string());
                self.screen = Screen::Reviewing { rows };
            }
        }
    }

    fn render_done(&mut self, ui: &mut egui::Ui, message: String) {
        self.cached_rows = None;
        ui.add_space(8.0);
        ui.colored_label(GREEN, egui::RichText::new(&message).heading().strong());
        ui.add_space(10.0);
        let resp = primary_button(ui, "Scan for more games", true);
        if activated(&resp, ui) {
            self.screen = Screen::Setup {
                userdata_candidates: Vec::new(),
            };
        } else {
            self.screen = Screen::Done { message };
        }
    }
}

fn render_game_card(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut GamesyncGuiApp,
    row: &mut GameRow,
) {
    let border = if row.accepted { TEAL } else { LINE };
    let fill = if row.accepted {
        BG_CARD
    } else {
        BG_CARD.gamma_multiply(0.7)
    };
    let heading_color = if row.accepted {
        TEXT_PRIMARY
    } else {
        TEXT_MUTED
    };
    egui::Frame::group(ui.style())
        .fill(fill)
        .stroke(egui::Stroke::new(1.0f32, border))
        .corner_radius(CARD_RADIUS)
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut row.accepted);
                ui.add_sized(
                    [260.0, 26.0],
                    egui::TextEdit::singleline(&mut row.display_name)
                        .font(egui::FontId::proportional(19.0))
                        .text_color(heading_color),
                )
                .on_hover_text("The name Steam will show for this game. Edit it freely.");
                if row.already_imported {
                    ui.colored_label(
                        TEAL,
                        "Already in Steam \u{2014} updating replaces its entry",
                    );
                }
            });
            if row.display_name != row.game.name {
                ui.label(
                    egui::RichText::new(format!("Originally: {}", row.game.name))
                        .small()
                        .color(TEXT_MUTED),
                );
            }
            ui.label(
                egui::RichText::new(row.game.exe_path.clone())
                    .small()
                    .color(TEXT_MUTED),
            );
            if row.accepted {
                ui.add_space(4.0);
                render_slots(ui, ctx, app, row);
            } else {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Skipped \u{2014} check the toggle to include this game")
                        .small()
                        .color(TEXT_MUTED),
                );
            }
        });
}

fn render_slots(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut GamesyncGuiApp,
    row: &mut GameRow,
) {
    ui.horizontal_wrapped(|ui| {
        for slot_choice in &mut row.slot_choices {
            ui.vertical(|ui| {
                ui.set_width(SLOT_COLUMN_WIDTH);
                let path = slot_choice.current_path();
                let tex = path.as_ref().and_then(|p| app.texture_for(ctx, p));
                render_thumbnail_box(ui, tex.as_ref());

                ui.add_sized(
                    [SLOT_COLUMN_WIDTH, 18.0],
                    egui::Label::new(egui::RichText::new(slot_choice.slot.label()).strong())
                        .truncate(),
                )
                .on_hover_text(slot_help(slot_choice.slot));

                let current_label = match &path {
                    Some(p) => short_path(p),
                    None => "(skip)".to_string(),
                };
                egui::ComboBox::from_id_salt(format!(
                    "{}-{:?}",
                    row.game.xboxgames_path, slot_choice.slot as u8
                ))
                .width(SLOT_COLUMN_WIDTH)
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            slot_choice.custom_path.is_none() && slot_choice.selected.is_none(),
                            "(skip)",
                        )
                        .clicked()
                    {
                        slot_choice.selected = None;
                        slot_choice.custom_path = None;
                    }
                    for (i, candidate) in slot_choice.ranked.iter().enumerate() {
                        let label = format!(
                            "{} ({}x{})",
                            short_path(&candidate.candidate.path),
                            candidate.candidate.width,
                            candidate.candidate.height
                        );
                        let is_selected =
                            slot_choice.custom_path.is_none() && slot_choice.selected == Some(i);
                        let clicked = ui
                            .horizontal(|ui| {
                                if let Some(tex) = app.texture_for(ctx, &candidate.candidate.path) {
                                    ui.add(
                                        egui::Image::new(&tex)
                                            .fit_to_exact_size(egui::vec2(28.0, 28.0))
                                            .corner_radius(3),
                                    );
                                } else {
                                    ui.allocate_space(egui::vec2(28.0, 28.0));
                                }
                                ui.selectable_label(is_selected, label).clicked()
                            })
                            .inner;
                        if clicked {
                            slot_choice.selected = Some(i);
                            slot_choice.custom_path = None;
                        }
                    }
                });

                if ui.small_button("Browse...").clicked() {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter(
                            "Image",
                            &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tiff", "ico"],
                        )
                        .pick_file()
                    {
                        slot_choice.custom_path = Some(file.to_string_lossy().to_string());
                    }
                }
            });
        }
    });
}

fn render_thumbnail_box(ui: &mut egui::Ui, tex: Option<&egui::TextureHandle>) {
    let box_size = egui::vec2(THUMB_SIZE, THUMB_SIZE);
    let (rect, _) = ui.allocate_exact_size(box_size, egui::Sense::hover());
    ui.painter().rect_filled(rect, WIDGET_RADIUS, BG_PANEL);
    match tex {
        Some(tex) => {
            let tex_size = tex.size_vec2();
            let scale = (rect.width() / tex_size.x).min(rect.height() / tex_size.y);
            let fitted = tex_size * scale;
            let target = egui::Rect::from_center_size(rect.center(), fitted);
            ui.painter().image(
                tex.id(),
                target,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "skip",
                egui::FontId::default(),
                TEXT_MUTED,
            );
        }
    }
    ui.painter().rect_stroke(
        rect,
        WIDGET_RADIUS,
        egui::Stroke::new(1.0f32, LINE),
        egui::StrokeKind::Inside,
    );
}

fn short_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}
