<script lang="ts">
  import { Bot, Check, Crosshair, Pause, Play, Wand2, X } from "lucide-svelte";
  import { assetUrl } from "$lib/api";
  import { clampCanvasPoint, ghostPointFor, moveSuggestionPoint, pointByName } from "$lib/rig-draft";
  import type { Asset, ProviderStatus, RigBone, RigPoint, RigSuggestion } from "$lib/types";

  let {
    masterAsset, bones, points, suggestion, selectedPointId, scale, stageImage,
    playing, previewPaths, previewIndex, previewBusy, rendering, suggesting, aiBusy,
    agentProviders, aiProviderId, aiMotion,
    onAddPoint, onUpdatePoint, onSelectPoint, onSuggestion, onScale, onPreview, onTogglePlay,
    onAutoSuggest, onAiSuggest, onAiProvider, onAiMotion, onApplySuggestion, onDismissSuggestion,
  }: {
    masterAsset?: Asset;
    bones: RigBone[];
    points: RigPoint[];
    suggestion?: RigSuggestion;
    selectedPointId?: string;
    scale: number;
    stageImage?: string;
    playing: boolean;
    previewPaths: string[];
    previewIndex: number;
    previewBusy: boolean;
    rendering: boolean;
    suggesting: boolean;
    aiBusy: boolean;
    agentProviders: ProviderStatus[];
    aiProviderId?: string;
    aiMotion: string;
    onAddPoint: (x: number, y: number) => void;
    onUpdatePoint: (id: string, patch: Partial<RigPoint>) => void;
    onSelectPoint: (id: string) => void;
    onSuggestion: (value: RigSuggestion) => void;
    onScale: (value: number) => void;
    onPreview: () => void;
    onTogglePlay: () => void;
    onAutoSuggest: () => void;
    onAiSuggest: () => void;
    onAiProvider: (id: string) => void;
    onAiMotion: (value: string) => void;
    onApplySuggestion: () => void;
    onDismissSuggestion: () => void;
  } = $props();

  let dragging: { kind: "point" | "ghost"; id: string } | undefined;

  function stageCoordinates(event: PointerEvent) {
    const stage = event.currentTarget as HTMLElement;
    const rect = stage.getBoundingClientRect();
    return { x: (event.clientX - rect.left) / scale, y: (event.clientY - rect.top) / scale };
  }
  function onStagePointerDown(event: PointerEvent) {
    if (event.target !== event.currentTarget) return;
    if (!masterAsset) return;
    const { x, y } = stageCoordinates(event);
    onAddPoint(x, y);
  }
  function beginDrag(kind: "point" | "ghost", id: string, event: PointerEvent) {
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    dragging = { kind, id };
    if (kind === "point") onSelectPoint(id);
  }
  function onStagePointerMove(event: PointerEvent) {
    if (!dragging || !masterAsset) return;
    const { x, y } = clampCanvasPoint(
      (event.clientX - (event.currentTarget as HTMLElement).getBoundingClientRect().left) / scale,
      (event.clientY - (event.currentTarget as HTMLElement).getBoundingClientRect().top) / scale,
      masterAsset.width,
      masterAsset.height,
    );
    if (dragging.kind === "point") onUpdatePoint(dragging.id, { x, y });
    else if (suggestion) onSuggestion(moveSuggestionPoint(suggestion, dragging.id, x, y));
  }
  function endDrag() { dragging = undefined; }
</script>

