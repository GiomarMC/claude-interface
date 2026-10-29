<script lang="ts">
  // Demo para las capturas del README: simula el backend con los mocks de
  // Tauri y reproduce una conversación a través del código real de la app.
  // Uso: /demo?scene=conversation|permission|start (solo en `pnpm dev`).
  import { onMount } from "svelte";
  import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
  import { emit } from "@tauri-apps/api/event";
  import App from "../+page.svelte";
  import { version } from "../../../package.json";
  import { STATUSLINE } from "./statusline";
  import { CWD, FIX_PROMPT, FIX_TURN, ORDERS_TS, PNG, PUSH_PROMPT, PUSH_TURN } from "./scenes";

  const scene = new URLSearchParams(location.search).get("scene") ?? "conversation";
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const line = (obj: unknown) => emit("claude-event", { key: "main", line: JSON.stringify(obj) });

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
            return null;
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
