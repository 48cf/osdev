use alloc::{
    collections::btree_map::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use hel::Handle;
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
            let result =
                unsafe { hel_sys::helAllocateMemory(length, 0, core::ptr::null(), &mut handle) };

            assert_eq!(result, hel_sys::kHelErrNone as _);

            let new_file = Arc::new(VfsNode::File(VfsFile {
                memory_handle: unsafe { Handle::from_raw(handle) },
                length: entry.file().len(),
            }));

            // TODO: Write the file contents to the memory view.

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
