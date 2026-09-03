# Decisiones

Una entrada por decisión de arquitectura. El campo **Se revisa si** existe para
que no volvamos a discutir lo mismo en tres meses: si el gatillo no se cumplió,
la decisión sigue en pie.

Formato: `D-NNN`, más nuevas abajo.

---

## D-001 — Signals en vez de mensajes

**Estado:** aceptada · 2026-09-02

**Por qué:** el problema real del diseño ELM original no era el estilo, era la
composición. `Input` implementaba `Model` con su propio `InputMsg`, y `App` no
podía embeberlo sin escribir a mano el `map` de mensajes. Ese muro es el que
empuja a buscar algo reactivo. Con signals el componente muta su propio estado y
el padre lee `input.value()`. No hay tipo `Message` que mapear.

**Descartado:** mantener ELM con un `Command::map` / `Message::wrap`. Funciona
(es lo que hace bubbletea) pero el boilerplate crece con cada nivel de anidamiento.

**Se revisa si:** aparece un caso donde haga falta reproducir o hacer time-travel
del estado, que es lo único que ELM da y esto no.

---

## D-002 — Signals `Copy` sobre arena, no `Rc<RefCell<T>>`

**Estado:** aceptada · 2026-09-02

**Por qué:** `Rc<RefCell<T>>` obliga a `let x = x.clone()` antes de cada closure.
Con el signal como índice a una arena thread-local, el handle es `Copy` y la
ceremonia desaparece. Es el truco de leptos_reactive. Son ~40 líneas.

**Techo conocido:** la arena no libera hasta el final del proceso. Aceptable para
una TUI que corre una vez y sale. Si algún día se crean signals en un loop
caliente, hace falta scoping o generational indices.

**Se revisa si:** aparece un caso donde se creen y descarten signals repetidamente
durante la vida del proceso.

---

## D-003 — Sin grafo de dependencias reactivo

**Estado:** aceptada · 2026-09-02

**Por qué:** un flag global de "algo cambió" que dispara re-render de la vista
entera, y el diff del buffer de terminal decide qué celdas pintar. En una
pantalla de unos miles de celdas, el tracking fino de dependencias no compra
nada medible, y ahorra memos, scheduling de efectos y orden topológico.

**Se revisa si:** una vista se vuelve medible-mente cara de recomputar. Entonces
se agrega `memo`, no el grafo completo. El camino de escalones está en D-008.

---

## D-004 — ratatui como backend de render interno

**Estado:** aceptada · 2026-09-02

**Por qué:** buffer, diff, ancho unicode, `Viewport::Inline` y estilos ya
resueltos y probados. El tiempo se gasta en el modelo reactivo, que es lo único
nuevo acá. No se expone en la API pública, así que nobubbles no es un wrapper de
ratatui — es una dependencia interna reemplazable.

**Descartado:** buffer de celdas propio (~300-400 líneas). El ancho unicode solo
ya es un pozo de casos borde.

**Se revisa si:** el `Viewport::Inline` de ratatui resulta demasiado rígido para
el modo inline, que es donde es más limitado.

---

## D-005 — Inline y App como tipos distintos, no un flag

**Estado:** aceptada · 2026-09-02

**Por qué:** difieren en layout (stack vertical vs `Rect` con resize), en
scrollback (lo respeta y deja transcript vs lo tapa), en terminación (devuelve un
valor vs corre hasta que salís) y en foco (secuencial vs registro con `Tab`).
`with_alt_screen(bool)` esconde cuatro diferencias detrás de un booleano.

Comparten motor: un prompt secuencial es un `inline::app` de un componente que
corre hasta completarse. Un solo loop, tres caras.

---

## D-006 — `Component` con `&self`, no `&mut self`

**Estado:** aceptada · propuesta 2026-09-02, confirmada 2026-09-03

```rust
pub trait Component {
    fn view(&self) -> impl Render;
    fn on_key(&self, k: Key) -> Handled { Handled::No }
}
```

**Por qué `&self`:** si todo lo mutable es un signal, `&self` alcanza. Da tres
cosas: (a) el dirty tracking es exacto y automático, porque la única forma de
mutar es escribir un signal; (b) dos partes del árbol pueden leer el mismo
estado, imposible con `&mut self`; (c) desaparecen las peleas con el borrow
checker, que son la razón por la que hacer TUI en Rust es miserable hoy.

