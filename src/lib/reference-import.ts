import { api } from "$lib/api";
import type { ReferenceCategory, ReferenceImage } from "$lib/types";

/** Maximum pasted reference size accepted by the composer. */
export const MAX_PASTED_REFERENCE_BYTES = 25 * 1024 * 1024;

/** Reference category implied by the active studio tab. */
export function composerReferenceCategory(activeTab: string): ReferenceCategory {
  return activeTab === "vfx" ? "vfx" : "other";
}

/** Remaining reference slots for the current provider. */
export function remainingReferenceSlots(maximum: number, activeCount: number): number {
  return maximum > 0 ? Math.max(0, maximum - activeCount) : 0;
}

/** Merge newly imported references into the library and the active chat list. */
export function mergeImportedReferences(
  references: ReferenceImage[],
  activeIds: string[],
  created: ReferenceImage[],
): { references: ReferenceImage[]; activeReferenceIds: string[] } {
  return {
    references: [...created, ...references],
    activeReferenceIds: [...activeIds, ...created.map(reference => reference.id)],
  };
}

/** Notice shown after references are attached to the open chat. */
export function attachedReferenceNotice(count: number): string {
  return `Attached ${count} reference image${count === 1 ? "" : "s"} to this chat`;
}

/** Notice when the user offered more files than the provider can accept. */
export function referenceOverflowNotice(attached: number, maximum: number | undefined): string {
  return `Attached ${attached}; this provider allows ${maximum} references`;
}

/** Error when a pasted file exceeds the composer size limit. */
export function pastedFileTooLarge(file: Pick<File, "name" | "size">, maxBytes = MAX_PASTED_REFERENCE_BYTES): string | undefined {
  if (file.size > maxBytes) return `${file.name || "Pasted image"} is larger than 25 MB`;
}

/** File name used when importing pasted bytes. */
export function pastedReferenceFileName(file: Pick<File, "name">, now: number): string {
  return file.name || `pasted-reference-${now}.png`;
}

/** Import disk paths into the worktree, truncated to the remaining slot count. */
export async function importReferencePaths(
  worktreeId: string,
  paths: string[],
  slots: number,
  category: ReferenceCategory,
): Promise<ReferenceImage[]> {
  const created: ReferenceImage[] = [];
  for (const path of paths.slice(0, slots)) {
    created.push(await api.importReferenceImage(worktreeId, path, category));
  }
  return created;
}

/** Import pasted files into the worktree, truncated to the remaining slot count. */
export async function importReferenceFiles(
  worktreeId: string,
  files: File[],
  slots: number,
  category: ReferenceCategory,
): Promise<ReferenceImage[]> {
  const created: ReferenceImage[] = [];
  for (const file of files.slice(0, slots)) {
    const oversized = pastedFileTooLarge(file);
    if (oversized) throw new Error(oversized);
    const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
    created.push(await api.importReferenceBytes(
      worktreeId,
      pastedReferenceFileName(file, Date.now()),
      bytes,
      category,
    ));
  }
  return created;
}

/** Mark imported references as active on the current conversation. */
export async function persistConversationReferences(conversationId: string, created: ReferenceImage[]): Promise<void> {
  for (const reference of created) await api.setConversationReference(conversationId, reference.id, true);
}
