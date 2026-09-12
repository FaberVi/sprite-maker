mod handlers;
mod schema;

pub use schema::run;

#[cfg(test)]
pub(crate) use handlers::{resolved_provider, style_context_line};
#[cfg(test)]
pub use schema::SpriteStudioMcp;

#[cfg(test)]
use handlers::{attach_references, export_item, studio_status, BUILTIN_STYLES};
#[cfg(test)]
use schema::{AttachReferencesParams, ExportParams};

#[cfg(test)]
mod tests;
