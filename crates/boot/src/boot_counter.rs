use alloc::{format, string::String};
use anyhow::{Context, Result, anyhow, bail};
use edera_sprout_bls::BootCounter;
use uefi::{
    CString16, Handle,
    data_types::Align,
    fs::PathBuf,
    mem::AlignedBuffer,
    proto::media::file::{File, FileAttribute, FileInfo, FileMode},
    proto::media::fs::SimpleFileSystem,
};

/// The size of the fixed part of a file info: three sizes, three timestamps and the attributes.
const FILE_INFO_HEADER_SIZE: usize = 3 * 8 + 3 * 16 + 8;

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
    /// Whether booting the entry uses up a try. An entry whose tries are not used up still
    /// has them, so it can be bad.
    pub counting: bool,
}

impl BootCounterTarget {
    /// Consumes one try by renaming the entry file, such as `foo+3.conf` to `foo+2-1.conf`.
    /// The file is renamed in place without copying it, as unified kernel images are large and
    /// a copy interrupted by a crash would leave a truncated image behind.
    /// Returns the path of the renamed file.
    pub fn consume(&self) -> Result<PathBuf> {
        let new_name = format!(
            "{}{}",
            self.counter.decremented().render(&self.id),
            self.extension
        );
        let new_name_16 =
            CString16::try_from(new_name.as_str()).context("invalid new entry file name")?;

        let mut fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(self.filesystem)
            .context("unable to open the entry filesystem")?;
        let mut root = fs
            .open_volume()
            .context("unable to open the entry volume")?;

        // Open the entry file for writing.
        let mut from = self.directory.clone();
        from.push(PathBuf::from(
            CString16::try_from(self.file_name.as_str()).context("invalid entry file name")?,
        ));
        let from_name =
            CString16::try_from(format!("{}", from).as_str()).context("invalid entry file path")?;
        let handle = root
            .open(&from_name, FileMode::ReadWrite, FileAttribute::empty())
            .context("unable to open the entry file")?;
        let mut file = handle
            .into_regular_file()
            .ok_or_else(|| anyhow!("the entry is not a regular file"))?;

        // Renaming a file is done by setting its file info with a different name.
        let info = file
            .get_boxed_info::<FileInfo>()
            .context("unable to get the entry file info")?;
        // A read-only file is left as it is, as the counter is not meant to be changed.
        if info.attribute().contains(FileAttribute::READ_ONLY) {
            bail!("the entry file is read-only");
        }
        // The info is a fixed header followed by the null-terminated UCS-2 name.
        let size =
            FILE_INFO_HEADER_SIZE + 2 * (new_name.len() + 1) + <FileInfo as Align>::alignment();
        let mut storage = AlignedBuffer::from_size_align(size, <FileInfo as Align>::alignment())
            .context("unable to allocate the entry file info")?;
        let renamed = FileInfo::new(
            storage.as_slice_mut(),
            info.file_size(),
            info.physical_size(),
            *info.create_time(),
            *info.last_access_time(),
            *info.modification_time(),
            info.attribute(),
            &new_name_16,
        )
        .map_err(|error| anyhow!("unable to build the renamed entry file info: {:?}", error))?;
        file.set_info(renamed)
            .context("unable to rename the entry file")?;
        file.flush()
            .context("unable to flush the renamed entry file")?;

        let mut to = self.directory.clone();
        to.push(PathBuf::from(new_name_16));
        Ok(to)
    }
}
