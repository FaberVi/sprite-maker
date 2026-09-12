import { describe, expect, test } from "bun:test";
import {
  addBone, addFrame, addPoint, clampCanvasPoint, currentInput, nextPreviewTick, removeBone, removePoint,
  setTransform, updateBone, updateFrame,
} from "../src/lib/rig-draft";
import type { RigPoint } from "../src/lib/types";

const createId = (prefix: string) => `${prefix}_1`;

describe("rig draft mutators", () => {
  test("adds a uniquely named point and maps the draft onto RigInput", () => {
    const first = addPoint([], 1.26, 2.24, createId);
    expect(first.point.name).toBe("point_1");
    expect(first.point.x).toBe(1.3);
    const second = addPoint(first.points, 4, 5, () => "p_2");
    expect(second.points).toHaveLength(2);
    expect(currentInput({
      id: "rig", workspaceId: "ws", name: "Hero", morphology: "biped", fps: 8, looping: true,
      points: second.points, bones: [], frames: [],
    }).assetId).toBeUndefined();
  });

  test("refuses a bone until two points exist, then removes bones that used a deleted point", () => {
    expect(addBone([], [], createId)).toBeUndefined();
    const points = addPoint(addPoint([], 0, 0, () => "p1").points, 8, 0, () => "p2").points;
    const bones = addBone(points, [], createId);
    expect(bones?.bone.startPoint).toBe("point_1");
    const removed = removePoint(points, bones!.bones, "p1");
    expect(removed.points).toHaveLength(1);
    expect(removed.bones).toHaveLength(0);
  });

  test("clears an empty parent, writes bone transforms, and advances preview playback", () => {
    const points: RigPoint[] = [
      { id: "p1", name: "hip", kind: "joint", x: 0, y: 0, confidence: 1, source: "user" },
      { id: "p2", name: "knee", kind: "joint", x: 0, y: 8, confidence: 1, source: "user" },
    ];
    const bones = addBone(points, [], createId)!.bones;
    const cleared = updateBone(bones, bones[0].id, { parent: "" });
    expect(cleared[0].parent).toBeUndefined();
    let frames = addFrame([]);
    frames = setTransform(frames, 0, bones[0].name, { rotate: 12, dx: 1 });
    expect(frames[0].transforms[0]).toMatchObject({ bone: bones[0].name, rotate: 12, dx: 1, scaleX: 1 });
    frames = updateFrame(frames, 0, { hold: true });
    expect(frames[0].hold).toBe(true);
    const leftover = removeBone(cleared, frames, bones[0].id);
    expect(leftover.frames[0].transforms).toEqual([]);
    expect(nextPreviewTick(0, 3, true)).toEqual({ previewIndex: 1, playing: true });
    expect(nextPreviewTick(2, 3, false)).toEqual({ previewIndex: 0, playing: false });
    expect(clampCanvasPoint(-4, 99, 16, 16)).toEqual({ x: 0, y: 15 });
  });
});
