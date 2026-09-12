import type { Asset } from "$lib/types";

/** Normalize workspace-relative paths for cross-platform manifest matching. */
export function normalizeManifestPath(path: string): string {
  return path.replace(/\\/g, "/").replace(/^\.\//, "");
}

export function manifestPathsMatch(left: string, right: string): boolean {
  return normalizeManifestPath(left) === normalizeManifestPath(right);
}

/** Resolve an asset by manifest path, ignoring slash style differences. */
export function findAssetByManifestPath(assets: Asset[], path: string): Asset | undefined {
  const normalized = normalizeManifestPath(path);
  return assets.find(asset => normalizeManifestPath(asset.relativePath) === normalized);
}
