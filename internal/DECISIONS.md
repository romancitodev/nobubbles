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
build empieza a molestar antes. **Se cumplió** — ver D-024: sigue en el
`Cargo.toml`, ahora detrás de una feature apagada.

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


---

## D-017 — `Ctx::render` toma `impl Render`, no un `Component`

**Estado:** aceptada · 2026-09-05

**Por qué:** la Fase 5 necesita renderizar cosas que no son un `Component` —
`inline::input` dibuja un `column![...]` armado en el momento, no un struct del
usuario. Las dos salidas eran un struct privado con su `impl Component` por
prompt (cuatro structs de andamio) o una firma que acepte cualquier `Render`.

```rust
pub fn render(&mut self, view: impl Render)   // antes: &impl Component
```

Un `Component` pasa `c.view()`, que es una llamada más en el único lugar que lo
usa.

**Consecuencia incómoda:** después de este cambio **ningún tipo de la librería
implementa `Component`**. `Input`, `Select`, `Progress` y compañía implementan
`Render` directo; los prompts inline no construyen ninguno. El único
implementador en todo el repo es el `Form` de un ejemplo.

**Por qué el trait se queda igual:** las razones de D-006 son sobre `&self`, no
sobre el trait. Que `field.value()` funcione después de que el loop terminó es
lo que hace posible toda la Fase 5, y eso lo da `&self` + signals, no
`Component`.

**Se revisa si:** la Fase 6 llega y los contenedores (`Tabs`, `List`, `Panel`)
no necesitan un trait para rutear eventos a hijos desconocidos. Si no lo
necesitan, `Component` no tiene razón de existir y se borra.

---

## D-018 — El canal de efectos lleva **datos**, no closures

**Estado:** aceptada · 2026-09-05 (corrige D-010)

**El agujero:** D-010 dice que el canal lleva `Box<dyn FnOnce() + Send>` y que
el worker "captura el resultado en una closure". **Esa closure no compila.** Si
captura un `Signal<T>`, y D-013 hace que `Signal` sea `!Send`, la closure es
`!Send` y no cruza el canal. Las dos decisiones se contradicen en el caso exacto
para el que se escribieron.

**Resolución:** el canal lleva un `T: Send` que define la app, y los handles de
signal se quedan del lado del loop.

```rust
let inbox = effects::inbox::<Update>();      // Update es de la app
inbox.spawn(|tx| { /* trabajo, tx.send(...) */ });
let sigue = inbox.drain(|update| update.apply(&rows));   // corre en el loop
```

**Dos cosas que van juntas y por eso son una sola llamada:**
1. `drain` lee la liveness **antes** de aplicar, así que un `false` nunca deja
   nada encolado atrás. Que fueran dos llamadas era un orden que cada app iba a
   tener que redescubrir.
2. Mientras haya trabajo vivo, `drain` marca la vista sucia. Sin eso el loop
   dormiría su timeout de idle, y como drenar sólo pasa adentro del closure de
   ui, una vista que dejó de dibujar dejaría de drenar.

**Descartado:** un contador global de trabajo en vuelo (`static AtomicUsize`).
Estado de proceso, no testeable en paralelo, y no hacía falta: la liveness es el
`Arc::strong_count` del propio inbox, por instancia.

**Se revisa si:** aparece un caso donde el worker tenga que decidir *qué* hacerle
al estado y no sólo reportar. Ahí la closure vuelve a tener sentido, y con ella
un handle `Send` que se resuelva del lado del loop.

---

## D-019 — Vocabulario de estilo propio, y `Text` como primitiva

**Estado:** aceptada · 2026-09-05

**Lo que pasó:** D-016 dice que ratatui no aparece en ninguna firma pública. Se
cumplía al pie de la letra y se rompía donde importa: **todos los ejemplos
importaban `ratatui::widgets::Paragraph`**, porque no había otra forma de poner
un string en pantalla. El blanket `impl<W: Widget> Render for W` hacía que el
camino fácil fuera reachear ratatui.

