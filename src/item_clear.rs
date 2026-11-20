//! `ClearItemTask`: every `timer` minutes, when the Overworld holds `maximum` dropped items or
//! more, warn in chat and clear them once the last warning has run out.
//!
//! The original's `java.util.Timer`s count wall-clock time and hand the work to the next server
//! tick. A wasm mod has no clock of its own, so the port counts server ticks, 20 a second: a
//! server that falls behind waits that much longer.
use mod_api::{EntityRef, ServerHost, Text};

use crate::config::Config;
use crate::text::colored;

/// Server ticks a second.
const TICKS_PER_SECOND: u64 = 20;

/// Work the clock hands to a server tick's end, as the original's `runNextTick` does.
#[derive(Clone, Debug, PartialEq)]
pub enum Due {
    /// `runClear`: count the items, and start the warnings past the maximum.
    Check,
    /// `/cu callItemClear`'s answer after its check.
    Report,
    /// A warning, `seconds` before the clear.
    Warning(i32),
    /// The clear after the last warning.
    Clear,
}

/// The ticks counted since the server started and the work waiting for them.
#[derive(Debug, Default)]
pub struct Clock {
    tick: u64,
    /// Ticks between checks, None while item clears are off.
    every: Option<u64>,
    next_check: u64,
    due: Vec<(u64, Due)>,
    /// `ClearItemTask.lastCount`.
    pub last_count: usize,
}

impl Clock {
    /// `ClearItemTask.start`: a fresh clock, checking every `timer` minutes (ten seconds at 0)
    /// when item clears are on and the warnings read.
    pub fn start(config: &Config) -> Clock {
        let mut clock = Clock::default();
        if config.item_clear && config.warnings.is_some() {
            let millis = match config.timer as u64 * 60 * 1000 {
                0 => 10_000,
                millis => millis,
            };
            let every = (millis / 50).max(1);
            clock.every = Some(every);
            clock.next_check = every;
        }
        clock
    }

    /// Hands `what` to the end of the current tick.
    pub fn next_tick(&mut self, what: Due) {
        self.due.push((self.tick + 1, what));
    }

    /// Hands `what` to the tick `seconds` from now (the current tick's end at 0).
    fn after(&mut self, seconds: i32, what: Due) {
        let ticks = u64::try_from(seconds).unwrap_or(0) * TICKS_PER_SECOND;
        self.due.push((self.tick + ticks.max(1), what));
    }

    /// A tick ended: the work due now, in the order it was handed over.
    pub fn advance(&mut self) -> Vec<Due> {
        self.tick += 1;
        if let Some(every) = self.every
            && self.tick >= self.next_check
        {
            self.next_check += every;
            self.due.push((self.tick, Due::Check));
        }
        let tick = self.tick;
        let (now, later) = std::mem::take(&mut self.due)
            .into_iter()
            .partition(|(at, _)| *at <= tick);
        self.due = later;
        now.into_iter().map(|(_, what)| what).collect()
    }
}

/// The Overworld's dropped items (`EntityType.ITEM`): the original counts the level its
/// `runNextTick` hands over, the Overworld.
fn overworld_items(host: &mut dyn ServerHost) -> Vec<EntityRef> {
    let Some(level) = host.level("minecraft:overworld") else {
        return Vec::new();
    };
    let entities = level.all_entities(host).unwrap_or_default();
    entities
        .into_iter()
        .filter(|e| host.item_entity(*e).is_some())
        .collect()
}

/// A line in every player's chat and the log (`broadcastSystemMessage`).
fn broadcast(host: &mut dyn ServerHost, line: Text) {
    let _ = host.messages().broadcast(line);
}

/// The chat warning `seconds` before the clear: the first `%` of the text replaced, in red,
/// between `[=== ` and ` ===]`.
fn warn(host: &mut dyn ServerHost, config: &Config, seconds: i32) {
    let text = config.warning_text.replacen('%', &seconds.to_string(), 1);
    let line = Text::literal("[=== ")
        .append(colored(&text, "red"))
        .append(Text::literal(" ===]"));
    broadcast(host, line);
}

