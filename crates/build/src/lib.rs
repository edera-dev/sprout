use std::path::PathBuf;
use std::{env, fs};

/// Block size of the sbat section.
const SBAT_BLOCK_SIZE: usize = 512;

/// Template contents for the sbat.generated.rs file.
const SBAT_RS_TEMPLATE: &str = include_str!("sbat.template.rs");

/// Template contents for the logo.generated.rs file.
#[cfg(not(target_os = "uefi"))]
const LOGO_RS_TEMPLATE: &str = include_str!("logo.template.rs");

/// The factor that the logo is shrunk by, as the source image is larger than the menu needs.
#[cfg(not(target_os = "uefi"))]
const LOGO_DOWNSCALE: usize = 2;

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

/// Generate a logo module from the Sprout logo in the assets directory of the workspace.
/// The logo is shrunk by averaging blocks of pixels and is stored as RGBA with premultiplied
/// alpha, which keeps the work done at boot to blending the pixels onto the framebuffer.
/// The output is included by a generated logo.generated.rs file.
#[cfg(not(target_os = "uefi"))]
pub fn generate_logo_module() {
    use std::fs::File;
    use std::io::BufReader;

    // The output directory to place the logo files into.
    let output_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // The path to the root of the crate.
    let crate_root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));

    // The logo is in the assets directory at the root of the workspace.
    let logo_file = crate_root.join("../../assets/logo-small.png");

    // Notify Cargo that if the logo changes, we need to regenerate the logo files.
    println!(
        "cargo:rerun-if-changed={}",
        logo_file
            .to_str()
            .expect("unable to convert logo path file to a string")
    );

    // Decode the logo as 8-bit RGBA, which is how the logo is stored.
    let decoder = png::Decoder::new(BufReader::new(
        File::open(&logo_file).expect("unable to open logo"),
    ));
    let mut reader = decoder.read_info().expect("unable to read logo");
    let mut source = vec![0; reader.output_buffer_size().expect("logo is too large")];
    let info = reader
        .next_frame(&mut source)
        .expect("unable to decode logo");
    assert_eq!(info.color_type, png::ColorType::Rgba, "logo is not RGBA");
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "logo is not 8-bit");

    // Any pixels on the right and bottom that don't fill a block are dropped.
    let width = info.width as usize / LOGO_DOWNSCALE;
    let height = info.height as usize / LOGO_DOWNSCALE;
    let block = (LOGO_DOWNSCALE * LOGO_DOWNSCALE) as u32;

    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            // Sum the premultiplied channels of every pixel in the block.
            let mut sum = [0u32; 4];
            for by in 0..LOGO_DOWNSCALE {
                for bx in 0..LOGO_DOWNSCALE {
                    let index =
                        ((y * LOGO_DOWNSCALE + by) * info.width as usize + x * LOGO_DOWNSCALE + bx)
                            * 4;
                    let alpha = source[index + 3] as u32;
                    for channel in 0..3 {
                        sum[channel] += source[index + channel] as u32 * alpha / 255;
                    }
                    sum[3] += alpha;
                }
            }
            pixels.extend(sum.map(|channel| (channel / block) as u8));
        }
    }

    // Write the logo.rgba file to the output directory.
    fs::write(output_dir.join("logo.rgba"), &pixels).expect("unable to write logo.rgba");

    // Generate the contents of the logo.generated.rs file.
    let logo_rs = LOGO_RS_TEMPLATE
        .replace("{width}", &width.to_string())
        .replace("{height}", &height.to_string());

    // Write the logo.generated.rs file to the output directory.
    fs::write(output_dir.join("logo.generated.rs"), logo_rs)
        .expect("unable to write logo.generated.rs");
}
