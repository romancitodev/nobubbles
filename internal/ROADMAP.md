# nobubbles — Roadmap

TUI reactivo en Rust. Inspirado en bubbletea, pero con signals en vez de
mensajes, y con modo inline (cliclack) como ciudadano de primera.

**Última sesión:** ver [LOG.md](LOG.md) · **Decisiones:** ver [DECISIONS.md](DECISIONS.md)

## Estado

**Fase 2 lista.** `src/render.rs`: constructores `inline`/`fullscreen`
genéricos sobre `Backend` (testeados con `TestBackend`, sin terminal real), más
`enter_fullscreen`/`leave_fullscreen` para el caso real con crossterm. 4 tests
en verde: altura fija del inline, área completa del fullscreen, crecer/encoger
recreando el `Terminal`, e `insert_before` sin romper el scrollback.

D-006 confirmada. **Siguiente: Fase 3 (Component + loop nuevo).**

---

## La idea en un párrafo

Un `Component` expone `view()` y `on_key()`, ambos con `&self`. Todo el estado
mutable vive en signals `Copy`. Eso elimina el tipo `Message` y con él todo el
plumbing de composición que hace doloroso a bubbletea, y le da al loop
información exacta de cuándo re-renderizar. Un solo motor sirve tres caras:
prompts secuenciales inline, vista viva inline, y app fullscreen.

## El código ELM se borra entero

`Model`, `Command`, `Program`, `Internal`, `QuitHandle`, `components/*` y los dos
ejemplos mueren en la Fase 3. **No se refactorizan antes** — ver D-009. Hasta
entonces quedan compilando, sin molestar. Las fases 1 y 2 no los tocan.

Excepción: el esqueleto del loop, que sí sobrevive. Por eso vale terminar el WIP.

---

## Fase 1 — Signals

Rust puro, sin terminal, sin tocar nada de lo que existe. Se testea con asserts.

- [ ] Arena thread-local: `thread_local!` con un `RefCell<Vec<Box<dyn Any>>>`
- [ ] `Signal<T>(usize, PhantomData<T>)` — `Copy` porque es un índice, no un `Rc`.
      Esto mata toda la ceremonia de clonar antes de cada closure.
- [ ] `signal(v)`, `.get()`, `.set(v)`, `.update(...)`
- [ ] `ReadSignal<T>` y `WriteSignal<T>`: mismo índice, misma slot, distintos
      tipos. `Signal::split()` y `.read_only()`. Ver D-008 — va desde el día uno
      porque agregarlo después es breaking, no porque acelere nada hoy.
- [ ] Flag global de dirty: toda escritura lo prende, el loop lo lee y lo apaga.

**Explícitamente NO:** grafo de dependencias, memos, efectos, orden topológico.
El split read/write de D-008 sí entra: son tipos, no tracking. Se re-renderiza la
vista entera y el diff del buffer decide qué pintar. En una terminal de unos
miles de celdas, el tracking fino no compra nada.

**Listo cuando:** creo dos signals, escribo uno, el flag se prendió y el otro no
se vio afectado. Más un check de compilación: escribir a un `ReadSignal` no debe
compilar.

**Investigar:** cómo lo hace leptos_reactive (arena + slotmap). El truco del
índice `Copy` es de ahí. Ojo con el leak: la arena no libera hasta el final del
proceso — aceptable para una TUI que corre una vez, anotarlo como techo conocido.

**Ojo:** la arena es thread-local. Eso define la firma del canal de efectos de la
Fase 6 — ver D-010.

---

## Fase 2 — Renderer

Se puede manejar con una vista hardcodeada. Todavía no hace falta ni loop ni
`Component`.

- [x] ratatui como dependencia **interna**, no expuesta en la API pública.
- [x] Dos modos: viewport inline y alt screen.
- [x] En inline: medir la altura que pide la vista y crecer/encoger.
- [x] `Terminal::insert_before` para empujar líneas al scrollback (es lo que
      deja el transcript de los prompts al salir).

**Listo cuando:** un contador inline que crece de 1 a 5 líneas y vuelve a 1 sin
romper el scrollback de arriba. Verificado con tests (`TestBackend`, sin
terminal real) en vez de un demo visual — no hay loop todavía que lo dibuje de
verdad, eso es Fase 3.

