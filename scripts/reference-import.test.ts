import { describe, expect, test } from "bun:test";
import {
  attachedReferenceNotice, composerReferenceCategory, mergeImportedReferences, pastedFileTooLarge,
  remainingReferenceSlots, referenceOverflowNotice,
} from "../src/lib/reference-import";
import type { ReferenceImage } from "../src/lib/types";

const reference = (id: string): ReferenceImage => ({
  id, projectId: "p", worktreeId: "w", name: id, path: `/ref/${id}.png`, relativePath: `${id}.png`,
  category: "other", format: "png", width: 32, height: 32, fileSize: 10, contentHash: "hash", createdAt: "now", updatedAt: "now",
});

describe("reference import", () => {
  test("uses vfx category only on the vfx tab", () => {
    expect(composerReferenceCategory("vfx")).toBe("vfx");
    expect(composerReferenceCategory("chat")).toBe("other");
  });

  test("counts remaining provider slots and merges new references first", () => {
    expect(remainingReferenceSlots(3, 2)).toBe(1);
    expect(remainingReferenceSlots(0, 0)).toBe(0);
    const merged = mergeImportedReferences([reference("old")], ["old"], [reference("new")]);
    expect(merged.references.map(item => item.id)).toEqual(["new", "old"]);
    expect(merged.activeReferenceIds).toEqual(["old", "new"]);
  });

  test("rejects oversized pastes and reports overflow", () => {
    expect(pastedFileTooLarge({ name: "big.png", size: 26 * 1024 * 1024 })).toContain("25 MB");
    expect(pastedFileTooLarge({ name: "ok.png", size: 10 })).toBeUndefined();
    expect(attachedReferenceNotice(1)).toBe("Attached 1 reference image to this chat");
    expect(referenceOverflowNotice(2, 4)).toContain("this provider allows 4");
  });
});