**Costo:** todo lo mutable tiene que ser signal. Para `Input` eso es
`Signal<String>` + `Signal<usize>`.

Es la pieza de la que cuelga todo lo demás y la que menos se puede cambiar
después, así que se confirmó antes de arrancar la Fase 3, sin objeciones.

**Pendiente:** la firma sólo cubre teclado. IDEA.md (punto 11) pide un input
unificado con mouse y resize también. Si la Fase 6 (fullscreen) necesita click
o scroll, generalizar `on_key` a algo como `on_event` ahí, temprano — no
después de tener varios componentes ya atados a `on_key`.

---

## D-007 — Efectos con threads, sin runtime async

**Estado:** aceptada · 2026-09-02

**Por qué:** `thread::spawn` + canal cubre lo que realmente hace una TUI: un
HTTP, leer un archivo, un timer. Un runtime async es una dependencia grande para
eso. Tokio detrás de una feature el día que alguien necesite esperar mil cosas
concurrentes.

**Nota:** la primera versión de esta decisión sacaba `compio` del `Cargo.toml`.
Revertido en D-011: se queda como backend candidato. Ver también D-010, que
define la firma del canal.

---

## D-008 — `Signal` / `ReadSignal` / `WriteSignal` desde el día uno

**Estado:** aceptada · 2026-09-02

```rust
pub struct Signal<T>(usize, PhantomData<T>);       // lee y escribe
pub struct ReadSignal<T>(usize, PhantomData<T>);
pub struct WriteSignal<T>(usize, PhantomData<T>);

impl<T> Signal<T> {
    fn split(self) -> (ReadSignal<T>, WriteSignal<T>);
    fn read_only(self) -> ReadSignal<T>;
}
```

Los tres son el mismo índice a la misma slot de arena. Todos `Copy`. La
distinción es puramente de tipos, sin costo en runtime.

**Por qué NO es performance:** bajo D-003 el flag de dirty es global, así que toda
escritura re-renderiza lo mismo. Hoy el split no acelera nada. Anotarlo explícito
para no vendernos una ganancia que no existe.

**Por qué sí, entonces:** es la costura exacta que necesita cualquier granularidad
futura — `ReadSignal::get()` es donde se registra una suscripción y
`WriteSignal::set()` donde se notifica. Con el split desde el principio, agregar
tracking es aditivo. Sin él, es un cambio breaking en el tipo que toca toda la
API pública. ~15 líneas ahora contra una migración después.

Beneficio secundario, ese sí inmediato: un componente que recibe `ReadSignal<T>`
no puede escribirlo. La firma documenta la intención y el compilador la sostiene.

**Camino de upgrade, en orden de costo:**
1. Hoy — flag global, re-render entero, diff del buffer. (D-003)
2. Si una vista se vuelve cara — dirty por *región*: el `WriteSignal` marca qué
   subárbol tocó, se re-renderiza sólo ese. Escalón intermedio, sin grafo.
3. Sólo si eso no alcanza — tracking fino de dependencias en `ReadSignal::get()`.

**Se revisa si:** nunca, en el sentido de que el split no se saca. Lo que se
revisa es en qué escalón del camino de arriba estamos.

**Referencia:** el patrón viene de Leptos (`ReadSignal`/`WriteSignal`, con
`RwSignal` para el caso de ambos). Freya y Dioxus tienen la misma idea con otros
nombres. Vale mirar cómo resuelven el caso de "quiero pasar sólo lectura a un
hijo" antes de fijar los nuestros.

---

## D-009 — Sin fase de refactor previo: ELM se borra, no se arregla

**Estado:** aceptada · 2026-09-02 (reemplaza la Fase 0 del plan original)

**Por qué:** el plan original tenía una Fase 0 que limpiaba el loop existente
antes de construir lo reactivo. Mal encuadre. `Model`, `Command`, `Program`,
`Internal`, `QuitHandle` y `components/*` se borran enteros en la Fase 3.
Refactorizar código con fecha de defunción cuesta lo mismo que borrarlo y encima
arrastra conceptos vestigiales — `Internal::User(M::Message)` no significa nada
cuando no existe `Message`.