**Decisión, dos partes:**
1. `components::text::Text` — una tirada de texto con un estilo. Es la primitiva
   que faltaba, y con ella los ejemplos no importan ratatui en ninguna línea.
2. `style::{Color, Style}` **propios**, no newtypes. `Color` son los 16 ANSI más
   `Rgb`; `Style` es `fg`, `bg`, `bold`, `dim`, `italic`, con builders
   encadenables y un `From` en el borde.

**Por qué propios y no un newtype:** lipgloss *es* un vocabulario. Envolver el
`Style` de ratatui sería el mismo tipo con otro nombre; esto es la semilla del
theme, y crece con lo que los widgets pintan de verdad y no con lo que ratatui
soporta.

**Ojo con qué es lipgloss:** lo lindo no sale del `Style`, sale de los verbos de
layout que ratatui no da y que la Fase 5 lista — `width()` con wrapping
ANSI-aware, `align()`, `margin`, `join_h`/`join_v`. El `Style` es la parte fácil.

**Se revisa si:** un widget necesita un atributo que no está (`underline`,
`strikethrough`, `blink`). Se agrega el campo, no se cambia el enfoque.

---

## D-020 — Enviar un prompt lo decide el `bool` de `on_key`

**Estado:** aceptada · 2026-09-05

**El choque:** un `Input` multilínea quiere Enter para partir la línea, y todos
los demás prompts quieren Enter para enviar. `ask` interceptaba Enter **antes**
de que el widget lo viera, así que multilínea era imposible.

**Resolución, sin agregar nada al trait:**

```rust
if is_submit(key) || (!on_key(key) && key.code == KeyCode::Enter) {
```

El widget dice si quiso la tecla. Un multilínea se queda con Enter y no se
envía; todos los demás lo rechazan y el prompt termina. Es exactamente el `bool`
que D-015 puso ahí.

**Por qué `ctrl+s` y no `ctrl+enter`:** la mayoría de las terminales mandan
`ctrl+enter` como un Enter pelado, así que serían indistinguibles justo en el
widget donde importa. `ctrl+s` llega bien (verificado sobre Windows Terminal).
Es también el alias que el autor había terminado usando en `simple-commits`.

**Consecuencia:** `Input::on_key` ignora cualquier `Char` con CONTROL. Sin eso,
`ctrl+s` escribía una `s`.

**Se revisa si:** se adopta el protocolo de teclado de Kitty, donde `ctrl+enter`
sí es distinguible.

---

## D-021 — rímel es un crate aparte, y se lleva `Color`/`Style`

**Estado:** aceptada · 2026-09-05

**Qué es:** `crates/norimel`, el ítem de lipgloss de la Fase 5 con nombre propio.
Un `Block` son filas de runs con estilo más utilidades de caja. El repo pasa a
ser un workspace con `nobubbles` en la raíz y los miembros en `crates/*`.

**Por qué aparte:** es la relación lipgloss/bubbletea. rímel no sabe nada del
loop ni de los signals, y sin la feature `ratatui` no sabe nada de ratatui
tampoco: sirve para un CLI que sólo imprime.

**Consecuencia grande:** `style::{Color, Style}` **se mudan a norimel**. Son el
vocabulario compartido, y tenerlo dos veces con un `From` en el medio es la
duplicación que D-019 evitó. `nobubbles::style` queda como re-export. El `From`
hacia ratatui tiene que vivir en norimel: desde nobubbles los dos tipos son
ajenos y la coherencia lo prohíbe.

**Por qué no se hizo opcional norimel:** no cuesta nada. crossterm y
unicode-segmentation ya estaban, unicode-width entraba por ratatui. Lo caro es
colorgrad, y eso sí es feature (D-024).

**Se revisa si:** norimel deja de compilar sin ratatui, o si aparece un tercer
crate que quiera el vocabulario de estilo y no el resto.

---

## D-022 — Se cae el blanket `impl<W: Widget> Render for W`

**Estado:** aceptada · 2026-09-05 (modifica D-016)

