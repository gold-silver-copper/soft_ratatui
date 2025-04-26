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

## Features

- ✅ Draws [`ratatui`]([https://github.com/ratatui/ratatui) Widgets into memory.
- ✅ Font rendering with [`cosmic-text`](https://github.com/pop-os/cosmic-text).
- ✅ Tiny-skia-based pixel rasterizer.
- ✅ High-quality text, color, and effects (bold, italic, underline, strikethrough).
- ✅ Single file output if you want — can draw into your own buffers.

---

## License

Dual-licensed under **MIT** or **Apache 2.0**.
Pick whichever suits you.

---

## Status

Experimental, but functional.
Expect some breaking API improvements as it evolves.