**Investigar:** `TerminalOptions` con `Viewport::Inline(u16)`, y
`Terminal::insert_before`. Es la parte de ratatui menos documentada — leer el
ejemplo `inline.rs` del repo.

---

## Fase 3 — Component + loop nuevo

Acá se borra ELM. El loop no se escribe de cero: se le cambian las líneas del
medio al que quedó del WIP.

- [x] Borrar `Model`, `Command`, `Internal`, `QuitHandle`, `components/*`, los
      dos ejemplos.
- [x] `engine.rs`: `poll_timeout(dirty, since_render, frame) -> Duration`,
      función pura, 3 tests.
- [x] `trait Component` con `view(&self) -> impl Render`, más `Render`/`Rect`/
      `Buffer` como newtypes sobre ratatui (D-016). `src/components/mod.rs`.
- [ ] `on_key(&self, k: Key) -> bool` en los widgets que lo necesiten — `true`
      = consumido. Reemplaza `Handled` (D-015): ya no hay un valor de retorno
      con semántica de "salir". Se agrega recién cuando `Input` (Fase 4) lo
      necesite, no antes.
- [x] `quit()` / `should_quit()` — flag global en el mismo `Runtime` de
      `signals.rs`, al lado de `dirty`.
- [x] `Inline::new(height, fps).run(|cx| ...)` — el entry point real (D-015),
      `src/app.rs`. `Ctx::render`/`Ctx::key` dan lo que hace falta. Falta el
      equivalente `Fullscreen`/`FullscreenApp` (mismo mecanismo sobre
      `render::enter_fullscreen`/`leave_fullscreen`, todavía sin usar).
- [ ] Dispatch/foco automático hacia un root queda pospuesto (D-015): lo
      orquesta el closure del usuario, o un widget compuesto que lo resuelve
      puertas adentro. Si hace falta volver a un dispatch automático, es
      opt-in — no el único camino.
- [ ] `Theme` global + structs `*Style` por componente.

**Listo cuando:** un ejemplo usa `App::inline().run(|cx| ...)`, compone un
`Input` adentro leyendo `input.value()`, y no existe ningún tipo `Message` ni
`Handled` en el ejemplo. **Cumplido** con `examples/counter.rs`: un
`Component` con un `Signal<i32>`, corriendo de punta a punta en una terminal
real (`Inline::run`, sin `Input` todavía porque eso es Fase 4 — un contador
alcanza para probar la cadena completa).

### La forma del loop

Lo que cambia respecto del WIP: hoy el loop prende `dirty` a mano después de
`update()`. En reactivo no lo prende nadie a mano — lo prendió la escritura al
signal. Lo que cambia respecto de la versión anterior de este mismo esqueleto:
salir ya no es un `Handled::Quit` que hay que bubblear desde `on_key` — es
`is_quitting()`, el mismo tipo de flag que `dirty` (D-015). Y en vez de
`render(&root)`, el redibujado corre el closure que el usuario le pasó a
`.run()`.

    const IDLE: Duration = Duration::from_millis(250);

    let frame = Duration::from_secs_f64(1.0 / fps as f64);
    let mut last_render = Instant::now();

    loop {
        let timeout = poll_timeout(is_dirty(), last_render.elapsed(), frame);

        if event::poll(timeout)? {
            let ev = event::read()?;
            // filtro de KeyEventKind::Press acá
            cx.set_event(ev); // lo que el closure del usuario puede leer
        }

        while let Ok(f) = rx.try_recv() { f(); }   // efectos, Fase 8 — ver D-010

        if is_quitting() { break; }

        if is_dirty() && last_render.elapsed() >= frame {
            terminal.draw(|frame| user_closure(&mut Cx::new(frame)))?;
            clear_dirty();
            last_render = Instant::now();
        }
    }

**Trampa 1 — latencia.** El instinto es "junto eventos durante un frame, después
dibujo". Eso mete 16ms de retraso en *cada* tecla y se siente pastoso. El
`saturating_sub` da cero cuando ya pasó el frame anterior, así que la primera
tecla después de un rato dibuja al instante. Sólo la segunda dentro del mismo
frame espera.

**Trampa 2 — idle.** Con un poll de un frame siempre, la app quieta se despierta
60 veces por segundo sin hacer nada. Por eso el `IDLE` de 250ms cuando no hay
nada pendiente: quieta son 4 despertadas por segundo.

