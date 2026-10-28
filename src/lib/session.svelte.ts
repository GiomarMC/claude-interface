import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { modelName } from "./format";

export type Attachment = {
  name: string;
  path: string;
  kind: "text" | "image" | "pdf" | "dir" | "ref";
  mime: string;
  size: number;
  data: string;
  note?: string | null;
};

export type ToolResult = {
  text: string;
  isError: boolean;
  images: string[];
  meta?: any;
};

export type Item =
  | { kind: "user"; id: number; text: string; attachments: Attachment[] }
  | { kind: "text"; id: number; text: string }
  | { kind: "thinking"; id: number; text: string }
  | {
      kind: "tool";
      id: number;
      toolUseId: string;
      name: string;
      input: any;
      result?: ToolResult;
      subtasks: number;
    }
  | { kind: "system"; id: number; text: string; tone: "info" | "error" | "warn" };

export type PermissionRequest = {
  requestId: string;
  toolName: string;
  input: any;
  description?: string;
  suggestions: any[];
  toolUseId?: string;
};

export type SlashCommand = { name: string; description: string; argumentHint?: string; local?: boolean };

export type PermissionMode = "default" | "acceptEdits" | "plan" | "auto" | "bypassPermissions";
/** Mismo orden que shift+tab en la TUI. */
export const MODES: PermissionMode[] = ["default", "acceptEdits", "plan", "auto", "bypassPermissions"];

type GitInfo = { repo: string; branch: string | null; insertions: number; deletions: number };

const LOCAL_COMMANDS: SlashCommand[] = [
  { name: "clear", description: "Empieza una conversación nueva", local: true },
  { name: "resume", description: "Retoma una conversación anterior", local: true },
  { name: "cd", description: "Cambia el directorio de trabajo", local: true },
  { name: "export", description: "Exporta la conversación a Markdown", local: true },
  { name: "model", description: "Cambia el modelo (opus, sonnet, haiku…)", argumentHint: "<modelo>", local: true },
  { name: "effort", description: "Cambia el nivel de esfuerzo", argumentHint: "low|medium|high|xhigh|max", local: true },
  { name: "font", description: "Fuente y tamaño de la app", argumentHint: "<familia>|<tamaño>|reset", local: true },
  { name: "terminal", description: "Estilo de terminal a imitar (fondo, fuente, paleta)", argumentHint: "auto|off|konsole|kitty|…", local: true },
  { name: "theme", description: "Tema de la app (por defecto, el de Claude Code)", argumentHint: "dark|light|…|auto|claude", local: true },
];

let nextId = 1;
const uid = () => nextId++;

export class Session {
  readonly key = "main";

  cwd = $state("");
  home = $state("");
  items = $state<Item[]>([]);
  live = $state<{ kind: "text" | "thinking"; text: string } | null>(null);
  busy = $state(false);
  running = $state(false);
  turnStart = $state(0);
  thinkingTokens = $state(0);
  permission = $state<PermissionRequest | null>(null);

  sessionId = $state<string | null>(null);
  model = $state("");
  modelPref = $state("");
  version = $state("");
  mode = $state<PermissionMode>("default");
  effort = $state("");
  thinking = $state(false);
  fast = $state(false);
  outputStyle = $state("default");
  /** `statusLine.command` de los settings efectivos, si el usuario tiene uno. */
  statusLineCommand = $state<string | null>(null);
  linesAdded = $state(0);
  linesRemoved = $state(0);
  commands = $state<SlashCommand[]>(LOCAL_COMMANDS);

  contextUsed = $state<number | null>(null);
  contextMax = $state<number | null>(null);
  contextPct = $state<number | null>(null);
  costUsd = $state<number | null>(null);
  durationMs = $state(0);
  apiMs = $state(0);
  fiveHour = $state<{ pct: number; resetsAt: number } | null>(null);
  sevenDay = $state<{ pct: number; resetsAt: number } | null>(null);
  git = $state<GitInfo | null>(null);
  stderr = $state<string[]>([]);

  /** Salidas de comandos `!` que se adjuntan como contexto al siguiente mensaje. */
  shellContext: string[] = [];

  /** /model y /effort elegidos en la app; si no, manda la config de Claude Code. */
  private modelOverride = false;
  private effortOverride = false;

