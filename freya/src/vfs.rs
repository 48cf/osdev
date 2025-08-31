use alloc::{
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use hel::{Handle, Mapping, MappingFlags};
use spin::{Lazy, Mutex};

#[derive(Default)]
pub struct VfsDirectory {
    entries: Mutex<BTreeMap<String, Arc<VfsNode>>>,
}

pub struct VfsFile {
    memory_handle: Handle,
    length: usize,
}

pub enum VfsNode {
    Directory(VfsDirectory),
    File(VfsFile),
}

static ROOT: Lazy<Arc<VfsNode>> =
    Lazy::new(|| Arc::new(VfsNode::Directory(VfsDirectory::default())));

pub fn unpack_initrd(initrd: &[u8]) {
    for entry in cpio_reader::iter_files(initrd) {
        let parts: Vec<_> = entry.name().split('/').collect();
        let name = parts[parts.len() - 1].to_string();

        let mut dir = ROOT.clone();

        for &part in &parts[0..parts.len() - 1] {
            let VfsNode::Directory(directory) = &*dir else {
                panic!("freya: Part of path is not a directory");
            };

            if part.is_empty() {
                continue;
            }

            let next = directory
                .entries
                .lock()
                .get(part)
                .cloned()
                .expect("freya: Part of path does not exist");

            dir = next;
        }

        let VfsNode::Directory(directory) = &*dir else {
            panic!("freya: Part of path is not a directory");
        };

        if entry.mode().contains(cpio_reader::Mode::DIRECTORY) {
            let new_dir = Arc::new(VfsNode::Directory(VfsDirectory::default()));

            directory.entries.lock().insert(name, new_dir);
        } else if entry.mode().contains(cpio_reader::Mode::REGULAR_FILE) {
            let mut handle = 0;

            let length = (entry.file().len() + 0xFFF) & !0xFFF;
            let result = unsafe {
                hel_sys::helAllocateMemory(
                    length,
                    hel_sys::kHelAllocOnDemand | hel_sys::kHelAllocContinuous,
                    core::ptr::null(),
                    &mut handle,
                )
            };

            assert_eq!(result, hel_sys::kHelErrNone as _);

            let memory_handle = unsafe { Handle::from_raw(handle) };
            let mapping: Mapping<u8> = unsafe {
                Mapping::new(
                    &memory_handle,
                    None,
                    0,
                    length,
                    MappingFlags::READ | MappingFlags::WRITE,
                )
                .expect("freya: Failed to map initrd file memory")
            };

            unsafe {
                mapping
                    .as_ptr()
                    .unwrap()
                    .as_ptr()
                    .copy_from_nonoverlapping(entry.file().as_ptr(), entry.file().len());
            }

            core::mem::forget(mapping);

            let new_file = Arc::new(VfsNode::File(VfsFile {
                length: entry.file().len(),
                memory_handle,
            }));

            directory.entries.lock().insert(name, new_file);
        } else {
            panic!("freya: Unknown file type in initrd");
        }
    }
}

pub fn lookup(path: &str) -> Option<Arc<VfsNode>> {
    let parts: Vec<_> = path.split('/').collect();
    let mut current = ROOT.clone();

    for part in parts {
        let VfsNode::Directory(directory) = &*current else {
            panic!("freya: Part of path is not a directory");
        };

        if part.is_empty() {
            continue;
        }

        let next = directory.entries.lock().get(part).cloned()?;

        current = next;
    }

    Some(current)
}
