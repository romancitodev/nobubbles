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

- [ ] Borrar `Model`, `Command`, `Internal`, `QuitHandle`, `components/*`, los
      dos ejemplos.
- [ ] `trait Component` con `view(&self) -> impl Render` y `on_key(&self, k) -> Handled`
- [ ] `Handled::{Yes, No, Quit}`
- [ ] Dispatch: hijo enfocado primero, después burbujea al padre.
- [ ] Registro de foco en contexto, Tab cicla.
- [ ] `Theme` global + structs `*Style` por componente.

**Listo cuando:** un componente padre tiene un `Input` como campo, lee
`self.input.value()`, y no existe ningún tipo `Message` en el ejemplo.

### La forma del loop

Lo que cambia respecto del WIP: hoy el loop prende `dirty` a mano después de
`update()`. En reactivo no lo prende nadie a mano — lo prendió la escritura al
signal, y `on_key` sólo devuelve si consumió la tecla o si hay que salir. El
resto del esqueleto es idéntico.

    const IDLE: Duration = Duration::from_millis(250);

    let frame = Duration::from_secs_f64(1.0 / fps as f64);
    let mut last_render = Instant::now();

    loop {
        let timeout = if dirty() {
            frame.saturating_sub(last_render.elapsed())
        } else {
            IDLE
        };

        if event::poll(timeout)? {
            let ev = event::read()?;
            // filtro de KeyEventKind::Press acá
            if let Handled::Quit = root.on_key(ev) { break; }
        }

        while let Ok(f) = rx.try_recv() { f(); }   // efectos, Fase 6 — ver D-010

        if dirty() && last_render.elapsed() >= frame {
            render(&root)?;
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

## Fase 4 — Componentes

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

## Fase 6 — Efectos

- [ ] Canal de closures `FnOnce() + Send` boxeadas, ejecutadas por el loop
      principal. La arena es thread-local, así que el thread de fondo **no
      puede** escribir signals — manda la closure, el loop la corre. Ver D-010.
- [ ] Suscripciones: `every(Duration)`, `on_channel(rx)`.

`compio` ya está en `Cargo.toml` como backend candidato — ver D-011.

---

## Fase 7 — Split de crates

- [ ] `nobubbles-core` ← loop, signals, render, terminal
- [ ] `nobubbles` ← facade + componentes + inline

Si los módulos van alineados, esto es un `git mv` y dos `Cargo.toml`. Hacerlo
antes son dos crates de 200 líneas peleándose por límites que todavía no
conocemos.
