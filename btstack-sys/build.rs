use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/stub_btstack.c");
    println!("cargo:rerun-if-changed=include/btstack_stub.h");
    println!("cargo:rerun-if-changed=vendor/btstack");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"));
    let vendor_dir = manifest_dir.join("vendor").join("btstack");

    if try_build_vendor_btstack(&vendor_dir) {
        println!("cargo:rustc-cfg=btstack_vendor_build");
        return;
    }

    build_local_stub();
}

fn try_build_vendor_btstack(vendor_dir: &Path) -> bool {
    if !vendor_dir.exists() {
        println!("cargo:warning=BTstack submodule is not initialized, falling back to local C shim");
        return false;
    }

    let cmake_lists = vendor_dir.join("CMakeLists.txt");
    if !cmake_lists.exists() {
        println!(
            "cargo:warning=Found vendor/btstack but CMakeLists.txt is missing, falling back to local C shim"
        );
        return false;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let build_dir = out_dir.join("btstack-cmake-build");

    let configure = Command::new("cmake")
        .arg("-S")
        .arg(vendor_dir)
        .arg("-B")
        .arg(&build_dir)
        .arg("-DBUILD_SHARED_LIBS=OFF")
        .arg("-DCMAKE_POSITION_INDEPENDENT_CODE=ON")
        .status();

    let Ok(configure_status) = configure else {
        println!("cargo:warning=cmake command is not available, falling back to local C shim");
        return false;
    };

    if !configure_status.success() {
        println!("cargo:warning=Failed to configure BTstack via CMake, falling back to local C shim");
        return false;
    }

    let build_status = Command::new("cmake")
        .arg("--build")
        .arg(&build_dir)
        .status();

    let Ok(build_status) = build_status else {
        println!("cargo:warning=Failed to run cmake --build, falling back to local C shim");
        return false;
    };

    if !build_status.success() {
        println!("cargo:warning=BTstack CMake build failed, falling back to local C shim");
        return false;
    }

    // Vendor build succeeded. We currently keep Rust API mapped to local shim symbols,
    // so until the full raw BTstack FFI layer is introduced, we still compile the shim.
    println!("cargo:warning=BTstack vendor build completed; compiling shim for bootstrap API compatibility");
    build_local_stub();
    true
}

fn build_local_stub() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let object_path = out_dir.join("stub_btstack.o");
    let library_path = out_dir.join("libbtstack_stub.a");

    let status = Command::new("cc")
        .args([
            "-c",
            "src/stub_btstack.c",
            "-Iinclude",
            "-o",
            object_path.to_str().expect("object path is valid UTF-8"),
        ])
        .status()
        .expect("failed to execute C compiler");

    assert!(status.success(), "C compiler failed");

    let status = Command::new("ar")
        .args([
            "crus",
            library_path.to_str().expect("library path is valid UTF-8"),
            object_path.to_str().expect("object path is valid UTF-8"),
        ])
        .status()
        .expect("failed to execute ar");

    assert!(status.success(), "ar failed");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=btstack_stub");
}
