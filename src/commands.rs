//! `CrashUtils.onRegisterCommands`: `/crashutilities`, its redirect `/cu` and the subcommands'
//! aliases, for operators of level 2 (`CommandUtils.PERMISSION_LEVEL`).
//!
//! Horizon takes a command as usages, not as Brigadier nodes with redirects, so each alias gets
//! the same usages under its own word.
use mod_api::{
    Command, CommandArgumentValue, CommandInvocation, CommandSource, LevelRef, ServerHost, Text,
};

use crate::block_entities;
use crate::entities::{self, Removal};
use crate::players;
use crate::text::send;

/// The command and its redirect.
const ROOTS: [&str; 2] = ["crashutilities", "cu"];
/// `EntitiesCommands` (`e`) and its redirects.
const ENTITIES: [&str; 3] = ["e", "entities", "entity"];
/// `TileEntitiesCommands` (`blockentity`) and its redirects.
const BLOCK_ENTITIES: [&str; 6] = [
    "blockentity",
    "te",
    "tileentities",
    "tileentity",
    "be",
    "blockentities",
];
/// `InventoryCommands` (`i`) and its redirect.
const INVENTORY: [&str; 2] = ["i", "inventory"];
/// `CommandUtils.PERMISSION_LEVEL`.
const LEVEL: u8 = 2;

/// An entity type argument: `ResourceLocationArgument.id()` with the entity types suggested.
const ENTITY_TYPE: &str = "resource:minecraft:entity_type";
/// A block entity type argument, likewise.
const BLOCK_ENTITY_TYPE: &str = "resource:minecraft:block_entity_type";

/// Every usage of the command.
pub fn usages() -> Vec<Command> {
    let mut rest: Vec<String> = [
        "tp <player:word> <pos:block_pos> [dim:dimension]",
        "tp <player:word> <otherPlayer:word>",
        "unstuck <name:word>",
        "callItemClear",
        "help",
    ]
    .map(str::to_owned)
    .to_vec();
    for e in ENTITIES {
        for usage in [
            "entityData [dim:dimension]".to_owned(),
            format!("find <res:{ENTITY_TYPE}> [dim:dimension]"),
            "remove".to_owned(),
            format!("remove byType <type:{ENTITY_TYPE}> [force:word]"),
            format!("remove <type:{ENTITY_TYPE}> [force:word]"),
            // The original's `items <force>` and `items <name> <force>`: Brigadier takes a
            // single word as `force`, so a name only counts with a word after it.
            "remove items [name:word] [force:word]".to_owned(),
            "remove hostile [force:word]".to_owned(),
            "remove regex <regex:text>".to_owned(),
        ] {
            rest.push(format!("{e} {usage}"));
        }
    }
    for b in BLOCK_ENTITIES {
        for usage in [
            "entityData [dim:dimension]".to_owned(),
            format!("find <res:{BLOCK_ENTITY_TYPE}> [dim:dimension]"),
            "delete <pos:block_pos>".to_owned(),
        ] {
            rest.push(format!("{b} {usage}"));
        }
    }
    for i in INVENTORY {
        for usage in [
            "read <name:word>",
            "remove <name:word> <slotType:word> <slot:integer>",
            "remove <name:word> <slot:integer>",
        ] {
            rest.push(format!("{i} {usage}"));
        }
    }
    ROOTS
        .iter()
        .flat_map(|root| {
            rest.iter()
                .map(move |r| Command::op(&format!("{root} {r}"), LEVEL))
        })
        .collect()
}

/// `HelpCommand`: the original's lines, as it has them.
const HELP: [&str; 22] = [
    "/cu entities",
    "   entityData: Lists all loaded Entities on the Server",
    "   find (name): Finds all Entities of this type on the Server",
    "   remove: (<force> removes the entities immediately without waiting for their next tick) ",
    "       byType (name) <force>: removes all entities of that type",
    "       hostile <force>: removes all entities that are tagged as hostile",
    "       regex (regex) <force>: removes all entities whose registry key matches this regex",
    "/cu tileentities",
    "   entityData: entityData all loaded Tile Entities on the Server",
    "   find (name): entityData all loaded Tile Entities of this type",
    "   remove (pos): remove the Tile Entity at this position (does not remove block!)",
    "/cu inventory",
    "   read (name): reads out the Inventory of this player",
    "   remove (name) <slot type>(slot): removes the item from the slot of this player",
    "   open (name): opens the inventory of a player",
    "   enderchest (name): opens the enderchest of a player",
    "/cu activity: entityData playeractivity over the last days,week,month",
    "/cu tp (name)",
    "   (pos) (dim): teleport to position in dimension",
    "   (name): teleport to player",
    "/cu log: post links to copying or uploading logs",
    "/cu chunks: report all loaded chunks",
];

/// What a parsed `/cu` asks for, ready to run.
pub enum Run {
    Help,
    CallItemClear,
    Tp {
        name: String,
        pos: Option<[i32; 3]>,
        to: Option<LevelRef>,
        other: Option<String>,
    },
    Unstuck(String),
    EntityList(Option<LevelRef>),
    EntityFind(String, Option<LevelRef>),
    EntityRemove(Removal),
    BlockEntityList(Option<LevelRef>),
    BlockEntityFind(String, Option<LevelRef>),
    BlockEntityDelete([i32; 3]),
    InventoryRead(String),
    InventoryRemove(String, String, i32),
}

