/** Strip Windows extended-length path prefixes for UI display. */
export function displayPath(path: string): string {
  return path.replace(/^\\\\\?\\/, "");
}

/** Last path segment, works with Windows and POSIX separators. */
export function projectNameFromPath(path: string): string {
  const segments = displayPath(path).split(/[/\\]/).filter(Boolean);
  return segments.at(-1) ?? "Project";
}