**El choque:** un blanket sobre un trait ajeno reclama **todos** los tipos
ajenos. Con él en el crate, `impl Render for norimel::Block` no compila:

    error[E0119]: conflicting implementations of trait `Render` for type `norimel::Block`
    note: upstream crates may add a new impl of `Widget` for `norimel::Block`

**Decisión:** se borra el blanket. Los widgets de acá ya llamaban
`Widget::render` a mano — el único uso real eran tres tests de
`components/mod.rs`, que ahora usan `Text`.

**Lo que se pierde:** meter un widget de ratatui pelado en `cx.render`. D-019 ya
había declarado que ese no es el camino: ningún ejemplo importa ratatui.

**Descartado:** que norimel implemente `Widget` (feature `ratatui`) y llegue a
`Render` por el blanket. Anda, pero `Render::height` cae al default de 1 fila y
un bloque con borde mide 4: el viewport inline lo recorta.

**Se revisa si:** alguien pide componer widgets de ratatui de terceros; el
reemplazo es un newtype local, no volver al blanket.

---

## D-023 — Las utilidades de rímel anotan, la forma se compone al final

**Estado:** aceptada · 2026-09-05

**Por qué:** la API se pidió tailwind-like — `bg`, `px`, `w`, `center`,
`rounded`. Aplicarlas en el momento tiene dos trampas conocidas: `w(20)` seguido
de `center()` no centra nada (el relleno ya está puesto), y `px(1).px(1)` deja
dos columnas. Las dos desaparecen si el builder sólo escribe un campo y `compose`
arma las filas una vez, en la única salida que hay.

**Consecuencia:** `PartialEq for Block` compara `compose()`, no los campos. Dos
bloques son iguales cuando se dibujan igual, que es lo que el test de
conmutatividad quiere decir.

**Costo:** `size()` y `runs()` componen cada vez. Son bloques de decenas de
celdas; si algún día no alcanza, se cachea adentro.

**Se revisa si:** aparece una utilidad que no se pueda expresar como un campo.

---

## D-024 — Todo lo caro va detrás de una feature

**Estado:** aceptada · 2026-09-05

**Por qué:** el tiempo de build. `cargo tree` del workspace estaba en 132 crates,
de los cuales 67 eran `compio` sin usar (D-011) y el resto crecía con cada
biblioteca linda que sumábamos.

**Lo que quedó:**

| feature | qué prende | costo |
|---|---|---|
| `gradient` | `Ramp` de rímel, sobre colorgrad | +9 crates, phf con proc-macros |
| `fx` | `Ctx::effect`, sobre tachyonfx | +6 crates, bon y prettyplease |
| `compio` | nada todavía (D-011) | +64 crates |
| `full` | `fx` + `gradient` | para correr los ejemplos |

Default: **80 crates**. Con `full`: 92. Con todo: 144.

**Actualización 2026-09-06:** `fuzzy-matcher` (D-025) entró al build por defecto — no
detrás de una feature, porque no es caro: `cargo add` sólo bloqueó 2 paquetes
nuevos (`fuzzy-matcher` + `thread_local`). Default pasa a **82 crates**, full a
**94**. La tabla de arriba queda como estaba al momento de D-024; este número
es el que vale hoy.

**Los ejemplos declaran `required-features`**, así `cargo build --examples` no
falla ni arrastra nada de más.

**Por qué colorgrad y tachyonfx y no a mano:** colorgrad da las paletas y la
interpolación; tachyonfx da los efectos sobre el buffer y — lo que importaba —
pide `ratatui ^0.30.2`, la misma que usamos. Escribir eso a mano era ~25 líneas
de HSL para lo primero y bastante más para lo segundo.

**Se revisa si:** alguna feature deja de ser opcional de hecho, o si el default
vuelve a crecer sin que nadie lo mire.

---

## D-025 — Search en `Select`/`MultiSelect`: `fuzzy-matcher`, filtra pero no reordena, detrás de `.filter()`

**Estado:** aceptada · 2026-09-06

