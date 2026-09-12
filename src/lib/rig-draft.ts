import type {
  Rig,
  RigBone,
  RigContact,
  RigFrame,
  RigInput,
  RigMorphology,
  RigPoint,
  RigSuggestion,
  RigTransform,
} from "$lib/types";

export type RigDraftSnapshot = {
  id?: string;
  workspaceId: string;
  worktreeId?: string;
  assetId?: string;
  name: string;
  morphology: RigMorphology;
  fps: number;
  looping: boolean;
  points: RigPoint[];
  bones: RigBone[];
  frames: RigFrame[];
};

/** Map a draft snapshot onto the save/render payload. */
export function currentInput(draft: RigDraftSnapshot): RigInput {
  return {
    id: draft.id,
    workspaceId: draft.workspaceId,
    worktreeId: draft.worktreeId,
    assetId: draft.assetId,
    name: draft.name,
    morphology: draft.morphology,
    fps: Number(draft.fps),
    looping: draft.looping,
    points: draft.points,
    bones: draft.bones,
    frames: draft.frames,
  };
}

/** Unique `point_N` name that does not collide with existing points. */
export function uniquePointName(points: RigPoint[], index: number): string {
  let candidate = `point_${index}`;
  let suffix = 1;
  while (points.some(point => point.name === candidate)) candidate = `point_${index}_${++suffix}`;
  return candidate;
}

/** Unique `bone_N` name that does not collide with existing bones. */
export function uniqueBoneName(bones: RigBone[], index: number): string {
  let candidate = `bone_${index}`;
  while (bones.some(bone => bone.name === candidate)) candidate = `bone_${index}_${Math.random().toString(36).slice(2, 5)}`;
  return candidate;
}

/** Append a user-placed joint and return the next point list. */
export function addPoint(
  points: RigPoint[],
  x: number,
  y: number,
  createId: (prefix: string) => string,
): { points: RigPoint[]; point: RigPoint } {
  const point: RigPoint = {
    id: createId("p"),
    name: uniquePointName(points, points.length + 1),
    kind: "joint",
    x: Math.round(x * 10) / 10,
    y: Math.round(y * 10) / 10,
    confidence: 1,
    source: "user",
  };
  return { points: [...points, point], point };
}

/** Patch one point by id. */
export function updatePoint(points: RigPoint[], id: string, patch: Partial<RigPoint>): RigPoint[] {
  return points.map(point => point.id === id ? { ...point, ...patch } : point);
}

/** Remove a point and any bones that referenced it by name. */
export function removePoint(
  points: RigPoint[],
  bones: RigBone[],
  id: string,
): { points: RigPoint[]; bones: RigBone[] } {
  const target = points.find(point => point.id === id);
  return {
    points: points.filter(point => point.id !== id),
    bones: target ? bones.filter(bone => bone.startPoint !== target.name && bone.endPoint !== target.name) : bones,
  };
}

/** Append a bone between the first two points, or nothing if fewer than two points exist. */
export function addBone(
  points: RigPoint[],
  bones: RigBone[],
  createId: (prefix: string) => string,
): { bones: RigBone[]; bone: RigBone } | undefined {
  if (points.length < 2) return;
  const bone: RigBone = {
    id: createId("b"),
    name: uniqueBoneName(bones, bones.length + 1),
    startPoint: points[0].name,
    endPoint: points[1].name,
    radius: 3,
    parent: undefined,
    z: 5,
  };
  return { bones: [...bones, bone], bone };
}

/** Patch one bone by id, treating an empty parent as unset. */
export function updateBone(bones: RigBone[], id: string, patch: Partial<RigBone>): RigBone[] {
  return bones.map(bone => bone.id === id
    ? { ...bone, ...patch, parent: patch.parent === "" ? undefined : (patch.parent ?? bone.parent) }
    : bone
  );
}

/** Remove a bone and any transforms/contacts that referenced it. */
export function removeBone(
  bones: RigBone[],
  frames: RigFrame[],
  id: string,
): { bones: RigBone[]; frames: RigFrame[] } {
  const target = bones.find(bone => bone.id === id);
  return {
    bones: bones.filter(bone => bone.id !== id),
    frames: target
      ? frames.map(frame => ({
        ...frame,
        transforms: frame.transforms.filter(t => t.bone !== target.name),
        contacts: frame.contacts.filter(c => c.bone !== target.name),
      }))
      : frames,
  };
}

/** Append an empty pose frame. */
export function addFrame(frames: RigFrame[]): RigFrame[] {
  return [...frames, { phase: undefined, hold: false, rootDx: 0, rootDy: 0, transforms: [], contacts: [] }];
}

/** Duplicate a pose frame immediately after the given index. */
export function duplicateFrame(frames: RigFrame[], index: number): RigFrame[] {
  const copy = JSON.parse(JSON.stringify(frames[index])) as RigFrame;
  return [...frames.slice(0, index + 1), copy, ...frames.slice(index + 1)];
}

/** Remove a pose frame and clamp the current index onto the remaining list. */
export function removeFrame(
  frames: RigFrame[],
  index: number,
  frameIndex: number,
): { frames: RigFrame[]; frameIndex: number } {
  const next = frames.filter((_, i) => i !== index);
  return { frames: next, frameIndex: Math.max(0, Math.min(frameIndex, next.length - 1)) };
}

/** Patch one pose frame by index. */
export function updateFrame(frames: RigFrame[], index: number, patch: Partial<RigFrame>): RigFrame[] {
  return frames.map((frame, i) => i === index ? { ...frame, ...patch } : frame);
}

