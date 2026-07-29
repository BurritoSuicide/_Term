# @_Term

Terminal roguelike built with **Rust**, **ratatui**, and **tachyonfx**.

Weave wand decks, raise corpses into an army that mirrors your casts, and clear procedural rooms until the boss — then spend gold in the shop.

## Run

```bash
cargo run
```

## Controls

| Key | Action |
|-----|--------|
| WASD | Move |
| Arrow keys | Aim |
| Space | Cast wand |
| Shift | Dash |
| R | Resurrect nearest corpse (costs credit) |
| Tab | Toggle Explore / Inventory (pauses) |
| Esc | Pause (Exit) |
| Enter | Advance through open door / buy in shop |
| c | Leave shop |
| f | Inventory: focus wand ↔ skills |
| h/l | Inventory: reorder wand slots |
| j/k | Inventory / shop cursor |
| q | Quit |

## Progression

Drop rarities (approx): Common 50% · Uncommon 30% · Rare 15% · Legendary 2% · Mythical 0.5%.

Critical hits: 1% of player shots deal **10×** damage (logged in `journalctl`).

## Spell ideas (backlog)

- **Mirror Maw** — swallow a hostile shot and spit it back (Rare)
- **Gravity Well** — pull enemies toward the impact point (Legendary)
- **Bookmark** — place a recall point; recast returns you there (Rare)
- **Paper Cut** — thin high-pierce line that leaves bleeding DoT (Uncommon)
- **Haunt** — corpse detonates after a delay when you cast (Legendary)
- **Static Cling** — next shot sticks to walls and zaps passers-by (Uncommon)
- **Overclock** — next cast is free but burns HP (Chaos / Rare)
- **Shepherd’s Crook** — yanks nearest minion to your aim tip (Common utility)
- **Ink Blot** — blinds shooters briefly (no damage) (Uncommon)
- **Quicksilver** — convert remaining mana into a single super bolt (Mythical)

## Run loop

Combat rooms with waves → every 3rd combat room is a boss → shop → repeat. Permadeath; seed is shown on the title screen and death overlay.
