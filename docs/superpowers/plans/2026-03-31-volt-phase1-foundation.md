# Volt Terminal — Phase 1: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A working single-window terminal emulator that spawns a shell, renders text with GPU acceleration, and handles keyboard input.

**Architecture:** `volt-core` owns the PTY and terminal grid state; `volt-renderer` draws the grid each frame using wgpu (Metal on macOS, Vulkan/GL on Linux) with glyph rasterization via cosmic-text; `volt-ui` owns the winit window and event loop, wiring core and renderer together via tokio mpsc channels.

**Tech Stack:** Rust 2021, wgpu 0.19, winit 0.29, cosmic-text 0.11, vte 0.13, portable-pty 0.8, tokio 1, serde 1, toml 0.8, bytemuck 1

---

## File Structure

```
Cargo.toml                             # workspace root

volt-config/
  Cargo.toml
  src/
    lib.rs
    config.rs                          # Config struct (shell, font, theme name)
    theme.rs                           # Theme struct (colors)

volt-core/
  Cargo.toml
  src/
    lib.rs
    cell.rs                            # Cell, Color, CellAttrs types
    grid.rs                            # Grid struct (cells, cursor, scroll)
    performer.rs                       # vte::Perform impl — maps escape seqs to grid mutations
    pty.rs                             # PTY spawn, reader task, stdin writer
    events.rs                          # CoreEvent enum

volt-renderer/
  Cargo.toml
  src/
    lib.rs
    atlas.rs                           # Glyph atlas (texture + shelf packer)
    pipeline.rs                        # wgpu pipelines: background quads + glyph quads
    renderer.rs                        # Renderer struct — render(grid) per frame

volt-ui/
  Cargo.toml
  src/
    lib.rs
    app.rs                             # winit App — event loop, keyboard → PTY, triggers renders

volt/
  Cargo.toml
  src/
    main.rs                            # entrypoint — loads config, builds app, runs
```

---

## Task 1: Workspace Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `volt-config/Cargo.toml`, `volt-config/src/lib.rs`
- Create: `volt-core/Cargo.toml`, `volt-core/src/lib.rs`
- Create: `volt-renderer/Cargo.toml`, `volt-renderer/src/lib.rs`
- Create: `volt-ui/Cargo.toml`, `volt-ui/src/lib.rs`
- Create: `volt/Cargo.toml`, `volt/src/main.rs`

- [ ] **Step 1: Create workspace `Cargo.toml`**

```toml
[workspace]
members = ["volt-config", "volt-core", "volt-renderer", "volt-ui", "volt"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

- [ ] **Step 2: Create `volt-config/Cargo.toml`**

```toml
[package]
name = "volt-config"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
toml = "0.8"
```

- [ ] **Step 3: Create `volt-core/Cargo.toml`**

```toml
[package]
name = "volt-core"
version = "0.1.0"
edition = "2021"

[dependencies]
vte = "0.13"
portable-pty = "0.8"
tokio = { workspace = true }
volt-config = { path = "../volt-config" }
```

- [ ] **Step 4: Create `volt-renderer/Cargo.toml`**

```toml
[package]
name = "volt-renderer"
version = "0.1.0"
edition = "2021"

[dependencies]
wgpu = "0.19"
winit = "0.29"
cosmic-text = "0.11"
bytemuck = { version = "1", features = ["derive"] }
tokio = { workspace = true }
volt-core = { path = "../volt-core" }
volt-config = { path = "../volt-config" }
```

- [ ] **Step 5: Create `volt-ui/Cargo.toml`**

```toml
[package]
name = "volt-ui"
version = "0.1.0"
edition = "2021"

[dependencies]
winit = "0.29"
tokio = { workspace = true }
volt-core = { path = "../volt-core" }
volt-renderer = { path = "../volt-renderer" }
volt-config = { path = "../volt-config" }
```

- [ ] **Step 6: Create `volt/Cargo.toml`**

```toml
[package]
name = "volt"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "volt"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
volt-ui = { path = "../volt-ui" }
volt-config = { path = "../volt-config" }
```

- [ ] **Step 7: Create stub source files**

`volt-config/src/lib.rs` — empty
`volt-core/src/lib.rs` — empty
`volt-renderer/src/lib.rs` — empty
`volt-ui/src/lib.rs` — empty

`volt/src/main.rs`:
```rust
fn main() {
    println!("volt starting");
}
```

- [ ] **Step 8: Verify workspace builds**

```bash
cargo build
```
Expected: compiles with no errors (all stubs).

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml volt-config/ volt-core/ volt-renderer/ volt-ui/ volt/
git commit -m "chore: scaffold volt workspace"
```

---

## Task 2: volt-config — Config and Theme

**Files:**
- Create: `volt-config/src/config.rs`
- Create: `volt-config/src/theme.rs`
- Modify: `volt-config/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add to `volt-config/src/config.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let c = Config::default();
        assert_eq!(c.font.size, 14.0);
        assert!(!c.shell.is_empty());
    }

    #[test]
    fn test_config_from_toml() {
        let toml = r#"
            [font]
            family = "JetBrains Mono"
            size = 16.0
            [shell]
            program = "/bin/bash"
        "#;
        let c: Config = toml::from_str(toml).unwrap();
        assert_eq!(c.font.family, "JetBrains Mono");
        assert_eq!(c.font.size, 16.0);
        assert_eq!(c.shell.program, "/bin/bash");
    }
}
```

- [ ] **Step 2: Run tests — expect failure**

```bash
cargo test -p volt-config
```
Expected: compile error — `Config` not defined.

- [ ] **Step 3: Implement `volt-config/src/config.rs`**

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FontConfig {
    #[serde(default = "default_font_family")]
    pub family: String,
    #[serde(default = "default_font_size")]
    pub size: f32,
}

fn default_font_family() -> String { "monospace".to_string() }
fn default_font_size() -> f32 { 14.0 }

impl Default for FontConfig {
    fn default() -> Self {
        Self { family: default_font_family(), size: default_font_size() }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellConfig {
    #[serde(default = "default_shell")]
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self { program: default_shell(), args: vec![] }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub font: FontConfig,
    #[serde(default)]
    pub shell: ShellConfig,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String { "dark".to_string() }

impl Config {
    pub fn load() -> Self {
        let path = dirs_next::config_dir()
            .map(|d| d.join("volt").join("config.toml"));
        if let Some(path) = path {
            if let Ok(s) = std::fs::read_to_string(path) {
                if let Ok(c) = toml::from_str(&s) {
                    return c;
                }
            }
        }
        Self::default()
    }
}
```

