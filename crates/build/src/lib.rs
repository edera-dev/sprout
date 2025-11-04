use std::path::PathBuf;
use std::{env, fs};

/// Block size of the sbat section.
const SBAT_BLOCK_SIZE: usize = 512;

/// Template contents for the sbat.generated.rs file.
const SBAT_RS_TEMPLATE: &str = include_str!("sbat.template.rs");

/// Pad with zeros the given `data` to a multiple of `block_size`.
fn block_pad(data: &mut Vec<u8>, block_size: usize) {
    let needed = data.len().div_ceil(block_size).max(1) * block_size;

    if needed != data.len() {
        data.resize(needed, 0);
    }
}

/// Generate an .sbat link section module. This should be coupled with including the sbat module in
/// the crate that intends to embed the sbat section.
/// We intake a sbat.template.csv file in the calling crate and output a sbat.dat
/// which is included by a generated sbat.generated.rs file.
pub fn generate_sbat_module() {
    // Notify Cargo that if the version changes, we need to regenerate the sbat.out file.
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    // The version of the package.
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set");

    // The output directory to place the sbat.csv into.
    let output_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // The output path to the sbat.out file.
    let out_file = output_dir.join("sbat.out");

    // The output path to the sbat.generated.rs file.
    let rs_file = output_dir.join("sbat.generated.rs");

    // The path to the root of the crate.
    let crate_root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));

    // The path to the sbat.template.tsv file is in the source directory of the crate.
    let sbat_template_file = crate_root.join("src/sbat.csv");

    // Notify Cargo that if sbat.csv changes, we need to regenerate the sbat.out file.
    println!(
        "cargo:rerun-if-changed={}",
        sbat_template_file
            .to_str()
            .expect("unable to convert sbat template path file to a string")
    );

    // Read the sbat.csv template file.
    let sbat_template =
        fs::read_to_string(&sbat_template_file).expect("unable to read sbat.csv file");

    // Replace the version placeholder in the template with the actual version.
    let sbat = sbat_template.replace("{version}", &version);

    // Encode the sbat.csv as bytes.
    let mut encoded = sbat.as_bytes().to_vec();

    // Pad the sbat.csv to the required block size.
    block_pad(&mut encoded, SBAT_BLOCK_SIZE);

    // Write the sbat.out file to the output directory.
    fs::write(&out_file, &encoded).expect("unable to write sbat.out");

    // Generate the contents of the sbat.generated.rs file.
    // The size must tbe size of the encoded sbat.out file.
    let sbat_rs = SBAT_RS_TEMPLATE.replace("{size}", &encoded.len().to_string());

    // Write the sbat.generated.rs file to the output directory.
    fs::write(&rs_file, sbat_rs).expect("unable to write sbat.generated.rs");
}
