/**
 * Transport store - manages playback controls.
 * 
 * This store provides methods to control playback (play, pause, stop, seek)
 * and automatically updates the session state.
 */

import { invoke } from "@tauri-apps/api/core";
import { sessionStore, type SessionSnapshot } from "./session.svelte";

class TransportStore {
  private _error = $state<string | null>(null);

  get error() {
    return this._error;
  }

  /**
   * Start playback.
   */
  async play(): Promise<void> {
    try {
      const snapshot = await invoke<SessionSnapshot>("transport_play");
      // Update session store with new state
      this.updateSession(snapshot);
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Pause playback.
   */
  async pause(): Promise<void> {
    try {
      const snapshot = await invoke<SessionSnapshot>("transport_pause");
      this.updateSession(snapshot);
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Stop playback.
   */
  async stop(): Promise<void> {
    try {
      const snapshot = await invoke<SessionSnapshot>("transport_stop");
      this.updateSession(snapshot);
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Seek to a specific tick position.
   */
  async seekToTick(tick: number): Promise<void> {
    try {
      const snapshot = await invoke<SessionSnapshot>("transport_seek_to_tick", { tick });
      this.updateSession(snapshot);
    } catch (err) {
      this._error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /**
   * Toggle play/pause.
   */
  async togglePlayPause(): Promise<void> {
    if (sessionStore.isPlaying) {
      await this.pause();
    } else {
      await this.play();
    }
  }

  private updateSession(snapshot: SessionSnapshot): void {
    // This is a bit of a hack - we're directly updating the session store's internal state
    // In a real app, we might want a more formal way to do this
    (sessionStore as any)._session = snapshot;
  }
}

export const transportStore = new TransportStore();

