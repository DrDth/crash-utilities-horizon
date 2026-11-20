//! `/cu entities`: `EntityData` and `RemoveEntitiesCommand`.
use mod_api::{EntityRef, LevelRef, MobCategory, ServerHost, Text};

use crate::text::{block_pos, colored, command_line, failed, field, send};

/// An entity as `WorldPos.getPosFromEntity` keeps it: its type, the block it stands on
/// (`getOnPos`) and its level's dimension.
struct Seen {
    kind: String,
    pos: [i32; 3],
    dimension: String,
}

/// Every entity of `levels`, in their order.
fn seen(host: &mut dyn ServerHost, levels: &[LevelRef]) -> Vec<Seen> {
    let mut out = Vec::new();
    for &level in levels {
        let Some(dimension) = level.dimension().id(host) else {
            continue;
        };
        for entity in level.all_entities(host).unwrap_or_default() {
            let (Some(kind), Some(pos)) = (entity.kind(host), entity.on_block(host)) else {
                continue;
            };
            out.push(Seen {
                kind: kind.id(host),
                pos,
                dimension: dimension.clone(),
            });
        }
    }
    out
}

/// The entries of `items` grouped by `key`, each group in first-seen order, sorted by size
/// (`Comparator.comparingInt` over the original's map: smallest first).
fn grouped<T, K: PartialEq>(items: Vec<T>, key: impl Fn(&T) -> K) -> Vec<(K, Vec<T>)> {
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();
    for item in items {
        let k = key(&item);
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, group)) => group.push(item),
            None => groups.push((k, vec![item])),
        }
    }
    groups.sort_by_key(|(_, group)| group.len());
    groups
}

/// `/cu entities entityData [dim]`: how many of each type, then the total.
pub fn list(host: &mut dyn ServerHost, levels: &[LevelRef]) {
    let all = seen(host, levels);
    let total = all.len();
    for (kind, group) in grouped(all, |s| s.kind.clone()) {
        // `CommandUtils.sendFindEMessage`.
        let line = colored(&group.len().to_string(), "blue")
            .append(colored("x ", "yellow"))
            .append(colored(&kind, "aqua"));
        let command = format!("/cu entities find {kind}");
        send(host, command_line(line, None, &command, true));
    }
    send(host, colored(&format!("{total} Entities"), "dark_aqua"));
}

/// `/cu entities find <res> [dim]`: the chunks holding entities of type `kind`, with a
/// teleport to one of them on a click.
pub fn find(host: &mut dyn ServerHost, levels: &[LevelRef], kind: &str, source: &str) {
    let of_kind: Vec<Seen> = seen(host, levels)
        .into_iter()
        .filter(|s| s.kind == kind)
        .collect();
    // The original's map only has the types it found; another type ends in an exception.
    if of_kind.is_empty() {
        failed(host);
        return;
    }
    send(host, colored(kind, "dark_blue"));
    // `LocationData.fillChunkMaps`: by chunk position (dimensions not told apart), teleporting
    // to the last entity of the chunk.
    for (_, group) in grouped(of_kind, |s| (s.pos[0] >> 4, s.pos[2] >> 4)) {
        let last = group.last().expect("a group has an entry");
        // `CommandUtils.sendChunkEntityMessage`.
        let line = colored(&format!("- {}", block_pos(last.pos)), "green")
            .append(colored(" Count ", "red"))
            .append(colored(&group.len().to_string(), "green"));
        let [x, y, z] = last.pos;
        let command = format!("/cu tp {source} {x} {y} {z} {}", last.dimension);
        send(host, command_line(line, None, &command, true));
    }
}

/// Which entities a `/cu entities remove` takes.
pub enum Removal {
    /// `remove`: every entity without a custom name.
    Unnamed,
    /// `remove <type>`, `remove byType <type>`.
    Kind(String),
    /// `remove items [<name> <force>]`: dropped items, those whose name holds `name`.
    Items(Option<String>),
    /// `remove hostile`: monsters without a custom name.
    Hostile,
    /// `remove regex <regex>`: the types whose id matches it whole.
    Regex(String),
}

/// Whether the entity has a custom name (`Entity.hasCustomName`): its saved `CustomName`.
fn named(host: &mut dyn ServerHost, entity: EntityRef) -> bool {
    entity
        .nbt(host)
        .is_some_and(|nbt| field(&nbt, "CustomName").is_some())
}

/// `RemoveEntitiesCommand`: discards the entities `removal` takes in every level and says how
/// many. Players are never removed: the host refuses them.
pub fn remove(host: &mut dyn ServerHost, removal: Removal) {
    let regex = match &removal {
        Removal::Regex(pattern) => match regex::Regex::new(&format!("^(?:{pattern})$")) {
            Ok(regex) => Some(regex),
            // `String.matches` throws `PatternSyntaxException`.
            Err(_) => {
                failed(host);
                return;
            }
        },
        _ => None,
    };
    let mut removed = 0;
    for level in host.levels().list_levels() {
        for entity in level.all_entities(host).unwrap_or_default() {
            let kind = entity.kind(host).map(|k| k.id(host));
            let take = match &removal {
                Removal::Unnamed => !named(host, entity),
                Removal::Kind(id) => kind.as_deref() == Some(id.as_str()),
                Removal::Items(name) => {
                    host.item_entity(entity).is_some()
                        && name
                            .as_deref()
                            .is_none_or(|name| entity.name(host).is_some_and(|n| n.contains(name)))
                }
                Removal::Hostile => {
                    entity.classification(host) == Some(MobCategory::Monster)
                        && !named(host, entity)
                }
                Removal::Regex(_) => kind
                    .as_deref()
                    .is_some_and(|id| regex.as_ref().is_some_and(|r| r.is_match(id))),
            };
            if take && entity.discard(host) {
                removed += 1;
            }
        }
    }
    send(host, Text::literal(format!("Removed {removed} Entities")));
}
