// Rust 側のコマンドを呼ぶ。Tauri の外（ブラウザ）では架空のデータで動く「デモ表示」になる。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type Found = { address: string; name: string | null; tls_port: number | null; pair_port: number | null; legacy_open: boolean };
export type Ranked = { id: string; title: string; score: number };
export type Connected = { host: string; model: string | null; android: string | null; lists: Ranked[]; has_applied: boolean };
export type Row = {
  package: string; category: string; category_label: string; state: "on" | "off" | "absent";
  note: string | null; lists: number; protected: string | null; light: boolean; keep: boolean;
};
export type PlanView = { list_id: string; list_title: string; header: string; rows: Row[]; notes: string[] };
export type ApplyView = {
  disabled: number; still_on: string[]; protected: number; problems: number; batches: number; stopped: string | null;
  rolled_back: string[]; by_category: [string, number][]; card_path: string | null; card_png: string | null; record_path: string;
};
export type DriftView = { has_applied: boolean; at: string | null; returned: string[]; new_apps: string[]; home_before: string | null; home_now: string | null; home_changed: boolean };
export type ProblemView = { kind: string; title: string; severity: number };
export type RescueView = { problems: ProblemView[]; records: { path: string; created: string }[] };
export type Progress = { kind: string; done?: number; total?: number; name?: string };

// ---- デモ用の架空データ ----
const demoRows: Row[] = [
  ["com.example.adservice", "ads", "Ads", "Serves ads", 41],
  ["com.example.telemetry", "tracking", "Tracking", "Sends usage data", 33],
  ["com.example.analytics", "tracking", "Tracking", "Usage statistics", 28],
  ["com.example.tv.recommend", "promo", "Promo rows", "Recommendation rows", 51],
  ["com.example.storedemo", "demo", "Store demo", "In-store promo screen", 12],
].map(([p, c, l, n, k]) => ({ package: p as string, category: c as string, category_label: l as string, state: "on", note: n as string, lists: k as number, protected: null, light: c === "ads" || c === "tracking", keep: false }));
demoRows.push({ package: "com.example.voiceassist", category: "other", category_label: "Other", state: "on", note: null, lists: 9, protected: null, light: false, keep: true });
demoRows.push({ package: "com.example.tvinput", category: "other", category_label: "Other", state: "on", note: "HDMI inputs", lists: 3, protected: "It drives the HDMI/AV/tuner inputs; disabled, they can stay on \"No Signal\"", light: false, keep: false });
demoRows.push({ package: "com.example.oldpromo", category: "ads", category_label: "Ads", state: "off", note: null, lists: 7, protected: null, light: true, keep: false });

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function demo(cmd: string, args: Record<string, unknown>): Promise<unknown> {
  await wait(250);
  switch (cmd) {
    case "ui_lang": return navigator.language.startsWith("ja") ? "ja" : "en";
    case "scan": return [
      { address: "192.168.10.23", name: "Living room TV", tls_port: 37021, pair_port: null, legacy_open: true },
      { address: "192.168.10.41", name: "Bedroom TV", tls_port: null, pair_port: null, legacy_open: true },
      { address: "192.168.10.57", name: "Study projector", tls_port: null, pair_port: null, legacy_open: false },
    ] satisfies Found[];
    case "connect": return { host: String(args.host), model: "ACME 55U8", android: "12", lists: [{ id: "acme-55u8", title: "ACME 55U8 (fictional)", score: 0.92 }, { id: "acme-43u6", title: "ACME 43U6 (fictional)", score: 0.71 }], has_applied: false } satisfies Connected;
    case "make_plan": return { list_id: String(args.listId), list_title: "ACME 55U8 (fictional)", header: "", rows: demoRows, notes: [] } satisfies PlanView;
    case "apply": return { disabled: (args.selected as string[]).length, still_on: [], protected: 1, problems: 0, batches: 2, stopped: null, rolled_back: [], by_category: [["Ads", 1], ["Tracking", 2]], card_path: "/demo/hushtv.png", card_png: null, record_path: "~/.cleanyourtv/recovery-demo.json" } satisfies ApplyView;
    case "check": return { has_applied: true, at: "2026-10-30T12:00:00+0000", returned: ["com.example.adservice"], new_apps: ["com.example.newpromo"], home_before: "com.example.home/.Main", home_now: "com.example.stockhome/.Main", home_changed: true } satisfies DriftView;
    case "fix_drift": return 1;
    case "rescue_diagnose": return { problems: [
      { kind: "no_home", title: "Home has no working launcher (now: setupwraith/.RecoveryActivity)", severity: 4 },
      { kind: "core_disabled", title: "2 core Android packages are off (boot loops)", severity: 5 },
      { kind: "setupwraith_disabled", title: "The setup wizard is off (lost Wi-Fi, casting fails)", severity: 2 },
      { kind: "debloated", title: "31 apps from the debloat lists are off", severity: 0 },
    ], records: [{ path: "~/.cleanyourtv/recovery-192.168.10.23-3f9a1c07.json", created: "2026-10-30T12:00:00+0000" }] } satisfies RescueView;
    case "rescue_run": return [] satisfies ProblemView[];
    case "restore": return 12;
    case "close_debugging": return true;
    case "save_card": return "~/Pictures/hushtv.png";
  }
  throw new Error(`demo: ${cmd}`);
}

export async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!inTauri) return (await demo(cmd, args)) as T;
  return invoke<T>(cmd, args);
}

export async function onProgress(f: (p: Progress) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  return listen<Progress>("progress", (e) => f(e.payload));
}

export async function openUrl(url: string) {
  if (!inTauri) return void window.open(url, "_blank");
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

export async function reveal(path: string) {
  if (!inTauri) return;
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}

export async function copyPng(base64: string) {
  if (!inTauri) return;
  const { writeImage } = await import("@tauri-apps/plugin-clipboard-manager");
  const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
  await writeImage(bytes);
}