**Por qué `fuzzy-matcher` y no `skim`:** el pedido original era buscar como
skim/fzf. La crate `skim` (skim-rs/skim) tal cual es una app de terminal
completa: por defecto trae `clap`, decodificación de imágenes e IPC (features
`cli`, `image`, `listen` prendidas), y corre su propio loop sobre la terminal —
que compite con el engine de nobubbles por ella. Lo que hacía falta era sólo el
algoritmo de scoring, publicado aparte como `fuzzy-matcher` (mismo autor, sin
deps por defecto): 2 crates nuevos (`fuzzy-matcher` + `thread_local`, ver
actualización en D-024) contra aislar el matcher interno de un binario entero.

**Filtra, no reordena:** `Select` y `MultiSelect` esconden lo que no matchea,
pero no cambian el orden de lo que queda. Reordenar por score andaría bien en
un `Select` plano, pero rompería el agrupamiento de `MultiSelect`: una fila que
sube de posición por tener mejor score se separaría de su heading, que es
exactamente lo que los grupos existen para evitar. Un solo comportamiento para
los dos widgets gana a una regla especial por cada uno.

**Detrás de `.filter()`, no siempre activo:** sin la llamada, `/` no hace nada
— igual que antes de que existiera la feature. Así una lista de opciones que
por casualidad tiene un literal `/` no gana un modo que nadie pidió.

**El query matchea el label O la `.note()`:** un hint es contenido; esconder
una opción porque el query pegó en el aside y no en el label leería raro. Sin
prioridad entre los dos — un match en la nota no pesa más que uno en el label,
decisión explícita de la sesión, no un descuido.

**Se revisa si:** aparece un caso real donde el orden estable no alcance — una
lista larga donde lo más parecido tiene que quedar arriba. Ahí se agrega
reordenamiento por score, no antes, y probablemente sólo para `Select` (sin
headers no hay nada que reordenar pueda romper).

---

## D-026 — `Autocomplete`: strict/relaxed es un validator, no estado del widget

**Estado:** aceptada · 2026-09-06

**Por qué:** `components::autocomplete::Autocomplete` no sabe si la respuesta
tiene que estar en la lista. `inline::autocomplete().strict()` sólo instala el
mismo `Check` que ya usa cualquier prompt de texto (`.validate()`): un closure
que refusa si el texto tipeado no es, palabra por palabra, uno de los
`.items()`. Si se llaman los dos, `.validate()` explícito pisa a `.strict()` —
no se combinan.

**Consecuencia:** el widget en sí siempre se comporta "relajado" — sólo
colecciona texto y una sugerencia resaltada. Toda la diferencia entre las dos
formas de usarlo vive en la capa `inline::`, en un closure de pocas líneas.
Cero campo `strict` en `Autocomplete`, cero rama nueva en su `on_key`.

**Se revisa si:** aparece un caso que necesite decidir "está en la lista o no"
en el render mismo — por ejemplo, tachar visualmente un valor tipeado que no
matchea ninguna opción. Ahí sí el widget necesitaría saberlo.

---

## D-027 — `Block::settled` limpia `restless` a mano; `map_cells` no

**Estado:** aceptada · 2026-09-06

**El choque:** `map_cells` preserva `restless` a propósito — es lo que deja que
un efecto propio (`Block::animated()` + tu propio reloj) siga pidiendo cuadros
después de recolorear, documentado en su propio doc comment. `Block::settled`
quiere exactamente lo contrario: apagar la animación para siempre, sea cual
sea su origen (`animate`/`pulse` con ramp, o `animated()` a mano). Llamar
`map_cells` sin más no apaga un bloque marcado `.animated()`, porque se lo
devuelve intacto — descubierto por un doctest que fallaba con
`assert!(!done.is_animated())`.

**Resolución:** `settled` llama `map_cells` (que ya vacía el campo `gradient`)
y después fuerza `restless = false` a mano. El test
`settled_stops_a_hand_rolled_effect_from_asking_for_more_frames` es el que
hubiera fallado con el primer intento (`map_cells` solo, sin el `restless =
false` extra).

**Se revisa si:** aparece un tercer significado de "detener la animación" que
`settled` no cubra.
