use super::native_patterns;
use super::{PieceMoveBackend, PieceMoveContext, PieceMovePattern};

pub struct NativeBackend;

impl PieceMoveBackend for NativeBackend {
    fn generate(&self, ctx: PieceMoveContext<'_>) -> PieceMovePattern {
        native_patterns::generate(ctx)
    }
}
