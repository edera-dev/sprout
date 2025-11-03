use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// The configuration of the edera action which boots the Edera hypervisor.
/// Edera is based on Xen but modified significantly with a Rust stack.
/// Sprout is a component of the Edera stack and provides the boot functionality of Xen.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct EderaConfiguration {
    /// The path to the Xen hypervisor EFI image.
    pub xen: String,
    /// The path to the kernel to boot for dom0.
    pub kernel: String,
    /// The path to the initrd to load for dom0.
    #[serde(default)]
    pub initrd: Option<String>,
    /// The options to pass to the kernel.
    #[serde(default, rename = "kernel-options")]
    pub kernel_options: Vec<String>,
    /// The options to pass to the Xen hypervisor.
    #[serde(default, rename = "xen-options")]
    pub xen_options: Vec<String>,
}
