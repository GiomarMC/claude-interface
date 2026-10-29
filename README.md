# Claude Interface

Interfaz de escritorio para [Claude Code](https://docs.claude.com/en/docs/claude-code) con el look de la TUI, pero con la interacción de una app gráfica.

No reemplaza a Claude Code: lanza tu `claude` instalado en modo `stream-json`, así que usa tu cuenta, tu `settings.json`, tus skills, MCP, `CLAUDE.md` y tus sesiones.

## Se adapta a cada usuario

- **Statusline:** ejecuta el `statusLine.command` de tus settings igual que Claude Code (mismo JSON por stdin) y pinta su salida ANSI. Sin statusline configurado, muestra uno sencillo.
- **Settings:** combina `~/.claude/settings.json`, `.claude/settings.json` y `.claude/settings.local.json` del proyecto, con la misma precedencia que Claude Code.
- **Tema:** usa el `/theme` de Claude Code (`dark`, `light`, `*-daltonized`, `*-ansi`). Se puede forzar otro con `/theme`.
- **Terminal:** Claude Code toma el fondo, la fuente y la paleta de la terminal donde corre; la app lee esos mismos datos de tu emulador (Konsole, GNOME Terminal, kitty, Alacritty, Ghostty, foot o Windows Terminal) y se ve igual. `/terminal` muestra cuál detectó y permite elegir otra o desactivarlo.
- **Fuente:** la de tu terminal por defecto; `/font <familia>`, `/font <tamaño>` o `Ctrl +`/`Ctrl -`/`Ctrl 0` la cambian.

## Funciones

- **Adjuntar archivos** arrastrándolos, con `Ctrl+O` o pegándolos desde el gestor de archivos. El contenido de los archivos de texto va directo en el mensaje (sin gastar llamadas de búsqueda/lectura); imágenes y PDF se envían directamente; archivos grandes y carpetas se pasan por ruta.
- **Pegar imágenes** del portapapeles con `Ctrl+V`.
- **Copiar** cada respuesta como Markdown y cada bloque de código con un clic.
- **Exportar** la conversación a `.md` (`/export` o `Ctrl+Shift+E`).
- **Permisos** con el mismo diálogo de la TUI (1/2/3, ↑/↓, Esc), `AskUserQuestion` y aprobación de planes.
- **Modos** con `Shift+Tab`: normal, accept edits, plan, auto, bypass.
- **Comandos `/`** con autocompletado de tus comandos y skills, más `/resume`, `/clear`, `/cd`, `/model`, `/effort`, `/copy`, `/font`, `/theme`, `/terminal`.
- **`!comando`** ejecuta en tu shell local sin gastar tokens; la salida se adjunta como contexto al siguiente mensaje.

## Requisitos

- [Claude Code](https://docs.claude.com/en/docs/claude-code/setup) instalado y autenticado (`claude` en el PATH o en `~/.local/bin`).

## Instalación

Descarga el instalador de tu sistema desde [Releases](../../releases):

| Sistema | Archivo |
|---|---|
| Fedora / RHEL | `.rpm` |
| Ubuntu / Debian | `.deb` |
| Cualquier Linux | `.AppImage` |
| Windows | `.msi` o `-setup.exe` |
| macOS Apple Silicon | `aarch64.dmg` |
| macOS Intel | `x64.dmg` |

En macOS la app no está firmada; después de copiarla a Aplicaciones ejecuta:

```sh
xattr -cr "/Applications/Claude Interface.app"
```

Se puede abrir en un directorio concreto: `claude-interface ~/mi-proyecto`.

### Actualizaciones

Al abrir (y cada 6 horas) la app busca una versión nueva y muestra un aviso para instalarla y reiniciar; la conversación abierta se retoma al volver. `/update` busca a mano y `/update off` desactiva la búsqueda automática.

- **Windows, macOS y AppImage:** se descarga y se instala sola.
- **.rpm / .deb:** descarga el paquete nuevo y lo instala con `pkexec` (pide tu contraseña en una ventana del sistema).

Las actualizaciones van firmadas y la app verifica la firma antes de instalarlas.

## Desarrollo

Requiere Node 22+, pnpm y Rust ([prerequisitos de Tauri](https://tauri.app/start/prerequisites/)).

```sh
pnpm install
pnpm tauri dev     # app en modo desarrollo
pnpm tauri build   # instaladores para el sistema actual
```

Para publicar una versión, sube la versión en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`, y un tag `v*`. GitHub Actions genera los instaladores de Linux, Windows y macOS y publica el release, que las apps instaladas detectan como actualización:

```sh
git tag v0.1.2 && git push origin v0.1.2
```

El workflow necesita los secrets `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` con la clave de firma del updater. Para generar instaladores en local con `pnpm tauri build` exporta esas mismas variables (o `TAURI_SIGNING_PRIVATE_KEY` con la ruta a la clave).

## Arquitectura

- `src-tauri/src/claude.rs`: lanza `claude -p --input-format stream-json --output-format stream-json --permission-prompt-tool stdio` y reenvía cada línea a la UI.
- `src-tauri/src/files.rs`: lectura de adjuntos, info de git, settings combinados, statusline, historial de sesiones (`~/.claude/projects`), shell local.
- `src-tauri/src/terminal.rs`: lectura de la configuración de los emuladores de terminal.
- `src/lib/session.svelte.ts`: estado de la sesión e interpretación del protocolo.
- `src/lib/prefs.svelte.ts`: tema, fuente y estilo de terminal.
- `src/lib/updater.svelte.ts` y `src-tauri/src/updates.rs`: actualizaciones (plugin de Tauri y paquetes .rpm/.deb).
- `src/lib/components/`: transcripción, herramientas, permisos, prompt y statusline.

La configuración propia de la app (fuente, tema o terminal forzados) se guarda en `config.json` dentro del directorio de configuración de la app (`~/.config/dev.giomarmc.claudeinterface/` en Linux).
