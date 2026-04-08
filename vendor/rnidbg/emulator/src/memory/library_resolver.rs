use std::path::Path;
use log::{error, info};
use crate::emulator::AndroidEmulator;
use crate::memory::library_file::{ElfLibraryFile, LibraryFile};

pub(crate) fn resolve_library_static<T: Clone>(emulator: &AndroidEmulator<T>, library_name: &str) -> anyhow::Result<LibraryFile> {
    if option_env!("EMU_LOG") == Some("1") {
        info!("resolve_library_static: {}", library_name);
    }
    let base_path = emulator.inner_mut().base_path.as_str();
    let lib64_root = if base_path.is_empty() {
        "./android/sdk23/system/lib64".to_string()
    } else {
        format!("{}/system/lib64", base_path)
    };
    let bin_root = if base_path.is_empty() {
        "./android/sdk23/system/bin".to_string()
    } else {
        format!("{}/system/bin", base_path)
    };
    let lib64_path = Path::new(&lib64_root);
    let bin_path = Path::new(&bin_root);

    let mut path = lib64_path.join(library_name);
    if !path.exists() && library_name == "ld-android.so" {
        let linker_path = bin_path.join("linker64");
        if linker_path.exists() {
            path = linker_path;
        }
    }

    if !path.exists() {
        error!("Library not found: {}", path.to_str().unwrap());
        return Err(anyhow::anyhow!("Library not found: {}", path.to_str().unwrap()));
    }

    let buffer = std::fs::read(path.as_path())?;
    Ok(LibraryFile::Elf(ElfLibraryFile::new(buffer, path.to_str().unwrap().to_string())))
}
