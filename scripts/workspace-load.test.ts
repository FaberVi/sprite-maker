import { describe, expect, test } from "bun:test";
import {
  activeWorktreeId, chatsForWorktree, filterReferenceIds, mergeWorkspaceList, selectSavedConversation, selectSavedWorktree,
} from "../src/lib/workspace-load";
import type { Conversation, ReferenceImage, SidebarSnapshot, Workspace, Worktree } from "../src/lib/types";

const worktree = (id: string, kind: Worktree["kind"] = "character"): Worktree => ({
  id, projectId: "p", name: id, slug: id, kind, createdAt: "now", updatedAt: "now",
});
const conversation = (id: string, worktreeId?: string): Conversation => ({
  id, workspaceId: "ws", worktreeId, title: id, provider: "codex", createdAt: "now", updatedAt: "now",
});
const workspace = (id: string): Workspace => ({ id, name: id, path: `/${id}`, createdAt: "now", lastOpenedAt: "now" });

describe("workspace load", () => {
  test("general worktrees see every chat; others filter by worktree id", () => {
    const chats = [conversation("a", "hero"), conversation("b", "cave")];
    expect(chatsForWorktree(chats, worktree("general", "general")).map(item => item.id)).toEqual(["a", "b"]);
    expect(chatsForWorktree(chats, worktree("hero")).map(item => item.id)).toEqual(["a"]);
  });

  test("general worktrees query animations and rigs without a worktree id", () => {
    expect(activeWorktreeId(worktree("general", "general"))).toBeUndefined();
    expect(activeWorktreeId(worktree("hero"))).toBe("hero");
  });

  test("keeps the opened project first when a snapshot was preloaded", () => {
    const active = workspace("open");
    const snapshot: SidebarSnapshot = { workspaces: [workspace("other"), active], worktrees: [], conversations: [] };
    expect(mergeWorkspaceList(snapshot, active, snapshot).map(item => item.id)).toEqual(["open", "other"]);
    expect(selectSavedWorktree([worktree("a"), worktree("b")], "b")?.id).toBe("b");
  });

  test("restores the saved conversation or falls back to the first one", () => {
    const chats = [conversation("a", "hero"), conversation("b", "hero")];
    expect(selectSavedConversation(chats, "b")?.id).toBe("b");
    expect(selectSavedConversation(chats, "missing")?.id).toBe("a");
    expect(selectSavedConversation([], "a")).toBeUndefined();
  });

  test("drops conversation reference ids that are no longer in the worktree", () => {
    const refs: ReferenceImage[] = [{
      id: "keep", projectId: "p", worktreeId: "w", name: "keep", path: "/keep.png", relativePath: "keep.png",
      category: "other", format: "png", width: 1, height: 32, fileSize: 1, contentHash: "h", createdAt: "now", updatedAt: "now",
    }];
    expect(filterReferenceIds(["keep", "gone"], refs)).toEqual(["keep"]);
  });
});
