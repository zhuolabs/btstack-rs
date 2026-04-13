use std::env;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/stub_btstack.c");
    println!("cargo:rerun-if-changed=include/btstack_stub.h");
    println!("cargo:rerun-if-changed=vendor/btstack");
    println!("cargo:rerun-if-env-changed=TARGET");

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

    let target = env::var("TARGET").unwrap_or_default();
    let source_dir = select_vendor_source_dir(vendor_dir, &target);
    let cmake_lists = source_dir.join("CMakeLists.txt");

    if !cmake_lists.exists() {
        println!(
            "cargo:warning=Expected CMakeLists.txt at {} but it is missing, falling back to local C shim",
            cmake_lists.display()
        );
        return false;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let cmake_out_dir = out_dir.join("btstack-cmake");

    if target.contains("linux") && !pkg_config_has_module("libusb-1.0") {
        println!(
            "cargo:warning=BTstack libusb build requires pkg-config module libusb-1.0; install libusb dev package (e.g. libusb-1.0-0-dev)"
        );
        return false;
    }

    let build_result = catch_unwind(AssertUnwindSafe(|| {
        let mut config = cmake::Config::new(&source_dir);
        config
            .out_dir(&cmake_out_dir)
            .profile("Release")
            .define("BUILD_SHARED_LIBS", "OFF")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .build_target("btstack");

        config.build()
    }));

    let cmake_install_dir = match build_result {
        Ok(path) => path,
        Err(_) => {
            if target.contains("linux") {
                println!(
                    "cargo:warning=Failed to configure/build BTstack libusb port; install libusb dev package (e.g. libusb-1.0-0-dev) and retry"
                );
            } else {
                println!("cargo:warning=Failed to configure/build BTstack via cmake crate, falling back to local C shim");
            }
            return false;
        }
    };

    let cmake_build_dir = cmake_out_dir.join("build");
    emit_btstack_link_settings(&cmake_install_dir, &cmake_build_dir, &target);
    true
}

fn pkg_config_has_module(module_name: &str) -> bool {
    Command::new("pkg-config")
        .arg("--exists")
        .arg(module_name)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn select_vendor_source_dir(vendor_dir: &Path, target: &str) -> PathBuf {
    if target.contains("windows") {
        let windows_port = vendor_dir.join("port").join("windows-winusb");
        if windows_port.join("CMakeLists.txt").exists() {
            return windows_port;
        }
    }

    if target.contains("linux") {
        let linux_port = vendor_dir.join("port").join("libusb");
        if linux_port.join("CMakeLists.txt").exists() {
            return linux_port;
        }
    }

    vendor_dir.to_path_buf()
}

fn emit_btstack_link_settings(cmake_install_dir: &Path, cmake_build_dir: &Path, target: &str) {
    println!("cargo:rustc-link-lib=static=btstack");

    for dir in [
        cmake_install_dir,
        &cmake_install_dir.join("lib"),
        cmake_build_dir,
        &cmake_build_dir.join("Release"),
        &cmake_build_dir.join("Debug"),
    ] {
        if dir.exists() {
            println!("cargo:rustc-link-search=native={}", dir.display());
        }
    }

    if target.contains("windows") {
        println!("cargo:rustc-link-lib=winusb");
        println!("cargo:rustc-link-lib=setupapi");
        println!("cargo:rustc-link-lib=ws2_32");
        println!("cargo:rustc-link-lib=bthprops");
    }

    if target.contains("linux") {
        println!("cargo:rustc-link-lib=usb-1.0");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=rt");
        println!("cargo:rustc-link-lib=m");
    }
}

fn build_local_stub() {
    cc::Build::new()
        .file("src/stub_btstack.c")
        .include("include")
        .compile("btstack_stub");
}
