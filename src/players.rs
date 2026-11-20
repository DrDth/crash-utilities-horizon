//! `/cu tp`, `/cu unstuck` and `/cu inventory`: `TeleportCommand`, `UnstuckCommand`,
//! `InventoryLookCommand` and `RemoveFromInventorySlotCommand`.
//!
//! The original loads a player who is away from their saved file (`WorldUtils.applyToPlayer`).
//! A mod can't reach that file yet, so these take players who are online.
use mod_api::{Heightmap, LevelRef, PlayerRef, ServerHost, Text};

use crate::text::{colored, command_line, fail, item_display_name, send};

/// The slots `Inventory` keeps per list, by the numbers `player_inventory_slots` uses.
const MAIN: std::ops::Range<u32> = 0..36;
const ARMOR: std::ops::Range<u32> = 36..40;
const OFFHAND: std::ops::Range<u32> = 40..41;

/// `WorldUtils.teleportPlayer`: `player` to `pos` in `level`, onto the surface when `pos` is at
/// height 0, one block east as the original's `teleportTo(x + 1, y, z)`.
fn teleport(host: &mut dyn ServerHost, player: PlayerRef, level: LevelRef, mut pos: [i32; 3]) {
    let Some(dimension) = level.dimension().id(host) else {
        return;
    };
    if pos[1] == 0 {
        // `chunk.getHeight(MOTION_BLOCKING_NO_LEAVES, x, z)`.
        if let Ok(heights) = host.surface_heights(
            &dimension,
            &[(pos[0], pos[2])],
            Heightmap::MotionBlockingNoLeaves,
        ) && let Some(Some(y)) = heights.first()
        {
            pos[1] = *y;
        }
    }
    let to = [f64::from(pos[0] + 1), f64::from(pos[1]), f64::from(pos[2])];
    player
        .entity()
        .teleport_to_level(host, &dimension, to, None);
}

/// `BlockPosArgument.getLoadedBlockPos`, against the source's level: the position, or the
/// translation key of the argument's error.
fn loaded(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Result<[i32; 3], &'static str> {
    let Some((min_y, height)) = level.height(host) else {
        return Err("argument.pos.unloaded");
    };
    // `hasChunkAt` asks for the column; `is_loaded` also wants the height inside the world.
    let inside = pos[1].clamp(min_y, min_y + height - 1);
    if !level.is_loaded(host, [pos[0], inside, pos[2]]) {
        return Err("argument.pos.unloaded");
    }
    let in_world = pos[1] >= min_y
        && i64::from(pos[1]) < i64::from(min_y) + i64::from(height)
        && pos[0].abs() < 30_000_000
        && pos[2].abs() < 30_000_000;
    if !in_world {
        return Err("argument.pos.outofworld");
    }
    Ok(pos)
}

/// `/cu tp <player> <pos> [dim]` and `/cu tp <player> <otherPlayer>`: moves `name` to `pos` in
/// `to` (the source's level unless given), or to `other`'s block in the source's level.
pub fn tp(
    host: &mut dyn ServerHost,
    source_level: LevelRef,
    name: &str,
    pos: Option<[i32; 3]>,
    to: Option<LevelRef>,
    other: Option<&str>,
) {
    let pos = match (pos, other) {
        (Some(pos), _) => match loaded(host, source_level, pos) {
            Ok(pos) => pos,
            Err(key) => return fail(host, Text::translate(key, Vec::new())),
        },
        (None, Some(other)) => {
            let Some(at) = host
                .players()
                .player_by_name(other)
                .and_then(|p| p.entity().on_block(host))
            else {
                return fail(host, Text::literal("Unable to load target players data"));
            };
            at
        }
        (None, None) => return,
    };
    let Some(player) = host.players().player_by_name(name) else {
        return fail(host, Text::literal("Unable to read source player data"));
    };
    teleport(host, player, to.unwrap_or(source_level), pos);
    send(
        host,
        Text::literal(format!(
            "Teleported {name} to {}",
            crate::text::block_pos(pos)
        )),
    );
}

/// `/cu unstuck <name>`: sends the player to the world spawn in the Overworld.
pub fn unstuck(host: &mut dyn ServerHost, name: &str) {
    let player = host.players().player_by_name(name);
    let overworld = host.level("minecraft:overworld");
    let spawn = overworld.and_then(|o| o.respawn_data(host));
    let (Some(player), Some(overworld), Some(spawn)) = (player, overworld, spawn) else {
        return send(
            host,
            Text::literal(format!("Unable to load playerdata for {name}")),
        );
    };
    teleport(host, player, overworld, spawn.pos);
    send(host, Text::literal(format!("Sent Player {name} to Spawn")));
}

/// `/cu inventory read <name>`: the player's offhand, armor and main inventory, each filled slot
/// with a click that puts its removal into the chat field.
pub fn read(host: &mut dyn ServerHost, name: &str) {
    // An unknown player gets no answer, as in the original.
    let Some(player) = host.players().player_by_name(name) else {
        return;
    };
    let slots = player.inventory_slots(host).unwrap_or_default();
    for (title, kind, range) in [
        ("Offhand", "offhand", OFFHAND),
        ("Armor", "armor", ARMOR),
        ("Inventory", "inventory", MAIN),
    ] {
        send(host, colored(title, "dark_aqua"));
        for (slot, stack) in slots.iter().filter(|(slot, _)| range.contains(slot)) {
            let index = slot - range.start;
            // `CommandUtils.sendItemInventoryRemovalMessage`.
            let line =
                colored(&format!("[{index}] "), "dark_blue").append(item_display_name(host, stack));
            let command = format!("/cu inventory remove {name} {kind} {index}");
            send(host, command_line(line, None, &command, false));
        }
    }
}

/// `/cu inventory remove <name> [<slotType>] <slot>`: empties the slot and names what it held.
pub fn remove(host: &mut dyn ServerHost, name: &str, kind: &str, index: i32) {
    let failure = |host: &mut dyn ServerHost| {
        send(
            host,
            Text::literal(format!(
                "Failed to delete item from slot{index}, slot is empty?"
            )),
        )
    };
    let Some(player) = host.players().player_by_name(name) else {
        return failure(host);
    };
    let range = match kind {
        "inventory" => MAIN,
        "armor" => ARMOR,
        "offhand" => OFFHAND,
        // Another type is a Curios slot in the original; without Curios nothing is removed and
        // the command still reports a removal, of nothing.
        _ => {
            return send(
                host,
                Text::literal(format!(" has been deleted from {name}'s InventorySlot")),
            );
        }
    };
    let slot = u32::try_from(index)
        .ok()
        .map(|i| range.start + i)
        .filter(|s| range.contains(s));
    let stack = slot.and_then(|slot| {
        player
            .inventory_slots(host)
            .unwrap_or_default()
            .into_iter()
            .find(|(s, _)| *s == slot)
    });
    let Some((slot, stack)) = stack else {
        return failure(host);
    };
    let held = item_display_name(host, &stack);
    if player.set_inventory_slot(host, slot, None).is_err() {
        return failure(host);
    }
    send(
        host,
        held.append(Text::literal(format!(
            " has been deleted from {name}'s InventorySlot"
        ))),
    );
}
