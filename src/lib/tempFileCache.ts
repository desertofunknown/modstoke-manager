import * as fs from "@tauri-apps/plugin-fs"

// Downloads land in the app's downloads folder. This remembers them so they can be removed once installed.
export class TempFileCache {
  private static _cache: string[] = [];

  public static track(filePath: string): void {
    this._cache.push(filePath);
  }

  // Removes every file tracked since the last clear.
  public static async clearCache(): Promise<void> {
    for (const filePath of this._cache) {
      try {
        if(await fs.exists(filePath))
        {
          await fs.remove(filePath);
        }
      } catch (error) {
        console.error(`Error deleting temp file ${filePath}:`, error);
      }
    }
    this._cache = []; // Empty the cache array
  }
}
