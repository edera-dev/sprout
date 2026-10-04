use crate::handle::open_shared;
use alloc::boxed::Box;
use alloc::vec::Vec;
use anyhow::{Context, Result};
use uefi::proto::device_path::{DevicePath, DevicePathNode, DeviceSubType, DeviceType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::partition::PartitionInfo;
use uefi::{Guid, guid};

/// The GPT partition type of the Extended Boot Loader Partition.
/// Reference: <https://uapi-group.org/specifications/specs/boot_loader_specification/>
pub const XBOOTLDR_PARTITION_TYPE: Guid = guid!("bc13c2ff-59e6-4262-a352-b275fd6f7172");

/// Whether `node` is the node of a partition on a hard drive.
fn is_hard_drive_node(node: &DevicePathNode) -> bool {
    node.full_type() == (DeviceType::MEDIA, DeviceSubType::MEDIA_HARD_DRIVE)
}

/// The device nodes of `path`, without the file path nodes that follow the device.
fn device_nodes(path: &DevicePath) -> Vec<&DevicePathNode> {
    path.node_iter()
        .filter(|node| node.full_type() != (DeviceType::MEDIA, DeviceSubType::MEDIA_FILE_PATH))
        .collect()
}

/// Find the Extended Boot Loader Partition on the same disk as the partition at `path`,
/// which is usually the EFI system partition that Sprout was loaded from. Only a partition
/// that has a filesystem the firmware can read is found. Returns the device path of its
/// filesystem, or None if there is no such partition, such as on a disk without a GPT.
pub fn find_xbootldr(path: &DevicePath) -> Result<Option<Box<DevicePath>>> {
    // The partition is the last hard drive node. The disk is everything before it.
    let own = device_nodes(path);
    let Some(partition_index) = own.iter().rposition(|node| is_hard_drive_node(node)) else {
        // There is no partition, such as when booting from an optical disc or the network.
        return Ok(None);
    };
    let disk = &own[..partition_index];
    let own_partition = own[partition_index];

    let handles =
        uefi::boot::find_handles::<SimpleFileSystem>().context("unable to find the filesystems")?;

    let mut found: Option<(u32, Box<DevicePath>)> = None;
    for handle in handles {
        let Ok(candidate_path) = open_shared::<DevicePath>(handle) else {
            continue;
        };
        let nodes = device_nodes(&candidate_path);

        // The candidate must be a partition of the same disk, and not the partition itself.
        if nodes.len() != disk.len() + 1
            || nodes[..disk.len()] != *disk
            || !is_hard_drive_node(nodes[disk.len()])
            || nodes[disk.len()] == own_partition
        {
            continue;
        }

        // The partition type is only available for a GPT partition.
        let Ok(info) = open_shared::<PartitionInfo>(handle) else {
            continue;
        };
        let Some(entry) = info.gpt_partition_entry() else {
            continue;
        };
        // The entry is packed, so the field is copied before it is compared.
        let partition_type = entry.partition_type_guid.0;
        if partition_type != XBOOTLDR_PARTITION_TYPE {
            continue;
        }

        // There should only be one, but if there are several, use the first partition.
        let number = <&uefi::proto::device_path::media::HardDrive>::try_from(nodes[disk.len()])
            .map(|drive| drive.partition_number())
            .unwrap_or(u32::MAX);
        if found.as_ref().is_none_or(|(best, _)| number < *best) {
            found = Some((number, candidate_path.to_boxed()));
        }
    }

    Ok(found.map(|(_, path)| path))
}