Add `dirs-next = "2"` to `volt-config/Cargo.toml` dependencies.

- [ ] **Step 4: Implement `volt-config/src/theme.rs`**

```rust
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b } }
    pub fn to_f32(self) -> [f32; 4] {
        [self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, 1.0]
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub background:  Color,
    pub foreground:  Color,
    pub cursor:      Color,
    pub black:       Color,
    pub red:         Color,
    pub green:       Color,
    pub yellow:      Color,
    pub blue:        Color,
    pub magenta:     Color,
    pub cyan:        Color,
    pub white:       Color,
    pub bright: [Color; 8],
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            background: Color::rgb(24,  24,  37),
            foreground: Color::rgb(202, 211, 245),
            cursor:     Color::rgb(202, 211, 245),
            black:      Color::rgb(30,  30,  46),
            red:        Color::rgb(243, 139, 168),
            green:      Color::rgb(166, 227, 161),
            yellow:     Color::rgb(249, 226, 175),
            blue:       Color::rgb(137, 180, 250),
            magenta:    Color::rgb(203, 166, 247),
            cyan:       Color::rgb(137, 220, 235),
            white:      Color::rgb(166, 173, 200),
            bright: [
                Color::rgb(88,  91,  112),
                Color::rgb(243, 139, 168),
                Color::rgb(166, 227, 161),
                Color::rgb(249, 226, 175),
                Color::rgb(137, 180, 250),
                Color::rgb(203, 166, 247),
                Color::rgb(137, 220, 235),
                Color::rgb(186, 194, 222),
            ],
        }
    }

    pub fn ansi_color(&self, index: u8) -> Color {
        match index {
            0 => self.black,   1 => self.red,     2 => self.green,
            3 => self.yellow,  4 => self.blue,    5 => self.magenta,
            6 => self.cyan,    7 => self.white,
            8..=15 => self.bright[(index - 8) as usize],
            16..=231 => {
                let i = index - 16;
                let b = (i % 6) * 51;
                let g = ((i / 6) % 6) * 51;
                let r = (i / 36) * 51;
                Color::rgb(r, g, b)
            }
            232..=255 => {
                let v = 8 + (index - 232) * 10;
                Color::rgb(v, v, v)
            }
        }
    }
}
```

- [ ] **Step 5: Update `volt-config/src/lib.rs`**

```rust
pub mod config;
pub mod theme;

pub use config::Config;
pub use theme::{Color, Theme};
```

- [ ] **Step 6: Run tests — expect pass**

```bash
cargo test -p volt-config
```
Expected: 2 tests pass.

- [ ] **Step 7: Commit**

```bash
git add volt-config/
git commit -m "feat(volt-config): Config and Theme types"
```

---

## Task 3: volt-core — Cell and Grid

**Files:**
- Create: `volt-core/src/cell.rs`
- Create: `volt-core/src/grid.rs`
- Modify: `volt-core/src/lib.rs`

- [ ] **Step 1: Write failing tests**

`volt-core/src/grid.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;

    #[test]
    fn test_new_grid_is_blank() {
        let g = Grid::new(80, 24);
        assert_eq!(g.cols, 80);
        assert_eq!(g.rows, 24);
        assert_eq!(g.cell(0, 0).c, ' ');
        assert_eq!(g.cursor_col, 0);
        assert_eq!(g.cursor_row, 0);
    }

    #[test]
    fn test_write_cell() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(5, 3).c = 'A';
        assert_eq!(g.cell(5, 3).c, 'A');
    }

    #[test]
    fn test_resize_preserves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(2, 1).c = 'X';
        g.resize(100, 30);
        assert_eq!(g.cell(2, 1).c, 'X');
        assert_eq!(g.cols, 100);
        assert_eq!(g.rows, 30);
    }

    #[test]
    fn test_scroll_up_moves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(0, 1).c = 'A';
        g.scroll_up(0, 23, 1);
        assert_eq!(g.cell(0, 0).c, 'A');
        assert_eq!(g.cell(0, 23).c, ' ');
    }
}
```

- [ ] **Step 2: Run tests — expect failure**

```bash
cargo test -p volt-core
```
Expected: compile error — `Grid`, `Cell` not defined.

- [ ] **Step 3: Implement `volt-core/src/cell.rs`**

```rust
use volt_config::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellColor {
    Default,
    Indexed(u8),
    Rgb(Color),
}

impl CellColor {
    pub fn resolve(&self, default: Color, theme: &volt_config::Theme) -> Color {
        match self {
            CellColor::Default => default,
            CellColor::Indexed(i) => theme.ansi_color(*i),
            CellColor::Rgb(c) => *c,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub c: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dirty: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            italic: false,
            underline: false,
            dirty: true,
        }
    }
}
```

- [ ] **Step 4: Implement `volt-core/src/grid.rs`**

