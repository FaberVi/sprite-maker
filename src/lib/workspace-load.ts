import { api } from "$lib/api";
import { normalizeGenerationProfile } from "$lib/generation-profiles";
import { parseCustomArts, parseCustomSkills, type CustomArtStyle, type CustomSkill } from "$lib/library-types";
import { parseConversationStyle, parseStylePreset, type ConversationStyleId, type StylePresetId } from "$lib/style-presets";
import { parseAnimationPolishMode } from "$lib/animation-polish-modes";
import type {
  Animation,
  AnimationPolishMode,
  AnimationTemplate,
  Asset,
  AssetPack,
  ChatGenerationProfile,
  Conversation,
  Message,
  ProviderMode,
  ProviderStatus,
  ReferenceImage,
  Rig,
  SidebarSnapshot,
  Workspace,
  Worktree,
} from "$lib/types";

/** Chat list for the selected worktree: general sees every chat, others filter by id. */
export function chatsForWorktree(all: Conversation[], worktree?: Worktree): Conversation[] {
  return !worktree || worktree.kind === "general" ? all : all.filter(item => item.worktreeId === worktree.id);
}

/** Worktree id sent to animation/rig queries: general sections are workspace-wide. */
export function activeWorktreeId(worktree?: Worktree): string | undefined {
  return worktree?.kind === "general" ? undefined : worktree?.id;
}

/** Keep the just-opened project first when a snapshot was already loaded. */
export function mergeWorkspaceList(
  preloaded: SidebarSnapshot | undefined,
  activeWorkspace: Workspace | undefined,
  snapshot: SidebarSnapshot,
): Workspace[] {
  return preloaded && activeWorkspace
    ? [activeWorkspace, ...snapshot.workspaces.filter(item => item.id !== activeWorkspace.id)]
    : snapshot.workspaces;
}

/** Restore the last selected worktree, or the first one. */
export function selectSavedWorktree(worktrees: Worktree[], savedId: unknown): Worktree | undefined {
  return worktrees.find(item => item.id === savedId) ?? worktrees[0];
}

/** Restore the last active conversation for a worktree, or the first one. */
export function selectSavedConversation(conversations: Conversation[], savedId: unknown): Conversation | undefined {
  if (!conversations.length) return undefined;
  return conversations.find(item => item.id === savedId) ?? conversations[0];
}

/** Drop conversation reference ids that are no longer in this worktree. */
export function filterReferenceIds(ids: string[], references: ReferenceImage[]): string[] {
  return ids.filter(id => references.some(reference => reference.id === id));
}

export type InitialShell = {
  activeId?: string;
  customSkills: CustomSkill[];
  customArts: CustomArtStyle[];
  defaultProvider: string;
  theme: string;
  snapshot: SidebarSnapshot;
  recent?: Workspace;
};

/** Settings plus the first sidebar snapshot needed to paint the shell. */
export async function fetchInitialShell(): Promise<InitialShell> {
  const [saved, nextSkills, nextArts, nextDefaultProvider, nextTheme] = await Promise.all([
    api.getSetting("activeWorkspaceId"),
    api.getSetting("custom-skills").then(parseCustomSkills),
    api.getSetting("custom-arts").then(parseCustomArts),
    api.getSetting("default-agent-provider").then(value => String(value ?? "codex")),
    api.getSetting("theme").then(value => String(value ?? "system")),
  ]);
  const activeId = typeof saved === "string" ? saved : undefined;
  const snapshot = await api.loadSidebarState(activeId);
  return {
    activeId,
    customSkills: nextSkills,
    customArts: nextArts,
    defaultProvider: nextDefaultProvider,
    theme: nextTheme,
    snapshot,
    recent: activeId ? snapshot.workspaces.find(item => item.id === activeId) : undefined,
  };
}

