/// The width of the Sprout logo in pixels.
pub const LOGO_WIDTH: usize = {width};

/// The height of the Sprout logo in pixels.
pub const LOGO_HEIGHT: usize = {height};

/// The pixels of the Sprout logo as rows of RGBA bytes with premultiplied alpha.
pub static LOGO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo.rgba"));
