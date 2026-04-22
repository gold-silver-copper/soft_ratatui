# Extra Example Apps

The apps in this folder are now managed as Cargo workspace members from the repository root.

## Run an app

Run these from `/Users/kisaczka/Desktop/programming/soft_ratatui`:

```bash
cargo run -p soft-ratatui-bevy-sprite
cargo run -p soft-ratatui-bevy-parley-cjk
cargo run -p soft-ratatui-bevy-parley-colors
cargo run -p soft-ratatui-bevy-parley-modifiers
cargo run -p soft-ratatui-bevy-cosmic-cjk
cargo run -p soft-ratatui-bevy-bdf-cjk
cargo run -p soft-ratatui-bevy-cube
cargo run -p soft-ratatui-bevy-cube-colors
cargo run -p soft-ratatui-bevy-demo
cargo run -p soft-ratatui-egui-colors
cargo run -p soft-ratatui-egui-parley-colors-rgb
cargo run -p soft-ratatui-egui-font-size-cycle
cargo run -p soft-ratatui-egui-modifiers-ttf
cargo run -p soft-ratatui-egui-modifiers-cosmic
```

## Why this layout is easier to maintain

- All example apps share one workspace lockfile and one `target/` directory.
- Dependency versions now live in the root [`Cargo.toml`](/Users/kisaczka/Desktop/programming/soft_ratatui/Cargo.toml), so updates happen in one place.
- Each app has a unique package name, which makes `cargo run -p ...`, `cargo check -p ...`, and CI filtering unambiguous.
- The wasm runner is configured once in [`.cargo/config.toml`](/Users/kisaczka/Desktop/programming/soft_ratatui/.cargo/config.toml) instead of being repeated in each Bevy app.

## Maintenance workflow

- Add a new app by creating a new folder under `more_examples_click_me/` with its own `Cargo.toml` and `src/main.rs`.
- In the new manifest, inherit shared metadata and dependencies with `workspace = true` and keep only app-specific feature flags.
- Use `cargo check --workspace` when you want to validate every example app together.
