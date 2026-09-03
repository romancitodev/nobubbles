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
