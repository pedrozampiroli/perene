// Wrappers tipados sobre os comandos Tauri.

import { invoke } from "@tauri-apps/api/core";
import type { Manifest, SessionRecord, Settings, UsageStats } from "./types";

export const api = {
  manifestLoad: () => invoke<Manifest>("manifest_load"),
  manifestSave: (manifest: Manifest) => invoke<void>("manifest_save", { manifest }),
  settingsLoad: () => invoke<Settings>("settings_load"),
  settingsSave: (settings: Settings) => invoke<void>("settings_save", { settings }),
  homeDir: () => invoke<string>("home_dir"),
  savePasteImage: (dataB64: string) => invoke<string>("save_paste_image", { dataB64 }),
  sessionHistoryLoad: () => invoke<SessionRecord[]>("session_history_load"),
  sessionTranscript: (record: SessionRecord) =>
    invoke<string>("session_transcript", { record }),
  usageLoad: () => invoke<UsageStats[]>("usage_load"),
};
