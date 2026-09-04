# Bitácora

Una entrada por sesión, **la más nueva arriba**. Si la plantilla se siente
pesada, dejá campos vacíos — una bitácora que no se llena no sirve de nada.

El campo que más vale a los tres meses es **Callejones sin salida**: es lo único
que no se puede reconstruir leyendo el código.

<!-- plantilla — copiar debajo de esta línea

## AAAA-MM-DD — Fase N: título corto

**Hecho:**
-

**Aprendido:**
-

**Callejones sin salida:** (qué probé que NO funcionó, y por qué)
-

**Abierto:**
-

**Siguiente:**
-

-->

---

## 2026-09-03 (d) — Fase 4: arranque de Input

**Hecho:**
- `on_key` se resolvió como método propio de `Input`, no del trait `Component`.
  `Component` se queda sólo con `view()`. El único caso real que pide dispatch
  genérico es un contenedor (`Tabs`/`List`/`Panel`, Fase 6) reenviando a un
  hijo desconocido, `Input` es hoja, no lo necesita.
- `src/components/input.rs`: `Input` (`Signal<String>` + `Signal<usize>`,
  cursor en índice de grafema vía `unicode-segmentation`), `on_key` maneja
  `Char`/`Backspace`/`Delete`/`Left`/`Right` por grafema, no por `char` ni por
  byte. `impl Render for Input` propio.
- `Render::height(&self, width) -> u16` (default 1), para que un contenedor
  pueda medir a sus hijos antes de dibujar.
- `Column`: composición de varios `Render` heterogéneos vía
  `Vec<Box<dyn ErasedRender>>` (trait interno con `self: Box<Self>`, para
  sortear que `Render::render` no es object safe con `self` por valor).
  `column!` macro como azúcar sobre `Column::new().child(...)`.
- `Column::render` usa `Constraint::Length(child.height(width))` por hijo en
  vez de `Fill(1)` parejo, se ajusta a lo que cada hijo mide.
- Viewport inline dinámico: `engine::poll` pasó a ser dueño del `Terminal`
  (no `&mut`) y recibe un closure `resize(u16) -> Terminal<B>`. Después de
  cada draw compara la altura medida (`Ctx::wanted_height`) contra la actual
  y reconstruye si cambió, con `terminal.clear()` antes para no dejar basura
  de la altura vieja pintada en pantalla.
- `Inline::new(height, fps)` pasó a `Inline::run(fps, ui)`, sin `height` — el
  loop la mide solo, no hace falta que el usuario la adivine.
- `examples/input.rs` y `examples/counter.rs` actualizados a la API nueva.

**Aprendido:**
- `impl Trait` en retorno sólo admite un tipo concreto fijo. Un `if/else` que
  devuelve `Paragraph` en una rama y algo distinto en la otra no compila.
  `Column::new().child(x)` y `.child(a).child(b)` son el mismo tipo (`Column`)
  sin importar cuántos hijos tenga el `Vec` interno, así que ramificar la
  vista según estado (formulario con "¿confirmaste?") sale gratis con
  `Column` y no salía con una versión basada en tuplas.
- `Terminal::resize()` de ratatui no sirve para cambiar la altura de un
  `Viewport::Inline` a una nueva, usa la altura con la que se construyó
  originalmente (confirmado leyendo el código fuente de
  `ratatui-core::terminal::resize`). La única vía sigue siendo reconstruir el
  `Terminal` entero, tal como ya había quedado anotado en Fase 2.
- `Terminal::clear()` en modo inline limpia desde el origen del viewport
  hacia abajo, preservando el cursor. Hace falta llamarlo en el `Terminal`
  viejo antes de reemplazarlo, si no el contenido de la altura anterior queda
  pintado en pantalla.

**Callejones sin salida:**
- `impl<A: Render, B: Render> Render for (A, B)` (y aridades mayores vía
  macro), rechazado con E0119 por el blanket impl sobre `Widget` más la
  coherencia de tipos "fundamentales" (las tuplas cuentan como tales). La
  salida fue envolver en un struct local (`Column`), que no tiene ese
  problema.
- El primer intento de forzar el primer dibujado usó
  `static FIRST_RENDER: std::sync::Once`. Fallaba porque `Once` es una única
  vez *por proceso*, no por invocación de `poll` — se hubiera roto en la
  Fase 7 (inline seguido de fullscreen en el mismo proceso). Se cambió a una
  variable local, `first_draw`.
