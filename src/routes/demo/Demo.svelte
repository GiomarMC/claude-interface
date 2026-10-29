<script lang="ts">
  // Demo para las capturas del README: simula el backend con los mocks de
  // Tauri y reproduce una conversación a través del código real de la app.
  // Uso: /demo?scene=conversation|permission|start|setup|terminal (solo en `pnpm dev`).
  import { onMount } from "svelte";
  import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
  import { emit } from "@tauri-apps/api/event";
  import App from "../+page.svelte";
  import { version } from "../../../package.json";
  import { STATUSLINE } from "./statusline";
  import { RESUME_KEY } from "$lib/updater.svelte";
  import { CWD, FIX_PROMPT, FIX_TURN, ORDERS_TS, PNG, PUSH_PROMPT, PUSH_TURN } from "./scenes";

  const scene = new URLSearchParams(location.search).get("scene") ?? "conversation";
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const line = (obj: unknown) => emit("claude-event", { key: "main:1", line: JSON.stringify(obj) });

  const COMMANDS = [
    { name: "review", description: "Revisa un pull request" },
    { name: "compact", description: "Compacta la conversación para liberar contexto" },
    { name: "init", description: "Genera un CLAUDE.md con la documentación del proyecto" },
    { name: "security-review", description: "Revisión de seguridad de los cambios pendientes" },
  ];

  // Escena "setup": Claude Code sin instalar → instalar → iniciar sesión.
  let claude = scene === "setup"
    ? { installed: false, loggedIn: null as boolean | null, version: null as string | null }
    : { installed: true, loggedIn: true, version: "2.1.312" };

  // Shell simulado para la escena "terminal": eco, un par de comandos y la
  // misma secuencia OSC 633 que emite la función `claude` de la integración.
  const shell = (() => {
    const ttys = new Map<string, { id: number; n: number; line: string; cwd: string; waiting: boolean }>();
    const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));
    const out = (key: string, data: string) => {
      const t = ttys.get(key)!;
      (window as any).__TAURI_INTERNALS__.runCallback(t.id, { index: t.n++, message: { kind: "data", data } });
    };
    const prompt = (key: string) => out(key, `\x1b[32mdev@laptop\x1b[0m:\x1b[34m${ttys.get(key)!.cwd.replace("/home/dev", "~")}\x1b[0m$ `);
    return {
      spawn(key: string, channel: { id: number }) {
        ttys.set(key, { id: channel.id, n: 0, line: "", cwd: CWD, waiting: false });
        setTimeout(() => prompt(key), 50);
      },
      write(key: string, data: string) {
        const t = ttys.get(key);
        if (!t) return;
        for (const ch of data) {
          if (ch !== "\r") {
            t.line += ch;
            if (!t.waiting) out(key, ch);
            continue;
          }
          const line = t.line;
          t.line = "";
          if (t.waiting) {
            if (line === "ok") continue;
            t.waiting = false;
            if (line.startsWith("cd:")) t.cwd = line.slice(3);
            prompt(key);
            continue;
          }
          out(key, "\r\n");
          const [cmd, ...rest] = line.trim().split(/\s+/);
          if (cmd === "claude") {
            t.waiting = true;
            out(key, `\x1b]633;ClaudeInterface;${b64(t.cwd)};${b64(rest.map((a) => a + "\0").join(""))}\x07`);
            continue;
          }
          if (cmd === "ls") out(key, "README.md  package.json  \x1b[34msrc\x1b[0m  \x1b[34mtests\x1b[0m\r\n");
          else if (cmd === "pwd") out(key, t.cwd + "\r\n");
          else if (cmd) out(key, `${cmd}: orden no encontrada\r\n`);
          prompt(key);
        }
      },
    };
  })();

  async function stream(chunks: string[], exit: number | null) {
    for (const c of chunks) {
      await sleep(120);
      await emit("setup-output", c);
    }
    if (exit !== null) await emit("setup-exit", exit);
  }

  const ATTACHMENTS: Record<string, unknown> = {
    [`${CWD}/src/routes/orders.ts`]: { kind: "text", mime: "text/plain", size: 4310, data: ORDERS_TS },
    "/home/dev/Imágenes/error-500.png": { kind: "image", mime: "image/png", size: 182_000, data: PNG },
    "/home/dev/Documentos/especificacion-api.pdf": { kind: "pdf", mime: "application/pdf", size: 1_240_000, data: "" },
    [`${CWD}/tests`]: { kind: "dir", mime: "", size: 0, data: "" },
  };

  {
    localStorage.clear();
    // Las escenas de conversación abren directamente la interfaz de Claude;
    // "terminal" empieza en un shell simulado.
    if (scene !== "terminal") localStorage.setItem(RESUME_KEY, JSON.stringify({ sessionId: "", cwd: CWD }));
    mockWindows("main");
    mockIPC(
      (cmd, args: any) => {
        switch (cmd) {
          case "initial_cwd":
            return CWD;
          case "plugin:path|resolve_directory":
            return "/home/dev";
          case "plugin:app|version":
            return version;
          case "claude_version":
            return "2.1.312";
          case "user_settings":
            return { effortLevel: "high", statusLine: { type: "command", command: "demo" } };
          case "claude_theme":
            return "dark";
          case "app_config":
            return {};
          case "terminal_style":
            // La escena de terminal muestra el estilo por defecto de Windows (Breeze).
            return scene === "terminal"
              ? {
                  terminal: "default",
                  profile: "Breeze",
                  fontFamily: "Cascadia Mono",
                  fontSize: 11,
                  background: "#232627",
                  foreground: "#fcfcfc",
                  palette: ["#232627", "#ed1515", "#11d116", "#f67400", "#1d99f3", "#9b59b6", "#1abc9c", "#fcfcfc", "#7f8c8d", "#c0392b", "#1cdc9a", "#fdbc4b", "#3daee9", "#8e44ad", "#16a085", "#ffffff"],
                  cursorShape: "bar",
                  cursorBlink: false,
                }
              : null;
          case "terminal_keys":
            return {
              terminal: "konsole",
              bindings: {
                new_tab: ["ctrl+shift+t"],
                close_pane: ["ctrl+shift+w", "ctrl+shift+x"],
                next_tab: ["shift+right", "ctrl+tab"],
                prev_tab: ["shift+left", "ctrl+shift+tab"],
                copy: ["ctrl+shift+c"],
                paste: ["ctrl+shift+v"],
                split_right: ["ctrl+("],
                split_down: ["ctrl+)"],
                focus_left: ["ctrl+shift+left"],
                focus_right: ["ctrl+shift+right"],
                focus_up: ["ctrl+shift+up"],
                focus_down: ["ctrl+shift+down"],
                maximize_pane: ["ctrl+shift+e"],
                goto_tab_1: ["alt+1"],
                goto_tab_2: ["alt+2"],
              },
            };
          case "install_kind":
            return "dev";
          case "claude_status":
            return claude;
          case "install_claude":
            stream(
              [
                "Setting up Claude Code...\n",
                "✔ Claude Code successfully installed!\n  Version: 2.1.312\n  Location: ~/.local/bin/claude\n",
                "\n✅ Installation complete!\n",
              ],
              0,
            );
            claude = { installed: true, loggedIn: false, version: "2.1.312" };
            return null;
          case "login_claude":
            stream(
              [
                "Opening browser to sign in…\n",
                "If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=…\n",
                "Paste code here if prompted > ",
              ],
              null,
            );
            return null;
          case "git_info":
            return { repo: "tienda-api", branch: "fix/carrito-vacio", insertions: 11, deletions: 1 };
          case "run_statusline":
            return STATUSLINE;
          case "read_attachment": {
            const path = args.path as string;
            return { name: path.split("/").pop(), path, note: null, ...(ATTACHMENTS[path] as object) };
          }
          case "pty_spawn":
            return shell.spawn(args.key, args.onEvent);
          case "pty_write":
            return shell.write(args.key, args.data);
          case "send_line": {
            const msg = JSON.parse(args.line);
            if (msg.type === "control_request") {
              const sub = msg.request.subtype;
              const response =
                sub === "initialize"
                  ? { commands: COMMANDS }
                  : sub === "get_context_usage"
                    ? { totalTokens: 186_000, maxTokens: 1_000_000, percentage: 18.6 }
                    : {};
              setTimeout(() =>
                line({ type: "control_response", response: { subtype: "success", request_id: msg.request_id, response } }),
              );
            }
            return null;
          }
        }
        return null;
      },
      { shouldMockEvents: true },
    );
  }

  async function waitFor<T>(fn: () => T | null | undefined): Promise<T> {
    for (;;) {
      const v = fn();
      if (v) return v;
      await sleep(50);
    }
  }

  async function drop(paths: string[]) {
    await emit("tauri://drag-drop", { paths, position: { x: 0, y: 0 } });
    await sleep(150);
  }

  async function type(text: string) {
    const ta = document.querySelector("textarea")!;
    ta.value = text;
    ta.dispatchEvent(new Event("input", { bubbles: true }));
    await sleep(50);
  }

  async function send(text: string) {
    await type(text);
    document
      .querySelector("textarea")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await sleep(100);
  }

  async function play(steps: unknown[]) {
    for (const s of steps) {
      await line(s);
      await sleep(40);
    }
  }

  onMount(async () => {
    if (scene === "terminal") return;
    if (scene === "setup") {
      const click = async (text: string) => {
        const btn = await waitFor(() =>
          [...document.querySelectorAll("button")].find((b) => b.textContent?.includes(text)),
        );
        btn.click();
      };
      await click("Instalar Claude Code");
      await click("Cuenta de Claude");
      return;
    }

    await waitFor(() => document.querySelector("textarea"));
    await sleep(400);

    if (scene === "start") {
      await drop(["/home/dev/Imágenes/error-500.png", "/home/dev/Documentos/especificacion-api.pdf", `${CWD}/tests`]);
      await type("/");
      return;
    }

    await drop([`${CWD}/src/routes/orders.ts`, "/home/dev/Imágenes/error-500.png"]);
    await send(FIX_PROMPT);
    await play(FIX_TURN);

    if (scene === "permission") {
      await send(PUSH_PROMPT);
      await play(PUSH_TURN);
    }
    document.body.dataset.ready = "1";
  });
</script>

<App />
