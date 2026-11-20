//! Crash Utilities for Horizon: `/crashutilities` (`/cu`) commands for operators to find and fix
//! what breaks a modded world, and an item clear on a timer.
//!
//! A port of Darkere's Crash Utilities (MIT), from its 1.21.1 sources (9.0.4, the version All the
//! Mods 10 ships). This part holds the server commands the mod API carries: entities, block
//! entities, player teleports and inventories of players online, the item clear and help.
mod block_entities;
mod commands;
mod config;
mod entities;
mod item_clear;
mod players;
mod text;

use std::sync::Mutex;

use mod_api::{
    ConfigPath, Flow, Hooks, RegisterCommandsEvent, RunModCommandEvent, ServerHost,
    ServerStartedEvent, ServerTickEvent, ServerTickPhase, Text,
};

use crate::commands::Run;
use crate::config::Config;
use crate::item_clear::{Clock, Due};

#[derive(Default)]
pub struct CrashUtils {
    /// `CrashUtils.SERVER_CONFIG`, read when the server starts.
    config: Mutex<Config>,
    /// The item clear's ticks and the work waiting for them.
    clock: Mutex<Clock>,
}

impl CrashUtils {
    /// Runs the work `what` at a tick's end. No lock is held while the game is asked.
    fn run(&self, host: &mut dyn ServerHost, what: Due) {
        let config = self.config.lock().unwrap().clone();
        let last_count = self.clock.lock().unwrap().last_count;
        let ran = item_clear::run(host, &config, last_count, what);
        self.clock.lock().unwrap().take(ran);
    }
}

#[mod_api::horizon_mod]
impl Hooks for CrashUtils {
    fn server_started(&self, host: &mut dyn ServerHost, _: &mut ServerStartedEvent) {
        let Ok(path) = ConfigPath::new(config::FILE) else {
            return;
        };
        let files = host.config_files();
        let text = match files.read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(_) => {
                // NeoForge writes the defaults when the file is missing.
                let _ = files.write(&path, config::DEFAULT.as_bytes());
                config::DEFAULT.to_owned()
            }
        };
        let config = Config::parse(&text);
        *self.clock.lock().unwrap() = Clock::start(&config);
        *self.config.lock().unwrap() = config;
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend(commands::usages());
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let Some(what) = commands::parse(event.command) else {
            return Flow::Continue;
        };
        if let Run::CallItemClear = what {
            // `ItemClearCommand`: a check now, then its count, both at this tick's end.
            if self.config.lock().unwrap().item_clear {
                let mut clock = self.clock.lock().unwrap();
                clock.next_tick(Due::Check);
                clock.next_tick(Due::Report);
            } else {
                let line = Text::literal("ItemClears are not enabled in the config");
                text::send(host, line);
            }
        } else {
            commands::run(host, event.command, event.source, what);
        }
        Flow::Cancel
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        // `ServerTickEvent.Post`.
        if !matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            return;
        }
        let due = self.clock.lock().unwrap().advance();
        for what in due {
            self.run(host, what);
        }
    }
}