/// What running a piece of work leaves for the clock.
#[derive(Debug, Default)]
pub struct Ran {
    /// The items a check counted.
    pub counted: Option<usize>,
    /// Work for later, in seconds from now.
    pub later: Vec<(i32, Due)>,
}

impl Clock {
    /// Takes what a run left.
    pub fn take(&mut self, ran: Ran) {
        if let Some(count) = ran.counted {
            self.last_count = count;
        }
        for (seconds, what) in ran.later {
            self.after(seconds, what);
        }
    }
}

/// Runs `what` at the end of a tick; `last_count` is the last check's count.
pub fn run(host: &mut dyn ServerHost, config: &Config, last_count: usize, what: Due) -> Ran {
    let mut ran = Ran::default();
    let Some(warnings) = config.warnings.as_ref() else {
        return ran;
    };
    match what {
        Due::Check => {
            let count = overworld_items(host).len();
            ran.counted = Some(count);
            if count < config.maximum as usize {
                return ran;
            }
            let Some(&last) = warnings.last() else {
                return ran;
            };
            for &warning in warnings {
                if warning == last {
                    ran.later.push((warning, Due::Clear));
                    // Its timer has no delay: it runs at once, before anything after the check.
                    warn(host, config, warning);
                } else {
                    ran.later.push((last - warning, Due::Warning(warning)));
                }
            }
            if config.title && !host.players().server_players().is_empty() {
                let title = json_string(&config.title_text);
                host.commands().run_command(&format!(
                    "title @a title {{\"text\":{title},\"color\":\"dark_red\"}}"
                ));
            }
        }
        Due::Report => broadcast(
            host,
            Text::literal(format!(
                "{last_count} Item Entities in World. Limit is set to {}",
                config.maximum
            )),
        ),
        Due::Warning(seconds) => warn(host, config, seconds),
        Due::Clear => {
            let items = overworld_items(host);
            let size = items.len();
            let line = if size > config.maximum as usize {
                for item in items {
                    item.discard(host);
                }
                format!("{size} Items cleared")
            } else {
                format!("Item Clear prevented. Only {size} items on the ground")
            };
            broadcast(host, Text::literal(line));
        }
    }
    ran
}

/// `text` as a JSON string. The original pastes the title text between quotes as it is; quoting
/// it keeps a `"` in the text from breaking the command.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(timer: i32) -> Config {
        Config {
            item_clear: true,
            timer,
            ..Config::default()
        }
    }

    #[test]
    fn the_check_comes_every_timer_minutes() {
        let mut clock = Clock::start(&on(1));
        let mut checks = Vec::new();
        for tick in 1..=2400 {
            if clock.advance().contains(&Due::Check) {
                checks.push(tick);
            }
        }
        assert_eq!(checks, [1200, 2400]);
        let mut off = Clock::start(&Config::default());
        assert!((0..5000).all(|_| off.advance().is_empty()));
    }

    #[test]
    fn a_timer_of_0_checks_every_ten_seconds() {
        let mut clock = Clock::start(&on(0));
        let first = (1..).find(|_| clock.advance().contains(&Due::Check));
        assert_eq!(first, Some(200));
    }

    #[test]
    fn work_handed_over_runs_at_the_ticks_end_in_order() {
        let mut clock = Clock::start(&Config::default());
        clock.next_tick(Due::Check);
        clock.next_tick(Due::Report);
        assert_eq!(clock.advance(), [Due::Check, Due::Report]);
        // One second: 20 ticks; none: the next tick's end.
        clock.after(1, Due::Warning(5));
        clock.after(0, Due::Warning(6));
        clock.after(1, Due::Clear);
        assert_eq!(clock.advance(), [Due::Warning(6)]);
        let mut later = Vec::new();
        for _ in 0..19 {
            later.extend(clock.advance());
        }
        assert_eq!(later, [Due::Warning(5), Due::Clear]);
    }
}