<div class="workspace">
  <div class="canvas-wrap">
    {#if masterAsset}
      <div class="stage" class:ghosting={Boolean(suggestion)} style={`width:${masterAsset.width * scale}px;height:${masterAsset.height * scale}px`}
        onpointerdown={onStagePointerDown} onpointermove={onStagePointerMove} onpointerup={endDrag} onpointercancel={endDrag} role="presentation">
        {#if stageImage}<img src={assetUrl(stageImage)} alt={masterAsset.name} draggable="false"/>{/if}
        <svg width="100%" height="100%" viewBox={`0 0 ${masterAsset.width} ${masterAsset.height}`} preserveAspectRatio="none">
          {#each bones as bone (bone.id)}
            {@const start = ghostPointFor(suggestion, pointByName(points, bone.startPoint, bone.id))}
            {@const end = ghostPointFor(suggestion, pointByName(points, bone.endPoint, bone.id))}
            <line x1={start.x} y1={start.y} x2={end.x} y2={end.y} class="bone" class:near={bone.z >= 7} class:far={bone.z <= 3} vector-effect="non-scaling-stroke"/>
            <circle cx={start.x} cy={start.y} r={bone.radius} class="capsule"/>
          {/each}
          {#if suggestion}{#each suggestion.points as point (point.id)}
            <circle cx={point.x} cy={point.y} r={Math.max(1.6, 4 / scale)} class="point ghost" class:selected={point.id === selectedPointId} role="button" tabindex="-1"
              onpointerdown={(event) => beginDrag("ghost", point.id, event)}/>
            {#if scale >= 2}<text x={point.x + 5 / scale} y={point.y - 4 / scale} class="point-label ghost">{point.name}</text>{/if}
          {/each}{/if}
          {#each points as point (point.id)}
            <circle cx={point.x} cy={point.y} r={Math.max(1.8, 4.5 / scale)} class="point" class:selected={point.id === selectedPointId} class:contact={point.kind === "contact"} role="button" tabindex="-1"
              onpointerdown={(event) => beginDrag("point", point.id, event)}/>
            {#if scale >= 2}<text x={point.x + 6 / scale} y={point.y - 5 / scale} class="point-label">{point.name}</text>{/if}
          {/each}
        </svg>
      </div>
    {:else}
      <div class="empty"><Crosshair size={26}/><span>Choose a source sprite, then place points</span><small>Click the canvas to add a point, or start from a template.</small></div>
    {/if}
  </div>
  <div class="toolbar">
    <button onclick={onAutoSuggest} disabled={!masterAsset || suggesting || aiBusy}><Wand2 size={12}/>{suggesting ? "Placing…" : "Auto place"}</button>
    <div class="ai-group">
      <button onclick={onAiSuggest} disabled={!masterAsset || aiBusy || suggesting || !agentProviders.length} class="accent"><Bot size={12}/>{aiBusy ? "Asking…" : "Ask AI"}</button>
      <select value={aiProviderId} aria-label="AI provider" disabled={aiBusy} onchange={(event) => onAiProvider(event.currentTarget.value)}>
        {#each agentProviders as provider}<option value={provider.id}>{provider.name}</option>{/each}
      </select>
      <input class="motion" placeholder="Motion intent, e.g. walk cycle (optional — also proposes poses)" value={aiMotion} oninput={(event) => onAiMotion(event.currentTarget.value)} disabled={aiBusy}/>
    </div>
    <label class="zoom">Zoom<select value={scale} onchange={(event) => onScale(Number(event.currentTarget.value))}>{#each [1, 2, 3, 4, 6, 8] as value}<option {value}>{value}×</option>{/each}</select></label>
    <div class="spacer"></div>
    <button onclick={onPreview} disabled={!masterAsset || !bones.length || previewBusy || rendering}>{previewBusy ? "Rendering…" : "Preview"}</button>
    <button class="play" onclick={() => { if (!previewPaths.length) onPreview(); onTogglePlay(); }} disabled={!previewPaths.length} title={playing ? "Pause preview" : "Play preview"}>{#if playing}<Pause size={13}/>{:else}<Play size={13} fill="currentColor"/>{/if}</button>
    {#if previewPaths.length}<span class="preview-count">{previewIndex + 1} / {previewPaths.length}</span>{/if}
  </div>
  {#if suggestion}
    <div class="suggestion">
      <div class="suggestion-text"><strong>{suggestion.source === "ai" ? "AI suggestion" : "Template suggestion"}</strong><span>{suggestion.points.length} points · {suggestion.bones.length} bones{#if suggestion.frames.length} · {suggestion.frames.length} pose frames{/if} — drag the dashed points to fine-tune.</span>{#if suggestion.reasoning}<p>{suggestion.reasoning}</p>{/if}</div>
      <button class="primary" onclick={onApplySuggestion}><Check size={12}/> Apply</button>
      <button onclick={onDismissSuggestion}><X size={12}/> Dismiss</button>
    </div>
  {/if}
</div>

<style>
  .workspace{min-width:0;min-height:0;display:flex;flex-direction:column;height:100%}.canvas-wrap{flex:1;min-height:0;overflow:auto;display:grid;place-items:center;padding:18px;background:var(--bg)}.stage{position:relative;background-color:var(--preview);background-image:linear-gradient(45deg,var(--checker) 25%,transparent 25%),linear-gradient(-45deg,var(--checker) 25%,transparent 25%),linear-gradient(45deg,transparent 75%,var(--checker) 75%),linear-gradient(-45deg,transparent 75%,var(--checker) 75%);background-size:16px 16px;background-position:0 0,0 8px,8px -8px,-8px 0;border:1px solid var(--border-strong);box-shadow:0 14px 36px #0005;touch-action:none;cursor:crosshair}.stage img{position:absolute;inset:0;width:100%;height:100%;object-fit:fill;image-rendering:pixelated;pointer-events:none}.stage svg{position:absolute;inset:0;width:100%;height:100%;pointer-events:none;overflow:visible}.stage .bone{stroke:#69d2c5;stroke-width:2;stroke-dasharray:none;opacity:.85}.stage .bone.near{stroke:#b8cf69}.stage .bone.far{stroke:#8a95a5}.stage .capsule{fill:#69d2c522;stroke:#69d2c566;stroke-width:1}.stage .point{fill:#69d2c5;stroke:#10110f;stroke-width:1.5;cursor:grab;pointer-events:all}.stage .point.contact{fill:#e0a458}.stage .point.selected{stroke:#fff;stroke-width:2.5}.stage .point.ghost{fill:#ffffff00;stroke:#69d2c5;stroke-width:1.5;stroke-dasharray:3 2;cursor:grab;pointer-events:all}.stage .point.ghost.selected{stroke-width:2.5}.stage .point-label{fill:#e8e7e2;stroke:#10110fcc;stroke-width:2.5px;paint-order:stroke;font-size:2.4px;font-family:inherit;pointer-events:none}.stage .point-label.ghost{fill:#9fe8de}.stage.ghosting{outline:1px dashed #69d2c5;outline-offset:3px}.empty{display:flex;flex-direction:column;gap:8px;align-items:center;color:var(--faint);font-size:11px}.empty small{font-size:10px;max-width:260px;text-align:center;line-height:1.5}
  .toolbar{border-top:1px solid var(--border);background:var(--sidebar);min-height:40px;display:flex;align-items:center;gap:6px;padding:5px 12px;flex-wrap:wrap}.toolbar button{height:27px;border:1px solid var(--border);background:var(--surface);color:var(--muted);border-radius:5px;display:flex;align-items:center;gap:5px;padding:0 9px;font:inherit;font-size:11px;cursor:pointer}.toolbar button.accent{border-color:var(--accent);color:var(--accent)}.toolbar button.play{width:32px;justify-content:center;background:var(--text);color:var(--bg)}.toolbar .spacer{flex:1}.toolbar select,.toolbar .motion{height:27px;box-sizing:border-box;background:var(--surface);border:1px solid var(--border);border-radius:4px;color:var(--text);font:inherit;font-size:11px;padding:0 6px;outline:0}.toolbar .motion{width:250px}.ai-group{display:flex;gap:4px;align-items:center}.zoom{display:flex;align-items:center;gap:5px;font-size:10px;color:var(--faint)}.zoom select{height:25px;background:var(--surface);border:1px solid var(--border);border-radius:4px;color:var(--text);font:inherit;font-size:11px;padding:0 4px}.preview-count{font-size:10px;color:var(--faint);margin-left:4px}
  .suggestion{border-top:1px solid var(--accent);background:var(--accent-dim);display:flex;align-items:center;gap:10px;padding:9px 14px}.suggestion-text{flex:1;min-width:0}.suggestion-text strong{font-size:11px}.suggestion-text span{font-size:10px;color:var(--muted);margin-left:8px}.suggestion-text p{font-size:10px;line-height:1.45;color:var(--muted);margin:4px 0 0;max-height:32px;overflow:hidden}.suggestion button{height:27px;border:1px solid var(--border);border-radius:5px;background:var(--surface);color:var(--muted);font:inherit;font-size:11px;display:flex;align-items:center;gap:5px;padding:0 9px;cursor:pointer}.suggestion button.primary{background:var(--accent);border-color:var(--accent);color:#10110f}
  button:disabled{opacity:.4;cursor:not-allowed}
</style>