**Consecuencia:** `fps` deja de significar "cada cuánto dibujo" y pasa a
significar "cuánto coalesco una ráfaga". Documentarlo así.

**Check:** extraé la decisión del timeout a función pura
`poll_timeout(dirty, since_render, frame) -> Duration` y ponele tres asserts. Es
lo único con lógica de verdad, y si se rompe falla en silencio — o la app queda
pastosa, o quema CPU.

---

## Fase 4 — Componentes de prompt

- [ ] `Text`, `Input`, `Select`, `MultiSelect`, `Confirm`, `Spinner`, `Progress`
- [ ] Cada uno con su `*Style` con las partes nombradas.

`Input` borrando: sobre `Signal<String>`, y ojo con grafemas compuestos — un
emoji con modificador de tono no se borra bien con un `pop()`.

---

## Fase 5 — Inline API + capa lipgloss

- [ ] `inline::{intro, input, select, confirm, spinner, outro}` — cada prompt es
      un `inline::app` de un componente que corre hasta completarse.
- [ ] Lo que ratatui **no** te da y lipgloss sí: `width()` con wrapping
      ANSI-aware, `align()`, margin sobre texto arbitrario, `join_h` / `join_v`.

---

## Fase 6 — Fullscreen: App + widgets compuestos

`App::fullscreen()` como entry point, y los widgets que IDEA.md pedía para la
vista fullscreen y que ninguna fase anterior cubre: `List`, `Table`, `Tabs`,
`Panel`.

- [ ] `App::fullscreen()` — alt screen. El registro de foco con `Tab` se
      construye acá, no antes: Fase 3 lo dejó pospuesto como opt-in (D-015).
- [ ] `List`, `Table`, `Tabs`, `Panel` sobre el `Layout` de ratatui
      (`Constraint::{Length, Percentage, Min, Fill}`). **No** se escribe un
      sistema de layout propio — es el mismo concepto que el Row/Column/Fixed/Flex
      de IDEA.md y ratatui ya lo resuelve.
- [ ] Mouse: click y scroll. Si `on_key(&self, k: Key)` (D-006) no alcanza para
      esto, generalizar a `on_event` antes de escribir el primer widget que lo
      necesite, no después de tener cinco componentes atados a la firma vieja.

**Listo cuando:** una vista con `List` + `Panel` responde a click de mouse y a
`Tab`, sin tocar el motor de Fase 3.

**Fuera de esta fase:** `Tree`, `Autocomplete`, `Forms`, `Animations`,
`Theming`, `Accessibility` — el "recién después" de IDEA.md. Se agregan sólo
si un caso real los pide.

---

## Fase 7 — Inline ↔ Fullscreen (la feature diferencial)

Punto 12 de IDEA.md, "la feature estrella". D-005 ya separó `Inline` y `App`
en tipos distintos que comparten el motor de Component + loop (Fase 3); acá se
construye el handoff real entre uno y otro dentro del mismo proceso.

- [ ] Terminar una tanda de prompts inline (Fase 5), dejar el transcript en
      scrollback, y arrancar `App::fullscreen()` sin reinicializar la terminal
      ni perder el estado que los prompts recolectaron.
- [ ] El costo de habilitarlo tiene que ser llamar a `App::fullscreen()` después
      del último `inline::*` — sin flag ni builder especial. Repetir ese error
      es exactamente lo que D-005 ya descartó para Inline vs App.

**Listo cuando:** un ejemplo corre 2-3 prompts inline (`Select`, `Confirm`) y al
terminar entra a una vista fullscreen sin parpadeo ni reset visible de la
terminal.

---

## Fase 8 — Efectos

- [ ] Canal de closures `FnOnce() + Send` boxeadas, ejecutadas por el loop
      principal. La arena es thread-local, así que el thread de fondo **no
      puede** escribir signals — manda la closure, el loop la corre. Ver D-010.
- [ ] Suscripciones: `every(Duration)`, `on_channel(rx)`.

`compio` ya está en `Cargo.toml` como backend candidato — ver D-011.

---

## Fase 9 — Split de crates

- [ ] `nobubbles-core` ← loop, signals, render, terminal
- [ ] `nobubbles` ← facade + componentes + inline

Si los módulos van alineados, esto es un `git mv` y dos `Cargo.toml`. Hacerlo
antes son dos crates de 200 líneas peleándose por límites que todavía no
conocemos.