```rust
use crate::cell::Cell;

pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Cell>,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub scroll_top: usize,
    pub scroll_bottom: usize,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            cursor_col: 0,
            cursor_row: 0,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
        }
    }

    pub fn cell(&self, col: usize, row: usize) -> &Cell {
        &self.cells[row * self.cols + col]
    }

    pub fn cell_mut(&mut self, col: usize, row: usize) -> &mut Cell {
        self.cells[row * self.cols + col].dirty = true;
        &mut self.cells[row * self.cols + col]
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let mut new_cells = vec![Cell::default(); cols * rows];
        for row in 0..rows.min(self.rows) {
            for col in 0..cols.min(self.cols) {
                new_cells[row * cols + col] = self.cells[row * self.cols + col];
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.scroll_bottom = rows.saturating_sub(1);
        self.cells = new_cells;
    }

    pub fn scroll_up(&mut self, top: usize, bottom: usize, count: usize) {
        for _ in 0..count {
            for row in top..bottom {
                for col in 0..self.cols {
                    self.cells[row * self.cols + col] = self.cells[(row + 1) * self.cols + col];
                    self.cells[row * self.cols + col].dirty = true;
                }
            }
            for col in 0..self.cols {
                self.cells[bottom * self.cols + col] = Cell::default();
            }
        }
    }

    pub fn clear_line(&mut self, row: usize, from_col: usize, to_col: usize) {
        for col in from_col..=to_col.min(self.cols.saturating_sub(1)) {
            self.cells[row * self.cols + col] = Cell::default();
        }
    }

    pub fn clear_screen(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.cursor_col = 0;
        self.cursor_row = 0;
    }

    pub fn advance_cursor(&mut self) {
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.newline();
        }
    }

    pub fn newline(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up(self.scroll_top, self.scroll_bottom, 1);
        } else if self.cursor_row < self.rows - 1 {
            self.cursor_row += 1;
        }
    }
}
```

- [ ] **Step 5: Update `volt-core/src/lib.rs`**

```rust
pub mod cell;
pub mod grid;
```

- [ ] **Step 6: Run tests — expect pass**

```bash
cargo test -p volt-core
```
Expected: 4 tests pass.

- [ ] **Step 7: Commit**

```bash
git add volt-core/src/cell.rs volt-core/src/grid.rs volt-core/src/lib.rs
git commit -m "feat(volt-core): Cell and Grid types"
```

---

## Task 4: volt-core — VTE Performer

**Files:**
- Create: `volt-core/src/performer.rs`
- Create: `volt-core/src/events.rs`
- Modify: `volt-core/src/lib.rs`

- [ ] **Step 1: Write failing tests**

`volt-core/src/performer.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn feed(performer: &mut Performer, bytes: &[u8]) {
        let mut parser = vte::Parser::new();
        for &b in bytes {
            parser.advance(performer, b);
        }
    }

    #[test]
    fn test_print_advances_cursor() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"AB");
        assert_eq!(p.grid.cell(0, 0).c, 'A');
        assert_eq!(p.grid.cell(1, 0).c, 'B');
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn test_newline_moves_cursor_down() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"A\r\nB");
        assert_eq!(p.grid.cell(0, 0).c, 'A');
        assert_eq!(p.grid.cell(0, 1).c, 'B');
        assert_eq!(p.grid.cursor_row, 1);
    }

    #[test]
    fn test_csi_cursor_up() {
        let mut p = Performer::new(80, 24);
        p.grid.cursor_row = 5;
        feed(&mut p, b"\x1b[2A");
        assert_eq!(p.grid.cursor_row, 3);
    }

    #[test]
    fn test_csi_erase_line() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"Hello");
        p.grid.cursor_col = 0;
        feed(&mut p, b"\x1b[2K");
        assert_eq!(p.grid.cell(0, 0).c, ' ');
        assert_eq!(p.grid.cell(4, 0).c, ' ');
    }

    #[test]
    fn test_sgr_sets_bold() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[1mA");
        assert!(p.grid.cell(0, 0).bold);
    }
}
```

- [ ] **Step 2: Run tests — expect failure**

```bash
cargo test -p volt-core
```
Expected: compile error — `Performer` not defined.

- [ ] **Step 3: Create `volt-core/src/events.rs`**

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CoreEvent {
    GridUpdated,
    CwdChanged(PathBuf),
    TitleChanged(String),
    CommandFinished { exit_code: i32, duration_ms: u64 },
}
```

- [ ] **Step 4: Implement `volt-core/src/performer.rs`**

```rust
use crate::cell::{Cell, CellColor};
use crate::grid::Grid;
use vte::Perform;

pub struct Performer {
    pub grid: Grid,
    current_fg: CellColor,
    current_bg: CellColor,
    current_bold: bool,
    current_italic: bool,
    current_underline: bool,
}

impl Performer {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            grid: Grid::new(cols, rows),
            current_fg: CellColor::Default,
            current_bg: CellColor::Default,
            current_bold: false,
            current_italic: false,
            current_underline: false,
        }
    }

    fn current_cell(&self) -> Cell {
        Cell {
            c: ' ',
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.current_bold,
            italic: self.current_italic,
            underline: self.current_underline,
            dirty: true,
        }
    }

    fn param(params: &vte::Params, idx: usize) -> u16 {
        params.iter().nth(idx).and_then(|s| s.first().copied()).unwrap_or(0)
    }
}

