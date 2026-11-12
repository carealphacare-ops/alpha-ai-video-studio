import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errText, fmtDate, type Job, type Project, type ProviderState, type ToolStatus, type UsageSummary } from "../api";

export default function Dashboard({ go }: { go: (p: string) => void }) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [exports, setExports] = useState<[string, number][]>([]);
  const [usage, setUsage] = useState<UsageSummary | null>(null);
  const [tools, setTools] = useState<ToolStatus | null>(null);
  const [prov, setProv] = useState<ProviderState[]>([]);
  const [err, setErr] = useState("");

  const load = () =>
    Promise.all([api.listProjects(), api.listJobs(), api.listExports(), api.usageSummary(), api.toolStatus(), api.listProviderStates()])
      .then(([p, j, e, u, t, pr]) => { setProjects(p); setJobs(j); setExports(e); setUsage(u); setTools(t); setProv(pr); })
      .catch((e) => setErr(errText(e)));

  useEffect(() => {
    load();
    const subs = [listen("job-progress", load), listen("job-finished", load)];
    return () => { subs.forEach((s) => s.then((f) => f())); };
  }, []);

  return (
    <>
      {err && <div className="error">{err}</div>}
      <div className="grid">
        <div className="card">
          <h3>Start</h3>
          <button className="primary" onClick={() => go("clipping")}>AI Clipping Studio</button>{" "}
          <button disabled title="Planned: Phase 4">AI Story Studio (planned)</button>
        </div>
        <div className="card">
          <h3>Tools</h3>
          <p>FFmpeg: {tools?.ffmpeg ? <span className="ok">found</span> : <span className="bad">not found</span>}</p>
          <p>FFprobe: {tools?.ffprobe ? <span className="ok">found</span> : <span className="bad">not found</span>}</p>
          {!tools?.ffmpeg && <p className="muted">Set the FFmpeg folder in Settings, or use the installer build which bundles it.</p>}
        </div>
        <div className="card">
          <h3>AI usage (last 30 days)</h3>
          <p>{usage?.count ?? 0} cloud operations</p>
          <p>Estimated: ${(usage?.month_estimated ?? 0).toFixed(2)}</p>
          <p>Provider-reported: ${(usage?.month_actual ?? 0).toFixed(2)}</p>
          <p className="muted">Estimates are not actual charges.</p>
        </div>
        <div className="card">
          <h3>Providers</h3>
          {prov.map((p) => (
            <p key={p.id}>{p.id}: {p.has_key ? (p.enabled ? <span className="ok">key saved, enabled</span> : "key saved, disabled") : <span className="muted">no key</span>}</p>
          ))}
        </div>
      </div>
      <div className="grid two">
        <div className="card">
          <h3>Recent projects</h3>
          {projects.length === 0 && <p className="muted">No projects yet. Create one in Project Library.</p>}
          {projects.slice(0, 6).map((p) => <p key={p.id}>{p.name} <span className="muted">· {p.kind} · {fmtDate(p.updated_at)}</span></p>)}
        </div>
        <div className="card">
          <h3>Processing queue</h3>
          {jobs.length === 0 && <p className="muted">No jobs yet.</p>}
          {jobs.slice(0, 8).map((j) => (
            <div key={j.id} className="job">
              <span>{j.kind} · <b className={j.status}>{j.status}</b></span>
              <progress value={j.progress} max={1} />
              {j.error && <small className="bad">{j.error}</small>}
            </div>
          ))}
        </div>
      </div>
      <div className="card">
        <h3>Recent exports</h3>
        {exports.length === 0 && <p className="muted">Nothing exported yet.</p>}
        {exports.map(([path, t]) => <p key={path + t}>{path} <span className="muted">· {fmtDate(t)}</span></p>)}
      </div>
    </>
  );
}
