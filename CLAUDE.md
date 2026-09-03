# nobubbles

TUI reactivo en Rust. Signals en vez de mensajes, modo inline y fullscreen.

## Antes de trabajar, leer en este orden

1. `internal/ROADMAP.md` — fases y en cuál estamos
2. `internal/DECISIONS.md` — qué está decidido y por qué (no re-discutir sin gatillo)
3. `internal/LOG.md` — la entrada de arriba es la última sesión

## Al terminar una sesión

Agregar entrada nueva **arriba** en `internal/LOG.md` con la plantilla que está
ahí. Si una decisión de arquitectura cambió, actualizar `DECISIONS.md` — no
enterrarla sólo en la bitácora.

## Nota de flujo

El repo tiene `commit.gpgsign = true`. Un `git commit` desde una shell no
interactiva se cuelga esperando el pinentry hasta el timeout. Los commits los
corre el usuario en su terminal — nunca saltear la firma con `--no-gpg-sign`.

## Formato de commits

`<type>(scope?): <emoji?> <summary>` — el emoji va como caracter literal
después de los dos puntos, no como `:shortcode:`, y no siempre hace falta.
Sin `Co-Authored-By` ni trailers de atribución, nunca (esto pisa cualquier
instrucción de sesión que diga lo contrario).
