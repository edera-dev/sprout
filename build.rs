use std::path::PathBuf;
use std::{env, fs};

/// The size of the sbat.csv file.
const SBAT_SIZE: usize = 512;

/// Generate the sbat.csv for the .sbat link section.
///
/// We intake a sbat.template.tsv and output a sbat.csv which is included by src/sbat.rs
fn generate_sbat_csv() {
    // Notify Cargo that if the Sprout version changes, we need to regenerate the sbat.csv.
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    // The version of the sprout crate.
    let sprout_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set");

    // The output directory to place the sbat.csv into.
    let output_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // The output path to the sbat.csv.
    let output_file = output_dir.join("sbat.csv");

    // The path to the root of the sprout crate.
    let sprout_root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));

    // The path to the sbat.template.tsv file is in the source directory of the sprout crate.
    let template_path = sprout_root.join("src/sbat.template.csv");

    // Read the sbat.csv template file.
    let template = fs::read_to_string(&template_path).expect("unable to read template file");

    // Replace the version placeholder in the template with the actual version.
    let sbat = template.replace("{version}", &sprout_version);

    // Encode the sbat.csv as bytes.
    let mut encoded = sbat.as_bytes().to_vec();

    if encoded.len() > SBAT_SIZE {
        panic!("sbat.csv is too large");
    }

    // Pad the sbat.csv to the required size.
    while encoded.len() < SBAT_SIZE {
        encoded.push(0);
    }

    // Write the sbat.csv to the output directory.
    fs::write(&output_file, encoded).expect("unable to write sbat.csv");
}

/// Build script entry point.
/// Right now, all we need to do is generate the sbat.csv file.
fn main() {
    // Generate the sbat.csv file.
    generate_sbat_csv();
}
