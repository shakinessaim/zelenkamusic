fn main() {
    slint_build::compile("ui/main.slint").expect("Could not compile the Slint interface");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/zelenka.ico");
        resource.set("ProductName", "ZelenkaMusic");
        resource.set("FileDescription", "ZelenkaMusic local music player");
        resource.set_manifest_file("assets/windows.manifest");
        resource
            .compile()
            .expect("Could not compile Windows application resources");
    }
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=assets");
}
