import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errText } from "../api";

const FOLDERS: [string, string][] = [
  ["projects_dir", "Projects folder"],
  ["exports_dir", "Default exports folder"],
  ["ffmpeg_dir", "FFmpeg folder (optional; contains ffmpeg and ffprobe)"],
];
const NUMBERS: [string, string][] = [
  ["monthly_budget", "Monthly AI budget (USD)"],
  ["project_budget", "Per-project budget (USD)"],
  ["max_generation_cost", "Max cost of a single generation (USD)"],
];

export default function SettingsPage() {
  const [s, setS] = useState<Record<string, string>>({});
  const [err, setErr] = useState("");
  const load = () => api.getSettings().then(setS).catch((e) => setErr(errText(e)));
  useEffect(() => { load(); }, []);
  const set = (k: string, v: string) => api.setSetting(k, v).then(() => { setErr(""); return load(); }).catch((e) => setErr(errText(e)));

  return (
    <>
      {err && <div className="error">{err}</div>}
      <div className="card">
        <h3>Folders</h3>
        {FOLDERS.map(([k, label]) => (
          <div className="row" key={k} style={{ marginBottom: 8 }}>
            <label style={{ width: 340 }}>{label}</label>
            <input readOnly value={s[k] ?? "(default)"} />
            <button onClick={async () => { const d = await open({ directory: true }); if (typeof d === "string") set(k, d); }}>Choose…</button>
            {s[k] && <button onClick={() => set(k, "")}>Reset</button>}
          </div>
        ))}
        <p className="muted">The app never deletes your source media files.</p>
      </div>
      <div className="card">
        <h3>Spending limits</h3>
        {NUMBERS.map(([k, label]) => (
          <div className="row" key={k} style={{ marginBottom: 8 }}>
            <label style={{ width: 340 }}>{label}</label>
            <input type="number" min={0} step={0.5} defaultValue={s[k] ?? ""} key={s[k]} onBlur={(e) => e.target.value !== (s[k] ?? "") && set(k, e.target.value)} />
          </div>
        ))}
        <label className="check">
          <input type="checkbox" checked={s.local_only === "1"} onChange={(e) => set("local_only", e.target.checked ? "1" : "0")} />
          Local-only mode (no cloud AI calls)
        </label>
        <p className="muted">Limits are stored now and will be enforced when cloud providers are connected (Phase 7).</p>
      </div>
    </>
  );
}