- `crossterm::execute!(stdout(), MoveToColumn(0))` antes de reconstruir el
  `Terminal` al achicar, probado en una terminal real (Windows) y **no
  arregla** el salto de línea de abajo. La hipótesis de la columna del
  cursor queda descartada, el fix se dejó en el código porque no hace daño,
  pero el bug real sigue sin causa confirmada.

**Abierto:**
- Bug visual sin resolver: al achicar el viewport (2 líneas a 1, al confirmar
  el formulario), el salto de línea contra el output previo de cargo se ve
  distinto al del camino de crecer. Pendiente de investigar de cero mañana,
  probablemente con `RUST_LOG`/tracing del lado de crossterm o comparando
  a mano qué secuencia de escape termina emitiendo cada camino.
- `examples/input.rs`: el guard `_ if !form.submitted.get()` en el `match` de
  `on_key` es manual. Recién en Fase 5 (prompts que devuelven un valor y
  terminan su propio loop) deja de hacer falta acordarse de esto a mano.
- El blanket `impl<W: Widget> Render for W` no mide un `Paragraph` multilínea
  de verdad, usa el default de 1. No importa hoy (sólo hay texto de una
  línea), pero si aparece un `Paragraph` con wrap, `height()` va a mentir.

**Siguiente:**
- Investigar de cero el bug del salto de línea al achicar el viewport, con
  la hipótesis de la columna del cursor ya descartada.
- Seguir Fase 4: `Select`, `Confirm`, `Spinner`, `Progress`, cada uno con su
  `*Style`.

---

## 2026-09-03 (c) — Fase 3: arranque

**Hecho:**
- Borrado ELM completo (D-009): `src/components/*`, `src/core/*` (`Model`,
  `Command`, `Internal`, `QuitHandle`, `ProgramBuilder`, `Program`),
  `examples/hello_world.rs`, `examples/steps.rs`.
- `lib.rs` actualizado: sin `pub mod components` / `pub mod core`, doc comment
  del ejemplo ELM reemplazado por una línea.
- `cargo check --lib` limpio. Sólo warnings `dead_code` en `render.rs`
  (esperado — nada lo llama todavía, eso lo cablea el loop de Fase 3).

**Abierto:**
- Nada de esto está commiteado todavía. Pendiente decidir si el borrado va en
  un commit separado del diff previo de `render.rs` (comentario doc +
  reformateo, sin commitear de antes de esta sesión, no tocado).
- `CLAUDE.md` modificado y `examples/signals.rs` sin trackear siguen sin
  commitear, de antes de esta sesión — tampoco tocados.

**Siguiente:**
- Extraer `poll_timeout(dirty: bool, since_render: Duration, frame: Duration)
  -> Duration` como función pura — primer paso de verdad de la Fase 3, antes
  de tocar `Component`:
  1. Crear `src/loop_.rs` (con guión bajo, `loop` es keyword).
  2. Declarar `mod loop_;` en `lib.rs` (sin `pub` todavía).
  3. Escribir la función: si `dirty`, `frame.saturating_sub(since_render)`; si
     no, la constante `IDLE` (250ms). El `saturating_sub` es la Trampa 1 del
     ROADMAP — sin él, underflow o pánico.
  4. `#[cfg(test)] mod tests` con 3 asserts: (a) dirty + frame no cumplido →
     resto positivo, (b) dirty + frame ya pasado → `Duration::ZERO`, (c) no
     dirty → siempre `IDLE`, sin importar `since_render`. Es la Trampa 2.
  5. `cargo test loop_` para validar antes de seguir con `trait Component`.
  - Lógica del loop viejo para copiar la forma del `if/else`:
    `git show HEAD~1:src/core/program.rs` (o `git log -- src/core/program.rs`
    si el borrado de arriba ya quedó commiteado para cuando retomes).

---

## 2026-09-03 (b) — Fase 2: renderer

**Hecho:**
- `src/render.rs`: `inline`/`fullscreen` genéricos sobre `Backend`, `pub(crate)`
  (D-004, no se exponen tipos de ratatui). `enter_fullscreen`/`leave_fullscreen`
  con raw mode + alt screen para el caso real (`CrosstermBackend<Stdout>`).
- 4 tests con `TestBackend`, sin terminal real: altura fija del inline, área
  completa del fullscreen, crecer/encoger recreando el `Terminal`, e
  `insert_before` sin romper el scrollback de arriba.
