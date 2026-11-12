import { useEffect, useState } from "react";
import { api, errText, fmtDate, type Project } from "../api";

export default function Projects() {
  const [list, setList] = useState<Project[]>([]);
  const [name, setName] = useState("");
  const [kind, setKind] = useState("clipping");
  const [err, setErr] = useState("");

  const load = () => api.listProjects().then(setList).catch((e) => setErr(errText(e)));
  useEffect(() => { load(); }, []);
  const run = (p: Promise<unknown>) => p.then(() => { setErr(""); return load(); }).catch((e) => setErr(errText(e)));

  return (
    <>
      {err && <div className="error">{err}</div>}
      <div className="card row">
        <input placeholder="Project name" value={name} onChange={(e) => setName(e.target.value)} />
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="clipping">Clipping</option>
          <option value="story">Story</option>
          <option value="editor">Editor</option>
        </select>
        <button className="primary" onClick={() => run(api.createProject(name, kind).then(() => setName("")))}>Create project</button>
      </div>
      <div className="card">
        {list.length === 0 && <p className="muted">No projects yet.</p>}
        {list.map((p) => (
          <div className="row between" key={p.id}>
            <span>{p.name} <span className="muted">· {p.kind} · {p.status} · updated {fmtDate(p.updated_at)}</span></span>
            <span>
              <button onClick={() => { const n = prompt("New name", p.name); if (n) run(api.renameProject(p.id, n)); }}>Rename</button>{" "}
              <button onClick={() => run(api.duplicateProject(p.id))}>Duplicate</button>{" "}
              <button className="danger" onClick={() => { if (confirm(`Delete project "${p.name}"? Your media files are not deleted.`)) run(api.deleteProject(p.id)); }}>Delete</button>
            </span>
          </div>
        ))}
      </div>
    </>
  );
}
