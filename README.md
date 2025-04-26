# soft_ratatui

**Software rendering backend for [`ratatui`](https://github.com/ratatui/ratatui). No GPU required. TUI everywhere.**

Fast, portable, no-bloat. Powered by [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) and [`cosmic-text`](https://github.com/pop-os/cosmic-text).

- Full Unicode + font fallback.
- Truecolor rendering.
- Runs anywhere: SDL, Wasm, Framebuffer, Emulators, Serial, whatever.
- Optimized for low memory and CPU use.

---

## Why

**`ratatui`** is great — but its rendering depends on terminal escape sequences.
Sometimes you just want a raw framebuffer and pixel-by-pixel control.
This crate gives you that.
**No GPU, no terminal, no nonsense.**

---

[](https://github.com/gold-silver-copper/soft_ratatui/blob/main/output_tiny_skia.png)

## Features

- ✅ Draws [`ratatui`]([https://github.com/ratatui/ratatui) Widgets into memory.
- ✅ Font rendering with [`cosmic-text`](https://github.com/pop-os/cosmic-text).
- ✅ Tiny-skia-based pixel rasterizer.
- ✅ High-quality text, color, and effects (bold, italic, underline, strikethrough).
- ✅ Single file output if you want — can draw into your own buffers.
- ✅ Super small binary (`lto`, `panic = abort`, `strip = symbols`).

---

## Example

```rust
let mut soft = SoftRatatui::new(...);

soft.draw_cell(&cell, x, y);
```

Render text manually, scanline it, postprocess it — it's all yours.

---

## Goals

- Keep it fast (pixels only).
- No dynamic allocations on hot paths.
- No unnecessary copies.
- No runtime deps (pure `no_std` someday?).

---

## License

Dual-licensed under **MIT** or **Apache 2.0**.
Pick whichever suits you.

---

## Status

Experimental, but functional.
Expect some breaking API improvements as it evolves.

---

## Building

Release build:

```bash
cargo build --release
```

Flags in `Cargo.toml` already tuned for small, fast binaries.

---

## Notes

- `cosmic-text` used without default features.
- `ratatui` used minimal — no terminal, no crossterm.
- Draws into tiny-skia Pixmaps.
- Ideal for retro terminals, embedded displays, weird environments.

---

### TODO
- [ ] Improve shaping performance (fallback shaping)
- [ ] Add batch text rendering
- [ ] Framebuffer examples
- [ ] Wasm32 support
- [ ] TUI on Gameboy? (maybe?)
