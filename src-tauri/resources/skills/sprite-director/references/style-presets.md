# Sprite style presets

Use these only when the user did not specify production dimensions. Treat named games as high-level inspiration and keep the output original.

| User language | Logical canvas | Default deliverable | FPS | Direction |
|---|---:|---:|---:|---|
| Pixel RPG, cozy farming RPG, 16-bit | 48×64 character | 4-frame gentle idle/walk-ready set | 8 | front or user-specified |
| Graphic adventure, angular concept art | 192×256 character | 4-frame idle set | 8 | three-quarter/front |
| Cozy chibi, rounded cartoon | 128×160 character | 4-frame idle set | 8 | front or three-quarter |
| limited-palette pixel art | 32×32 character or object | 1 static sprite or motion-ready set | 8 | user-specified |
| 2:1 isometric pixel art | 64×64 object or creature | 1 static sprite | 1 | isometric three-quarter |
| painterly fantasy | 192×192 object or creature | 1 static sprite | 1 | three-quarter |
| cel-shaded cartoon | 128×128 object or character | 1 static sprite or 6-frame action set | 10 | side or three-quarter |
| one-bit monochrome | 32×32 object or creature | 1 static sprite | 1 | strongest silhouette angle |
| classic top-down adventure | 16×24 character | 4-frame idle/walk-ready set | 8 | down/front |
| SNES-era action RPG | 24×32 character | 4-frame idle | 10 | three-quarter/front |
| compact roguelike | 16×16 character | 2-frame idle | 6 | top-down/front hybrid |
| pixel platformer | 32×32 character | 6-frame run-ready set | 12 | side view |
| NES 8-bit | 32×32 character or object | 1 static sprite or motion-ready set | 8 | user-specified |
| Dark fantasy pixel | 48×64 character | 4-frame idle/walk-ready set | 8 | front or user-specified |
| Paper cutout | 128×160 character | 4-frame idle set | 8 | front or three-quarter |
| Watercolor | 192×192 object or creature | 1 static sprite | 1 | three-quarter |
| Comic ink | 128×128 object or character | 1 static sprite or 6-frame action set | 10 | side or three-quarter |
| Neon synth | 128×128 object or character | 1 static sprite or 6-frame action set | 10 | side or three-quarter |
| Clay | 128×128 object or character | 1 static sprite or 6-frame action set | 10 | side or three-quarter |
| Voxel | 64×64 object or creature | 1 static sprite | 1 | isometric three-quarter |
| fighting or large action character | 64×64 character | 6-frame idle/action set | 12 | side view |
| inventory icon, item, prop | 24×24 prop | 1 frame | 1 | three-quarter icon |
| UI icon, tiny pickup | 16×16 prop | 1 frame | 1 | centered icon |
| portrait or dialogue bust | 64×64 character | 1 frame | 1 | front/three-quarter |
| terrain, tilemap, tileset | 384×256 terrain atlas | 1 PNG containing a coherent 32px-grid tile family | 1 | top-down |
| impact, sparkle, smoke effect | 32×32 effect | 5-frame effect | 12 | centered |

## Inference rules

- A simple request for a style-inspired character should produce a useful animated set, not ask for size.
- A style named directly in the user prompt overrides the workspace or chat preset. Preserve the named traits across every asset in a pack.
- Use the logical canvas dimensions exactly; Sprite Studio previews can scale pixels without changing the source.
- For a single object or icon, do not invent animation.
- For walking, running, attacking, idling, spellcasting, or effects, generate multiple frames even when the user omits a frame count.
- Use 3–6 main colors plus outline and highlight for small sprites. Add colors only when readability requires them.
- Place character feet within the bottom two rows and keep the pivot stable across frames.
- A transparent pixel is not a palette color. Avoid semi-transparent edge pixels for crisp pixel art.

## Style translation

- “Stardew-like” means cozy farming-RPG proportions, warm readable colors, compact pixel clusters, and an original outfit and face. Use the Pixel RPG preset; never reproduce an existing farmer sprite.
- “Pokemon-like” means compact top-down readability, strong color blocking, and an original silhouette; never reproduce an existing creature or trainer.
- “Zelda-like” means readable top-down adventure proportions and iconic equipment shapes; never reproduce Link or franchise symbols.
- “SNES action RPG” means taller 24×32 hero proportions, chunky 16-bit clusters, and a three-quarter idle stance; keep the palette warm and restrained.
- “Roguelike tiny” means a high-contrast 16×16 silhouette with hybrid top-down/front facing; prioritize readability over detail.
- “Platformer pixel” means a side-view run-ready silhouette with grounded feet and a clear jump pose at about 32×32.
- “NES 8-bit” means a tight four-color ramp, chunky blocks, and no antialiasing or soft gradients.
- “Dark fantasy pixel” keeps Pixel RPG’s 48×64 scale but uses a muted grim ramp (ash, rust, deep greens) with restrained highlights.
- “Paper cutout” means stacked colored paper layers with visible edge thickness and a soft drop shadow, not painted shading.
- “Watercolor” means pigment blooms, paper grain, and controlled wet edges while keeping a readable gameplay silhouette.
- “Comic ink” means bold contours, flat fills, and sparse hatching like a graphic print.
- “Neon synth” means magenta/cyan glow on a dark ground with a night-readable silhouette.
- “Clay” means soft sculpted stop-motion volumes, fingerprint texture, and studio lighting.
- “Voxel” means hard cubic volumes in an isometric three-quarter view with limited face colors.
- When the user names any living artist, studio, or game, describe the transferable traits and create a distinct original asset.