Caso concreto que lo disparó: el plan pedía arreglar el bug UTF-8 de
`InputMsg::Delete`. Pero `InputMsg` sólo existe porque ELM necesita que el
componente emita mensajes. Con signals, `Input` muta su propio `Signal<String>` y
borrar es una función sobre ese signal. El tipo entero desaparece.

**Qué se rescató:** el análisis del loop no se tira. La lógica del timeout, el
coalescing por frame y las dos trampas (latencia y wakeups en idle) son
independientes de ELM — sólo cambian cuatro líneas en el medio del loop. Se mudó
al cuerpo de la Fase 3, donde se escribe una vez en vez de dos.

**Nuevo orden:** 1 signals (Rust puro, sin terminal) → 2 renderer (vista
hardcodeada) → 3 borrar ELM + `Component` + loop nuevo. Las fases 1 y 2 no tocan
nada de lo que existe, así que el código viejo queda compilando sin molestar
hasta que se borra.

---

## D-010 — La arena es thread-local: el canal de efectos lleva closures

**Estado:** aceptada · 2026-09-02

**Tensión que resuelve:** D-002 pone los signals en una arena `thread_local!`.
D-007 dice que los efectos corren en `thread::spawn`. Un thread de fondo **no ve
la arena**, así que no puede escribir un signal. Descubierto al reordenar el plan;
condiciona la Fase 8 (Efectos) y hay que tenerlo escrito antes de llegar.

**Resolución:** el canal no lleva valores para que alguien los aplique, lleva
`Box<dyn FnOnce() + Send>`. El thread de fondo hace el trabajo, captura el
resultado en una closure, y el loop principal la ejecuta en su propio thread,
donde la arena sí existe.

```rust
rx: mpsc::Receiver<Box<dyn FnOnce() + Send>>
// en el loop:  while let Ok(f) = rx.try_recv() { f(); }
```

**Consecuencia de tipos:** lo que el thread devuelve tiene que ser `Send`, pero
el signal al que escribe no necesita serlo. La closure es la frontera.

**Alternativa descartada:** arena global con `Mutex` en vez de thread-local.
Haría que los signals crucen threads, pero mete un lock en cada `.get()`, que es
la operación más caliente del render.

---

## D-011 — `compio` se queda en `Cargo.toml` sin usar

**Estado:** aceptada · 2026-09-02 (revierte lo que decía D-007)

**Por qué:** decisión del autor. Gusta el manejo de buffers de compio y queda
reservado como backend candidato para la Fase 8 (Efectos).

**Costo real:** tiempo de compilación y una entrada de más en `cargo tree`. Nada
en runtime — no se linkea lo que no se llama. Aceptable.

**Se revisa si:** llegamos a la Fase 8 y se elige otro backend, o si el tiempo de
build empieza a molestar antes.

---

## D-012 — Arena a mano, no `slotmap` ni `generational-box`

**Estado:** aceptada · 2026-09-02

**Las tres opciones no están al mismo nivel.** `slotmap` es una estructura de
datos (claves versionadas); `generational-box` es la solución completa a este
problema, de la gente de Dioxus. Hacerlo a mano es no usar ninguna.

**Por qué a mano:** todo lo que `generational-box` da de más — generaciones,
detección de handle muerto, `Owner` para liberar por scope — existe para
resolver el reuso de slots. Si la arena nunca libera, no hay reuso y el downcast
es infalible por construcción. Son ~40 líneas y son el núcleo de todo lo demás:
no es el lugar para una caja negra la primera vez.

**Por qué `slotmap` es el medio equivocado para nosotros:** te da claves con
versión, pero el `Box<dyn Any>` y el `RefCell` los seguís escribiendo igual —
pasás de 40 líneas a 35 y sumás una dependencia. Leptos sí lo usa, porque su
slot guarda un *nodo del grafo reactivo* (valor + suscriptores + fuentes +
estado), un struct rico y propio. Nosotros no tenemos grafo (D-003): el slot es
un valor pelado. La ventaja no aplica.

**El upgrade es barato:** la API pública (`Signal<T>` newtype `Copy`, `.get()`,
`.set()`) es la misma con un `Vec` adentro o con un `GenerationalBox`. Se
cambian las tripas sin que nadie se entere.

