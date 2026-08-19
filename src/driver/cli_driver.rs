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

use std::io::{self, Write};

use thiserror::Error;

use crate::adapter::steam_adapter::{self, SteamShortcuts};
use crate::controller::import_controller::{
    is_weak_match, ControllerError, DefaultImportController,
};
use crate::types::game_types::Game;
use crate::types::shortcut_types::{ArtworkSlot, GameProposal, GameSelection};

#[derive(Debug, Error)]
pub enum DriverError {
    #[error(transparent)]
    Controller(#[from] ControllerError),
    #[error("talking to steam: {0}")]
    Steam(#[from] steam_adapter::AdapterError),
    #[error("reading input: {0}")]
    Input(#[from] io::Error),
}

pub struct CliArgs {
    pub xboxgames_root: String,
    pub shortcuts_vdf_path: String,
    pub grid_dir: String,
    pub state_path: String,
    pub dry_run: bool,
}

pub struct CliDriver {
    pub controller: DefaultImportController,
    pub steam: Box<dyn SteamShortcuts>,
}

impl CliDriver {
    pub fn run(&self, args: &CliArgs) -> Result<(), DriverError> {
        let proposals = self.controller.scan(
            &args.xboxgames_root,
            &args.shortcuts_vdf_path,
            &args.state_path,
            false,
        )?;

        if proposals.is_empty() {
            println!("no new games found under {}", args.xboxgames_root);
            return Ok(());
        }

        println!("found {} new game(s):", proposals.len());
        for (i, proposal) in proposals.iter().enumerate() {
            println!("  {}. {}", i + 1, proposal.game.name);
        }

        let mut selections = Vec::new();
        for proposal in &proposals {
            let selection = review_game(proposal)?;
            if let Some(selection) = selection {
                selections.push(selection);
            }
        }

        if selections.is_empty() {
            println!("no games accepted, nothing to write");
            return Ok(());
        }

        if args.dry_run {
            println!(
                "dry run: would write {} game(s), stopping here",
                selections.len()
            );
            return Ok(());
        }

        wait_for_steam_closed(self.steam.as_ref())?;

        let imported: Vec<(Game, String)> = selections
            .iter()
            .map(|s| (s.game.clone(), s.display_name.clone()))
            .collect();
        let built = self
            .controller
            .build_entries(&selections, &std::collections::HashMap::new());

        let mut existing = self
            .steam
            .read_shortcuts(&args.shortcuts_vdf_path)
            .map_err(DriverError::Steam)?;
        for built_game in &built {
            existing.push(steam_adapter::shortcut_entry_to_vdf(&built_game.entry));
            for (slot, path) in &built_game.artwork {
                let Some(path) = path else { continue };
                self.steam
                    .place_grid_image(&args.grid_dir, built_game.entry.appid, *slot, path)
                    .map_err(DriverError::Steam)?;
            }
        }
        self.steam
            .write_shortcuts(&args.shortcuts_vdf_path, &existing)
            .map_err(DriverError::Steam)?;

        self.controller.mark_imported(&args.state_path, &imported)?;

        println!(
            "done: added {} game(s) to {}",
            built.len(),
            args.shortcuts_vdf_path
        );
        Ok(())
    }
}

fn wait_for_steam_closed(steam: &dyn SteamShortcuts) -> Result<(), DriverError> {
    if !steam.is_steam_running()? {
        return Ok(());
    }
    println!("steam is running. close it, then press enter.");
    const MAX_RETRIES: u32 = 30;
    for _ in 0..MAX_RETRIES {
        read_line()?;
        if !steam.is_steam_running()? {
            return Ok(());
        }
        println!("steam still running. close it, then press enter.");
    }
    println!("steam still running after several checks, writing anyway");
    Ok(())
}

fn review_game(proposal: &GameProposal) -> Result<Option<GameSelection>, DriverError> {
    println!();
    println!("{}", proposal.game.name);
    for slot_candidates in &proposal.slots {
        print_slot_summary(slot_candidates);
    }
    print!("[enter] accept defaults, s skip game, r review slots: ");
    io::stdout().flush()?;
    let input = read_line()?;

    match input.trim() {
        "s" | "S" => Ok(None),
        "r" | "R" => Ok(Some(review_slots(proposal)?)),
        _ => Ok(Some(GameSelection {
            game: proposal.game.clone(),
            display_name: proposal.suggested_name.clone(),
            previous_app_name: previous_app_name(proposal),
            chosen_paths: proposal
                .slots
                .iter()
                .map(|sc| (sc.slot, sc.ranked.first().map(|c| c.candidate.path.clone())))
                .collect(),
        })),
    }
}

fn previous_app_name(proposal: &GameProposal) -> Option<String> {
    proposal
        .already_imported
        .then(|| proposal.suggested_name.clone())
}

fn print_slot_summary(slot_candidates: &crate::types::shortcut_types::SlotCandidates) {
    match slot_candidates.ranked.first() {
        None => println!(
            "  {}: no candidate found, will skip",
            slot_candidates.slot.label()
        ),
        Some(top) => {
            let weak = if is_weak_match(top.score) {
                " (weak match)"
            } else {
                ""
            };
            println!(
                "  {}: {} ({}x{}){}",
                slot_candidates.slot.label(),
                top.candidate.path,
                top.candidate.width,
                top.candidate.height,
                weak
            );
        }
    }
}

fn review_slots(proposal: &GameProposal) -> Result<GameSelection, DriverError> {
    let mut chosen: Vec<(ArtworkSlot, Option<String>)> = proposal
        .slots
        .iter()
        .map(|sc| (sc.slot, sc.ranked.first().map(|c| c.candidate.path.clone())))
        .collect();

    loop {
        println!();
        for (i, sc) in proposal.slots.iter().enumerate() {
            let current = chosen[i]
                .1
                .clone()
                .unwrap_or_else(|| "(skipped)".to_string());
            println!("  {}. {}: {}", i + 1, sc.slot.label(), current);
        }
        print!("slot number to change, or [enter] to continue: ");
        io::stdout().flush()?;
        let input = read_line()?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }
        let Ok(index) = trimmed.parse::<usize>() else {
            println!("not a number");
            continue;
        };
        if index == 0 || index > proposal.slots.len() {
            println!("out of range");
            continue;
        }
        let slot_candidates = &proposal.slots[index - 1];
        chosen[index - 1].1 = choose_alternate(slot_candidates)?;
    }

    Ok(GameSelection {
        game: proposal.game.clone(),
        display_name: proposal.suggested_name.clone(),
        previous_app_name: previous_app_name(proposal),
        chosen_paths: chosen,
    })
}

fn choose_alternate(
    slot_candidates: &crate::types::shortcut_types::SlotCandidates,
) -> Result<Option<String>, DriverError> {
    if slot_candidates.ranked.is_empty() {
        println!("no candidates for {}", slot_candidates.slot.label());
        return Ok(None);
    }
    for (i, c) in slot_candidates.ranked.iter().enumerate() {
        println!(
            "    {}. {} ({}x{})",
            i + 1,
            c.candidate.path,
            c.candidate.width,
            c.candidate.height
        );
    }
    print!("    pick number, c for custom path, x to skip: ");
    io::stdout().flush()?;
    let input = read_line()?;
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("x") {
        return Ok(None);
    }
    if trimmed.eq_ignore_ascii_case("c") {
        print!("    path: ");
        io::stdout().flush()?;
        let path = read_line()?;
        return Ok(Some(path.trim().to_string()));
    }
    match trimmed.parse::<usize>() {
        Ok(i) if i >= 1 && i <= slot_candidates.ranked.len() => {
            Ok(Some(slot_candidates.ranked[i - 1].candidate.path.clone()))
        }
        _ => {
            println!("not a valid choice, keeping current");
            Ok(slot_candidates
                .ranked
                .first()
                .map(|c| c.candidate.path.clone()))
        }
    }
}

fn read_line() -> Result<String, io::Error> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line)
}
