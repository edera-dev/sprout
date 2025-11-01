use bitflags::bitflags;

bitflags! {
    /// Feature bitflags for the bootloader interface.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct LoaderFeatures: u64 {
        /// Bootloader supports LoaderConfigTimeout.
        const ConfigTimeout = 1 << 0;
        /// Bootloader supports LoaderConfigTimeoutOneShot.
        const ConfigTimeoutOneShot = 1 << 1;
        /// Bootloader supports LoaderEntryDefault.
        const EntryDefault = 1 << 2;
        /// Bootloader supports LoaderEntryOneShot.
        const EntryOneShot = 1 << 3;
        /// Bootloader supports boot counting.
        const BootCounting = 1 << 4;
        /// Bootloader supports detection from XBOOTLDR partitions.
        const Xbootldr = 1 << 5;
        /// Bootloader supports the handling of random seeds.
        const RandomSeed = 1 << 6;
        /// Bootloader supports loading drivers.
        const LoadDriver = 1 << 7;
        /// Bootloader supports sort keys.
        const SortKey = 1 << 8;
        /// Bootloader supports saved entries.
        const SavedEntry = 1 << 9;
        /// Bootloader supports device trees.
        const DeviceTree = 1 << 10;
        /// Bootloader supports secure boot enroll.
        const SecureBootEnroll = 1 << 11;
        /// Bootloader retains the shim.
        const RetainShim = 1 << 12;
        /// Bootloader supports disabling the menu via the menu timeout variable.
        const MenuDisable = 1 << 13;
        /// Bootloader supports multi-profile UKI.
        const MultiProfileUki = 1 << 14;
        /// Bootloader reports URLs.
        const ReportUrl = 1 << 15;
        /// Bootloader supports type-1 UKIs.
        const Type1Uki = 1 << 16;
        /// Bootloader supports type-1 UKI urls.
        const Type1UkiUrl = 1 << 17;
        /// Bootloader indicates TPM2 active PCR banks.
        const Tpm2ActivePcrBanks = 1 << 18;
    }
}
