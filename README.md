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

  let name = inline::input("Project name").placeholder("my-app").ask()?;
  let manager = inline::select("Package manager")
    .items(["bun", "pnpm", "npm"])
    .note(0, "(recommended)")
    .filter()
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
cargo run --example deep-thought --features gradient  # the whole crate, autocomplete included
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

Everything expensive is opt-in. The default build is 82 crates; with `full`, 94.

| feature | what it turns on | pulls in |
| --- | --- | --- |
| *(default)* | prompts, components, signals, rímel, fuzzy search | crossterm, ratatui, eyre, fuzzy-matcher |
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
| `src/components/` | the widgets — input, select, multiselect, autocomplete, confirm, progress, text |
| `src/inline/` | sequential prompts: `intro`, `input`, `select`, `autocomplete`, `task`, `outro` |
| `crates/norimel/` | rímel — styled text blocks, tailwind-ish utilities |
| `examples/` | one runnable file per idea |
| `internal/` | roadmap, decisions and log — why things are the way they are |

## Search & autocomplete

`Select` and `MultiSelect` grow a `/`-triggered fuzzy search with `.filter()` — opt-in, so a
list of options that happens to contain a literal `/` doesn't suddenly grow a mode nobody
asked for. A query matches an option's own text or its `.note()`, so a hint is fair game too.

```rust
inline::multiselect("Dependencies")
  .items(["react", "svelte", "vue"])
  .filter()
  .ask()?;
```

`Autocomplete` is the other direction: a text field with suggestions narrowed live as you
type. Relaxed by default — a typed value that matches nothing is still a fine answer, for
"pick one fast, or make one up." Call `.strict()` when the answer has to be one of the items,
word for word:

```rust
let tag = inline::autocomplete("Tag").items(["bug", "feature", "chore"]).ask()?;          // relaxed
let pm = inline::autocomplete("Package manager").items(["bun", "pnpm", "npm"]).strict().ask()?;
```

`Input` (and anything built on it, `Autocomplete` included) also takes `.placeholder()` for
muted hint text while empty, and knows ctrl+←/→ to jump a word, ctrl+backspace/delete to
remove one, and shift+arrows (ctrl+shift for a whole word) to select — Backspace, Delete, or
typing over a selection replaces it, the way it does everywhere else.

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

### Effects

An animation is a function from a clock to a block; you rebuild it every frame. `animate`
slides a colour ramp across the text, `pulse` gives the whole block one colour and walks that
along the ramp instead. Bring your own ramp with `Ramp::new` — colorgrad is re-exported so you
don't need it in your manifest. Any of the three paints the foreground by default; chain
`.on_bg()` to have the ramp land on the background instead.

For a motion neither of those covers, `map_cells` hands you every cell once the shape is
settled and takes back the style you want, and `animated()` tells the loop to keep the frames
coming. That's a complete effect, in your crate, without patching this one. The animation
section of the `norimel` docs walks through it. Once the movement should stop for good — an
answered prompt, a finished task — `settled(color)` freezes it to one still colour.

## License

MIT.