**Se revisa si:** se crean componentes dinámicamente en un loop — una lista con
filas que se agregan y quitan, un file browser. Ahí cada fila que desaparece deja
sus signals colgados y el leak deja de ser teórico. Entonces va
`generational-box` directo: meterle generaciones al `Vec` a mano es reescribir
esa crate, peor.

Los prompts secuenciales de la Fase 5 **no** son ese caso — diez prompts son
veinte signals.

---

## D-013 — `Signal<T>` debe ser `!Send` y `!Sync`

**Estado:** aceptada · 2026-09-02

**El problema:** `usize` es `Send + Sync` y `PhantomData<T>` es `Send` si `T` lo
es. O sea que `Signal<T>(usize, PhantomData<T>)` es `Send`, y el compilador deja
copiarlo dentro de un `thread::spawn`. Pero la arena es `thread_local`: en ese
thread es *otra* arena. En el mejor caso panica; en el peor el índice existe con
un tipo que coincide y se lee el signal equivocado en silencio.

**El arreglo, una línea:**

```rust
pub struct Signal<T>(usize, PhantomData<*const T>);
```

Un puntero crudo no es `Send` ni `Sync`, así que el `Signal` tampoco. Sigue
siendo `Copy`: `PhantomData<X>` es `Copy` para cualquier `X`.

**Consecuencia buena:** D-010 pasa de ser disciplina a ser regla del compilador.
El canal de efectos lleva closures precisamente porque los signals no pueden
cruzar threads — y ahora, si alguien lo intenta, no compila.

**Ojo:** el `derive(Copy)` sobre estos tipos va a exigir `T: Copy` por el
`PhantomData`. Hay que implementar `Copy` y `Clone` a mano. Es el error clásico
la primera vez.

---

## D-014 — Guards RAII, sin `Deref` sobre `Signal`

**Estado:** aceptada · 2026-09-03

**Lo que no se puede:** `impl Deref for Signal<T>`. El valor vive detrás de un
préstamo verificado en runtime, y `Deref` promete un `&T` atado a `&self` sin
lugar donde guardar el guard. Es la misma razón por la que `RefCell` tiene
`borrow() -> Ref<T>` y `Mutex` tiene `lock() -> MutexGuard<T>`.

`DerefMut` es peor: devuelve `&mut T` sin punto de cierre, así que **no puede
prender el dirty**. `*contador += 1` mutaría sin que la pantalla se entere.
Silencioso y a depurar meses después.

**Lo que sí:** `SignalRef<T>` y `SignalRefMut<T>`, dueños del `Rc`, con el
préstamo adentro. `SignalRefMut` prende el dirty en su `Drop` — ese es el punto
de cierre que `DerefMut` sobre `Signal` no tenía.

**El precio:** son structs auto-referenciales (el guard toma prestado de algo que
el guard posee), así que llevan un `transmute` de lifetime. **El invariante es el
orden de los campos**: `value` antes que `_slot`, para que el préstamo se suelte
antes de que la allocation pueda liberarse. Invertirlos es use-after-free, y
ningún test lo agarra — por eso Miri.

**Alternativa descartada:** `self_cell`, que genera structs auto-referenciales
sin escribir `unsafe`. Son 30 líneas contra una dependencia, y el `unsafe` está
acotado a dos funciones. Si el módulo crece, se revisa.

**Se revisa si:** aparece un tercer guard, o el invariante de orden deja de
alcanzar.

---

## D-015 — `App::run(closure)` como entry point; `Component` no es la app

**Estado:** aceptada · 2026-09-03

**Contexto:** al planear la Fase 3, tres formas candidatas de entry point:
`nobubbles::inline::run(app)`, `nobubbles::terminal::run(app)`, y
`App::run(|cx| cx.render(...))`. Las dos primeras implican que el framework
empuja eventos a un `Component` top-level que el usuario implementa. La
tercera implica que el loop vive adentro de `.run()` pero la lógica de cada
vuelta es un closure del usuario.

**Por qué NO la primera forma:** si el framework maneja el dispatch hacia un
`Component` raíz, `on_key` necesita un protocolo para decidir cuándo termina
la app entera — de ahí salía `Handled::Quit`. Eso mezcla dos preguntas sin
relación: "¿el hijo consumió la tecla?" (local, del dispatch) y "¿hay que
salir?" (global, no depende de qué componente tenía foco). Un enum de tres
variantes que carga ambas es el síntoma, no el problema.

