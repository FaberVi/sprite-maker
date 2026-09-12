<script lang="ts">
  import { Bone, ChevronDown, Layers3, WandSparkles } from "lucide-svelte";
  import { ANIMATION_POLISH_MODES, animationPolishModeOption } from "$lib/animation-polish-modes";
  import type { AnimationPolishMode } from "$lib/types";

  let { value, disabled = false, onChange }: {
    value: AnimationPolishMode;
    disabled?: boolean;
    onChange: (mode: AnimationPolishMode) => void | Promise<void>;
  } = $props();

  let menu = $state<HTMLDetailsElement>();
  const selected = $derived(animationPolishModeOption(value));

  async function choose(mode: AnimationPolishMode) {
    if (disabled) return;
    await onChange(mode);
    menu?.removeAttribute("open");
  }

  $effect(() => {
    if (disabled) menu?.removeAttribute("open");
  });
</script>

<details class="animation-mode-menu" bind:this={menu}>
  <summary title="Animation finish mode for /animate" class:disabled tabindex={disabled ? -1 : 0} onkeydown={(event)=>{if(disabled&&(event.key===" "||event.key==="Enter"))event.preventDefault();}} onclick={(event)=>{if(disabled)event.preventDefault();}}>
    <span class="mode-icon">
      {#if value === "ai-polish"}<WandSparkles size={13}/>{:else if value === "full-redraw"}<Layers3 size={13}/>{:else}<Bone size={13}/>{/if}
    </span>
    <span>{selected.shortLabel}</span>
    <ChevronDown size={12}/>
  </summary>
  <div class="popover">
    <strong>Animation mode</strong>
    <p>Used for <code>/animate</code> and motion requests in this chat.</p>
    <div class="options">
      {#each ANIMATION_POLISH_MODES as option}
        <button class:active={option.id === value} disabled={disabled} onclick={() => choose(option.id)}>
          <span class="mode-icon">
            {#if option.id === "ai-polish"}<WandSparkles size={14}/>{:else if option.id === "full-redraw"}<Layers3 size={14}/>{:else}<Bone size={14}/>{/if}
          </span>
          <div>
            <strong>{option.label}{#if option.id === "rig"} <em>Default</em>{/if}{#if option.id === "full-redraw"} <em>Experimental</em>{/if}</strong>
            <small>{option.description}</small>
          </div>
        </button>
      {/each}
    </div>
  </div>
</details>

<style>
  .animation-mode-menu{position:relative}.animation-mode-menu summary{height:34px;display:flex;align-items:center;gap:7px;border:1px solid var(--border);border-radius:7px;padding:0 9px 0 7px;background:var(--surface);color:var(--muted);font-size:12px;cursor:pointer;list-style:none}.animation-mode-menu summary::-webkit-details-marker{display:none}.animation-mode-menu summary:hover,.animation-mode-menu[open] summary{border-color:var(--border-strong);color:var(--text)}.animation-mode-menu summary.disabled{opacity:.55;pointer-events:none}.mode-icon{width:22px;height:22px;display:grid;place-items:center;border:1px solid var(--border);border-radius:6px;color:var(--accent);background:var(--bg)}.popover{position:absolute;z-index:100;top:40px;right:0;width:min(320px,calc(100vw - 40px));padding:14px;background:var(--surface);border:1px solid var(--border-strong);border-radius:10px;box-shadow:0 18px 54px #000a}.popover>strong{font-size:13px}.popover>p{font-size:11px;color:var(--muted);margin:4px 0 0;line-height:1.45}.popover code{font-size:10px;color:var(--accent)}.options{display:grid;gap:5px;margin-top:12px}.options button{display:grid;grid-template-columns:28px minmax(0,1fr);gap:9px;align-items:flex-start;width:100%;padding:9px;border:1px solid transparent;border-radius:7px;background:transparent;color:var(--text);font:inherit;text-align:left;cursor:pointer}.options button:hover{background:var(--surface-hover);border-color:var(--border)}.options button.active{background:var(--accent-dim);border-color:#7c8c45}.options button:disabled{opacity:.55;cursor:default}.options strong{display:block;font-size:11px}.options em{font-style:normal;font-size:8px;color:var(--accent);margin-left:4px}.options small{display:block;margin-top:3px;font-size:10px;line-height:1.4;color:var(--faint)}
</style>
