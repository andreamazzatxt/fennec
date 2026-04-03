fn main() {
    cc::Build::new()
        .file("src/accel_iokit.c")
        .compile("accel_iokit");

    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}
