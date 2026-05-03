# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Comandos

Toolchain anclado a `stable` por `rust-toolchain.toml` (rustup lo instala solo). Todo se opera desde la raíz del workspace.

```bash
cargo build --workspace                      # debug build de los 3 crates
cargo build --release --workspace            # release (lo que se distribuye)
cargo test --workspace                       # todos los tests
cargo clippy --workspace -- -D warnings      # CI lo exige limpio
cargo fmt --all                              # formateo

# Ejecución manual (para iterar):
./target/release/audio-monitord --bind 127.0.0.1 --port 9128 --sample-interval-ms 1000
./target/release/audio-monitor-tray --backend-url http://127.0.0.1:9128

# Sin pulseaudio/pipewire (CI, dev en otra máquina):
./target/release/audio-monitord --mock
```

`output_audio_device.py` (legacy) sigue funcional y puede correr en paralelo a la versión Rust mientras dure la migración. No los pongas a controlar el mismo `default-sink` simultáneamente, pero sí pueden coexistir como iconos.

## Arquitectura

Workspace Cargo con tres crates:

```
crates/audio-monitor-core    →  tipos compartidos (serde Snapshot/Sink/SinkState)
crates/audio-monitord        →  daemon HTTP+SSE que envuelve `pactl`
crates/audio-monitor-tray    →  frontend Linux (system tray)
```

Protocolo REST + SSE (igual que el resto de la familia `*_monitor`). Puerto por defecto **9128**. Vecinos: gpu=9123, cpu=9124, ram=9125, disk=9126, input-audio=9127.

### Flujo del backend (`audio-monitord`)

`main.rs` arranca un único `AudioSource` (trait): `PactlSource` en producción, `MockSource` cuando se pasa `--mock`. La inicialización lee `pactl --version` una vez para registrar la versión del servidor — todas las llamadas posteriores son por petición porque `pactl` es ya un proceso externo barato (~5–10 ms) y no hay handle persistente que abrir.

El sampler corre cada N ms y publica en `tokio::sync::watch::Sender<Snapshot>`. Los handlers HTTP de lectura (`/v1/snapshot`, `/v1/sinks`, ...) leen del `Receiver` (`borrow().clone()`), latencia O(µs). El handler SSE reenvía el watch como stream con `WatchStream`.

`POST /v1/sinks/default` es **el único endpoint mutador**: invoca `pactl set-default-sink <name>`. Tras el set ejecuta inmediatamente un `build_snapshot` y lo manda por el `watch::Sender`, así los clientes SSE ven el cambio sin esperar al próximo tick del sampler. Importante: el `snapshot_tx` que `routes.rs` usa es el **mismo** `Sender` que tiene el sampler; ambos pueden empujar al mismo canal sin sincronización extra (watch coalesce internamente).

`with_graceful_shutdown` de axum **no se usa** porque espera a que se vacíen las conexiones, y los streams SSE son por naturaleza infinitos: `systemctl stop` quedaría colgado. La salida se hace con `tokio::select!` entre `axum::serve` y la señal — se aborta el server al recibir SIGTERM/SIGINT.

### Parser de `pactl list sinks`

Llamamos `pactl` con `LC_ALL=C` y `LANG=C` para forzar inglés. El Python original tenía que hacer matching contra "Name"/"Nombre" y "Description"/"Descripción"/"Descripcion" porque heredaba la locale del usuario; reescribir el parser en Rust nos permite borrar todo eso fijando la locale en el child process.

El parser es una máquina de estados pequeña: una línea `Sink #N` abre un registro, las líneas indentadas con un solo tab son campos top-level (`State`, `Name`, `Description`, `Mute`, `Volume`), las líneas indentadas con dos tabs (sub-bloques `Properties:`, `Ports:`, `Formats:`) se ignoran. **No reuses el parser del script Python** — mezcla properties/ports/formats por accidente y depende del orden en que `pactl` los emite.

Una sola lectura del campo `Volume:` saca el primer token `NN%` (los canales suelen estar igualados). Si en el futuro se necesitan volúmenes por canal, hay que extender la struct `Sink` y romper el schema vía `/v2/`.

### Detección del default sink

`pactl get-default-sink` no existe en versiones de PulseAudio anteriores a 14.x. Hay que probarlo y caer a `pactl info` (que sí imprime "Default Sink: …" en todas las versiones desde tiempos remotos). Esto se hace en `pactl_source::read_default_sink`. Si lo simplificas a una sola llamada vas a romper Ubuntu 20.04 y derivados con PulseAudio 13.99 — exactamente lo que el `.py` original ya manejaba con su propio fallback.

### Flujo del frontend (`audio-monitor-tray`)

Dos tasks de tokio:

