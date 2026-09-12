mod prompt;
mod routing;

pub use prompt::{rig_suggestion_prompt, studio_prompt};
#[cfg(test)]
use routing::{explicit_size, infer_brief, HarnessKind, SpriteBrief};

#[cfg(test)]
mod tests;
