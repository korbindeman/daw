/**
 * Store for remembering the last-used directories for different file dialogs.
 * Each dialog type automatically maintains its own separate directory history.
 */

const STORAGE_KEY = "daw-dialog-paths";

type DialogPaths = Record<string, string>;

class DialogPathStore {
  private paths = $state<DialogPaths>(this.loadFromStorage());

  private loadFromStorage(): DialogPaths {
    if (typeof window === "undefined") return {};
    const stored = localStorage.getItem(STORAGE_KEY);
    return stored ? JSON.parse(stored) : {};
  }

  private saveToStorage() {
    if (typeof window === "undefined") return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(this.paths));
  }

  /**
   * Get the last-used directory for a specific dialog type.
   */
  getPath(dialogType: string): string | undefined {
    return this.paths[dialogType];
  }

  /**
   * Save the directory path for a specific dialog type.
   * Automatically extracts the directory from the full file path.
   */
  setPath(dialogType: string, filePath: string) {
    const lastSlash = filePath.lastIndexOf("/");
    if (lastSlash !== -1) {
      this.paths[dialogType] = filePath.substring(0, lastSlash);
      this.saveToStorage();
    }
  }

  /**
   * Build a default path by combining the last-used directory with a filename.
   */
  buildPath(dialogType: string, fileName?: string): string | undefined {
    const dir = this.paths[dialogType];
    if (!dir) return fileName;
    if (!fileName) return dir;
    return `${dir}/${fileName}`;
  }
}

export const dialogPathStore = new DialogPathStore();
