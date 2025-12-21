/// Define the SBAT attestation by including the sbat.csv file.
/// See this document for more details: <https://github.com/rhboot/shim/blob/main/SBAT.md>
/// NOTE: This data must be aligned by 512 bytes.
#[used]
#[unsafe(link_section = ".sbat")]
static SBAT: [u8; {size}] = *include_bytes!(concat!(env!("OUT_DIR"), "/sbat.out"));
