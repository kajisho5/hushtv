//! data/launcher/tvhome.apk（CI が cleanyourtv の launcher から作る）があれば、アプリに同梱する。
fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_launcher)");
    let apk =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/launcher/tvhome.apk");
    println!("cargo::rerun-if-changed={}", apk.display());
    if apk.is_file() {
        println!("cargo::rustc-cfg=has_launcher");
    }
}
