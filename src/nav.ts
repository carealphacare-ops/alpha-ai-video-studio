export type Status = "working" | "partial" | "planned";

export interface NavItem {
  id: string;
  label: string;
  status: Status;
  /** Honest description shown on planned/partial pages. */
  summary: string;
  needs?: string[];
  next?: string;
}

export const NAV: NavItem[] = [
  { id: "dashboard", label: "Dashboard", status: "working", summary: "Projects, jobs, exports, usage and tool status from your local database." },
  { id: "clipping", label: "AI Clipping Studio", status: "partial", summary: "Import a local file, preview and trim it, transcribe it locally with Whisper, correct the text, export SRT/VTT/ASS, and export a clip with animated word-highlight captions. Highlight detection and vertical reframing are not built yet.",
    needs: ["FFmpeg (bundled in the installer)"], next: "Highlight detection (needs a language-model key), then vertical reframing." },
  { id: "story", label: "AI Story Studio", status: "planned", summary: "Idea/script to scenes to video. Not built yet.",
    needs: ["A language-model API key", "An image or video provider key (paid credits)", "A voice provider key or local TTS"], next: "Phase 4: provider adapters, then script-to-scenes." },
  { id: "videogen", label: "AI Video Generator", status: "planned", summary: "Direct text/image-to-video via provider APIs. Not built yet.", needs: ["Provider API key", "Paid credits"], next: "Phase 7: Runway adapter first (pricing verified, see docs/PROVIDERS.md)." },
  { id: "imagegen", label: "AI Image Generator", status: "planned", summary: "Image generation via provider APIs. Not built yet.", needs: ["Provider API key", "Paid credits"], next: "Phase 4: OpenAI image adapter." },
  { id: "characters", label: "Character Studio", status: "planned", summary: "Reusable characters with reference images and voices. Not built yet.", next: "Phase 4." },
  { id: "voice", label: "Voice Studio", status: "planned", summary: "AI narration and dialogue. Not built yet.", needs: ["Voice provider key, or a local TTS model"], next: "Phase 4." },
  { id: "subtitles", label: "Subtitle Studio", status: "planned", summary: "Word-level animated captions, SRT/VTT/ASS export. Not built yet.", needs: ["Transcription (local faster-whisper or cloud)"], next: "Phase 3." },
  { id: "editor", label: "Video Editor", status: "planned", summary: "Multi-track timeline editor. Not built yet.", next: "Phase 5." },
  { id: "batch", label: "Batch Production", status: "partial", summary: "Job list with real status and progress is live (see Dashboard). Batch creation, pause and retry are not built yet.", next: "Phase 6." },
  { id: "media", label: "Media Library", status: "planned", summary: "Searchable library with tags and folders. Database table exists; UI not built yet.", next: "Phase 2." },
  { id: "projects", label: "Project Library", status: "working", summary: "Create, rename, duplicate and delete projects." },
  { id: "social", label: "Social Media Manager", status: "planned", summary: "Platform export presets and metadata. Not built yet. Direct publishing needs official OAuth APIs and per-platform app review.", next: "Phase 8." },
  { id: "assistant", label: "AI Assistant", status: "planned", summary: "Natural-language control of the app. Not built yet.", needs: ["Language-model API key"], next: "Phase 7+." },
  { id: "providers", label: "AI Provider Manager", status: "partial", summary: "Store API keys securely, enable/disable providers, pick a model. Connection tests are not built yet.", next: "Phase 7: per-provider test calls." },
  { id: "settings", label: "Settings", status: "working", summary: "Storage folders, FFmpeg location and spending limits." },
];
