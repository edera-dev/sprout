/// SBAT must be aligned by 512 bytes.
const SBAT_SIZE: usize = 512;

/// Define the SBAT attestation by including the sbat.csv file.
/// See this document for more details: https://github.com/rhboot/shim/blob/main/SBAT.md
/// NOTE: Alignment can't be enforced by an attribute, so instead the alignment is currently
/// enforced by the SBAT_SIZE being 512. The build.rs will ensure that the sbat.csv is padded.
/// This code will not compile if the sbat.csv is a different size than SBAT_SIZE.
#[used]
#[unsafe(link_section = ".sbat")]
static SBAT: [u8; SBAT_SIZE] = *include_bytes!(concat!(env!("OUT_DIR"), "/sbat.csv"));
