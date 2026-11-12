import { useEffect, useState } from "react";
import { api, errText, type ProviderState } from "../api";

interface Meta { label: string; kind: string; docs: string; adapter: "planned" | "next"; note: string }

// Pricing is deliberately NOT hardcoded here. See docs/PROVIDERS.md for verified sources and dates.
const META: Record<string, Meta> = {
  anthropic: { label: "Anthropic (Claude)", kind: "Language model", docs: "https://docs.claude.com", adapter: "planned", note: "Script writing, highlight scoring." },
  openai: { label: "OpenAI", kind: "Language, image, transcription", docs: "https://developers.openai.com/api/docs", adapter: "planned", note: "Images and cloud transcription." },
  gemini: { label: "Google Gemini", kind: "Language, image", docs: "https://ai.google.dev/gemini-api/docs", adapter: "planned", note: "Language and image models." },
  veo: { label: "Google Veo", kind: "Video", docs: "https://ai.google.dev/gemini-api/docs", adapter: "planned", note: "Video through the Gemini API." },
  runway: { label: "Runway", kind: "Video, image", docs: "https://docs.dev.runwayml.com", adapter: "next", note: "Official API with published per-second credit pricing." },
  kling: { label: "Kling", kind: "Video", docs: "https://app.klingai.com", adapter: "planned", note: "Official dev platform exists; pricing not yet verified from the official page." },
  seedance: { label: "Seedance (BytePlus)", kind: "Video", docs: "https://docs.byteplus.com/docs/ModelArk", adapter: "planned", note: "Via BytePlus ModelArk; regions and pricing not yet verified." },
  recraft: { label: "Recraft", kind: "Image", docs: "https://www.recraft.ai", adapter: "planned", note: "Not yet researched." },
  elevenlabs: { label: "ElevenLabs", kind: "Voice", docs: "https://elevenlabs.io/docs", adapter: "planned", note: "Check Urdu/Hindi model support and commercial terms for your plan." },
  google_tts: { label: "Google Cloud TTS", kind: "Voice", docs: "https://cloud.google.com/text-to-speech/docs", adapter: "planned", note: "Not yet researched." },
};

export default function Providers() {
  const [list, setList] = useState<ProviderState[]>([]);
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");

  const load = () => api.listProviderStates().then(setList).catch((e) => setErr(errText(e)));
  useEffect(() => { load(); }, []);
  const run = (p: Promise<unknown>, ok = "") => p.then(() => { setErr(""); setMsg(ok); return load(); }).catch((e) => { setMsg(""); setErr(errText(e)); });

  return (
    <>
      {err && <div className="error">{err}</div>}
      {msg && <div className="okbox">{msg}</div>}
      <div className="card">
        <p>API keys are stored in the Windows Credential Manager, not in the app database or project files. No provider calls are made yet: adapters and connection tests are planned (see Next step on each row).</p>
      </div>
      {list.map((p) => {
        const m = META[p.id];
        return (
          <div className="card" key={p.id}>
            <div className="row between">
              <b>{m.label}</b>
              <span className="muted">{m.kind}</span>
            </div>
            <p className="muted">{m.note} Adapter: {m.adapter === "next" ? "next to be built" : "planned"}. <a href={m.docs} target="_blank" rel="noreferrer">Docs</a></p>
            <div className="row">
              <input type="password" placeholder={p.has_key ? "Key saved (enter a new one to replace)" : "API key"} value={keys[p.id] ?? ""} onChange={(e) => setKeys({ ...keys, [p.id]: e.target.value })} />
              <button className="primary" onClick={() => run(api.saveApiKey(p.id, keys[p.id] ?? "").then(() => setKeys({ ...keys, [p.id]: "" })), "Key saved securely.")}>Save key</button>
              {p.has_key && <button className="danger" onClick={() => run(api.removeApiKey(p.id), "Key removed.")}>Remove key</button>}
              <label className="check"><input type="checkbox" checked={p.enabled} onChange={(e) => run(api.setProviderEnabled(p.id, e.target.checked))} /> Enabled</label>
            </div>
            <div className="row" style={{ marginTop: 8 }}>
              <input placeholder="Model name (as listed in the provider docs)" defaultValue={p.model ?? ""} onBlur={(e) => e.target.value !== (p.model ?? "") && run(api.setProviderModel(p.id, e.target.value))} />
              <button disabled title="Not built yet">Test connection (planned)</button>
            </div>
          </div>
        );
      })}
    </>
  );
}