  private pending = new Map<string, (r: any) => void>();
  private toolIndex = new Map<string, number>();
  private unlisten: (() => void)[] = [];

  async init() {
    this.unlisten.push(
      await listen<{ key: string; line: string }>("claude-event", (e) => {
        if (e.payload.key !== this.key) return;
        try {
          this.handle(JSON.parse(e.payload.line));
        } catch (err) {
          console.error("línea inválida", e.payload.line, err);
        }
      }),
      await listen<string>("claude-exit", (e) => {
        if (e.payload !== this.key) return;
        this.running = false;
        this.busy = false;
        this.live = null;
      }),
      await listen<{ key: string; line: string }>("claude-stderr", (e) => {
        if (e.payload.key !== this.key) return;
        this.stderr = [...this.stderr.slice(-30), e.payload.line];
      }),
    );
  }

  private async loadSettings() {
    const settings: any = await invoke("user_settings", { cwd: this.cwd });
    if (!this.effortOverride) this.effort = settings?.effortLevel ?? "";
    if (!this.modelOverride) this.modelPref = settings?.model ?? "";
    const sl = settings?.statusLine;
    this.statusLineCommand = sl?.type === "command" && sl.command ? sl.command : null;
  }

  // ── Ciclo de vida ────────────────────────────────────────────────

  async start(opts: { cwd: string; resume?: string; keepItems?: boolean }) {
    this.cwd = opts.cwd;
    if (!opts.keepItems) {
      this.items = [];
      this.toolIndex.clear();
      this.costUsd = null;
      this.durationMs = 0;
      this.apiMs = 0;
      this.contextUsed = null;
      this.contextPct = null;
      this.linesAdded = 0;
      this.linesRemoved = 0;
      this.model = "";
      await this.loadSettings();
    }
    this.live = null;
    this.busy = false;
    this.permission = null;
    this.sessionId = opts.resume ?? null;
    this.stderr = [];

    if (opts.resume && !opts.keepItems) await this.loadHistory(opts.resume);

    await invoke("start_session", {
      opts: {
        key: this.key,
        cwd: opts.cwd,
        model: this.modelOverride ? this.modelPref : null,
        permissionMode: this.mode === "default" ? null : this.mode,
        effort: this.effortOverride ? this.effort : null,
        resume: opts.resume ?? null,
      },
    });
    this.running = true;
    this.refreshGit();

    const init = await this.control({ subtype: "initialize" });
    const cmds: SlashCommand[] = (init?.commands ?? []).map((c: any) => ({
      name: c.name,
      description: c.description ?? "",
      argumentHint: c.argumentHint,
    }));
    const local = new Set(LOCAL_COMMANDS.map((c) => c.name));
    this.commands = [...LOCAL_COMMANDS, ...cmds.filter((c) => !local.has(c.name))];
    this.refreshContext();
  }

  /** Reinicia el proceso conservando la conversación (para cambiar effort, etc.). */
  async restartKeeping() {
    await this.start({ cwd: this.cwd, resume: this.sessionId ?? undefined, keepItems: true });
  }

  async stop() {
    await invoke("stop_session", { key: this.key });
    this.running = false;
  }

  // ── Envío ────────────────────────────────────────────────────────

  private write(obj: unknown) {
    return invoke("send_line", { key: this.key, line: JSON.stringify(obj) });
  }

