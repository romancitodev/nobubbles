
La idea central sería: **`nobubbles` como framework de terminal que pueda ir desde un prompt inline hasta una TUI fullscreen, usando los mismos componentes.**

### 1. Inline prompts

Interacciones que ocupan solamente las líneas necesarias.

```text
? Project name › my-app
```

```rust
let name = Input::new("Project name").run()?;
```

---

### 2. Select / multiselect

```text
? Environment
  › development
    staging
    production
```

```rust
let env = Select::new("Environment")
    .items(["development", "staging", "production"])
    .run()?;
```

---

### 3. Confirmaciones

```text
? Deploy to production? (y/N)
```

```rust
Confirm::new("Deploy to production?")
    .default(false)
    .run()?;
```

---

### 4. Spinners / progreso

```text
⠋ Building project...
```

y:

```text
Uploading ███████████████░░░░ 74%
```

```rust
Spinner::new("Building project").run(...)?;

Progress::new()
    .total(100)
    .run(...)?;
```

---

### 5. Mensajes persistentes

Una vez terminado algo, queda en la terminal:

```text
✔ Build completed
✔ Uploaded
✔ Deployment successful
```

Ideal para CLIs.

---

### 6. Fullscreen TUI

Entrar al alternate screen:

```text
┌──────────────────────────────────────────────┐
│ Projects                                     │
├──────────────────────────────────────────────┤
│ › my-api                                     │
│   frontend                                   │
│   backend                                    │
│   infrastructure                             │
├──────────────────────────────────────────────┤
│ ↑↓ Navigate   Enter Select   q Quit          │
└──────────────────────────────────────────────┘
```

```rust
App::fullscreen()
    .run(|cx| {
        cx.render(ProjectView)
    })?;
```

---

### 7. Widgets reutilizables

La misma base debería permitir:

```text
Text
Input
Select
MultiSelect
Confirm
Checkbox
Radio
Progress
Spinner
Table
List
Tree
Tabs
Panel
```

---

### 8. Composición

Poder construir interfaces complejas combinando componentes:

```rust
Column::new()
    .child(Header::new("Projects"))
    .child(
        Row::new()
            .child(ProjectList::new())
            .child(ProjectDetails::new())
    )
    .child(StatusBar::new());
```

---

### 9. Layout system

Algo parecido a CSS/Flexbox:

```rust
Row::new()
    .child(Fixed(30, sidebar))
    .child(Flex(1, content))
```

Con:

* rows
* columns
* fixed
* percentage
* flex
* padding
* margin
* alignment

---

### 10. Estilos

```rust
Text::new("Error")
    .fg(Color::Red)
    .bold()
```

Y estilos reutilizables:

```rust
let danger = Style::new()
    .fg(Color::Red)
    .bold();
```

---

### 11. Input unificado

Una abstracción común para:

```text
keyboard
mouse
resize
ctrl+c
escape
enter
arrows
```

para que los widgets no tengan que preocuparse demasiado por cómo llega el evento.

---

### 12. Inline → fullscreen

**Esta sería una de las features estrella.**

Una CLI puede empezar así:

```text
? Environment › production
? Version › v2.4.1
✔ Configuration complete

Opening deployment monitor...
```

y pasar automáticamente a:

```text
┌──────────────────────────────────────────┐
│ Deployment v2.4.1                        │
│                                          │
│ ✓ Build                                  │
│ ✓ Upload                                 │
│ → Deploying...                           │
│ ○ Health check                           │
└──────────────────────────────────────────┘
```

La aplicación no necesita cambiar completamente de paradigma.

---

### 13. Mismo widget, distintos renderers

Arquitectónicamente:

```text
                Widget Tree
                     │
              ┌──────┴──────┐
              │             │
          Inline         Fullscreen
          Renderer         Renderer
              │             │
          Terminal       Alternate
```

Eso probablemente debería ser **el corazón de `nobubbles`**.

---

### 14. Animaciones / redibujado

Para fullscreen:

```rust
Spinner::new()
    .interval(Duration::from_millis(80))
```

y eventualmente:

* transiciones
* spinners
* progress animado
* loaders
* cursor
* blinking

---

### 15. Event loop manejado por el framework

El usuario debería poder escribir:

```rust
App::new()
    .render(ui)
    .run()?;
```

sin tener que implementar manualmente:

```text
terminal init
alternate screen
raw mode
poll events
handle resize
redraw
restore terminal
```

---

## El roadmap que yo haría

**Fase 1 — Core**

```text
Terminal
Renderer
Events
Widget
Layout
Style
```

**Fase 2 — Inline**

```text
Input
Select
Confirm
Spinner
Progress
```

**Fase 3 — Fullscreen**

```text
App
Layout
List
Table
Panel
Tabs
Mouse
```

**Fase 4 — La feature diferencial**

```text
Inline ↔ Fullscreen
```

Y recién después:

```text
Tree
Autocomplete
Forms
Animations
Theming
Accessibility
```