/// The string of argument `name`; an id as `namespace:path`.
fn string(command: &CommandInvocation, name: &str) -> Option<String> {
    match command.argument(name)? {
        CommandArgumentValue::String(s) => Some(s.clone()),
        CommandArgumentValue::Id(id) => Some(id.to_string()),
        _ => None,
    }
}

/// The level of `dimension` argument `name`.
fn dimension(command: &CommandInvocation, name: &str) -> Option<LevelRef> {
    match command.argument(name)? {
        CommandArgumentValue::Dimension(level) => Some(*level),
        _ => None,
    }
}

/// What `command` asks for, None when it is no `/cu` usage.
pub fn parse(command: &CommandInvocation) -> Option<Run> {
    let literals: Vec<&str> = command.literals.iter().map(String::as_str).collect();
    let [root, rest @ ..] = literals.as_slice() else {
        return None;
    };
    if !ROOTS.contains(root) {
        return None;
    }
    Some(match rest {
        ["help"] => Run::Help,
        ["callItemClear"] => Run::CallItemClear,
        ["tp"] => Run::Tp {
            name: string(command, "player")?,
            pos: match command.argument("pos") {
                Some(CommandArgumentValue::BlockPos(pos)) => Some(*pos),
                _ => None,
            },
            to: dimension(command, "dim"),
            other: string(command, "otherPlayer"),
        },
        ["unstuck"] => Run::Unstuck(string(command, "name")?),
        [e, rest @ ..] if ENTITIES.contains(e) => match rest {
            ["entityData"] => Run::EntityList(dimension(command, "dim")),
            ["find"] => Run::EntityFind(string(command, "res")?, dimension(command, "dim")),
            ["remove"] => Run::EntityRemove(match string(command, "type") {
                Some(kind) => Removal::Kind(kind),
                None => Removal::Unnamed,
            }),
            ["remove", "byType"] => Run::EntityRemove(Removal::Kind(string(command, "type")?)),
            ["remove", "items"] => Run::EntityRemove(Removal::Items(
                string(command, "force").and(string(command, "name")),
            )),
            ["remove", "hostile"] => Run::EntityRemove(Removal::Hostile),
            ["remove", "regex"] => Run::EntityRemove(Removal::Regex(string(command, "regex")?)),
            _ => return None,
        },
        [b, rest @ ..] if BLOCK_ENTITIES.contains(b) => match rest {
            ["entityData"] => Run::BlockEntityList(dimension(command, "dim")),
            ["find"] => Run::BlockEntityFind(string(command, "res")?, dimension(command, "dim")),
            ["delete"] => match command.argument("pos")? {
                CommandArgumentValue::BlockPos(pos) => Run::BlockEntityDelete(*pos),
                _ => return None,
            },
            _ => return None,
        },
        [i, rest @ ..] if INVENTORY.contains(i) => match rest {
            ["read"] => Run::InventoryRead(string(command, "name")?),
            ["remove"] => Run::InventoryRemove(
                string(command, "name")?,
                string(command, "slotType").unwrap_or_else(|| "inventory".to_owned()),
                match command.argument("slot")? {
                    CommandArgumentValue::Integer(slot) => *slot,
                    _ => return None,
                },
            ),
            _ => return None,
        },
        _ => return None,
    })
}

/// The levels a `[dim]` names: that one, else every level (`getWorldsFromDimensionArgument`).
fn levels(host: &mut dyn ServerHost, level: Option<LevelRef>) -> Vec<LevelRef> {
    match level {
        Some(level) => vec![level],
        None => host.levels().list_levels(),
    }
}

/// The name the original puts into its teleport clicks: the player's, else `Console`.
fn source_name(host: &dyn ServerHost, source: &CommandSource) -> String {
    match source {
        CommandSource::Player { player, .. } => player.name(host).unwrap_or_default(),
        _ => "Console".to_owned(),
    }
}

/// Runs a parsed `/cu` other than `callItemClear`, which needs the item clear's clock.
pub fn run(
    host: &mut dyn ServerHost,
    command: &CommandInvocation,
    source: &CommandSource,
    what: Run,
) {
    match what {
        Run::Help => {
            for line in HELP {
                send(host, Text::literal(line));
            }
        }
        Run::CallItemClear => {}
        Run::Tp {
            name,
            pos,
            to,
            other,
        } => {
            let Some(source_level) = command.dimension.id(host).and_then(|id| host.level(&id))
            else {
                return;
            };
            players::tp(host, source_level, &name, pos, to, other.as_deref());
        }
        Run::Unstuck(name) => players::unstuck(host, &name),
        Run::EntityList(level) => {
            let levels = levels(host, level);
            entities::list(host, &levels);
        }
        Run::EntityFind(kind, level) => {
            let levels = levels(host, level);
            let source = source_name(host, source);
            entities::find(host, &levels, &kind, &source);
        }
        Run::EntityRemove(removal) => entities::remove(host, removal),
        Run::BlockEntityList(level) => {
            let levels = levels(host, level);
            block_entities::list(host, &levels);
        }
        Run::BlockEntityFind(kind, level) => {
            let levels = levels(host, level);
            let source = source_name(host, source);
            block_entities::find(host, &levels, &kind, &source);
        }
        Run::BlockEntityDelete(pos) => {
            if let Some(level) = command.dimension.id(host).and_then(|id| host.level(&id)) {
                block_entities::delete(host, level, pos);
            }
        }
        Run::InventoryRead(name) => players::read(host, &name),
        Run::InventoryRemove(name, kind, slot) => players::remove(host, &name, &kind, slot),
    }
}
