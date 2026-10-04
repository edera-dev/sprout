use alloc::{format, string::String};
use anyhow::{Context, Result};
use edera_sprout_bls::BootCounter;
use uefi::{
    CString16, Handle,
    fs::{FileSystem, PathBuf},
    proto::media::fs::SimpleFileSystem,
};

/// Where the boot counter of an entry is stored, so that a try can be consumed when the
/// entry is booted.
#[derive(Clone)]
pub struct BootCounterTarget {
    /// The counter as it is currently encoded in the file name.
    pub counter: BootCounter,
    /// The handle of the filesystem that holds the entry file.
    pub filesystem: Handle,
    /// The directory that holds the entry file.
    pub directory: PathBuf,
    /// The entry id, which is the file name without the counter and extension.
    pub id: String,
    /// The current file name, including the counter and the extension.
    pub file_name: String,
    /// The extension of the file name, such as `.conf`, in its original case.
    pub extension: String,
}

impl BootCounterTarget {
    /// Consumes one try by renaming the entry file, such as `foo+3.conf` to `foo+2-1.conf`.
    /// Returns the path of the renamed file.
    pub fn consume(&self) -> Result<PathBuf> {
        let new_name = format!(
            "{}{}",
            self.counter.decremented().render(&self.id),
            self.extension
        );

        let mut from = self.directory.clone();
        from.push(PathBuf::from(
            CString16::try_from(self.file_name.as_str()).context("invalid entry file name")?,
        ));
        let mut to = self.directory.clone();
        to.push(PathBuf::from(
            CString16::try_from(new_name.as_str()).context("invalid new entry file name")?,
        ));

        let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(self.filesystem)
            .context("unable to open the entry filesystem")?;
        let mut fs = FileSystem::new(fs);
        fs.rename(&from, &to)
            .context("unable to rename the entry file")?;
        Ok(to)
    }
}
