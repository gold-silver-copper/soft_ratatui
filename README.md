# soft_ratatui

[![Crates.io](https://img.shields.io/crates/v/soft_ratatui.svg)](https://crates.io/crates/soft_ratatui)
[![Documentation](https://docs.rs/soft_ratatui/badge.svg)](https://docs.rs/soft_ratatui/latest/soft_ratatui/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/bevyengine/bevy/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/soft_ratatui.svg)](https://crates.io/crates/soft_ratatui)

**Software rendering backend for [`ratatui`](https://github.com/ratatui/ratatui). No GPU required. TUI everywhere.**

Fast, portable, no-bloat. Powered by [`cosmic-text`](https://github.com/pop-os/cosmic-text).

- Full Unicode + font fallback.
- Optimized for speed, generally faster than running ratatui inside a terminal. 120+ fps on normal workloads.

---

## Features

- Font rendering with [`cosmic-text`](https://github.com/pop-os/cosmic-text).
- Portable pixel rasterizer.
- `egui` integration provided by [`egui_ratatui`](https://github.com/gold-silver-copper/egui_ratatui). Have a TUI inside your GUI!

---

## TODO

- Colored Fonts/ Emojis


---

## License

Dual-licensed under **MIT** or **Apache 2.0**.
Pick whichever suits you.

---

## Status

Mostly complete, comments and suggestions are appreciated.
