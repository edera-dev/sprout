use anyhow::{Error, Result};
use std::str::FromStr;

#[derive(Default, Debug, Clone)]
pub struct BlsEntry {
    pub title: Option<String>,
    pub options: Option<String>,
    pub linux: Option<String>,
    pub initrd: Option<String>,
    pub efi: Option<String>,
}

impl FromStr for BlsEntry {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut title: Option<String> = None;
        let mut options: Option<String> = None;
        let mut linux: Option<String> = None;
        let mut initrd: Option<String> = None;
        let mut efi: Option<String> = None;

        for line in input.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once(" ") else {
                continue;
            };

            match key {
                "title" => {
                    title = Some(value.trim().to_string());
                }

                "options" => {
                    options = Some(value.trim().to_string());
                }

                "linux" => {
                    linux = Some(value.trim().to_string());
                }

                "initrd" => {
                    initrd = Some(value.trim().to_string());
                }

                "efi" => {
                    efi = Some(value.trim().to_string());
                }

                _ => {
                    continue;
                }
            }
        }

        Ok(BlsEntry {
            title,
            options,
            linux,
            initrd,
            efi,
        })
    }
}

impl BlsEntry {
    pub fn is_valid(&self) -> bool {
        self.linux.is_some() || self.efi.is_some()
    }

    pub fn chainload_path(&self) -> Option<String> {
        self.linux
            .clone()
            .or(self.efi.clone())
            .map(|path| path.replace("/", "\\").trim_start_matches("\\").to_string())
    }

    pub fn initrd_path(&self) -> Option<String> {
        self.initrd
            .clone()
            .map(|path| path.replace("/", "\\").trim_start_matches("\\").to_string())
    }

    pub fn options(&self) -> Option<String> {
        self.options.clone()
    }

    pub fn title(&self) -> Option<String> {
        self.title.clone()
    }
}
