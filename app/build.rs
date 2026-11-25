use std::path::PathBuf;
use std::process::Command;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_swift_compatibility_libs();
    }
}

// As pontes Swift do crate screencapturekit referenciam libswiftCompatibility56, e o
// screencapturekit procura no caminho do Xcode completo. Com só o Command Line Tools,
// a lib fica em <CLT>/usr/lib/swift/macosx.
fn link_swift_compatibility_libs() {
    let Ok(out) = Command::new("xcrun").args(["--find", "swiftc"]).output() else {
        return;
    };
    let swiftc = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if let Some(usr) = swiftc.parent().and_then(|bin| bin.parent()) {
        let dir = usr.join("lib/swift/macosx");
        if dir.exists() {
            println!("cargo:rustc-link-search=native={}", dir.display());
        }
    }
}