  control(request: Record<string, unknown>): Promise<any> {
    const id = `ui-${crypto.randomUUID()}`;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.write({ type: "control_request", request_id: id, request }).catch((e) => {
        this.pending.delete(id);
        this.notice(String(e), "error");
        resolve(null);
      });
      setTimeout(() => {
        if (this.pending.delete(id)) resolve(null);
      }, 15000);
    });
  }

  async send(text: string, attachments: Attachment[]) {
    if (!this.running) await this.start({ cwd: this.cwd, resume: this.sessionId ?? undefined, keepItems: true });
    const content: any[] = [];
    const byPath: string[] = [];
    for (const a of attachments) {
      if (a.kind === "image") {
        content.push({ type: "image", source: { type: "base64", media_type: a.mime, data: a.data } });
        if (a.path) byPath.push(`${a.path} (imagen adjunta arriba)`);
      } else if (a.kind === "pdf") {
        content.push({ type: "document", source: { type: "base64", media_type: "application/pdf", data: a.data } });
        byPath.push(`${a.path} (PDF adjunto arriba)`);
      } else if (a.kind === "text") {
        content.push({ type: "text", text: `<file path="${a.path}">\n${a.data}\n</file>` });
      } else {
        byPath.push(a.kind === "dir" ? `${a.path}/ (directorio)` : a.path);
      }
    }
    if (this.shellContext.length) {
      content.push({ type: "text", text: this.shellContext.join("\n\n") });
      this.shellContext = [];
    }
    let prompt = text;
    if (byPath.length) prompt = `${prompt}\n\nArchivos referenciados:\n${byPath.map((p) => `- ${p}`).join("\n")}`;
    if (prompt.trim()) content.push({ type: "text", text: prompt });
    if (!content.length) return;

    this.items.push({ kind: "user", id: uid(), text, attachments });
    if (!this.busy) {
      this.busy = true;
      this.turnStart = Date.now();
      this.thinkingTokens = 0;
    }
    await this.write({
      type: "user",
      session_id: this.sessionId ?? "",
      parent_tool_use_id: null,
      message: { role: "user", content },
    });
  }

  async interrupt() {
    if (!this.busy) return;
    if (this.permission) this.answerPermission({ behavior: "deny", message: "Interrumpido por el usuario", interrupt: true });
    await this.control({ subtype: "interrupt" });
    this.notice("Interrumpido · ¿qué debería hacer Claude en su lugar?", "warn");
  }

  answerPermission(response: Record<string, unknown>) {
    const req = this.permission;
    if (!req) return;
    this.permission = null;
    if (response.behavior === "allow" && !("updatedInput" in response)) response.updatedInput = req.input;
    this.write({
      type: "control_response",
      response: { subtype: "success", request_id: req.requestId, response },
    });
    const perms = (response.updatedPermissions as any[]) ?? [];
    const setMode = perms.find((p) => p.type === "setMode");
    if (setMode) this.mode = setMode.mode;
  }

  async setMode(mode: PermissionMode) {
    this.mode = mode;
    if (this.running) await this.control({ subtype: "set_permission_mode", mode });
  }

  cycleMode() {
    const i = MODES.indexOf(this.mode);
    return this.setMode(MODES[(i + 1) % MODES.length]);
  }

  async setModel(model: string) {
    this.modelPref = model;
    this.modelOverride = true;
    const r = this.running ? await this.control({ subtype: "set_model", model }) : null;
    this.notice(`Modelo: ${model}`, "info");
    if (r === null && this.running) await this.restartKeeping();
    this.refreshContext();
  }

  async setEffort(level: string) {
    this.effort = level;
    this.effortOverride = true;
    this.notice(`Esfuerzo: ${level}`, "info");
    if (this.running && !this.busy) await this.restartKeeping();
  }

  async runShell(command: string) {
    const toolUseId = `local-${uid()}`;
    this.items.push({ kind: "user", id: uid(), text: `! ${command}`, attachments: [] });
    this.toolIndex.set(toolUseId, this.items.length);
    this.items.push({ kind: "tool", id: uid(), toolUseId, name: "Bash", input: { command }, subtasks: 0 });
    const tool = this.items[this.items.length - 1] as Extract<Item, { kind: "tool" }>;
    try {
      const r = await invoke<{ output: string; code: number | null }>("run_shell", { cwd: this.cwd, command });
      tool.result = { text: r.output, isError: r.code !== 0, images: [] };
      const out = r.output.length > 20000 ? r.output.slice(0, 20000) + "\n…(truncado)" : r.output;
      this.shellContext.push(
        `<bash-input>${command}</bash-input>\n<bash-stdout>${out}</bash-stdout>\n(exit code ${r.code ?? "?"}; el usuario ejecutó este comando localmente)`,
      );
    } catch (e) {
      tool.result = { text: String(e), isError: true, images: [] };
    }
    this.refreshGit();
  }

  notice(text: string, tone: "info" | "error" | "warn" = "info") {
    this.items.push({ kind: "system", id: uid(), text, tone });
  }

  async refreshContext() {
    if (!this.running) return;
    const r = await this.control({ subtype: "get_context_usage" });
    if (!r) return;
    this.contextUsed = r.totalTokens ?? null;
    this.contextMax = r.maxTokens ?? null;
    this.contextPct = r.percentage ?? (r.totalTokens && r.maxTokens ? (100 * r.totalTokens) / r.maxTokens : null);
    if (r.model && !this.model) this.model = r.model;
  }

  async refreshGit() {
    if (this.cwd) this.git = await invoke<GitInfo>("git_info", { cwd: this.cwd });
  }

  // ── Protocolo ────────────────────────────────────────────────────

  private handle(m: any) {
    switch (m.type) {
      case "system":
        return this.handleSystem(m);
      case "stream_event":
        if (!m.parent_tool_use_id) this.handleStream(m.event);
        return;
      case "assistant":
        return this.handleAssistant(m.message, m.parent_tool_use_id);
      case "user":
        return this.handleUser(m.message, m.parent_tool_use_id, m.tool_use_result);
      case "result":
        return this.handleResult(m);
      case "rate_limit_event": {
        const w = m.rate_limit_info?.unifiedWindows;
        if (w?.five_hour) this.fiveHour = { pct: w.five_hour.utilization * 100, resetsAt: w.five_hour.resetsAt };
        if (w?.seven_day) this.sevenDay = { pct: w.seven_day.utilization * 100, resetsAt: w.seven_day.resetsAt };
        return;
      }
      case "control_request":
        return this.handleControlRequest(m);
      case "control_response": {
        const r = m.response;
        const resolve = this.pending.get(r?.request_id);
        if (resolve) {
          this.pending.delete(r.request_id);
          resolve(r.subtype === "success" ? (r.response ?? {}) : null);
        }
        return;
      }
    }
  }

  private handleSystem(m: any) {
    if (m.subtype === "init") {
      this.sessionId = m.session_id;
      this.model = m.model;
      this.version = m.claude_code_version ?? this.version;
      this.outputStyle = m.output_style ?? "default";
      const mode = m.permissionMode === "manual" ? "default" : m.permissionMode;
      if (MODES.includes(mode)) this.mode = mode;
      this.fast = m.fast_mode_state === "on";
    } else if (m.subtype === "thinking_tokens") {
      this.thinkingTokens = m.estimated_tokens ?? this.thinkingTokens;
    } else if (m.subtype === "compact_boundary") {
      this.notice("✻ Conversación compactada", "info");
    } else if (m.subtype === "status" && m.status === "requesting" && !this.busy) {
      this.busy = true;
      this.turnStart = Date.now();
    }
  }

  private handleStream(ev: any) {
    if (!ev) return;
    if (ev.type === "content_block_start") {
      const t = ev.content_block?.type;
      this.live = t === "text" || t === "thinking" ? { kind: t, text: "" } : null;
      if (t === "thinking") this.thinking = true;
    } else if (ev.type === "content_block_delta" && this.live) {
      const d = ev.delta;
      if (d?.type === "text_delta") this.live.text += d.text;
      else if (d?.type === "thinking_delta") this.live.text += d.thinking;
    }
  }

  private handleAssistant(msg: any, parent: string | null) {
    if (parent) {
      const i = this.toolIndex.get(parent);
      const tool = i != null ? this.items[i] : null;
      if (tool?.kind === "tool") {
        tool.subtasks += (msg.content ?? []).filter((b: any) => b.type === "tool_use").length;
      }
      return;
    }
    for (const b of msg.content ?? []) {
      if (b.type === "text" && b.text?.trim()) {
        this.items.push({ kind: "text", id: uid(), text: b.text });
      } else if (b.type === "thinking" && b.thinking?.trim()) {
        this.items.push({ kind: "thinking", id: uid(), text: b.thinking });
      } else if (b.type === "tool_use") {
        this.toolIndex.set(b.id, this.items.length);
        this.items.push({ kind: "tool", id: uid(), toolUseId: b.id, name: b.name, input: b.input ?? {}, subtasks: 0 });
      }
    }
    this.live = null;
  }

  private handleUser(msg: any, parent: string | null, meta?: any) {
    if (parent) return;
    const content = msg?.content;
    if (typeof content === "string") {
      this.pushUserText(content);
      return;
    }
    for (const b of content ?? []) {
      if (b.type === "tool_result") {
        const i = this.toolIndex.get(b.tool_use_id);
        const tool = i != null ? this.items[i] : null;
        if (tool?.kind !== "tool") continue;
        const parts = typeof b.content === "string" ? [{ type: "text", text: b.content }] : (b.content ?? []);
        tool.result = {
          text: parts.filter((p: any) => p.type === "text").map((p: any) => p.text).join("\n"),
          images: parts
            .filter((p: any) => p.type === "image" && p.source?.data)
            .map((p: any) => `data:${p.source.media_type};base64,${p.source.data}`),
          isError: !!b.is_error,
          meta: typeof meta === "object" ? meta : undefined,
        };
        if (!b.is_error) this.countLines(tool.name, tool.input, tool.result.meta);
      } else if (b.type === "text") {
        this.pushUserText(b.text);
      }
    }
  }

  private countLines(name: string, input: any, meta: any) {
    if (name === "Write") {
      this.linesAdded += String(input?.content ?? "").split("\n").length;
    } else if (name === "Edit" || name === "MultiEdit") {
      for (const h of meta?.structuredPatch ?? []) {
        for (const l of h.lines ?? []) {
          if (l[0] === "+") this.linesAdded++;
          else if (l[0] === "-") this.linesRemoved++;
        }
      }
    }
  }

  /** Mismo JSON que Claude Code envía por stdin al comando del statusline. */
  statusLineInput(): string {
    const pct = this.contextPct;
    const encoded = this.cwd.replace(/[^A-Za-z0-9]/g, "-");
    const sep = this.home.includes("\\") ? "\\" : "/";
    const model = this.model || this.modelPref;
    return JSON.stringify({
      hook_event_name: "Status",
      session_id: this.sessionId ?? "",
      transcript_path: this.sessionId
        ? [this.home, ".claude", "projects", encoded, `${this.sessionId}.jsonl`].join(sep)
        : "",
      cwd: this.cwd,
      model: { id: model, display_name: modelName(model) || model },
      workspace: { current_dir: this.cwd, project_dir: this.cwd },
      version: this.version,
      output_style: { name: this.outputStyle },
      cost: {
        total_cost_usd: this.costUsd ?? 0,
        total_duration_ms: this.durationMs,
        total_api_duration_ms: this.apiMs,
        total_lines_added: this.linesAdded,
        total_lines_removed: this.linesRemoved,
      },
      context_window: this.contextMax
        ? {
            context_window_size: this.contextMax,
            total_input_tokens: this.contextUsed ?? 0,
            used_percentage: pct ?? 0,
            remaining_percentage: 100 - (pct ?? 0),
          }
        : undefined,
      exceeds_200k_tokens: (this.contextUsed ?? 0) > 200_000,
      rate_limits:
        this.fiveHour || this.sevenDay
          ? {
              five_hour: this.fiveHour
                ? { used_percentage: this.fiveHour.pct, resets_at: this.fiveHour.resetsAt }
                : undefined,
              seven_day: this.sevenDay
                ? { used_percentage: this.sevenDay.pct, resets_at: this.sevenDay.resetsAt }
                : undefined,
            }
          : undefined,
      effort: this.effort ? { level: this.effort } : undefined,
      thinking: { enabled: this.thinking },
      fast_mode: this.fast,
      permission_mode: this.mode,
    });
  }

  /** Texto de usuario que llega desde el historial (o salida de comandos locales). */
  private pushUserText(text: string) {
    const stdout = text.match(/<local-command-stdout>([\s\S]*?)<\/local-command-stdout>/);
    if (stdout) {
      if (stdout[1].trim()) this.notice(stdout[1].trim(), "info");
      return;
    }
    const cmd = text.match(/<command-name>([\s\S]*?)<\/command-name>/);
    if (cmd) {
      const args = text.match(/<command-args>([\s\S]*?)<\/command-args>/)?.[1] ?? "";
      this.items.push({ kind: "user", id: uid(), text: `${cmd[1]} ${args}`.trim(), attachments: [] });
      return;
    }
    if (text.startsWith("<") || !text.trim()) return;
    this.items.push({ kind: "user", id: uid(), text, attachments: [] });
  }

  private handleResult(m: any) {
    this.busy = false;
    this.live = null;
    this.durationMs += m.duration_ms ?? 0;
    this.apiMs += m.duration_api_ms ?? 0;
    if (typeof m.total_cost_usd === "number") this.costUsd = m.total_cost_usd;
    const usage = Object.values(m.modelUsage ?? {})[0] as any;
    if (usage?.contextWindow) this.contextMax = usage.contextWindow;
    if (m.is_error && m.subtype !== "error_during_execution") {
      this.notice(m.result || m.api_error_status || `Error: ${m.subtype}`, "error");
    }
    this.refreshContext();
    this.refreshGit();
  }

  private handleControlRequest(m: any) {
    const r = m.request;
    if (r?.subtype === "can_use_tool") {
      this.permission = {
        requestId: m.request_id,
        toolName: r.tool_name,
        input: r.input ?? {},
        description: r.description,
        suggestions: r.permission_suggestions ?? [],
        toolUseId: r.tool_use_id,
      };
      return;
    }
    this.write({
      type: "control_response",
      response: { subtype: "error", request_id: m.request_id, error: `No soportado: ${r?.subtype}` },
    });
  }

  // ── Historial ────────────────────────────────────────────────────

  private async loadHistory(id: string) {
    const lines = await invoke<any[]>("load_session", { cwd: this.cwd, id });
    for (const l of lines) {
      if (l.type === "assistant") this.handleAssistant(l.message, null);
      else if (l.type === "user") this.handleUser(l.message, null, l.toolUseResult);
    }
    this.notice("── conversación retomada ──", "info");
  }

  // ── Exportar ─────────────────────────────────────────────────────

  toMarkdown(): string {
    const out: string[] = [`# Conversación · ${this.git?.repo ?? this.cwd}`, ""];
    for (const it of this.items) {
      if (it.kind === "user") {
        out.push(`> **Tú:** ${it.text.replace(/\n/g, "\n> ")}`);
        for (const a of it.attachments) out.push(`> 📎 \`${a.path || a.name}\``);
        out.push("");
      } else if (it.kind === "text") {
        out.push(it.text, "");
      } else if (it.kind === "tool") {
        out.push(`\`⏺ ${it.name}(${toolArg(it.name, it.input, this.cwd)})\``, "");
      } else if (it.kind === "system") {
        out.push(`*${it.text}*`, "");
      }
    }
    return out.join("\n");
  }
}

