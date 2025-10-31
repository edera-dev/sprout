/// SBAT must be aligned by 512 bytes.
const SBAT_SIZE: usize = 512;

/// Define the SBAT attestation by including the sbat.csv file.
/// See this document for more details: https://github.com/rhboot/shim/blob/main/SBAT.md
#[used]
#[unsafe(link_section = ".sbat")]
static SBAT: [u8; SBAT_SIZE] = *include_bytes!(concat!(env!("OUT_DIR"), "/sbat.csv"));
