use std::env;

const C_FILES: &[&str] = &[
    "qhull/src/libqhull_r/global_r.c",
    "qhull/src/libqhull_r/stat_r.c",
    "qhull/src/libqhull_r/geom2_r.c",
    "qhull/src/libqhull_r/poly2_r.c",
    "qhull/src/libqhull_r/merge_r.c",
    "qhull/src/libqhull_r/libqhull_r.c",
    "qhull/src/libqhull_r/geom_r.c",
    "qhull/src/libqhull_r/poly_r.c",
    "qhull/src/libqhull_r/qset_r.c",
    "qhull/src/libqhull_r/mem_r.c",
    "qhull/src/libqhull_r/random_r.c",
    "qhull/src/libqhull_r/usermem_r.c",
    "qhull/src/libqhull_r/userprintf_r.c",
    "qhull/src/libqhull_r/io_r.c",
    "qhull/src/libqhull_r/user_r.c",
    "src/ext.c",
];

// These are header files we want to watch for changes too
const HEADER_FILES: &[&str] = &[
    "qhull/src/libqhull_r/libqhull_r.h",
    "qhull/src/libqhull_r/qhull_ra.h",
    "qhull/src/libqhull_r/stat_r.h",
    "qhull/src/libqhull_r/user_r.h",
    "qhull/src/libqhull_r/mem_r.h",
    "qhull/src/libqhull_r/qset_r.h",
    "qhull/src/libqhull_r/random_r.h",
    "qhull/src/libqhull_r/io_r.h",
    "qhull/src/libqhull_r/merge_r.h",
    "qhull/src/libqhull_r/poly_r.h",
    "qhull/src/libqhull_r/geom_r.h",
];

fn main() {
    let mut build = cc::Build::new();
    build.include("qhull/src/")
        // .flag("-O3")
        .pic(true)
        .flag("-ansi")
        .flag("-Wno-unused-but-set-variable");
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        build.flag("-DWASM").flag("-flto");
        build.flag(&format!("--sysroot={}", env::var("WASM_SYSROOT").unwrap()));
        println!("cargo:rerun-if-env-changed=WASM_SYSROOT");
    }
    for c_file in C_FILES {
        build.file(c_file);
    }
    for header_file in HEADER_FILES {
        println!("cargo:rerun-if-changed={}", header_file);
    }
    build.compile("qhull_r");
}
