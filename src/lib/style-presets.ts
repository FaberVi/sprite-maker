import type { CustomArtStyle } from "$lib/library-types";

export type StylePresetId = string;
export type ConversationStyleId = string;

export type StylePreset = {
  id: StylePresetId;
  name: string;
  description: string;
  thumbnail: string;
  prompt: string;
};

export const STYLE_PRESETS: StylePreset[] = [
  {
    id: "pixel-rpg",
    name: "Pixel RPG",
    description: "Crisp clusters, warm palette, compact game scale",
    thumbnail: "/style-presets/pixel-rpg.webp",
    prompt: "crisp handcrafted pixel RPG character, compact readable clusters, warm restrained palette, clear face and silhouette",
  },
  {
    id: "graphic-adventure",
    name: "Graphic adventure",
    description: "Angular shapes, bold silhouette, painted planes",
    thumbnail: "/style-presets/graphic-adventure.webp",
    prompt: "premium graphic adventure character, bold angular silhouette, simplified painted planes, controlled asymmetry and layered costume shapes",
  },
  {
    id: "cozy-chibi",
    name: "Cozy chibi",
    description: "Rounded proportions, expressive face, clean outlines",
    thumbnail: "/style-presets/cozy-chibi.webp",
    prompt: "polished cozy chibi game character, rounded proportions, oversized expressive head, clean dark outline and simple readable shapes",
  },
  {
    id: "limited-palette",
    name: "Limited palette",
    description: "Tight color ramp, deliberate clusters, crisp dithering",
    thumbnail: "/style-presets/limited-palette.svg",
    prompt: "handcrafted limited-palette pixel art, deliberate pixel clusters, one compact color ramp, selective dithering, crisp silhouette and no soft antialiasing",
  },
  {
    id: "isometric-pixel",
    name: "Isometric pixel",
    description: "2:1 projection, readable planes, consistent top lighting",
    thumbnail: "/style-presets/isometric-pixel.svg",
    prompt: "polished 2:1 isometric pixel game art, consistent projection and top-left lighting, readable top and side planes, compact controlled palette",
  },
  {
    id: "painterly-fantasy",
    name: "Painterly fantasy",
    description: "Soft painted planes, storybook texture, rich materials",
    thumbnail: "/style-presets/painterly-fantasy.svg",
    prompt: "original painterly fantasy game art, softly textured brushwork, layered material shapes, atmospheric color harmony, readable gameplay silhouette",
  },
  {
    id: "cel-shaded",
    name: "Cel shaded",
    description: "Bold contour, flat shadow shapes, saturated accents",
    thumbnail: "/style-presets/cel-shaded.svg",
    prompt: "clean cel-shaded 2D game art, confident dark contour, flat graphic shadow shapes, saturated accent colors, highly readable silhouette",
  },
  {
    id: "one-bit",
    name: "One-bit",
    description: "Two colors, strong negative space, retro clarity",
    thumbnail: "/style-presets/one-bit.svg",
    prompt: "high-clarity one-bit pixel art using exactly two colors, bold negative space, intentional clusters, no gray pixels and no antialiasing",
  },
  {
    id: "top-down-adventure",
    name: "Top-down adventure",
    description: "Overhead Zelda-like scale, iconic gear, down-facing walk",
    thumbnail: "/style-presets/top-down-adventure.webp",
    prompt: "classic top-down adventure pixel character, compact 16 by 24 scale, down-facing walk-ready silhouette, short body and clear head, iconic readable equipment shapes such as shield and sword, warm limited palette, crisp clusters, no antialiasing",
  },
  {
    id: "snes-action-rpg",
    name: "SNES-era action RPG",
    description: "Taller 16-bit hero, chunky clusters, three-quarter idle",
    thumbnail: "/style-presets/snes-action-rpg.webp",
    prompt: "SNES-era action RPG pixel hero, taller 24 by 32 proportions, chunky readable clusters, three-quarter idle stance, restrained warm 16-bit ramp, clear cape and weapon shapes, crisp pixels without soft antialiasing",
  },
  {
    id: "compact-roguelike",
    name: "Compact roguelike",
    description: "Tiny 16px silhouette, high contrast, hybrid facing",
    thumbnail: "/style-presets/compact-roguelike.webp",
    prompt: "tiny compact roguelike pixel character on a 16 by 16 canvas, high-contrast clusters, hybrid top-down and front facing, bold readable silhouette, minimal color count, no antialiasing",
  },
  {
    id: "pixel-platformer",
    name: "Pixel platformer",
    description: "Side-view run silhouette, grounded feet, clear jump",
    thumbnail: "/style-presets/pixel-platformer.webp",
    prompt: "side-view pixel platformer character, 32 by 32 scale, run-ready silhouette, grounded feet and clear weight, readable jump pose, compact clusters, saturated platformer palette, crisp pixels",
  },
  {
    id: "nes-eight-bit",
    name: "NES 8-bit",
    description: "Four-color NES ramp, chunky blocks, no antialias",
    thumbnail: "/style-presets/nes-eight-bit.webp",
    prompt: "authentic NES-era 8-bit pixel character, tight four-color ramp, chunky blocky clusters, bold silhouette, no antialiasing, no gradients, 32 by 32 game scale",
  },
  {
    id: "dark-fantasy-pixel",
    name: "Dark fantasy pixel",
    description: "Grim muted ramp, same compact RPG scale",
    thumbnail: "/style-presets/dark-fantasy-pixel.webp",
    prompt: "dark fantasy pixel RPG character at 48 by 64 game scale, muted grim palette of ash, rust, and deep greens, compact readable clusters, clear silhouette armor and cloak, restrained highlights, no soft antialiasing",
  },
  {
    id: "paper-cutout",
    name: "Paper cutout",
    description: "Stacked paper layers, edge thickness, soft drop shadow",
    thumbnail: "/style-presets/paper-cutout.webp",
    prompt: "handmade paper-cutout game character, stacked colored paper layers with visible edge thickness, slight drop shadow, collage craft look, flat matte paper colors, readable silhouette, not photorealistic",
  },
  {
    id: "watercolor",
    name: "Watercolor",
    description: "Pigment blooms, paper grain, wet controlled edges",
    thumbnail: "/style-presets/watercolor.webp",
    prompt: "soft watercolor game character illustration, pigment blooms and paper grain, controlled wet edges, translucent layered washes, readable silhouette, storybook fantasy palette, not photorealistic",
  },
  {
    id: "comic-ink",
    name: "Comic ink",
    description: "Bold ink contours, flat fills, sparse hatching",
    thumbnail: "/style-presets/comic-ink.webp",
    prompt: "inked comic-book game character, bold black contours, flat color fills, sparse hatching and graphic print look, high readability silhouette, saturated print palette, not photorealistic",
  },
  {
    id: "neon-synth",
    name: "Neon synth",
    description: "Magenta-cyan glow, dark ground, night silhouette",
    thumbnail: "/style-presets/neon-synth.webp",
    prompt: "neon synthwave game character on a dark ground, electric magenta and cyan rim lighting, night silhouette, glossy dark materials with glowing accents, readable shape, not photorealistic",
  },
  {
    id: "clay",
    name: "Clay",
    description: "Sculpted clay volumes, soft studio light, fingerprints",
    thumbnail: "/style-presets/clay.webp",
    prompt: "clay stop-motion game character, soft sculpted clay volumes, subtle fingerprint texture, rounded handmade forms, warm studio lighting, readable silhouette, not photorealistic",
  },
  {
    id: "voxel",
    name: "Voxel",
    description: "Hard cubes, isometric three-quarter, limited faces",
    thumbnail: "/style-presets/voxel.webp",
    prompt: "cubic voxel game character, hard cube volumes, isometric three-quarter view, limited face colors per block, blocky Minecraft-like construction but original design, crisp cubic edges, readable silhouette",
  },
];

export function allStylePresets(custom: CustomArtStyle[] = []): StylePreset[] {
  return [...STYLE_PRESETS, ...custom.map(art => ({ id: art.id, name: art.name, description: art.description, thumbnail: art.thumbnail, prompt: art.prompt }))];
}

export function parseStylePreset(value: unknown, custom: CustomArtStyle[] = []): StylePresetId {
  return allStylePresets(custom).some(preset => preset.id === value) ? String(value) : "pixel-rpg";
}

export function parseConversationStyle(value: unknown, custom: CustomArtStyle[] = []): ConversationStyleId {
  return value === "inherit" ? "inherit" : allStylePresets(custom).some(preset => preset.id === value) ? String(value) : "inherit";
}

export function stylePreset(id: StylePresetId, custom: CustomArtStyle[] = []): StylePreset {
  return allStylePresets(custom).find(preset => preset.id === id) ?? STYLE_PRESETS[0];
}
