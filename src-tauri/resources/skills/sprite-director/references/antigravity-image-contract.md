# Antigravity image contract

This run uses Antigravity's native image generation. Treat this section as an override of any ImageGen / Codex skill text elsewhere in the prompt.

## Tool

Use the native **generate_image** tool. **Do not call** `image_gen__imagegen` and do not use a Codex ImageGen skill.

Put the logical canvas size, transparency, subject, style, and view in the text prompt to generate_image. Ask for one original transparent source on a transparent background, with no labels, watermark, mockup, or pose sheet.

## Save path

generate_image may write to a tool-managed path such as the Antigravity scratch directory. After a successful generation, copy or move that exact file to:

- `.sprite-studio/imagegen-sources/<slug>/master.png` for a character, creature, effect, or prop master
- `.sprite-studio/imagegen-sources/<slug>/tileset-master.png` for a terrain atlas

Create the directory if needed. Keep exactly one source image for character, creature, effect, or terrain. Inspect the saved file before continuing the harness. Game-ready frames still belong under `assets/<category>/`.

## Failure

If generate_image is unavailable (tool missing, timeout, quota, or permission denial), stop with `GENERATION_FAILED` and say that Antigravity image generation is unavailable. Never replace the requested source with hand-drawn primitives, JSON shapes, or a silent chat-only fallback.