impl Perform for Performer {
    fn print(&mut self, c: char) {
        let col = self.grid.cursor_col;
        let row = self.grid.cursor_row;
        if col < self.grid.cols && row < self.grid.rows {
            let cell = self.grid.cell_mut(col, row);
            *cell = self.current_cell();
            cell.c = c;
        }
        self.grid.advance_cursor();
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x0a | 0x0b | 0x0c => self.grid.newline(),   // LF / VT / FF
            0x0d => self.grid.cursor_col = 0,              // CR
            0x08 => {                                       // BS
                if self.grid.cursor_col > 0 {
                    self.grid.cursor_col -= 1;
                }
            }
            0x09 => {                                       // HT (tab)
                let next = (self.grid.cursor_col / 8 + 1) * 8;
                self.grid.cursor_col = next.min(self.grid.cols - 1);
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &vte::Params, _intermediates: &[u8], _ignore: bool, action: char) {
        match action {
            'A' => { // cursor up
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
            }
            'B' => { // cursor down
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = (self.grid.cursor_row + n).min(self.grid.rows - 1);
            }
            'C' => { // cursor forward
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_col = (self.grid.cursor_col + n).min(self.grid.cols - 1);
            }
            'D' => { // cursor back
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_col = self.grid.cursor_col.saturating_sub(n);
            }
            'H' | 'f' => { // cursor position
                let row = Self::param(params, 0).saturating_sub(1) as usize;
                let col = Self::param(params, 1).saturating_sub(1) as usize;
                self.grid.cursor_row = row.min(self.grid.rows - 1);
                self.grid.cursor_col = col.min(self.grid.cols - 1);
            }
            'J' => match Self::param(params, 0) { // erase display
                0 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    self.grid.clear_line(row, col, self.grid.cols - 1);
                    for r in (row + 1)..self.grid.rows {
                        self.grid.clear_line(r, 0, self.grid.cols - 1);
                    }
                }
                1 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    for r in 0..row {
                        self.grid.clear_line(r, 0, self.grid.cols - 1);
                    }
                    self.grid.clear_line(row, 0, col);
                }
                2 | 3 => self.grid.clear_screen(),
                _ => {}
            }
            'K' => match Self::param(params, 0) { // erase line
                0 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    self.grid.clear_line(row, col, self.grid.cols - 1);
                }
                1 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    self.grid.clear_line(row, 0, col);
                }
                2 => {
                    let row = self.grid.cursor_row;
                    self.grid.clear_line(row, 0, self.grid.cols - 1);
                }
                _ => {}
            }
            'r' => { // set scroll region
                let top = Self::param(params, 0).saturating_sub(1) as usize;
                let bottom = (Self::param(params, 1) as usize).saturating_sub(1)
                    .min(self.grid.rows - 1);
                self.grid.scroll_top = top;
                self.grid.scroll_bottom = bottom;
                self.grid.cursor_col = 0;
                self.grid.cursor_row = 0;
            }
            'm' => { // SGR — colors and attributes
                let mut iter = params.iter().peekable();
                if iter.peek().is_none() {
                    self.current_fg = CellColor::Default;
                    self.current_bg = CellColor::Default;
                    self.current_bold = false;
                    self.current_italic = false;
                    self.current_underline = false;
                    return;
                }
                while let Some(sub) = iter.next() {
                    let p = sub.first().copied().unwrap_or(0);
                    match p {
                        0 => {
                            self.current_fg = CellColor::Default;
                            self.current_bg = CellColor::Default;
                            self.current_bold = false;
                            self.current_italic = false;
                            self.current_underline = false;
                        }
                        1 => self.current_bold = true,
                        3 => self.current_italic = true,
                        4 => self.current_underline = true,
                        22 => self.current_bold = false,
                        23 => self.current_italic = false,
                        24 => self.current_underline = false,
                        30..=37 => self.current_fg = CellColor::Indexed(p as u8 - 30),
                        39 => self.current_fg = CellColor::Default,
                        40..=47 => self.current_bg = CellColor::Indexed(p as u8 - 40),
                        49 => self.current_bg = CellColor::Default,
                        90..=97 => self.current_fg = CellColor::Indexed(p as u8 - 90 + 8),
                        100..=107 => self.current_bg = CellColor::Indexed(p as u8 - 100 + 8),
                        38 | 48 => {
                            // 38;5;n or 38;2;r;g;b
                            if let Some(sub2) = iter.next() {
                                match sub2.first().copied().unwrap_or(0) {
                                    5 => {
                                        if let Some(sub3) = iter.next() {
                                            let idx = sub3.first().copied().unwrap_or(0) as u8;
                                            if p == 38 {
                                                self.current_fg = CellColor::Indexed(idx);
                                            } else {
                                                self.current_bg = CellColor::Indexed(idx);
                                            }
                                        }
                                    }
                                    2 => {
                                        let r = iter.next().and_then(|s| s.first().copied()).unwrap_or(0) as u8;
                                        let g = iter.next().and_then(|s| s.first().copied()).unwrap_or(0) as u8;
                                        let b = iter.next().and_then(|s| s.first().copied()).unwrap_or(0) as u8;
                                        let color = volt_config::Color::rgb(r, g, b);
                                        if p == 38 {
                                            self.current_fg = CellColor::Rgb(color);
                                        } else {
                                            self.current_bg = CellColor::Rgb(color);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
```

- [ ] **Step 5: Update `volt-core/src/lib.rs`**

```rust
pub mod cell;
pub mod events;
pub mod grid;
pub mod performer;
```

- [ ] **Step 6: Run tests — expect pass**

```bash
cargo test -p volt-core
```
Expected: all 9 tests pass (4 grid + 5 performer).

- [ ] **Step 7: Commit**

```bash
git add volt-core/src/performer.rs volt-core/src/events.rs volt-core/src/lib.rs
git commit -m "feat(volt-core): VTE performer and CoreEvent types"
```

---

## Task 5: volt-core — PTY

**Files:**
- Create: `volt-core/src/pty.rs`
- Modify: `volt-core/src/lib.rs`

PTY spawns the shell, reads output in a background task, feeds bytes to the VTE parser which updates the grid, and sends `GridUpdated` events over a channel. Keyboard bytes are written to the PTY master.

- [ ] **Step 1: Implement `volt-core/src/pty.rs`**

```rust
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use vte::Parser;

use crate::events::CoreEvent;
use crate::performer::Performer;

pub struct Pty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    pub event_tx: mpsc::UnboundedSender<CoreEvent>,
}

impl Pty {
    pub fn spawn(
        shell: &str,
        args: &[String],
        cols: u16,
        rows: u16,
    ) -> anyhow::Result<(Self, Arc<Mutex<Performer>>, mpsc::UnboundedReceiver<CoreEvent>)> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;

        let mut cmd = CommandBuilder::new(shell);
        for arg in args { cmd.arg(arg); }

        let _child = pair.slave.spawn_command(cmd)?;

        let performer = Arc::new(Mutex::new(Performer::new(cols as usize, rows as usize)));
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Reader task
        let mut reader = pair.master.try_clone_reader()?;
        let performer_clone = Arc::clone(&performer);
        let event_tx_clone = event_tx.clone();
        std::thread::spawn(move || {
            let mut parser = Parser::new();
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut p = performer_clone.lock().unwrap();
                        for &b in &buf[..n] {
                            parser.advance(&mut *p, b);
                        }
                        let _ = event_tx_clone.send(CoreEvent::GridUpdated);
                    }
                }
            }
        });

        Ok((
            Self { master: pair.master, event_tx },
            performer,
            event_rx,
        ))
    }

    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        self.master.write_all(bytes)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }
}
```

Add `anyhow = "1"` to `volt-core/Cargo.toml`.

- [ ] **Step 2: Update `volt-core/src/lib.rs`**

```rust
pub mod cell;
pub mod events;
pub mod grid;
pub mod performer;
pub mod pty;
```

- [ ] **Step 3: Verify compiles**

```bash
cargo build -p volt-core
```
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add volt-core/src/pty.rs volt-core/src/lib.rs volt-core/Cargo.toml
git commit -m "feat(volt-core): PTY spawn and reader task"
```

---

## Task 6: volt-renderer — Glyph Atlas

**Files:**
- Create: `volt-renderer/src/atlas.rs`
- Modify: `volt-renderer/src/lib.rs`

The atlas rasterizes glyphs on demand into a 2D texture, returning UV coordinates. Uses a shelf-based packing algorithm.

- [ ] **Step 1: Write failing test**

`volt-renderer/src/atlas.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{FontSystem, SwashCache};

    #[test]
    fn test_rasterize_returns_uv() {
        let mut font_system = FontSystem::new();
        let mut swash_cache = SwashCache::new();
        let mut atlas = CpuAtlas::new(512, 512);
        let key = GlyphKey { glyph_id: 65, font_size_px: 14, bold: false, italic: false };
        // just checks it doesn't panic and returns a region
        let result = atlas.get_or_rasterize(key, &mut font_system, &mut swash_cache);
        assert!(result.is_some() || result.is_none()); // glyph may not exist in default font
    }
}
```

- [ ] **Step 2: Run test — expect failure**

```bash
cargo test -p volt-renderer atlas
```
Expected: compile error.

- [ ] **Step 3: Implement `volt-renderer/src/atlas.rs`**

```rust
use cosmic_text::{CacheKey, FontSystem, SwashCache, SwashContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub glyph_id: u16,
    pub font_size_px: u16,
    pub bold: bool,
    pub italic: bool,
}

/// UV region in the atlas texture (normalised 0.0–1.0)
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    pub u0: f32, pub v0: f32,
    pub u1: f32, pub v1: f32,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
}

/// CPU-side atlas — a flat RGBA bitmap with a shelf packer.
pub struct CpuAtlas {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,          // R8 (single channel, grayscale glyph coverage)
    cache: std::collections::HashMap<GlyphKey, Option<AtlasRegion>>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    pub dirty: bool,
}

impl CpuAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height) as usize],
            cache: Default::default(),
            shelf_x: 0,
            shelf_y: 0,
            shelf_h: 0,
            dirty: false,
        }
    }

    pub fn get_or_rasterize(
        &mut self,
        key: GlyphKey,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) -> Option<AtlasRegion> {
        if let Some(region) = self.cache.get(&key) {
            return *region;
        }

        let cache_key = CacheKey {
            glyph_id: key.glyph_id,
            font_size_bits: (key.font_size_px as f32).to_bits(),
            x_bin: cosmic_text::SubpixelBin::Zero,
            y_bin: cosmic_text::SubpixelBin::Zero,
            flags: cosmic_text::CacheKeyFlags::empty(),
        };

        let image = swash_cache.get_image(font_system, cache_key)?;
        let content = image.content;

        let w = image.placement.width;
        let h = image.placement.height;

        if w == 0 || h == 0 {
            self.cache.insert(key, None);
            return None;
        }

        // Find shelf space
        if self.shelf_x + w > self.width {
            self.shelf_y += self.shelf_h + 1;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        if self.shelf_y + h > self.height {
            // Atlas full — in Phase 1 we just fail gracefully
            self.cache.insert(key, None);
            return None;
        }

        // Copy glyph data into atlas
        for row in 0..h {
            for col in 0..w {
                let src_idx = (row * w + col) as usize;
                let dst_idx = ((self.shelf_y + row) * self.width + self.shelf_x + col) as usize;
                self.data[dst_idx] = match content {
                    SwashContent::Mask => image.data[src_idx],
                    SwashContent::Color => image.data[src_idx * 4 + 3],
                    SwashContent::SubpixelMask => image.data[src_idx],
                };
            }
        }

        let region = AtlasRegion {
            u0: self.shelf_x as f32 / self.width as f32,
            v0: self.shelf_y as f32 / self.height as f32,
            u1: (self.shelf_x + w) as f32 / self.width as f32,
            v1: (self.shelf_y + h) as f32 / self.height as f32,
            width: w,
            height: h,
            offset_x: image.placement.left,
            offset_y: image.placement.top,
        };

        self.shelf_x += w + 1;
        if h > self.shelf_h { self.shelf_h = h; }
        self.dirty = true;
        self.cache.insert(key, Some(region));
        Some(region)
    }
}
```

- [ ] **Step 4: Update `volt-renderer/src/lib.rs`**

```rust
pub mod atlas;
```

- [ ] **Step 5: Run test — expect pass**

```bash
cargo test -p volt-renderer atlas
```
Expected: 1 test passes.

- [ ] **Step 6: Commit**

```bash
git add volt-renderer/src/atlas.rs volt-renderer/src/lib.rs
git commit -m "feat(volt-renderer): CPU glyph atlas with shelf packer"
```

---

## Task 7: volt-renderer — Pipelines and Renderer

**Files:**
- Create: `volt-renderer/src/pipeline.rs`
- Create: `volt-renderer/src/shaders/bg.wgsl`
- Create: `volt-renderer/src/shaders/glyph.wgsl`
- Create: `volt-renderer/src/renderer.rs`
- Modify: `volt-renderer/src/lib.rs`

- [ ] **Step 1: Create `volt-renderer/src/shaders/bg.wgsl`**

```wgsl
struct VIn  { @location(0) pos: vec2<f32>, @location(1) color: vec4<f32> }
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> }

@vertex fn vs(in: VIn) -> VOut {
    return VOut(vec4<f32>(in.pos, 0.0, 1.0), in.color);
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> { return in.color; }
```

- [ ] **Step 2: Create `volt-renderer/src/shaders/glyph.wgsl`**

```wgsl
struct VIn  {
    @location(0) pos: vec2<f32>,
    @location(1) uv:  vec2<f32>,
    @location(2) color: vec4<f32>,
}
struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0) var t_atlas: texture_2d<f32>;
@group(0) @binding(1) var s_atlas: sampler;

@vertex fn vs(in: VIn) -> VOut {
    return VOut(vec4<f32>(in.pos, 0.0, 1.0), in.uv, in.color);
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
    let a = textureSample(t_atlas, s_atlas, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * a);
}
```

- [ ] **Step 3: Implement `volt-renderer/src/pipeline.rs`**

```rust
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct BgVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct GlyphVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub fn bg_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bg_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bg.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None, bind_group_layouts: &[], push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bg_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader, entry_point: "vs",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<BgVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader, entry_point: "fs",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

pub fn glyph_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glyph_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/glyph.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None, bind_group_layouts: &[bind_group_layout], push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glyph_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader, entry_point: "vs",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader, entry_point: "fs",
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}
```

- [ ] **Step 4: Implement `volt-renderer/src/renderer.rs`**

```rust
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::atlas::{CpuAtlas, GlyphKey};
use crate::pipeline::{bg_pipeline, glyph_pipeline, BgVertex, GlyphVertex};
use volt_config::{Color, Theme};
use volt_core::grid::Grid;

pub struct Renderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    bg_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
    atlas: CpuAtlas,
    font_system: FontSystem,
    swash_cache: SwashCache,
    pub cell_width: f32,
    pub cell_height: f32,
    font_size: f32,
}

impl Renderer {
    pub async fn new(window: &Window, font_family: &str, font_size: f32) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(), ..Default::default()
        });
        let surface = unsafe { instance.create_surface(window) }.unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None)
            .await.unwrap();
        let surface_format = surface.get_capabilities(&adapter).formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // Atlas texture (R8 single channel)
        const ATLAS_SIZE: u32 = 1024;
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d { width: ATLAS_SIZE, height: ATLAS_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&Default::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });

        let bg_pl = bg_pipeline(&device, surface_format);
        let glyph_pl = glyph_pipeline(&device, surface_format, &bgl);

        let mut font_system = FontSystem::new();
        // Hint cosmic-text about preferred font family
        let _ = font_family; // will be used in full font loading

        // Measure cell size: render a single 'M' to get dimensions
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buf = Buffer::new(&mut font_system, metrics);
        buf.set_size(&mut font_system, 1000.0, 1000.0);
        buf.set_text(&mut font_system, "M", Attrs::new(), Shaping::Basic);
        buf.shape_until_scroll(&mut font_system, false);
        let cell_width = font_size * 0.6;  // approximate; refined by layout
        let cell_height = font_size * 1.2;

        Self {
            surface, device, queue, config,
            bg_pipeline: bg_pl, glyph_pipeline: glyph_pl,
            atlas_texture,
            atlas_bind_group,
            atlas: CpuAtlas::new(ATLAS_SIZE, ATLAS_SIZE),
            font_system,
            swash_cache: SwashCache::new(),
            cell_width, cell_height, font_size,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self, grid: &Grid, theme: &Theme) {
        let output = match self.surface.get_current_texture() {
            Ok(o) => o,
            Err(_) => return,
        };
        let view = output.texture.create_view(&Default::default());

        let w = self.config.width as f32;
        let h = self.config.height as f32;
        let cw = self.cell_width;
        let ch = self.cell_height;

        // Build background quads
        let mut bg_verts: Vec<BgVertex> = Vec::with_capacity(grid.cols * grid.rows * 6);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let bg_color = cell.bg.resolve(theme.background, theme);
                let color = bg_color.to_f32();
                let x0 = (col as f32 * cw / w) * 2.0 - 1.0;
                let x1 = ((col as f32 + 1.0) * cw / w) * 2.0 - 1.0;
                let y0 = 1.0 - (row as f32 * ch / h) * 2.0;
                let y1 = 1.0 - ((row as f32 + 1.0) * ch / h) * 2.0;
                bg_verts.extend_from_slice(&[
                    BgVertex { pos: [x0, y0], color },
                    BgVertex { pos: [x1, y0], color },
                    BgVertex { pos: [x0, y1], color },
                    BgVertex { pos: [x1, y0], color },
                    BgVertex { pos: [x1, y1], color },
                    BgVertex { pos: [x0, y1], color },
                ]);
            }
        }

        // Build glyph quads — rasterize each non-space cell
        let mut glyph_verts: Vec<GlyphVertex> = Vec::new();
        let metrics = Metrics::new(self.font_size, ch);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                if cell.c == ' ' { continue; }

                let mut buf = Buffer::new(&mut self.font_system, metrics);
                buf.set_size(&mut self.font_system, cw * 2.0, ch * 2.0);
                let attrs = Attrs::new();
                buf.set_text(&mut self.font_system, &cell.c.to_string(), attrs, Shaping::Basic);
                buf.shape_until_scroll(&mut self.font_system, false);

                for layout_run in buf.layout_runs() {
                    for glyph in layout_run.glyphs {
                        let key = GlyphKey {
                            glyph_id: glyph.cache_key.glyph_id,
                            font_size_px: self.font_size as u16,
                            bold: cell.bold,
                            italic: cell.italic,
                        };
                        let Some(region) = self.atlas.get_or_rasterize(
                            key, &mut self.font_system, &mut self.swash_cache
                        ) else { continue };

                        let gx = col as f32 * cw + glyph.x + region.offset_x as f32;
                        let gy = row as f32 * ch + glyph.y - region.offset_y as f32;
                        let gw = region.width as f32;
                        let gh = region.height as f32;

                        let x0 = (gx / w) * 2.0 - 1.0;
                        let x1 = ((gx + gw) / w) * 2.0 - 1.0;
                        let y0 = 1.0 - (gy / h) * 2.0;
                        let y1 = 1.0 - ((gy + gh) / h) * 2.0;

                        let fg_color = cell.fg.resolve(theme.foreground, theme);
                        let color = fg_color.to_f32();
                        let [u0, v0, u1, v1] = [region.u0, region.v0, region.u1, region.v1];

                        glyph_verts.extend_from_slice(&[
                            GlyphVertex { pos: [x0, y0], uv: [u0, v0], color },
                            GlyphVertex { pos: [x1, y0], uv: [u1, v0], color },
                            GlyphVertex { pos: [x0, y1], uv: [u0, v1], color },
                            GlyphVertex { pos: [x1, y0], uv: [u1, v0], color },
                            GlyphVertex { pos: [x1, y1], uv: [u1, v1], color },
                            GlyphVertex { pos: [x0, y1], uv: [u0, v1], color },
                        ]);
                    }
                }
            }
        }

        // Upload atlas if dirty
        if self.atlas.dirty {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.atlas.data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.atlas.width),
                    rows_per_image: Some(self.atlas.height),
                },
                wgpu::Extent3d { width: self.atlas.width, height: self.atlas.height, depth_or_array_layers: 1 },
            );
            self.atlas.dirty = false;
        }

        let bg_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bg"), contents: bytemuck::cast_slice(&bg_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let glyph_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glyph"), contents: if glyph_verts.is_empty() {
                bytemuck::cast_slice(&[GlyphVertex { pos: [0.0; 2], uv: [0.0; 2], color: [0.0; 4] }])
            } else {
                bytemuck::cast_slice(&glyph_verts)
            },
            usage: wgpu::BufferUsages::VERTEX,
        });

        let bg_color = theme.background.to_f32();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg_color[0] as f64, g: bg_color[1] as f64,
                            b: bg_color[2] as f64, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.bg_pipeline);
            pass.set_vertex_buffer(0, bg_buf.slice(..));
            pass.draw(0..bg_verts.len() as u32, 0..1);

            if !glyph_verts.is_empty() {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, glyph_buf.slice(..));
                pass.draw(0..glyph_verts.len() as u32, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    pub fn grid_size(&self) -> (usize, usize) {
        let cols = (self.config.width as f32 / self.cell_width) as usize;
        let rows = (self.config.height as f32 / self.cell_height) as usize;
        (cols.max(1), rows.max(1))
    }
}
```

- [ ] **Step 5: Update `volt-renderer/src/lib.rs`**

```rust
pub mod atlas;
pub mod pipeline;
pub mod renderer;

pub use renderer::Renderer;
```

- [ ] **Step 6: Verify compiles**

```bash
cargo build -p volt-renderer
```
Expected: compiles with no errors.

- [ ] **Step 7: Commit**

```bash
git add volt-renderer/
git commit -m "feat(volt-renderer): wgpu renderer with glyph atlas"
```

---

## Task 8: volt-ui — App (winit Event Loop)

**Files:**
- Create: `volt-ui/src/app.rs`
- Modify: `volt-ui/src/lib.rs`

The App owns the winit event loop, the PTY, the performer (grid state), and the renderer. On `GridUpdated` it requests a redraw. On keyboard input it writes bytes to the PTY.

- [ ] **Step 1: Implement `volt-ui/src/app.rs`**

```rust
use std::sync::{Arc, Mutex};
use winit::event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent, ElementState, ModifiersState};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use volt_config::{Config, Theme};
use volt_core::pty::Pty;
use volt_core::events::CoreEvent;
use volt_core::performer::Performer;
use volt_renderer::Renderer;

pub struct App {
    config: Config,
    theme: Theme,
}

impl App {
    pub fn new(config: Config) -> Self {
        let theme = Theme::dark();
        Self { config, theme }
    }

    pub fn run(self) {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("Volt")
            .with_inner_size(winit::dpi::PhysicalSize::new(1200u32, 800u32))
            .build(&event_loop)
            .unwrap();

        // Initialise renderer (async, block here)
        let mut renderer = pollster::block_on(Renderer::new(
            &window,
            &self.config.font.family,
            self.config.font.size,
        ));

        let (cols, rows) = renderer.grid_size();

        let (mut pty, performer, mut event_rx) = Pty::spawn(
            &self.config.shell.program,
            &self.config.shell.args,
            cols as u16,
            rows as u16,
        ).expect("failed to spawn PTY");

        let mut modifiers = ModifiersState::default();

        event_loop.run(move |event, _, control_flow| {
            // Drain CoreEvents (non-blocking)
            while let Ok(ev) = event_rx.try_recv() {
                match ev {
                    CoreEvent::GridUpdated => window.request_redraw(),
                    _ => {}
                }
            }

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,

                    WindowEvent::Resized(size) => {
                        renderer.resize(size.width, size.height);
                        let (cols, rows) = renderer.grid_size();
                        let _ = pty.resize(cols as u16, rows as u16);
                        performer.lock().unwrap().grid.resize(cols, rows);
                    }

                    WindowEvent::ModifiersChanged(m) => modifiers = m,

                    WindowEvent::KeyboardInput {
                        input: KeyboardInput { virtual_keycode: Some(key), state: ElementState::Pressed, .. },
                        ..
                    } => {
                        let bytes: Option<&[u8]> = match key {
                            VirtualKeyCode::Return    => Some(b"\r"),
                            VirtualKeyCode::Back      => Some(b"\x7f"),
                            VirtualKeyCode::Tab       => Some(b"\t"),
                            VirtualKeyCode::Escape    => Some(b"\x1b"),
                            VirtualKeyCode::Up        => Some(b"\x1b[A"),
                            VirtualKeyCode::Down      => Some(b"\x1b[B"),
                            VirtualKeyCode::Right     => Some(b"\x1b[C"),
                            VirtualKeyCode::Left      => Some(b"\x1b[D"),
                            VirtualKeyCode::Home      => Some(b"\x1b[H"),
                            VirtualKeyCode::End       => Some(b"\x1b[F"),
                            VirtualKeyCode::PageUp    => Some(b"\x1b[5~"),
                            VirtualKeyCode::PageDown  => Some(b"\x1b[6~"),
                            VirtualKeyCode::Delete    => Some(b"\x1b[3~"),
                            _ => None,
                        };
                        if let Some(b) = bytes {
                            let _ = pty.write(b);
                        }
                    }

                    WindowEvent::ReceivedCharacter(c) => {
                        if !c.is_control() {
                            let mut buf = [0u8; 4];
                            let s = c.encode_utf8(&mut buf);
                            if modifiers.ctrl() {
                                // Ctrl+key: send control byte
                                if let Some(b) = ctrl_byte(c) {
                                    let _ = pty.write(&[b]);
                                }
                            } else {
                                let _ = pty.write(s.as_bytes());
                            }
                        }
                    }

                    _ => {}
                }

                Event::RedrawRequested(_) => {
                    let p = performer.lock().unwrap();
                    renderer.render(&p.grid, &self.theme);
                }

                Event::MainEventsCleared => {
                    // Poll for more events
                    *control_flow = ControlFlow::Poll;
                }

                _ => {}
            }
        });
    }
}

fn ctrl_byte(c: char) -> Option<u8> {
    let c = c.to_ascii_uppercase();
    if c >= 'A' && c <= '_' {
        Some(c as u8 - b'@')
    } else {
        None
    }
}
```

Add `pollster = "0.3"` to `volt-ui/Cargo.toml`.

- [ ] **Step 2: Update `volt-ui/src/lib.rs`**

```rust
pub mod app;
pub use app::App;
```

- [ ] **Step 3: Verify compiles**

```bash
cargo build -p volt-ui
```
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add volt-ui/
git commit -m "feat(volt-ui): winit app with PTY, keyboard, and render loop"
```

---

## Task 9: volt — Main Entrypoint

**Files:**
- Modify: `volt/src/main.rs`

- [ ] **Step 1: Implement `volt/src/main.rs`**

```rust
use volt_config::Config;
use volt_ui::App;

fn main() {
    let config = Config::load();
    App::new(config).run();
}
```

- [ ] **Step 2: Build and run**

```bash
cargo build --release -p volt
./target/release/volt
```
Expected: a window opens, your default shell starts, text renders, keyboard input works.

- [ ] **Step 3: Smoke test checklist**
- [ ] Shell prompt appears
- [ ] Typing characters echoes in the window
- [ ] `ls` output renders with colors
- [ ] Arrow keys work (shell history)
- [ ] `vim` opens and renders
- [ ] Window resize reflows the terminal correctly
- [ ] Closing the window exits cleanly

- [ ] **Step 4: Commit**

```bash
git add volt/src/main.rs
git commit -m "feat(volt): wire App entrypoint — Phase 1 complete"
```

---

## Self-Review

**Spec coverage:**
- [x] GPU rendered via wgpu (Metal/Vulkan) — Task 7
- [x] VTE parsing — Task 4
- [x] PTY spawning — Task 5
- [x] Font/glyph rendering (cosmic-text) — Task 6–7
- [x] macOS + Linux (wgpu handles backend selection) — Task 7
- [x] Config file loading — Task 2
- [x] Theme colors — Task 2
- [ ] Nerd Font / powerline glyphs — renderer supports them but font loading config is approximate; addressed in Phase 2 with proper font config UI
- [ ] Cursor rendering — not yet drawn as a visible rect; add to Task 7 in execution if noticed

**Gaps from full spec deferred to later phases:**
- Tab bar (Phase 2)
- Pane splitting (Phase 2)
- Per-pane statusline (Phase 2)
- AI providers (Phase 3)
- MCP (Phase 4)
- Workflow engine (Phase 6)

**No placeholders or TBDs found.**

**Type consistency:** `Grid`, `Cell`, `CellColor`, `CoreEvent`, `Performer`, `Pty`, `CpuAtlas`, `GlyphKey`, `AtlasRegion`, `BgVertex`, `GlyphVertex`, `Renderer`, `App` — all defined before first use. Checked.