export type WorkspaceLoadResult = {
  workspace: Workspace;
  workspaces: Workspace[];
  worktrees: Worktree[];
  sidebarConversations: Conversation[];
  selectedWorktree?: Worktree;
  conversations: Conversation[];
  assets: Asset[];
  packs: AssetPack[];
  worktreeAssetIds: string[];
  animations: Animation[];
  rigs: Rig[];
  animationTemplates: AnimationTemplate[];
  workspaceStyle: StylePresetId;
  references: ReferenceImage[];
  selectedConversation?: Conversation;
  messages: Message[];
  conversationStyle: ConversationStyleId;
  conversationAnimationMode: AnimationPolishMode;
  generationProfile: ChatGenerationProfile;
  activeReferenceIds: string[];
  focusedReferenceId?: string;
};

/** Open a project and load its worktree, chats, and first conversation session. */
export async function fetchWorkspaceLoad(
  selected: Workspace,
  preloaded: SidebarSnapshot | undefined,
  customArts: CustomArtStyle[],
  providers: ProviderStatus[],
  shouldAbort: () => boolean,
  onStage: (stage: string) => void,
): Promise<WorkspaceLoadResult | undefined> {
  onStage("opening project");
  const openedWorkspace = await api.touchWorkspace(selected.id);
  if (shouldAbort()) return;
  onStage("saving the active project");
  await api.setSetting("activeWorkspaceId", openedWorkspace.id);
  onStage("loading project assets and tools");
  const [scan, snapshot] = await Promise.all([
    Promise.all([api.listAssets(openedWorkspace.id), api.listAssetPacks(openedWorkspace.id)]),
    Promise.resolve(preloaded ?? api.loadSidebarState(openedWorkspace.id)),
  ]);
  if (shouldAbort()) return;
  const [assets, packs] = scan;
  const workspaces = mergeWorkspaceList(preloaded, openedWorkspace, snapshot);
  onStage("restoring the active worktree");
  const savedWorktree = await api.getSetting(`active-worktree:${openedWorkspace.id}`);
  const selectedWorktree = selectSavedWorktree(snapshot.worktrees, savedWorktree);
  onStage("loading chats and animations");
  const conversations = chatsForWorktree(snapshot.conversations, selectedWorktree);
  const collections = await fetchWorkspaceCollections(openedWorkspace.id, selectedWorktree, customArts);
  if (shouldAbort()) return;
  const savedConversationId = selectedWorktree
    ? await api.getSetting(`active-conversation:${selectedWorktree.id}`)
    : undefined;
  const selectedConversation = selectSavedConversation(conversations, savedConversationId);
  let messages: Message[] = [];
  let conversationStyle: ConversationStyleId = "inherit";
  let conversationAnimationMode: AnimationPolishMode = "rig";
  let generationProfile = normalizeGenerationProfile(null);
  let activeReferenceIds: string[] = [];
  let focusedReferenceId: string | undefined;
  if (selectedConversation) {
    const session = await fetchConversationSession(
      selectedConversation,
      customArts,
      providers.find(provider => provider.id === selectedConversation.provider),
    );
    if (shouldAbort()) return;
    messages = session.messages;
    conversationStyle = session.style;
    conversationAnimationMode = session.animationMode;
    generationProfile = session.profile;
    activeReferenceIds = filterReferenceIds(session.referenceIds, collections.references);
    const focus = await restoreConversationFocus(selectedConversation, collections.references, activeReferenceIds);
    focusedReferenceId = focus.focusedReferenceId;
    activeReferenceIds = focus.activeReferenceIds;
  }
  return {
    workspace: openedWorkspace,
    workspaces,
    worktrees: snapshot.worktrees,
    sidebarConversations: snapshot.conversations,
    selectedWorktree,
    conversations,
    assets,
    packs,
    ...collections,
    selectedConversation,
    messages,
    conversationStyle,
    conversationAnimationMode,
    generationProfile,
    activeReferenceIds,
    focusedReferenceId,
  };
}

