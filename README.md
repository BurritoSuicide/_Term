# @_Term

Terminal roguelike built with **Rust**, **ratatui**, and **tachyonfx**.

## Run

```bash
cargo run --release
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

## Run loop

Combat rooms with waves → every 3rd combat room is a boss → shop → repeat. Permadeath; seed is shown on the title screen and death overlay.
