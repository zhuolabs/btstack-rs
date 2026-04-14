use std::env;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=cmake/btstack-core-only/CMakeLists.txt");
    println!("cargo:rerun-if-changed=cmake/btstack-core-only/btstack_config.h");
    println!("cargo:rerun-if-changed=vendor/btstack");
    println!("cargo:rerun-if-env-changed=CMAKE");
    println!("cargo:rerun-if-env-changed=BTSTACK_CMAKE");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"));
    let vendor_dir = manifest_dir.join("vendor").join("btstack");

    if try_build_vendor_btstack_core(&manifest_dir, &vendor_dir) {
        println!("cargo:rustc-cfg=btstack_vendor_build");
        return;
    }

    panic!(
        "Failed to build BTstack core from vendor directory. Please initialize the BTstack submodule and ensure CMake is available."
    );
}

fn try_build_vendor_btstack_core(manifest_dir: &Path, vendor_dir: &Path) -> bool {
    if !vendor_dir.exists() {
        println!("cargo:warning=BTstack submodule is not initialized");
        return false;
    }

    if let Some(cmake_path) = resolve_cmake_executable() {
        // SAFETY: build scripts may mutate process environment.
        unsafe {
            env::set_var("CMAKE", &cmake_path);
        }
    } else {
        println!("cargo:warning=CMake is not available");
        return false;
    }

    let source_dir = manifest_dir.join("cmake").join("btstack-core-only");
    let cmake_lists = source_dir.join("CMakeLists.txt");

    if !cmake_lists.exists() {
        println!(
            "cargo:warning=Expected CMakeLists.txt at {} but it is missing",
            cmake_lists.display()
        );
        return false;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let cmake_out_dir = out_dir.join("btstack-cmake");

    let build_result = catch_unwind(AssertUnwindSafe(|| {
        let mut config = cmake::Config::new(&source_dir);
        config
            .out_dir(&cmake_out_dir)
            .profile("Release")
            .define("BUILD_SHARED_LIBS", "OFF")
            .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
            .define("BTSTACK_ROOT", vendor_dir)
            .build_target("btstack");

        config.build()
    }));

    let cmake_install_dir = match build_result {
        Ok(path) => path,
        Err(_) => {
            println!("cargo:warning=Failed to configure/build BTstack core via cmake crate");
            return false;
        }
    };

    let cmake_build_dir = cmake_out_dir.join("build");
    emit_btstack_link_settings(&cmake_install_dir, &cmake_build_dir);
    true
}

fn resolve_cmake_executable() -> Option<PathBuf> {
    if let Some(explicit) = env::var_os("BTSTACK_CMAKE").or_else(|| env::var_os("CMAKE")) {
        let explicit = PathBuf::from(explicit);
        if command_works(&explicit, "--version") {
            return Some(explicit);
        }
    }

    if command_works(Path::new("cmake"), "--version") {
        return Some(PathBuf::from("cmake"));
    }

    #[cfg(windows)]
    {
        let candidate = find_visual_studio_bundled_cmake_from_compiler()?;
        if command_works(&candidate, "--version") {
            return Some(candidate);
        }
    }

    None
}

fn emit_btstack_link_settings(cmake_install_dir: &Path, cmake_build_dir: &Path) {
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
}

fn command_works(program: &Path, arg: &str) -> bool {
    std::process::Command::new(program)
        .arg(arg)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn find_visual_studio_bundled_cmake_from_compiler() -> Option<PathBuf> {
    let compiler = cc::Build::new().get_compiler();
    let compiler_path = compiler.path();

    let vc_dir = compiler_path
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "VC"))?;

    vc_dir
        .parent()?
        .join(r"Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe")
        .canonicalize()
        .ok()
}