/** Transform for a named bone in a pose frame. */
export function transformOf(frame: RigFrame, boneName: string): RigTransform | undefined {
  return frame.transforms.find(t => t.bone === boneName);
}

/** Set or merge a bone transform on a pose frame. */
export function setTransform(
  frames: RigFrame[],
  frameIndexValue: number,
  boneName: string,
  patch: Partial<{ rotate: number; dx: number; dy: number }>,
): RigFrame[] {
  const frame = frames[frameIndexValue];
  if (!frame) return frames;
  const existing = frame.transforms.find(t => t.bone === boneName);
  const next = existing ? { ...existing, ...patch } : { bone: boneName, dx: 0, dy: 0, rotate: 0, scaleX: 1, scaleY: 1, ...patch };
  return frames.map((item, i) => i === frameIndexValue
    ? { ...item, transforms: [...item.transforms.filter(t => t.bone !== boneName), next] }
    : item
  );
}

/** Pin the first bone as a planted contact on a pose frame. */
export function addContact(frames: RigFrame[], frameIndexValue: number, bones: RigBone[]): RigFrame[] {
  const frame = frames[frameIndexValue];
  if (!frame || !bones.length) return frames;
  return updateFrame(frames, frameIndexValue, { contacts: [...frame.contacts, { bone: bones[0].name, x: 0, y: 0, bend: 1 }] });
}

/** Patch one planted contact on a pose frame. */
export function updateContact(
  frames: RigFrame[],
  frameIndexValue: number,
  index: number,
  patch: Partial<Pick<RigContact, "bone" | "x" | "y" | "bend">>,
): RigFrame[] {
  const frame = frames[frameIndexValue];
  if (!frame) return frames;
  return updateFrame(frames, frameIndexValue, {
    contacts: frame.contacts.map((contact, i) => i === index ? { ...contact, ...patch } : contact),
  });
}

/** Remove one planted contact from a pose frame. */
export function removeContact(frames: RigFrame[], frameIndexValue: number, index: number): RigFrame[] {
  const frame = frames[frameIndexValue];
  if (!frame) return frames;
  return updateFrame(frames, frameIndexValue, { contacts: frame.contacts.filter((_, i) => i !== index) });
}

/** Suggestion point that shares a name with a placed point, otherwise the placed point. */
export function ghostPointFor(suggestion: RigSuggestion | undefined, point: RigPoint): RigPoint {
  return suggestion?.points.find(candidate => candidate.name === point.name) ?? point;
}

/** Lookup a named point, or a dummy off-canvas point when the name is missing. */
export function pointByName(points: RigPoint[], name: string, fallbackId: string): RigPoint {
  return points.find(point => point.name === name) ?? {
    x: -10, y: -10, name, id: fallbackId, kind: "joint", confidence: 0, source: "user",
  };
}

/** Clamp a canvas coordinate onto the master sprite. */
export function clampCanvasPoint(x: number, y: number, width: number, height: number): { x: number; y: number } {
  return {
    x: Math.max(0, Math.min(width - 1, Math.round(x * 10) / 10)),
    y: Math.max(0, Math.min(height - 1, Math.round(y * 10) / 10)),
  };
}

/** Move one suggestion ghost point. */
export function moveSuggestionPoint(suggestion: RigSuggestion, id: string, x: number, y: number): RigSuggestion {
  return { ...suggestion, points: suggestion.points.map(point => point.id === id ? { ...point, x, y } : point) };
}

/** Copy a suggestion onto the editable draft. */
export function cloneSuggestionDraft(suggestion: RigSuggestion): Pick<RigDraftSnapshot, "morphology"> & {
  points: RigPoint[];
  bones: RigBone[];
  frames: RigFrame[];
} {
  return {
    morphology: suggestion.morphology,
    points: suggestion.points.map(point => ({ ...point })),
    bones: suggestion.bones.map(bone => ({ ...bone })),
    frames: suggestion.frames.map(frame => ({
      ...frame,
      transforms: frame.transforms.map(t => ({ ...t })),
      contacts: frame.contacts.map(c => ({ ...c })),
    })),
  };
}

/** Copy a saved rig into the editable draft fields. */
export function cloneRigDraft(rig: Rig): {
  loadedRigId: string;
  rigId: string;
  name: string;
  morphology: RigMorphology;
  fps: number;
  looping: boolean;
  masterAssetId?: string;
  points: RigPoint[];
  bones: RigBone[];
  frames: RigFrame[];
} {
  return {
    loadedRigId: rig.id,
    rigId: rig.id,
    name: rig.name,
    morphology: rig.morphology,
    fps: rig.fps,
    looping: rig.looping,
    masterAssetId: rig.assetId,
    points: rig.points.map(point => ({ ...point })),
    bones: rig.bones.map(bone => ({ ...bone })),
    frames: rig.frames.map(frame => ({
      ...frame,
      transforms: frame.transforms.map(t => ({ ...t })),
      contacts: frame.contacts.map(c => ({ ...c })),
    })),
  };
}

/** Advance preview playback by one frame. */
export function nextPreviewTick(
  previewIndex: number,
  previewLength: number,
  looping: boolean,
): { previewIndex: number; playing: boolean } {
  if (previewIndex < previewLength - 1) return { previewIndex: previewIndex + 1, playing: true };
  return looping ? { previewIndex: 0, playing: true } : { previewIndex: 0, playing: false };
}
