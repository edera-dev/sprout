use edera_sprout_build::{generate_logo_module, generate_sbat_module};

/// Build script entry point for Sprout.
fn main() {
    // Generate the sbat.generated.rs file.
    generate_sbat_module();

    // Generate the logo.generated.rs file.
    generate_logo_module();
}
