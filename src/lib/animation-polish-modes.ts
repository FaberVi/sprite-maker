import type { AnimationPolishMode } from "$lib/types";

export type AnimationPolishModeOption = {
  id: AnimationPolishMode;
  label: string;
  shortLabel: string;
  description: string;
};

export const ANIMATION_POLISH_MODES: AnimationPolishModeOption[] = [
  {
    id: "rig",
    label: "Rig only",
    shortLabel: "Rig",
    description: "Fast deterministic frames from the native rig renderer. No ImageGen for poses.",
  },
  {
    id: "ai-polish",
    label: "AI polish",
    shortLabel: "Polish",
    description: "Render the rig first, then repair small joints and outline defects without changing poses.",
  },
  {
    id: "full-redraw",
    label: "Full redraw",
    shortLabel: "Redraw",
    description: "Redraw rig frames with AI while keeping pose, timing, and canvas placement from the rig.",
  },
];

export function parseAnimationPolishMode(value: unknown): AnimationPolishMode {
  if (value === "rig" || value === "ai-polish" || value === "full-redraw") return value;
  return "rig";
}

export function animationPolishModeOption(mode: AnimationPolishMode): AnimationPolishModeOption {
  return ANIMATION_POLISH_MODES.find(option => option.id === mode) ?? ANIMATION_POLISH_MODES[0];
}
