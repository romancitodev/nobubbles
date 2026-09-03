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