// ── Utilidades de presentación de herramientas ──────────────────────

export function relPath(p: string, cwd: string): string {
  if (!p) return "";
  if (cwd && p.startsWith(cwd + "/")) return p.slice(cwd.length + 1);
  return p;
}

export function toolArg(name: string, input: any, cwd: string): string {
  if (!input) return "";
  switch (name) {
    case "Bash":
      return input.command ?? "";
    case "Read":
    case "Write":
    case "Edit":
    case "MultiEdit":
    case "NotebookEdit":
      return relPath(input.file_path ?? input.notebook_path ?? "", cwd);
    case "Glob":
      return input.pattern ?? "";
    case "Grep":
      return `pattern: "${input.pattern ?? ""}"${input.path ? `, path: "${relPath(input.path, cwd)}"` : ""}`;
    case "WebFetch":
      return input.url ?? "";
    case "WebSearch":
      return input.query ?? "";
    case "Task":
    case "Agent":
      return input.description ?? "";
    case "Skill":
      return input.skill ?? "";
    case "TodoWrite":
      return "";
  }
  const first = Object.values(input).find((v) => typeof v === "string") as string | undefined;
  return first ? (first.length > 80 ? first.slice(0, 80) + "…" : first) : "";
}

export function toolTitle(name: string): string {
  if (name === "Bash") return "Bash";
  const mcp = name.match(/^mcp__(.+?)__(.+)$/);
  if (mcp) return `${mcp[1].replace(/^claude_ai_/, "")} - ${mcp[2]} (MCP)`;
  if (name === "Edit" || name === "MultiEdit") return "Update";
  if (name === "Task" || name === "Agent") return "Task";
  return name;
}
