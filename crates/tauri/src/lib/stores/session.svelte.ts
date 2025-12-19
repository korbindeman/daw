/**
 * Session store - manages the current DAW session state.
 * 
 * This store holds the complete session snapshot including tracks, clips,
 * tempo, time signature, and playback state.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface TimeSignature {
  numerator: number;
  denominator: number;
}

export interface ClipSummary {
  id: number;
  name: string;
  startTick: number;
  endTick: number;
}

export interface TrackSummary {
  id: number;
  name: string;
  enabled: boolean;
  soloed: boolean;
  volume: number;
  pan: number;
  clips: ClipSummary[];
}

export interface MetronomeState {
  enabled: boolean;
  volume: number;
}

export type PlaybackState = "stopped" | "playing" | "paused";

export interface SessionSnapshot {
  name: string;
  tempo: number;
  timeSignature: TimeSignature;
  maxTick: number;
  currentTick: number;
  playbackState: PlaybackState;
  tracks: TrackSummary[];
  metronome: MetronomeState;
}

export interface SessionTickEvent {
  tick: number;
  playbackState: PlaybackState;
}

class SessionStore {
  private _session = $state<SessionSnapshot | null>(null);
  private _loading = $state(false);
  private _error = $state<string | null>(null);

  constructor() {
    // Listen for session tick events from the backend
    listen<SessionTickEvent>("session-tick", (event) => {
      if (this._session) {
        this._session.currentTick = event.payload.tick;
        this._session.playbackState = event.payload.playbackState;
      }
    });
  }

  get session() {
    return this._session;
  }

  get loading() {
    return this._loading;
  }

  get error() {
    return this._error;
  }

  get isPlaying() {
    return this._session?.playbackState === "playing";
  }

  /**
   * Load a project file and create a new session.
   */
  async loadProject(path: string): Promise<void> {
    this._loading = true;
    this._error = null;

    try {
      const snapshot = await invoke<SessionSnapshot>("session_load_project", { path });
      this._session = snapshot;
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    } finally {
      this._loading = false;
    }
  }

  /**
   * Get the current session state.
   */
  async refresh(): Promise<void> {
    try {
      const snapshot = await invoke<SessionSnapshot>("session_get_state");
      this._session = snapshot;
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
    }
  }

  /**
   * Save the current session.
   */
  async save(): Promise<void> {
    try {
      await invoke("session_save");
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Save the current session to a new path.
   */
  async saveAs(path: string): Promise<void> {
    try {
      await invoke("session_save_as", { path });
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Render the current session to a WAV file.
   */
  async render(path: string): Promise<void> {
    try {
      await invoke("session_render", { path });
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Convert ticks to musical time (bars:beats:ticks).
   */
  ticksToMusicalTime(tick: number): string {
    if (!this._session) return "0:0:0";

    const PPQN = 960;
    const { numerator, denominator } = this._session.timeSignature;
    
    // Calculate ticks per bar
    const ticksPerBeat = PPQN * (4 / denominator);
    const ticksPerBar = ticksPerBeat * numerator;
    
    const bar = Math.floor(tick / ticksPerBar) + 1;
    const remainingTicks = tick % ticksPerBar;
    const beat = Math.floor(remainingTicks / ticksPerBeat) + 1;
    const tickInBeat = Math.floor(remainingTicks % ticksPerBeat);
    
    return `${bar}:${beat}:${tickInBeat}`;
  }
}

export const sessionStore = new SessionStore();

