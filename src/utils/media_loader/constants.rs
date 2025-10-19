/// These GUIDs are specific to Linux itself.
pub mod linux {
    use uefi::{Guid, guid};

    /// The device path GUID for the Linux EFI initrd.
    pub const LINUX_EFI_INITRD_MEDIA_GUID: Guid = guid!("5568e427-68fc-4f3d-ac74-ca555231cc68");
}

/// These GUIDs were created by Edera to support Xen loading data
/// from Sprout and other EFI bootloaders.
pub mod xen {
    use uefi::{Guid, guid};

    /// The device path GUID for the Xen EFI config.
    pub const XEN_EFI_CONFIG_MEDIA_GUID: Guid = guid!("bf61f458-a28e-46cd-93d7-07dac5e8cd66");
    /// The device path GUID for the Xen EFI config.
    pub const XEN_EFI_KERNEL_MEDIA_GUID: Guid = guid!("4010c8bf-6ced-40f5-a53f-e820aee8f34b");
    pub const XEN_EFI_RAMDISK_MEDIA_GUID: Guid = guid!("5db1fd01-c3cb-4812-b2ba-8791e52d4a89");
}
