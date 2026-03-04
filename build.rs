use std::process::Command;

fn main() {
    // The screencapturekit crate's Swift bridge needs Swift runtime libraries
    // at runtime. The crate's build.rs sets link args but those only apply to
    // the crate itself, not the final binary. We must add rpaths here.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    if let Ok(output) = Command::new("xcode-select").arg("-p").output()
        && output.status.success()
    {
        let xcode = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let swift_55 =
            format!("{xcode}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx");
        let swift_new =
            format!("{xcode}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{swift_55}");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{swift_new}");
    }
}
