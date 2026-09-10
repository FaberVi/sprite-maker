# Cursor image contract

This run uses Cursor's native image generation. Treat this section as an override of any ImageGen / Codex skill text elsewhere in the prompt.

## Tool

Use the native **GenerateImage** tool (the runtime may expose it as `image_generator` or `GenerateImage`). **Do not call** `image_gen__imagegen` and do not use a Codex ImageGen skill.

GenerateImage has no aspect-ratio parameter. Put the logical canvas size, transparency, subject, style, and view in the text `description`. Ask for one original transparent source on a transparent background, with no labels, watermark, mockup, or pose sheet.

## Save path

GenerateImage may write to a tool-managed path such as `assets/`. After a successful generation, copy or move that exact file to:

- `.sprite-studio/imagegen-sources/<slug>/master.png` for a character, creature, effect, or prop master
- `.sprite-studio/imagegen-sources/<slug>/tileset-master.png` for a terrain atlas

Create the directory if needed. Keep exactly one source image for character, creature, effect, or terrain. Inspect the saved file before continuing the harness. Game-ready frames still belong under `assets/<category>/`.

## Failure

If GenerateImage / image generation is unavailable (Cursor CLI older than 2.4, tool missing, timeout, or 429), stop with `GENERATION_FAILED` and say that Cursor image generation is unavailable. Never replace the requested source with hand-drawn primitives, JSON shapes, or a silent chat-only fallback.