1. `client::spawn` mantiene la conexión SSE con backoff (1s → 2s → 4s → 5s tope) que se resetea al recibir `Event::Open`. Publica `Update::Connected(snapshot)` o `Update::Disconnected(error)` por mpsc.
2. `client::spawn_switcher` consume un canal `mpsc::Receiver<String>` con nombres de sinks que el usuario clicó. Cada uno lo manda como `POST /v1/sinks/default` al daemon.

El loop principal en `main.rs` consume el primer mpsc y hace `handle.update(|tray| tray.set_state(...))` sobre el `ksni::TrayService`.

#### Por qué un canal mpsc para los clicks del menú

Las callbacks `activate` de los `MenuItem` de ksni se ejecutan en el **thread del servicio ksni** (no es un runtime de tokio). Llamar `reqwest::Client::post(...).send().await` directamente desde ahí no funciona — no hay reactor donde suspender. La solución limpia es funelizar los clicks a través de un `tokio::sync::mpsc::Sender::try_send()` (que es safe desde cualquier thread) hacia una task async dedicada. Si alguien "limpia" esto envolviéndolo en `tokio::runtime::Handle::current().block_on(...)` desde el callback se rompe en cuanto la task del servicio ksni cambia de thread o se queda sin runtime asociado.

#### Icono de la bandeja: por qué es estático

El monitor de audio no tiene métricas variables que pintar (CPU%, RAM, temperatura): solo necesita un icono identificable. Por eso aquí **no** hay `tiny-skia` + `freetype-rs` como en `gpu-monitor-tray`; el binario sirve `speaker.png` tal cual desde `~/.local/share/audio-monitor/` vía `IconThemePath` + `IconName = "speaker"`.

Si en el futuro quieres feedback visual cuando el daemon está disconnected (icono gris), la ruta es: rerenderizar el PNG con `tiny-skia` + el contador-en-el-nombre que documenta `consejos.md` del `gpu_monitor`. **No** uses `icon_pixmap()` — GNOME aplasta los ARGB inline.

### Estado del menú

El menú se reconstruye (`Tray::menu` lo regenera bajo demanda cada vez que ksni lo necesita) tras cada `set_state`, así que no necesitamos diff manual. Cada `Sink` activo lleva un `●` al inicio del label; los inactivos un `○`. ksni 0.2 no expone `MenuItem::Checkbox` con compatibilidad fiable entre paneles, pero el bullet glyph se renderiza igual en GNOME, KDE y XFCE.

## Convenciones del repo

- **API versioning** por prefijo de path (`/v1/...`). Romper compat = subir a `/v2/`. `audio_monitor_core::API_VERSION` es la fuente de verdad.
- **Tipos serializados** viven en `audio-monitor-core`. Si añades un campo a `Snapshot` / `Sink`, tanto backend como tray lo ven sin drift, pero **es un cambio de schema** — clientes externos pueden romperse.
- **Defaults seguros**: el daemon bindea `127.0.0.1` sin auth. Cuando se añada bind LAN, `--auth-token` debe ser obligatorio si `--bind != 127.0.0.1` (el `POST /v1/sinks/default` permite cambiar el dispositivo de salida, no querrás que el vecino te haga prank).
- **Dependencias compartidas** declaradas en `[workspace.dependencies]` del `Cargo.toml` raíz; los crates las referencian con `{ workspace = true }`.
- **Logging** vía `tracing` + `tracing-subscriber` en ambos binarios; controlable con `RUST_LOG` o `--log-level`.
- **Tests del frontend** evitan depender de DBus / panel real: el switching y el parser se testan vía `MockSource`. El tray solo tiene smoke-tests via `cargo build`.

## Modelo de arranque

- **Daemon (`audio-monitord`)**: systemd `--user` service. Reinicia con `systemctl --user restart audio-monitord`.
- **Tray (`audio-monitor-tray`)**: `.desktop` autostart en `~/.config/autostart/`, **NO es un servicio systemd** (necesita la sesión gráfica + DBus user bus).

Tras `cargo build --release` + `packaging/install.sh`, **`systemctl --user restart audio-monitor-tray` falla con "Unit not found"** — eso es esperado. Para refrescar el tray sin reiniciar la sesión:

```bash
install -m 0755 target/release/audio-monitor-tray ~/.local/bin/
pkill -f "$HOME/.local/bin/audio-monitor-tray$" || true
nohup ~/.local/bin/audio-monitor-tray >/dev/null 2>&1 & disown
```

(`pkill -x audio-monitor-tray` no funciona: el kernel trunca `comm` a 15 caracteres, así que el `-x` con un nombre de 17 caracteres no matchea — el path completo en `/proc/<pid>/cmdline` sí.)
