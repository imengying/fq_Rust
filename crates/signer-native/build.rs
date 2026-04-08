use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SDK_FILES: &[(&str, bool)] = &[
    ("system/bin/ls", true),
    ("system/bin/sh", true),
    ("system/lib64/libc++.so", false),
    ("system/lib64/libc.so", false),
    ("system/lib64/libcrypto.so", false),
    ("system/lib64/libdl.so", false),
    ("system/lib64/liblog.so", false),
    ("system/lib64/libm.so", false),
    ("system/lib64/libssl.so", false),
    ("system/lib64/libstdc++.so", false),
    ("system/lib64/libz.so", false),
];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let local_sdk31 = manifest_dir.join("../../local/rnidbg/sdk31");
    let vendor_sdk23 = manifest_dir.join("../../vendor/rnidbg/android/sdk23");

    println!("cargo:rerun-if-changed={}", local_sdk31.display());
    println!("cargo:rerun-if-changed={}", vendor_sdk23.display());

    let (sdk_name, include_root) = if sdk_has_required_files(&local_sdk31) {
        println!(
            "cargo:warning=embedding local rnidbg runtime from {}",
            local_sdk31.display()
        );
        ("sdk31", "/../../local/rnidbg/sdk31")
    } else {
        println!(
            "cargo:warning=local sdk31 not found, embedding vendor runtime from {}",
            vendor_sdk23.display()
        );
        ("sdk23", "/../../vendor/rnidbg/android/sdk23")
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let generated = render_embedded_sdk(sdk_name, include_root);
    fs::write(out_dir.join("embedded_sdk.rs"), generated).expect("write embedded_sdk.rs");
}

fn sdk_has_required_files(root: &Path) -> bool {
    SDK_FILES.iter().all(|(relative, _)| root.join(relative).is_file())
}

fn render_embedded_sdk(sdk_name: &str, include_root: &str) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "const EMBEDDED_SDK_NAME: &str = {:?};\n",
        sdk_name
    ));
    output.push_str("const EMBEDDED_SDK_FILES: &[EmbeddedFile] = &[\n");

    for (relative_path, executable) in SDK_FILES {
        output.push_str("    EmbeddedFile {\n");
        output.push_str(&format!("        relative_path: {:?},\n", relative_path));
        output.push_str(&format!(
            "        bytes: include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {:?})),\n",
            format!("{include_root}/{relative_path}")
        ));
        output.push_str(&format!("        executable: {},\n", executable));
        output.push_str("    },\n");
    }

    output.push_str("];\n");
    output
}
