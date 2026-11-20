//! `/cu blockentity` (`te`, `tileentities`, `tileentity`, `be`, `blockentities`):
//! `TileEntityData`, `FindLoadedTEsCommand` and `DeleteTECommand`.
use mod_api::{LevelRef, ServerHost, Text};

use crate::text::{colored, command_line, send};

/// A block entity as `TileEntityData.createLists` keeps it: its type, position and level.
struct Seen {
    kind: String,
    pos: [i32; 3],
    dimension: String,
}

/// The block entities of `levels`, chunk by chunk. The original reads the chunks whose status
/// is `BLOCK_TICKING` or past it; a chunk whose level says its blocks don't tick is left out.
fn seen(host: &mut dyn ServerHost, levels: &[LevelRef]) -> Vec<Seen> {
    let mut out = Vec::new();
    for &level in levels {
        let (Some(dimension), Some((min_y, height))) =
            (level.dimension().id(host), level.height(host))
        else {
            continue;
        };
        let top = min_y.saturating_add(height);
        for chunk in level.loaded_chunks(host).unwrap_or_default() {
            let [x, z] = chunk.pos().map(|c| c * 16);
            // Only the level in place answers; another one's chunks are read as they are.
            if level.should_tick_blocks_at(host, [x + 8, min_y, z + 8]) == Some(false) {
                continue;
            }
            let found = level
                .block_entities(host, [x, min_y, z], [x + 16, top, z + 16])
                .unwrap_or_default();
            out.extend(found.into_iter().map(|at| Seen {
                kind: at.id,
                pos: at.pos,
                dimension: dimension.clone(),
            }));
        }
    }
    out
}

/// `/cu blockentity entityData [dim]`: how many of each type, fewest first.
pub fn list(host: &mut dyn ServerHost, levels: &[LevelRef]) {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for seen in seen(host, levels) {
        match counts.iter_mut().find(|(kind, _)| *kind == seen.kind) {
            Some((_, count)) => *count += 1,
            None => counts.push((seen.kind, 1)),
        }
    }
    counts.sort_by_key(|(_, count)| *count);
    for (kind, count) in counts {
        // `CommandUtils.sendFindTEMessage`; the original never marks one as ticking.
        let line = colored(&kind, "aqua")
            .append(colored(" Count ", "dark_red"))
            .append(colored(&count.to_string(), "green"));
        let command = format!("/cu tileentities find {kind}");
        send(host, command_line(line, None, &command, true));
    }
}

/// `/cu blockentity find <res> [dim]`: where the block entities of type `kind` are, each with
/// a teleport there on a click.
pub fn find(host: &mut dyn ServerHost, levels: &[LevelRef], kind: &str, source: &str) {
    for seen in seen(host, levels).into_iter().filter(|s| s.kind == kind) {
        // `CommandUtils.sendTEMessage`.
        let [x, y, z] = seen.pos;
        let line = colored(&format!(" - [{x},{y},{z}]"), "green");
        let command = format!("/cu tp {source} {x} {y} {z} {}", seen.dimension);
        send(host, command_line(line, None, &command, true));
    }
}

/// `/cu blockentity delete <pos>`: removes the block entity at `pos` of the source's level and
/// leaves its block, saying nothing.
pub fn delete(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) {
    match crate::players::loaded(host, level, pos) {
        Ok(pos) => {
            level.remove_block_entity(host, pos);
        }
        Err(key) => crate::text::fail(host, Text::translate(key, Vec::new())),
    }
}
