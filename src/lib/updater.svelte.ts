import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export const RESUME_KEY = "ci.resumeAfterUpdate";
const CHECK_EVERY = 6 * 60 * 60 * 1000;

type Status = "idle" | "checking" | "available" | "downloading" | "installing" | "error";

class Updater {
  status = $state<Status>("idle");
  version = $state("");
  notes = $state("");
  progress = $state<number | null>(null);
  error = $state("");
  dismissed = $state(false);
  /** "appimage" | "rpm" | "deb" | "bundle" | "other" | "dev" */
  kind = $state("");

  private update: Update | null = null;
  private timer: ReturnType<typeof setInterval> | undefined;

  get canInstall(): boolean {
    return ["appimage", "rpm", "deb", "bundle"].includes(this.kind);
  }

  async start(enabled: boolean) {
    this.kind = await invoke<string>("install_kind");
    clearInterval(this.timer);
    if (!enabled || this.kind === "dev") return;
    setTimeout(() => this.check(), 5000);
    this.timer = setInterval(() => this.check(), CHECK_EVERY);
  }

  stop() {
    clearInterval(this.timer);
  }

  /** Devuelve true si hay una versión nueva. */
  async check(): Promise<boolean> {
    if (this.status === "downloading" || this.status === "installing") return true;
    this.status = "checking";
    this.error = "";
    try {
      this.update = await check();
      if (this.update) {
        this.version = this.update.version;
        this.notes = this.update.body ?? "";
        this.status = "available";
        this.dismissed = false;
        return true;
      }
      this.status = "idle";
      return false;
    } catch (e) {
      this.status = "error";
      this.error = String(e);
      return false;
    }
  }

  /** Instala y reinicia; `sessionId` se retoma al volver a abrir. */
  async install(sessionId: string | null, cwd: string) {
    if (!this.update) return;
    this.error = "";
    try {
      if (this.kind === "rpm" || this.kind === "deb") {
        this.status = "installing";
        await invoke("install_linux_package", { version: this.version });
      } else {
        this.status = "downloading";
        let total = 0;
        let done = 0;
        await this.update.downloadAndInstall((ev) => {
          if (ev.event === "Started") total = ev.data.contentLength ?? 0;
          else if (ev.event === "Progress") {
            done += ev.data.chunkLength;
            this.progress = total ? Math.round((100 * done) / total) : null;
          } else if (ev.event === "Finished") this.status = "installing";
        });
      }
      try {
        if (sessionId) localStorage.setItem(RESUME_KEY, JSON.stringify({ sessionId, cwd }));
      } catch {}
      await relaunch();
    } catch (e) {
      this.status = "available";
      this.error = String(e);
    }
  }
}

export const updater = new Updater();