/** Worktree-scoped ids plus workspace animations, rigs, templates, style, and references. */
export async function fetchWorkspaceCollections(
  workspaceId: string,
  selectedWorktree: Worktree | undefined,
  customArts: CustomArtStyle[],
) {
  const worktreeId = activeWorktreeId(selectedWorktree);
  const [worktreeAssetIds, animations, rigs, animationTemplates, workspaceStyle, references] = await Promise.all([
    selectedWorktree ? api.listWorktreeAssetIds(selectedWorktree.id) : Promise.resolve([] as string[]),
    api.listAnimations(workspaceId, worktreeId),
    api.listRigs(workspaceId, worktreeId),
    api.listAnimationTemplates(workspaceId),
    api.getSetting(`workspace-style:${workspaceId}`).then(value => parseStylePreset(value, customArts)),
    selectedWorktree ? api.listReferenceImages(selectedWorktree.id) : Promise.resolve([] as ReferenceImage[]),
  ]);
  return { worktreeAssetIds, animations, rigs, animationTemplates, workspaceStyle, references };
}

/** Switch the open worktree: assets, animations, rigs, and references for that id. */
export async function fetchWorktreeStudio(workspaceId: string, worktree: Worktree) {
  const worktreeId = activeWorktreeId(worktree);
  const [worktreeAssetIds, animations, rigs, references] = await Promise.all([
    api.listWorktreeAssetIds(worktree.id),
    api.listAnimations(workspaceId, worktreeId),
    api.listRigs(workspaceId, worktreeId),
    api.listReferenceImages(worktree.id),
  ]);
  return { worktreeAssetIds, animations, rigs, references };
}

/** Messages, style, generation profile, and reference ids for one chat. */
export async function fetchConversationSession(
  conversation: Conversation,
  customArts: CustomArtStyle[],
  provider?: ProviderStatus,
) {
  const [messages, style, animationMode, profile, referenceIds] = await Promise.all([
    api.listMessages(conversation.id),
    api.getSetting(`conversation-style:${conversation.id}`).then(value => parseConversationStyle(value, customArts)),
    api.getSetting(`conversation-animation-mode:${conversation.id}`).then(parseAnimationPolishMode),
    loadGenerationProfile(conversation, provider?.modes ?? []),
    api.listConversationReferenceIds(conversation.id),
  ]);
  return { messages, style, animationMode, profile, referenceIds };
}

/** Saved generation profile for a conversation, normalized to the provider's modes. */
export async function loadGenerationProfile(conversation: Conversation, modes: ProviderMode[]): Promise<ChatGenerationProfile> {
  return normalizeGenerationProfile(
    await api.getSetting(`conversation-generation:${conversation.id}`),
    modes,
    conversation.provider,
  );
}

/** Restore focused-reference order for a chat, persisting a valid saved focus. */
export async function restoreConversationFocus(
  conversation: Conversation | undefined,
  worktreeReferences: ReferenceImage[],
  activeIds: string[],
): Promise<{ focusedReferenceId?: string; activeReferenceIds: string[] }> {
  if (!conversation) return { focusedReferenceId: undefined, activeReferenceIds: activeIds };
  const saved = await api.getSetting(`conversation-focus:${conversation.id}`);
  const candidate = typeof saved === "string" ? saved : undefined;
  const focusId = candidate && activeIds.includes(candidate) && worktreeReferences.some(reference => reference.id === candidate)
    ? candidate
    : undefined;
  return {
    focusedReferenceId: focusId,
    activeReferenceIds: focusId ? [focusId, ...activeIds.filter(id => id !== focusId)] : activeIds,
  };
}

/** Rescan assets and optionally link newly rendered files to the open worktree. */
export async function refreshWorktreeMedia(
  workspaceId: string,
  worktreeQueryId: string | undefined,
  selectedWorktreeId?: string,
  linkAssetIds?: string[],
) {
  if (selectedWorktreeId && linkAssetIds?.length) {
    await Promise.all(linkAssetIds.map(id => api.linkAssetToWorktree(selectedWorktreeId, id)));
  }
  const assets = await api.scanAssets(workspaceId);
  const worktreeAssetIds = selectedWorktreeId ? await api.listWorktreeAssetIds(selectedWorktreeId) : [];
  const animations = await api.listAnimations(workspaceId, worktreeQueryId);
  return { assets, worktreeAssetIds, animations };
}
