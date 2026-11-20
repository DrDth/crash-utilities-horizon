//! `CommandUtils`: the original's chat lines.
use mod_api::text::{TextClick, TextStyle};
use mod_api::{Compound, Host, ItemStack, ServerHost, Tag, Text, TextHover};

/// `CommandUtils.coloredComponent`: `text` in `color`.
pub fn colored(text: &str, color: &str) -> Text {
    Text::literal(text).with_style(color_style(color))
}

/// A style with only `color` set.
pub fn color_style(color: &str) -> TextStyle {
    let mut style = TextStyle::default();
    style.color = Some(color.to_owned());
    style
}

/// `sendSuccess`: a line to the command's source.
pub fn send(host: &mut dyn ServerHost, line: Text) {
    let _ = host.commands().feedback(line);
}

/// `sendFailure`: the line in red.
pub fn fail(host: &mut dyn ServerHost, line: Text) {
    send(
        host,
        Text::literal("")
            .append(line)
            .with_style(color_style("red")),
    );
}

/// A command that failed inside, as Brigadier answers an exception: `command.failed` in red.
pub fn failed(host: &mut dyn ServerHost) {
    fail(host, Text::translate("command.failed", Vec::new()));
}

/// `CommandUtils.sendCommandMessage`: `text`, colored `color`, running `command` on a click
/// (`run`) or putting it into the chat field, with the command on hover.
pub fn command_line(text: Text, color: Option<&str>, command: &str, run: bool) -> Text {
    let mut style = TextStyle::default();
    style.color = color.map(str::to_owned);
    style.click = Some(if run {
        TextClick::RunCommand(command.to_owned())
    } else {
        TextClick::SuggestCommand(command.to_owned())
    });
    // The original's literal keeps its legacy codes: the command in gold.
    style.hover = Some(TextHover::Text(Box::new(Text::literal(format!(
        "Click to execute \u{a7}6{command}\u{a7}r"
    )))));
    Text::literal("").append(text).with_style(style)
}

/// Java's `BlockPos.toString`.
pub fn block_pos(pos: [i32; 3]) -> String {
    format!("BlockPos{{x={}, y={}, z={}}}", pos[0], pos[1], pos[2])
}

/// The value of `key` in `compound`.
pub fn field<'a>(compound: &'a Compound, key: &str) -> Option<&'a Tag> {
    compound
        .iter()
        .find(|(k, _)| k.to_string_lossy() == key)
        .map(|(_, v)| v)
}

/// A text component as item components store it, as text: plain strings, `text`, `translate`
/// with its `with`, and `extra`. Styles are left out.
fn stored_text(tag: &Tag) -> Text {
    match tag {
        Tag::String(s) => Text::literal(s.to_string_lossy()),
        Tag::List(parts) => {
            let mut parts = parts.iter().map(stored_text);
            let first = parts.next().unwrap_or_default();
            parts.fold(first, Text::append)
        }
        Tag::Compound(c) => {
            let mut text = match (field(c, "translate"), field(c, "text")) {
                (Some(Tag::String(key)), _) => {
                    let with = match field(c, "with") {
                        Some(Tag::List(args)) => args.iter().map(stored_text).collect(),
                        _ => Vec::new(),
                    };
                    Text::translate(key.to_string_lossy(), with)
                }
                (_, Some(Tag::String(s))) => Text::literal(s.to_string_lossy()),
                _ => Text::literal(""),
            };
            if let Some(Tag::List(extra)) = field(c, "extra") {
                for part in extra {
                    text = text.append(stored_text(part));
                }
            }
            text
        }
        _ => Text::literal(""),
    }
}

/// `ItemStack.getDisplayName`: the hover name in square brackets, in the rarity's colour (italic
/// when custom), showing the item on hover.
pub fn item_display_name(host: &dyn Host, stack: &ItemStack) -> Text {
    let item = stack.item();
    let id = item.id(host);
    let components = stack
        .to_nbt(host)
        .and_then(|nbt| match field(&nbt, "components") {
            Some(Tag::Compound(c)) => Some(c.clone()),
            _ => None,
        });
    let own = |key: &str| components.as_ref().and_then(|c| field(c, key)).cloned();
    let component = |key: &str| own(key).or_else(|| host.item_default_component(item, key));
    let custom = own("minecraft:custom_name");
    let mut name = match custom
        .as_ref()
        .or(component("minecraft:item_name").as_ref())
    {
        Some(tag) => stored_text(tag),
        None => Text::literal(id.clone()),
    };
    if custom.is_some() {
        let mut style = TextStyle::default();
        style.italic = Some(true);
        name = Text::literal("").append(name).with_style(style);
    }
    // `ItemStack.getRarity`: an enchanted stack's rarity goes up a step, to rare at least.
    let rarity = match component("minecraft:rarity") {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => "common".to_owned(),
    };
    let enchanted =
        matches!(own("minecraft:enchantments"), Some(Tag::Compound(e)) if !e.is_empty());
    let rarity = match (rarity.as_str(), enchanted) {
        ("common" | "uncommon", true) => "rare",
        ("rare", true) => "epic",
        (r, _) => match r {
            "uncommon" => "uncommon",
            "rare" => "rare",
            "epic" => "epic",
            _ => "common",
        },
    };
    let mut style = color_style(match rarity {
        "uncommon" => "yellow",
        "rare" => "aqua",
        "epic" => "light_purple",
        _ => "white",
    });
    style.hover = Some(TextHover::Item {
        id,
        count: stack.count(),
        components: components.filter(|c| !c.is_empty()).map(Tag::Compound),
    });
    Text::translate("chat.square_brackets", vec![name]).with_style(style)
}
