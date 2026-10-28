# Claude Interface

Interfaz de escritorio para [Claude Code](https://docs.claude.com/en/docs/claude-code) con el look de la TUI —tema oscuro, acento naranja, fuente monoespaciada y statusline de tres filas— pero con la interacción de una app gráfica.

No reemplaza a Claude Code: lanza tu `claude` instalado en modo `stream-json`, así que usa tu cuenta, tu `settings.json`, tus skills, MCP, `CLAUDE.md` y tus sesiones.

## Funciones

- **Adjuntar archivos** arrastrándolos, con `Ctrl+O` o pegándolos desde el gestor de archivos. El contenido de los archivos de texto va directo en el mensaje (sin gastar llamadas de búsqueda/lectura); imágenes y PDF se envían directamente; archivos grandes y carpetas se pasan por ruta.
- **Pegar imágenes** del portapapeles con `Ctrl+V`.
- **Copiar** cada respuesta como Markdown y cada bloque de código con un clic.
- **Exportar** la conversación a `.md` (`/export` o `Ctrl+Shift+E`).
- **Permisos** con el mismo diálogo de la TUI (1/2/3, ↑/↓, Esc), `AskUserQuestion` y aprobación de planes.
- **Modos** con `Shift+Tab`: normal, accept edits, plan, bypass.
- **Comandos `/`** con autocompletado de tus comandos y skills, más `/resume`, `/clear`, `/cd`, `/model`, `/effort`, `/copy`.
- **`!comando`** ejecuta en tu shell local sin gastar tokens; la salida se adjunta como contexto al siguiente mensaje.
- **Statusline**: modelo, repo, rama y diff, effort, barra de contexto, límites de 5h/7d, coste y duración.

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

## Desarrollo

Requiere Node 22+, pnpm y Rust ([prerequisitos de Tauri](https://tauri.app/start/prerequisites/)).

```sh
pnpm install
pnpm tauri dev     # app en modo desarrollo
pnpm tauri build   # instaladores para el sistema actual
```

Para publicar una versión, sube un tag `v*` y GitHub Actions genera los instaladores de Linux, Windows y macOS en un release en borrador:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

## Arquitectura

- `src-tauri/src/claude.rs`: lanza `claude -p --input-format stream-json --output-format stream-json --permission-prompt-tool stdio` y reenvía cada línea a la UI.
- `src-tauri/src/files.rs`: lectura de adjuntos, info de git, historial de sesiones (`~/.claude/projects`), shell local.
- `src/lib/session.svelte.ts`: estado de la sesión e interpretación del protocolo.
- `src/lib/components/`: transcripción, herramientas, permisos, prompt y statusline.
