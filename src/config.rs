use serde::{Deserialize, Serialize};
use uefi::cstr16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::media::fs::SimpleFileSystem;

#[derive(Serialize, Deserialize, Default)]
pub struct RootConfiguration {
    #[serde(default)]
    pub modules: Vec<ModuleConfiguration>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ModuleConfiguration {
    #[serde(default)]
    pub chainloader: Option<ChainloaderConfiguration>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ChainloaderConfiguration {
    pub path: String,
}

pub fn load() -> RootConfiguration {
    let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(
        uefi::boot::get_handle_for_protocol::<SimpleFileSystem>().expect("no filesystem protocol"),
    )
    .expect("unable to open filesystem protocol");
    let mut fs = FileSystem::new(fs);
    let content = fs
        .read(Path::new(cstr16!("sprout.toml")))
        .expect("unable to read sprout.toml file");
    toml::from_slice(&content).expect("unable to parse sprout.toml file")
}
