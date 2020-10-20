use std::{env, path::PathBuf};

fn main() {
    // Tell rustc to link to epan
    println!("cargo:rustc-link-lib=wireshark");

    // Invalidate if wrapper.h changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // bindgen stuff
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        // Specify include folders
        // Might differ on other machines
        .clang_arg("-I/usr/include/wireshark")
        .clang_arg("-I/usr/include/glib-2.0")
        .clang_arg("-I/usr/lib64/glib-2.0/include")
        // Include plugin specific definitions
        .clang_arg("-DHAVE_PLUGINS")
        // Blacklist anything using u128
        .blacklist_function("g_test_log_msg_free")
        .blacklist_function("g_test_log_buffer_pop")
        .blacklist_function("strtold")
        .blacklist_function("qecvt")
        .blacklist_function("qfcvt")
        .blacklist_function("qgcvt")
        .blacklist_function("qecvt_r")
        .blacklist_function("qfcvt_r")
        .blacklist_function("g_assertion_message_cmpnum")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Error creating bindgen bindings");

    // Write to bindings.rs
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
