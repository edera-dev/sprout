use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use anyhow::{Context, Result, anyhow, bail};
use edera_sprout_bls::{BlsEntry, PeImage, ReadAt, UKI_SECTIONS, read_pe, strip_extension};
use log::warn;
use uefi::{
    CString16, Handle, Status,
    proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType, RegularFile},
    proto::media::fs::SimpleFileSystem,
};

/// A unified kernel image that was found, with the metadata Sprout reads.
pub struct UkiFile {
    /// The name of the file, such as `fedora+3.efi`.
    pub file_name: String,
    /// The machine type of the image.
    pub machine: u16,
    /// Whether the image has an `.osrel` section, which unified kernel images always have.
    pub has_osrel: bool,
    /// The entry made from the sections of the image.
    pub entry: BlsEntry,
}

/// Reads a file at arbitrary offsets, so that a large image isn't loaded to read its headers.
struct FileReader(RegularFile);

impl ReadAt for FileReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.0
            .set_position(offset)
            .map_err(|error| anyhow!("unable to seek: {}", error))?;
        let mut done = 0;
        while done < buf.len() {
            let read = self
                .0
                .read(&mut buf[done..])
                .map_err(|error| anyhow!("unable to read: {}", error))?;
            if read == 0 {
                bail!("unexpected end of file");
            }
            done += read;
        }
        Ok(())
    }
}

/// Finds the `.efi` files in `directory` on `filesystem` and reads their sections.
/// A missing directory has no images. Files that can't be read as a PE image are skipped,
/// as one broken file should not prevent booting any of the other entries.
pub fn scan(filesystem: Handle, directory: &str) -> Result<Vec<UkiFile>> {
    let mut fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(filesystem)
        .context("unable to open the unified kernel image filesystem")?;
    let mut root = fs
        .open_volume()
        .context("unable to open the unified kernel image volume")?;

    let name = CString16::try_from(directory).context("invalid unified kernel image path")?;
    let handle = match root.open(&name, FileMode::Read, FileAttribute::empty()) {
        Ok(handle) => handle,
        Err(error) if error.status() == Status::NOT_FOUND => return Ok(Vec::new()),
        Err(error) => return Err(error).context("unable to open the unified kernel image path"),
    };
    let FileType::Dir(mut directory_file) = handle
        .into_type()
        .context("unable to inspect the unified kernel image path")?
    else {
        bail!("the unified kernel image path is not a directory");
    };

    // Collect the file names first, as files can't be opened while the directory is read.
    let mut names = Vec::new();
    while let Some(info) = directory_file
        .read_entry_boxed()
        .context("unable to read the unified kernel image directory")?
    {
        let info: &FileInfo = &info;
        if info.attribute().contains(FileAttribute::DIRECTORY) {
            continue;
        }
        let file_name = info.file_name().to_string();
        // Hidden files, such as the AppleDouble files that macOS writes, are not images.
        if !file_name.starts_with('.') && strip_extension(&file_name, ".efi").is_some() {
            names.push(file_name);
        }
    }

    let mut found = Vec::new();
    for file_name in names {
        let path = format!("{}\\{}", directory, file_name);
        match read_image(&mut root, &path) {
            Ok(image) => found.push(UkiFile {
                file_name,
                machine: image.machine,
                has_osrel: image.sections.contains_key(".osrel"),
                // The sections are dropped here, so only the small entry is kept per image.
                entry: BlsEntry::from_uki(&image.sections, &path),
            }),
            Err(error) => warn!("unable to read unified kernel image {}: {:#}", path, error),
        }
    }
    Ok(found)
}

/// Reads the sections of the PE image at `path`.
fn read_image(root: &mut uefi::proto::media::file::Directory, path: &str) -> Result<PeImage> {
    let name = CString16::try_from(path).context("invalid path")?;
    let handle = root
        .open(&name, FileMode::Read, FileAttribute::empty())
        .context("unable to open")?;
    let Some(file) = handle.into_regular_file() else {
        bail!("not a regular file");
    };
    read_pe(&mut FileReader(file), &UKI_SECTIONS)
}
