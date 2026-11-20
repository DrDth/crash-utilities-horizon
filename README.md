# Crash Utilities for Horizon

A port of [Crash Utilities](https://github.com/Darkere/CrashUtilities) by Darkere to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3. Built
as a wasm mod from the original's 1.21.1 sources (9.0.4), the version All the Mods 10 ships.

Operators (level 2) get `/crashutilities`, or `/cu`, to find and fix what breaks a modded world:

- `/cu entities entityData [dim]` counts the loaded entities by type; `/cu entities find <type> [dim]` lists the
  chunks holding a type, with a teleport there on a click.
- `/cu entities remove` removes entities: all without a custom name, `<type>` or `byType <type>`, `items`
  (`items <name> force` those whose name holds `<name>`), `hostile` (monsters without a custom name), or
  `regex <regex>` (types whose id matches).
- `/cu blockentity entityData [dim]` counts the loaded block entities by type; `/cu blockentity find <type> [dim]`
  lists where they are, with a teleport there on a click; `/cu blockentity delete <pos>` removes the block entity at
  a position and leaves its block, for one whose tick crashes the world.
- `/cu inventory read <player>` lists a player's offhand, armor and inventory; a click on a slot puts
  `/cu inventory remove <player> <slotType> <slot>` into the chat field, which empties it.
- `/cu tp <player> <pos> [dim]` and `/cu tp <player> <otherPlayer>` move a player, `/cu unstuck <player>` sends
  one to the world spawn.
- The item clear: with `enabled = true` in `config/crashutilities-server.toml`, the Overworld's dropped items are
  counted every `timer` minutes; past `maximum`, chat warnings count down and the items go. `/cu callItemClear`
  runs a check now.
- `/cu help`.

`entities`, `entity` and `e` are the same command; so are `blockentity`, `blockentities`, `tileentity`,
`tileentities`, `te` and `be`, and `inventory` and `i`. The mod is server side only.

## Not yet ported

These need parts Horizon's mod API doesn't have yet:

- `/cu chunks`: chunk load states and tickets.
- `/cu activity`: when players were last online.
- `/cu memoryCheck` and the memory checker.
- `/cu inventory open` and `/cu inventory enderchest`, and every command on a player who is offline.
- The screen the original opens with a key (chunk map, entity and block entity lists, player inventories).
- Curios slots in `/cu inventory`, and the FTB Chunks unloading.

## Differences from the original

- The settings live in the game's `config/crashutilities-server.toml`, one for all worlds. The original keeps them
  in NeoForge's server config of the same name. Only the Item Clear section is read.
- The item clear counts server ticks, 20 a second, where the original counts wall-clock time: a server that falls
  behind waits longer.
- A command's answer goes to the one who ran it. The original also shows it to the other operators.
- `/cu entities remove` leaves players alone. The original's `remove` without a type calls `remove` on them too.
- `/cu unstuck` on a player who is online says "Sent Player ... to Spawn". The original sends them and then fails
  with an internal error.
- `regex` is Rust's regular expression syntax: no look-around or back references.

## Building

The mod needs three additions to Horizon's mod API: command words with uppercase letters (`callItemClear`,
`entityData`, `byType`; [horizon-rust/horizon#19041](https://github.com/horizon-rust/horizon/pull/19041)) and the
block entities in a box ([#19048](https://github.com/horizon-rust/horizon/pull/19048)), both merged, and removing a
block entity while keeping its block ([#19052](https://github.com/horizon-rust/horizon/pull/19052)). Until a Horizon
release has them all, build the mod inside a Horizon checkout that does:

```sh
mkdir -p <horizon>/.local/dev-mods/crash_utilities
cp -R Cargo.toml horizon-mod.toml src LICENSE <horizon>/.local/dev-mods/crash_utilities/
cd <horizon>/.local/dev-mods/crash_utilities
cargo run --manifest-path ../../../Cargo.toml -p cargo-horizon-mod -- horizon-mod build --wasm
```

## License

MIT, as the original. Copyright (c) 2020 Darkere; the Horizon port by DrDth. See [LICENSE](LICENSE).