- D-006 confirmada: `Component` con `&self`.
- `ratatui` agregado con `default-features = false, features = ["crossterm"]`
  únicamente — nada de `all-widgets` ni el resto.

**Aprendido:**
- `Viewport::Inline(height)` es una altura fija desde la construcción.
  `Terminal::resize` existe, pero para inline usa el `height` que ya estaba
  guardado en `self.viewport`, no uno nuevo. `autoresize` sólo reacciona a que
  cambió el tamaño de la ventana de la terminal, no a que la vista quiere más o
  menos líneas. La única forma de crecer/encoger es recrear el `Terminal`
  entero con un `Viewport::Inline` nuevo — para `CrosstermBackend<Stdout>` es
  gratis porque `Stdout` es sólo un handle, no arrastra estado.
- `Terminal` no da forma de recuperar el backend que se le pasó por valor (sin
  `into_inner` ni nada similar) — confirmado grepeando el crate entero. No
  importa para el caso real, pero condiciona cómo se testea con `TestBackend`
  (no se puede encadenar dos `Terminal` sobre el mismo backend).
- Trampa real: `Frame::area()` (adentro del closure de `draw`) y el campo
  `area` del `CompletedFrame` que devuelve `draw()` **no son lo mismo** —
  el primero es el viewport, el segundo es `last_known_area` (el tamaño entero
  del backend). Incluso con el nombre repetido, hay que leer el height desde
  adentro del closure.
- `ratatui-core` es para autores de widgets, no para apps — la propia doc del
  crate lo dice. La facade `ratatui` re-exporta `Terminal`/`Viewport`/etc. tal
  cual, así que bajar a `ratatui-core` no gana nada.

**Callejones sin salida:**
- `cargo add ratatui -F scrolling-regions` rompe la resolución: `ratatui
  v0.30.2` pide `ratatui-termwiz = "^0.1.2"`, que no existe publicado en
  crates.io (sólo `0.1.0`). Pasa aunque no actives el feature `termwiz`, porque
  Cargo igual resuelve la versión de todo dependency opcional declarado. Se
  revirtió a `features = ["crossterm"]` solo.

**Abierto:**
- `enter_fullscreen`/`leave_fullscreen` sin guard de panic — si algo panickea
  entre medio, la terminal del usuario queda en raw mode + alt screen. Se
  resuelve con un guard `Drop` o panic hook cuando exista el loop de Fase 3, no
  antes.
- Pulido menor de `signals.rs` (Fase 1) sigue sin tocar: ver LOG del
  2026-09-03 (a).

**Siguiente:**
- Fase 3: borrar ELM (D-009), `trait Component` con `&self` (D-006), loop
  nuevo.

---

## 2026-09-03 (a) — Fase 1: signals

**Hecho:**
- `src/signals.rs` completo: arena thread-local, `Signal`/`ReadSignal`/`WriteSignal`
  `Copy`, guards `SignalRef`/`SignalRefMut`, flag de dirty.
- 8 tests en verde. Miri limpio sobre los dos `unsafe`.
- D-012 (arena a mano), D-013 (`!Send`), D-014 (guards RAII).

**Aprendido:**
- Un `Rc<RefCell<_>>` por slot, en vez de un `RefCell` alrededor de toda la
  arena, es lo que permite `total.update(|t| *t += precio.get())` sin panic.
  El truco es sacar el `Rc` y soltar el préstamo de la arena antes de tocar el
  valor.
- `DerefMut` no puede prender el dirty porque no tiene punto de cierre. Un guard
  con `Drop` sí. Esa es la razón de ser de los guards, no la ergonomía.
- Las implementaciones reales viven en `ReadSignal`/`WriteSignal` y `Signal`
  reenvía. Así cada `unsafe` queda en un solo lugar.

**Callejones sin salida:**
- `impl Deref for Signal<T>`. Imposible: el guard no tiene dónde vivir. Y el
  intento con `downcast_ref::<&T>()` **compilaba y panicaba el 100% de las
  veces**, porque `&'static T` es `Copy` y se copiaba afuera del guard. Regla
  que sale de ahí: `downcast` pide el tipo *guardado*, nunca el de la referencia
  que querés sacar. Ver D-014.
- `use std::borrow::BorrowMut` (auto-import de rust-analyzer) secuestró
  `slot.borrow_mut()`. La impl blanket `BorrowMut<T> for T` matchea el receptor
  antes de derefear al `RefCell`, así que devolvía `&mut Rc<...>` en vez de
  `RefMut`. El error era E0282 en el closure y no mencionaba el trait por ningún
  lado. Perdí un rato largo. Si `.borrow()` anda y `.borrow_mut()` no, mirar los
  imports antes que el código.
