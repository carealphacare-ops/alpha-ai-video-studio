import { useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { api, errText, fmtTime } from "../api";

interface Probe { duration: number; streams: { codec_type: string; codec_name: string; width?: number; height?: number }[] }

const EXTS = ["mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "m4a"];

export default function Clipping() {
  const [path, setPath] = useState("");
  const [info, setInfo] = useState<Probe | null>(null);
  const [start, setStart] = useState(0);
  const [end, setEnd] = useState(0);
  const [allowed, setAllowed] = useState(false);
  const [jobId, setJobId] = useState("");
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState("");
  const [err, setErr] = useState("");
  const [url, setUrl] = useState("");
  const video = useRef<HTMLVideoElement>(null);

  async function load(p: string) {
    setErr(""); setInfo(null); setStatus(""); setProgress(0);
    try {
      const raw = JSON.parse(await api.probeMedia(p));
      const dur = parseFloat(raw.format.duration);
      setInfo({ duration: dur, streams: raw.streams });
      setPath(p); setStart(0); setEnd(Math.min(dur, 60));
    } catch (e) { setErr(errText(e)); }
  }

  useEffect(() => {
    const drop = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type === "drop" && ev.payload.paths[0]) load(ev.payload.paths[0]);
    });
    const prog = listen<{ id: string; progress: number }>("job-progress", (e) => setProgress(e.payload.progress));
    const fin = listen<{ id: string; status: string; error: string | null }>("job-finished", (e) => {
      setStatus(e.payload.status);
      if (e.payload.error) setErr(e.payload.error);
      setJobId("");
    });
    return () => { [drop, prog, fin].forEach((s) => s.then((f) => f())); };
  }, []);

  async function pick() {
    const p = await open({ multiple: false, filters: [{ name: "Media", extensions: EXTS }] });
    if (typeof p === "string") load(p);
  }

  async function exportClip() {
    const out = await save({ defaultPath: "clip.mp4", filters: [{ name: "MP4", extensions: ["mp4"] }] });
    if (!out) return;
    setErr(""); setStatus("processing"); setProgress(0);
    try { setJobId(await api.startTrimExport(path, out, start, end)); }
    catch (e) { setErr(errText(e)); setStatus(""); }
  }

  const v = info?.streams.find((s) => s.codec_type === "video");
  const isAudioOnly = info && !v;
  const valid = info && end > start && start >= 0 && end <= info.duration + 0.01;

  return (
    <>
      {err && <div className="error">{err}</div>}
      <div className="card">
        <h3>Source</h3>
        <div className="row">
          <button className="primary" onClick={pick}>Import local file…</button>
          <span className="muted">or drag a file into this window. Formats: {EXTS.join(", ")}</span>
        </div>
        <div className="row" style={{ marginTop: 12 }}>
          <input placeholder="Paste a YouTube link (planned)" value={url} onChange={(e) => setUrl(e.target.value)} />
          <button disabled title="Not built yet">Fetch (planned)</button>
        </div>
        <p className="muted">Link import is not available yet. It will only use official/authorized access and will never bypass DRM or login walls. For now, download videos you are allowed to use and import the file.</p>
      </div>

      {info && (
        <div className="card">
          <h3>{path.split(/[\\/]/).pop()}</h3>
          <p className="muted">
            {fmtTime(info.duration)} · {v ? `${v.width}×${v.height} ${v.codec_name}` : "audio only"} · audio {info.streams.find((s) => s.codec_type === "audio")?.codec_name ?? "none"}
          </p>
          {!isAudioOnly && <video ref={video} src={convertFileSrc(path)} controls style={{ maxWidth: "100%", maxHeight: 360 }} />}
          <div className="row" style={{ marginTop: 12 }}>
            <label>Start (s) <input type="number" min={0} step={0.1} value={start} onChange={(e) => setStart(+e.target.value)} /></label>
            <button onClick={() => video.current && setStart(+video.current.currentTime.toFixed(2))} disabled={!!isAudioOnly}>Use playhead</button>
            <label>End (s) <input type="number" min={0} step={0.1} value={end} onChange={(e) => setEnd(+e.target.value)} /></label>
            <button onClick={() => video.current && setEnd(+video.current.currentTime.toFixed(2))} disabled={!!isAudioOnly}>Use playhead</button>
          </div>
          <label className="check">
            <input type="checkbox" checked={allowed} onChange={(e) => setAllowed(e.target.checked)} />
            I own this content or have permission to use and edit it.
          </label>
          <div className="row">
            <button className="primary" disabled={!valid || !allowed || !!jobId} onClick={exportClip}>Export trimmed MP4</button>
            {jobId && <button onClick={() => api.cancelJob(jobId)}>Cancel</button>}
            {status && <b className={status}>{status}</b>}
          </div>
          {(jobId || status === "completed") && <progress value={progress} max={1} style={{ width: "100%", marginTop: 8 }} />}
          <p className="muted">Output is re-encoded to H.264/AAC so cuts are frame-accurate. Audio-only sources are exported as video-less MP4 files.</p>
        </div>
      )}

      <div className="card">
        <h3>Not built yet in this studio</h3>
        <ul>
          <li>Transcription and word-level captions</li>
          <li>Highlight detection, scoring and multi-clip suggestions</li>
          <li>Vertical reframing with face tracking, silence removal, animated subtitles</li>
        </ul>
      </div>
    </>
  );
}
