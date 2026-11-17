import { useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { api, errText, fmtTime, Transcript } from "../api";

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

  // Captions
  const [txReady, setTxReady] = useState<{ ready: boolean; detail: string } | null>(null);
  const [language, setLanguage] = useState("auto");
  const [txPath, setTxPath] = useState("");
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [capStyle, setCapStyle] = useState("bold_yellow");
  const [rangeOnly, setRangeOnly] = useState(false);
  const [find, setFind] = useState("");
  const [replace, setReplace] = useState("");
  const [dirty, setDirty] = useState(false);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState<{ kind: string; id: string; progress: number } | null>(null);
  const kinds = useRef<Record<string, string>>({});
  const early = useRef<Record<string, { status: string; error: string | null }>>({});
  const pathRef = useRef("");
  const txPathRef = useRef("");

  async function load(p: string) {
    setErr(""); setInfo(null); setStatus(""); setProgress(0);
    try {
      const raw = JSON.parse(await api.probeMedia(p));
      const dur = parseFloat(raw.format.duration);
      setInfo({ duration: dur, streams: raw.streams });
      setPath(p); setStart(0); setEnd(Math.min(dur, 60)); setTranscript(null); setTxPath(""); setNote("");
    } catch (e) { setErr(errText(e)); }
  }

  useEffect(() => {
    const drop = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type === "drop" && ev.payload.paths[0]) load(ev.payload.paths[0]);
    });
    const prog = listen<{ id: string; progress: number }>("job-progress", (e) => {
      if (kinds.current[e.payload.id]) setProgress(e.payload.progress);
      setBusy((b) => (b && b.id === e.payload.id ? { ...b, progress: e.payload.progress } : b));
    });
    const fin = listen<{ id: string; status: string; error: string | null }>("job-finished", (e) => finished(e.payload.id, e.payload.status, e.payload.error));
    return () => { [drop, prog, fin].forEach((s) => s.then((f) => f())); };
  }, []);

  async function finished(id: string, st: string, error: string | null) {
    const kind = kinds.current[id];
    if (!kind) { early.current[id] = { status: st, error }; return; }
    delete kinds.current[id];
    setBusy(null); setJobId("");
    if (error) setErr(error);
    if (kind === "trim") setStatus(st);
    if (kind === "transcribe") {
      if (st === "completed") {
        try { setTranscript(await api.loadTranscript(txPathRef.current)); setDirty(false); setNote("Transcript ready."); }
        catch (e) { setErr(errText(e)); }
      } else if (st === "cancelled") setNote("Transcription cancelled.");
    }
    if (kind === "caption") setNote(st === "completed" ? "Clip with captions saved." : st === "cancelled" ? "Cancelled." : "");
  }

  function track(id: string, kind: string) {
    kinds.current[id] = kind;
    const e = early.current[id];
    if (e) { delete early.current[id]; finished(id, e.status, e.error); }
  }

  useEffect(() => { api.transcribeStatus().then(setTxReady).catch((e) => setErr(errText(e))); }, []);
  useEffect(() => { pathRef.current = path; }, [path]);

  async function transcribeNow() {
    setErr(""); setNote(""); setTranscript(null);
    try {
      const j = await api.startTranscribe(path, language);
      setTxPath(j.output); txPathRef.current = j.output;
      setBusy({ kind: "transcribe", id: j.id, progress: 0 });
      track(j.id, "transcribe");
    } catch (e) { setErr(errText(e)); }
  }

  async function exportCaptionFile(fmt: "srt" | "vtt" | "ass") {
    const out = await save({ defaultPath: `captions.${fmt}`, filters: [{ name: fmt.toUpperCase(), extensions: [fmt] }] });
    if (!out) return;
    setErr(""); setNote("");
    try {
      await api.exportCaptions(txPath, fmt, out, capStyle, rangeOnly ? start : 0, rangeOnly ? end : 0, v?.width ?? 1080, v?.height ?? 1920);
      setNote(`Saved ${out}`);
    } catch (e) { setErr(errText(e)); }
  }

  async function captionClip() {
    const out = await save({ defaultPath: "clip-captions.mp4", filters: [{ name: "MP4", extensions: ["mp4"] }] });
    if (!out) return;
    setErr(""); setNote("");
    try {
      const j = await api.startCaptionClip(path, txPath, out, start, end, capStyle);
      setBusy({ kind: "caption", id: j.id, progress: 0 });
      track(j.id, "caption");
    } catch (e) { setErr(errText(e)); }
  }

  async function applyReplace() {
    if (!transcript || !find) return;
    const re = new RegExp(find.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
    let n = 0;
    const words = transcript.words.map((w) => {
      if (!re.test(w.text)) { re.lastIndex = 0; return w; }
      re.lastIndex = 0; n++;
      return { ...w, text: w.text.replace(re, replace) };
    });
    setTranscript({ ...transcript, words }); setDirty(n > 0);
    setNote(n ? `Changed ${n} word(s). Press "Save corrections".` : "No matching words.");
  }

  async function saveCorrections() {
    if (!transcript) return;
    try { await api.saveTranscript(txPath, transcript); setDirty(false); setNote("Corrections saved."); }
    catch (e) { setErr(errText(e)); }
  }

  async function pick() {
    const p = await open({ multiple: false, filters: [{ name: "Media", extensions: EXTS }] });
    if (typeof p === "string") load(p);
  }

  async function exportClip() {
    const out = await save({ defaultPath: "clip.mp4", filters: [{ name: "MP4", extensions: ["mp4"] }] });
    if (!out) return;
    setErr(""); setStatus("processing"); setProgress(0);
    try { const id = await api.startTrimExport(path, out, start, end); setJobId(id); track(id, "trim"); }
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


      {info && (
        <div className="card">
          <h3>Captions (local transcription)</h3>
          {txReady && !txReady.ready && <p className="muted">{txReady.detail}</p>}
          <div className="row">
            <label>Spoken language{" "}
              <select value={language} onChange={(e) => setLanguage(e.target.value)}>
                <option value="auto">Detect automatically</option>
                <option value="en">English</option>
                <option value="ur">Urdu</option>
                <option value="hi">Hindi</option>
              </select>
            </label>
            <button className="primary" disabled={!txReady?.ready || !!busy || !info.streams.some((s) => s.codec_type === "audio")} onClick={transcribeNow}>
              {transcript ? "Transcribe again" : "Transcribe"}
            </button>
            {busy?.kind === "transcribe" && <button onClick={() => api.cancelJob(busy.id)}>Cancel</button>}
          </div>
          {busy?.kind === "transcribe" && <progress value={busy.progress} max={1} style={{ width: "100%", marginTop: 8 }} />}
          <p className="muted">Runs on your computer, no internet or API key needed. Long videos take a while on CPU. Accuracy for Urdu and Hindi is lower than for English; check the text before exporting.</p>

          {transcript && (
            <>
              <p className="muted">Language: {transcript.language || "unknown"} · {transcript.words.length} words. Click a word to jump there.</p>
              <div style={{ maxHeight: 220, overflow: "auto", lineHeight: 1.7, padding: 8, background: "rgba(255,255,255,0.04)", borderRadius: 8 }} dir="auto">
                {transcript.words.map((w, i) => (
                  <span key={i} title={fmtTime(w.start)} style={{ cursor: "pointer" }} onClick={() => { if (video.current) video.current.currentTime = w.start; }}>{w.text}{" "}</span>
                ))}
              </div>
              <div className="row" style={{ marginTop: 8 }}>
                <input placeholder="Find (for example a misheard name)" value={find} onChange={(e) => setFind(e.target.value)} />
                <input placeholder="Replace with" value={replace} onChange={(e) => setReplace(e.target.value)} />
                <button onClick={applyReplace} disabled={!find}>Replace</button>
                <button onClick={saveCorrections} disabled={!dirty}>Save corrections</button>
              </div>

              <div className="row" style={{ marginTop: 12 }}>
                <label>Caption style{" "}
                  <select value={capStyle} onChange={(e) => setCapStyle(e.target.value)}>
                    <option value="bold_yellow">Bold yellow keyword</option>
                    <option value="white_outline">White with black outline</option>
                    <option value="karaoke">Karaoke highlight</option>
                    <option value="minimal">Minimal (podcast)</option>
                    <option value="center_large">Large centre-screen</option>
                    <option value="gaming">Gaming</option>
                  </select>
                </label>
                <label className="check"><input type="checkbox" checked={rangeOnly} onChange={(e) => setRangeOnly(e.target.checked)} /> Only the Start to End range</label>
              </div>
              <div className="row" style={{ marginTop: 8 }}>
                <button onClick={() => exportCaptionFile("srt")} disabled={dirty}>Export SRT</button>
                <button onClick={() => exportCaptionFile("vtt")} disabled={dirty}>Export VTT</button>
                <button onClick={() => exportCaptionFile("ass")} disabled={dirty}>Export ASS</button>
                <button className="primary" disabled={!valid || !allowed || !!busy || !v || dirty} onClick={captionClip}>Export clip with captions (MP4)</button>
                {busy?.kind === "caption" && <button onClick={() => api.cancelJob(busy.id)}>Cancel</button>}
              </div>
              {busy?.kind === "caption" && <progress value={busy.progress} max={1} style={{ width: "100%", marginTop: 8 }} />}
              <p className="muted">The clip uses the Start and End above and needs the permission box ticked. Burned-in captions use the font Arial: Urdu letters work, Hindi (Devanagari) letters may show as boxes until a font option is added.</p>
            </>
          )}
          {note && <p><b>{note}</b></p>}
        </div>
      )}

      <div className="card">
        <h3>Not built yet in this studio</h3>
        <ul>
          <li>Highlight detection, scoring and multi-clip suggestions</li>
          <li>Vertical reframing with face tracking and silence removal</li>
        </ul>
      </div>
    </>
  );
}
