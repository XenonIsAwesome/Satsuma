/** Extracts the file name from a path, handling both `/` and `\` separators
 * since dropped paths can come from either Windows or Linux. */
export function basename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}

/** Returns the lowercased extension (without the dot), or `null` if the file
 * has none (including dotfiles like `.gitignore`, which have no extension in
 * this sense). Mirrors `satsuma-core`'s `extension_of`. */
export function extensionOf(path: string): string | null {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return null;
  return name.slice(dot + 1).toLowerCase();
}
