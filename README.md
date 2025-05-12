# soft_ratatui

[![Crates.io](https://img.shields.io/crates/v/soft_ratatui.svg)](https://crates.io/crates/soft_ratatui)
[![Documentation](https://docs.rs/soft_ratatui/badge.svg)](https://docs.rs/soft_ratatui/latest/soft_ratatui/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/bevyengine/bevy/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/soft_ratatui.svg)](https://crates.io/crates/soft_ratatui)

**Software rendering backend for [`ratatui`](https://github.com/ratatui/ratatui). No GPU required. TUI everywhere.**

Fast, portable, no-bloat. Powered by [`cosmic-text`](https://github.com/pop-os/cosmic-text).


- Optimized for speed, generally faster than running ratatui inside a terminal. 120+ fps on normal workloads.
- Only one dependency, Unicode Font rendering powered by [`cosmic-text`](https://github.com/pop-os/cosmic-text)
- Custom portable pixel rasterizer.
---

## Features

- [`egui`](https://github.com/emilk/egui) integration provided by [`egui_ratatui`](https://github.com/gold-silver-copper/egui_ratatui). Have a TUI inside your GUI!
- [`bevy_ratatui`](https://github.com/cxreiff/bevy_ratatui) integration allows you to turn an existing terminal app built with bevy_ratatui into a native or web app. The best way to build a terminal app!!
- [`bevy`](https://github.com/bevyengine/bevy) game engine examples provided in the repo, so you can create your own game UI or world textures with ratatui
- WASM compatible, deploy your ratatui application on the web!

---

## TODO

- Colored Emojis
- More Examples
- no-std support


---

## License

Dual-licensed under **MIT** or **Apache 2.0**.
Pick whichever suits you.

---

## Status

Mostly complete, comments and suggestions are appreciated.
