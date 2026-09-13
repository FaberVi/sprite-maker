import { describe, expect, test } from "bun:test";
import { extractAssetPathsFromResponse, findAssetByManifestPath, manifestPathsMatch, normalizeManifestPath } from "../src/lib/manifest-path";
import type { Asset } from "../src/lib/types";

const asset = (relativePath: string): Asset => ({
  id: relativePath,
  name: relativePath,
  workspaceId: "ws",
  path: `/tmp/${relativePath}`,
  relativePath,
  category: "characters",
  format: "png",
  width: 64,
  height: 64,
  fileSize: 100,
  hasAlpha: true,
  createdAt: "now",
});

describe("manifest path normalization", () => {
  test("normalizes slashes and leading ./", () => {
    expect(normalizeManifestPath(".\\assets\\frame-01.png")).toBe("assets/frame-01.png");
    expect(normalizeManifestPath("./assets/frame-01.png")).toBe("assets/frame-01.png");
    expect(manifestPathsMatch("assets/a.png", ".\\assets\\a.png")).toBe(true);
  });

  test("finds assets across path separator styles", () => {
    const library = [asset("assets/characters/hero.png")];
    expect(findAssetByManifestPath(library, "assets\\characters\\hero.png")?.id).toBe("assets/characters/hero.png");
  });

  test("extracts asset output paths from provider prose and markdown links", () => {
    const response = "Saved assets/characters/astral_cartographer.png.\n\n[Preview](assets/characters/astral_cartographer.png)";
    expect(extractAssetPathsFromResponse(response)).toEqual(["assets/characters/astral_cartographer.png"]);
    expect(extractAssetPathsFromResponse("Wrote assets\\characters\\knight.png on Windows.")).toEqual(["assets/characters/knight.png"]);
  });
});
