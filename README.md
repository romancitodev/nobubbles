# nobubbles

Reactive TUI for Rust. Signals instead of messages, inline and fullscreen.

A component exposes `view()` and `on_key()`, both taking `&self`. Everything mutable lives in
`Copy` signals, so there is no `Message` type to map through and no plumbing between parents
and children. One loop drives three faces: sequential inline prompts, a live inline view, and
a fullscreen app.

Still early — the API moves.

<!-- gif: prompts -->

```rust
use nobubbles::inline;

fn main() -> eyre::Result<()> {
  let session = inline::intro("create-app")?;

  let name = inline::input("Project name").ask()?;
  let manager = inline::select("Package manager")
    .items(["bun", "pnpm", "npm"])
    .note(0, "(recommended)")
    .strict()
    .ask()?;

  inline::task("Installing", |report| {
    report.say("resolving");
    report.say("linking");
  })?;

  inline::outro(session).with(format!("cd {name} && {} dev", ["bun", "pnpm", "npm"][manager]));
  Ok(())
}
```

## Examples

```
cargo run --example create              # every prompt in the crate, as a scaffolder
cargo run --example styles              # one CLI, three looks
cargo run --example commit              # runs git for real and waits for it
cargo run --example build               # 20 crates, 4 cores, one bar per core
cargo run --example download --features gradient   # a bar that walks a colour ramp
cargo run --example fx --features full             # animated text and tachyonfx effects
```

<!-- gif: styles -->

<!-- gif: download -->

## Features

Everything expensive is opt-in. The default build is 80 crates; with `full`, 92.

| feature | what it turns on | pulls in |
| --- | --- | --- |
| *(default)* | prompts, components, signals, rímel | crossterm, ratatui, eyre |
| `gradient` | `Ramp`, `Block::gradient`, `Block::animate` | colorgrad (+9, phf proc-macros) |
| `fx` | `Ctx::effect` | tachyonfx (+6, bon, prettyplease) |
| `full` | `fx` + `gradient` | both of the above |
| `compio` | nothing yet — reserved async backend | compio (+64) |

Examples that need a feature declare it, so `cargo build --examples` never pulls what it
doesn't use.

## Layout

| path | what |
| --- | --- |
| `src/` | the engine: loop, signals, effects, render |
| `src/components/` | the widgets — input, select, multiselect, confirm, progress, text |
| `src/inline/` | sequential prompts: `intro`, `input`, `select`, `task`, `outro` |
| `crates/norimel/` | rímel — styled text blocks, tailwind-ish utilities |
| `examples/` | one runnable file per idea |
| `internal/` | roadmap, decisions and log — why things are the way they are |

## Colours

Everything ships painted in [Catppuccin Mocha](https://catppuccin.com). The palette is
`norimel::palette` (re-exported as `nobubbles::style::palette`), and every `*Style` struct
takes an override, so a theme is a struct literal away.

## rímel

The styling crate next door, usable on its own. A block is rows of styled runs; the utilities
only take notes and the shape is composed once, at the end, so they commute.

```rust
use norimel::{self as rimel, Color};

let badge = rimel::text("astro").bg(Color::Magenta).fg(Color::Black).bold().px(1);
println!("{}", rimel::row([badge, rimel::text("  ready in 300ms")]));
```

It prints ANSI through `Display` and paints cells through `Block::runs`, which is how the same
value works in a `println!` and inside a live view.

## License

MIT.