**Decisión:**
1. El entry point es un closure sobre `App::run`, no un `Component` que el
   framework maneja: `App::inline().run(|cx| ...)` / `App::fullscreen().run(|cx| ...)`.
   Adentro, el usuario compone `Component`s (`Input`, `Select`, vistas propias)
   y llama `cx.render(...)` para dibujarlos.
2. `Component` (D-006) se queda para composición interna de widgets — no es
   la interfaz que el usuario implementa para "ser la app".
3. `quit` deja de ser una variante de `Handled`. Es un flag global igual a
   `dirty` (D-003), mismo `Runtime` de `signals.rs`: `quit()` lo prende, el
   loop lo lee. Nadie bubblea nada para salir.
4. `on_key`, donde exista (widgets que lo necesiten), devuelve `bool`
   ("consumido"), no `Handled`. `Handled::{Yes, No, Quit}` se descarta entero.

**Costo:** el framework hace menos por vos. Ya no hay dispatch automático de
teclas hacia un root `Component` — el usuario orquesta foco/bubbling desde el
closure, salvo lo que un widget resuelva puertas adentro. Es exactamente lo
que se pidió: controlar el evento de salida uno mismo, no que lo decida un
valor de retorno bubbleado.

**Se revisa si:** el closure-por-frame se siente repetitivo con árboles
grandes y hace falta dispatch automático de vuelta. Ahí `Component` podría
absorber ese rol, pero como opt-in — no como único camino, para no volver al
problema que esta decisión resuelve.

---

## D-016 — `Render`, `Rect`, `Buffer` como newtypes propios sobre ratatui

**Estado:** aceptada · 2026-09-03

**Contexto:** el primer borrador de `Component::view` devolvía
`impl ratatui::widgets::Widget` directamente. Contradice D-004: cualquiera que
implemente `Component` tendría que importar ratatui, y su trait `Widget`
quedaría en el contrato público — nobubbles pasa a ser un wrapper, no una
dependencia interna reemplazable.

**Decisión:** `Render`, `Rect` y `Buffer` son tipos propios que envuelven a
los de ratatui, con conversiones en el borde:

```rust
pub struct Rect(ratatui::layout::Rect);
pub struct Buffer(ratatui::buffer::Buffer);

pub trait Render {
    fn render(self, area: Rect, buf: &mut Buffer);
}

impl<W: ratatui::widgets::Widget> Render for W {
    fn render(self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Widget::render(self, area.into(), buf.inner_mut());
    }
}
```

**Por qué el newtype y no el reexport plano** (que era mi recomendación por
YAGNI): decisión explícita del autor — pagar la capa de conversión ahora a
cambio de que ratatui no aparezca estructuralmente en ninguna firma pública.
El día que se cambie de backend, el cambio queda contenido en esas
conversiones — ningún `Component` de afuera se toca.

**Por qué NO es el buffer de celdas 100% propio que D-004 ya descartó:** este
newtype no reimplementa nada de lógica (ancho unicode, diffing, estilos) —
sólo envuelve. "Cero tipos de ratatui en la firma pública" (esto) es una
decisión distinta de "cero ratatui en la implementación" (lo que D-004
rechazó por el costo).

**Costo:** por cada tipo de ratatui que un widget público termine necesitando
exponer (`Style`, `Color`, `Modifier`...) hay que decidir si también se
envuelve. No hace falta resolverlo ahora — se decide cuando la Fase 4 lo pida
de verdad.

**Consecuencia de módulos:** `Rect`/`Buffer`/`Render`/`Component` tienen que
vivir en un módulo `pub`, no en `render.rs` (que sigue `pub(crate)` a
propósito — ver D-004/Fase 2). La visibilidad de un ítem queda tapada por la
de su módulo contenedor, así que un `pub struct Rect` dentro de un
`pub(crate) mod render` sigue siendo invisible afuera del crate.

**Se revisa si:** la capa de conversión crece tanto que se vuelve el archivo
más grande del crate sin que nadie haya cambiado de backend — ahí vale
preguntarse si compró algo real.