- `should_panic(expected = "already borrowed: BorrowMutError")` era la dirección
  equivocada: el caso es `borrow_mut` primero y `borrow` adentro, o sea
  "already mutably borrowed".

**Abierto:**
- Pulido de `signals.rs`: falta el `//!` con los dos `compile_fail` (D-008 y
  D-013 no tienen verificación), los tests `split_halves_share_the_slot` y
  `can_create_a_signal_inside_update`, las anotaciones `'_` que deberían ser
  `'static`, `RT`/`Runtime` que siguen públicas, y el marcador `ponytail:`.
- D-006 (`Component` con `&self`) sigue sin confirmar. Antes de Fase 3.

**Siguiente:**
- Fase 2: renderer sobre ratatui.

---

## 2026-09-02 (b) — Loop en un solo thread

**Hecho:**
- Terminado el loop single-thread. `event::poll` en el main, sin threads, sin
  `Arc<Mutex>`, bounds `Send + Sync` fuera, `Internal::Tick` borrada.
- Cuatro commits, árbol limpio. Checkpoint en `119aade`.

**Aprendido:**
- El bloque de render tiene que ser hermano del `while` que drena el canal, no
  hijo. Como al canal no le escribe nadie (sin threads, sólo `QuitHandle`),
  `try_recv()` falla en la primera vuelta y todo lo que esté adentro del `while`
  es inalcanzable. Se cayó dos veces en la misma trampa, con una llave de
  diferencia cada vez.
- El filtro de teclas quedó invertido una vez (`==` en vez de `!=`), que descarta
  justo los `Press`. Síntoma: la app no responde a nada. Fácil de confundir con
  el bug de render, que da "responde pero no dibuja".

**Callejones sin salida:**
- Los dos bugs de arriba no se ven leyendo el código, sólo trazando. Ambos
  compilan y ambos dan una app que arranca. Si algo vuelve a fallar así, trazar
  qué escribe al canal antes de mirar la lógica.

**Abierto:**
- D-006 (`Component` con `&self`) sigue sin confirmar. Antes de Fase 3.
- Warnings restantes: todos del código ELM que muere en Fase 3. Ignorar.

**Siguiente:**
- Fase 1: signals.

---

## 2026-09-02 — Diseño, sin código

**Hecho:**
- Revisado todo lo que había: `program.rs`, `command.rs`, `components/*`, los dos ejemplos.
- Definido el rumbo: signals en vez de mensajes. `ROADMAP.md` + `DECISIONS.md` (D-001 a D-011).
- Reordenado el plan: se cayó la Fase 0 de refactor. Ahora arranca en signals.

**Aprendido:**
- El dolor no era "ELM vs reactivo", era composición: `Input` implementa `Model`
  con su propio `InputMsg` y `App` no puede embeberlo sin un `map` a mano.
- El loop tiene tres problemas que se comen entre sí: el tick thread manda
  `Tick` a 60fps siempre (re-render infinito con la app idle), el event thread
  lockea el modelo para `event()` mientras el main lo lockea para `update()`, y
  `render()` hace `Clear(FromCursorDown)` + `Print` sin diff.
- La forma del loop reactivo es la misma que la del ELM salvo cuatro líneas en el
  medio. Lo que cambia de verdad es qué prende el dirty: antes lo prendía el
  loop a mano, ahora lo prende la escritura al signal.
- La arena thread-local choca con los efectos en threads: un thread de fondo no
  ve la arena y no puede escribir signals. Se resuelve mandando closures por el
  canal (D-010), pero condiciona la Fase 6.

**Callejones sin salida:**
- Arrancar por refactorizar el loop ELM (la vieja Fase 0). El fix del bug UTF-8
  de `InputMsg::Delete` fue la señal: `InputMsg` sólo existe porque ELM necesita
  que el componente emita mensajes, así que el tipo entero desaparece con la
  reactividad. Arreglar código con fecha de defunción. Ver D-009.

**Abierto:**
- D-006 (`Component` con `&self` en vez de `&mut self`) sigue sin confirmar. Es la
  pieza de la que cuelga todo lo demás. Decidir antes de la Fase 3.

**Siguiente:**
- Fase 1: signals. Rust puro, sin terminal, sin tocar nada de lo que existe.
