use serde::{Deserialize, Serialize};

/// The configuration of the chainload action.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChainloadConfiguration {
    /// The path to the image to chainload.
    /// This can be a Linux EFI stub (vmlinuz usually) or a standard EFI executable.
    pub path: String,
    /// The options to pass to the image.
    /// The options are concatenated by a space and then passed to the EFI application.
    #[serde(default)]
    pub options: Vec<String>,
    /// An optional path to a Linux initrd.
    /// This uses the [LINUX_EFI_INITRD_MEDIA_GUID] mechanism to load the initrd into the EFI stack.
    /// For Linux, you can also use initrd=\path\to\initrd as an option, but this option is
    /// generally better and safer as it can support additional load options in the future.
    #[serde(default, rename = "linux-initrd")]
    pub linux_initrd: Option<String>,
}
