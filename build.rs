fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icons/app/app-icon.ico");

        // Embeds the icon as a Win32 resource in the .exe itself, which is what
        // Explorer, the Start menu and the taskbar's pinned entries read. The
        // window icon set in `app.rs` only covers the live window, so both are
        // needed. Resource id 1 is the icon Windows treats as the application's.
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/app/app-icon.ico");
        resource.set("ProductName", "Posture Tracker");
        resource.set("FileDescription", "Posture Tracker");

        if let Err(error) = resource.compile() {
            println!("cargo:warning=unable to embed the Windows icon resource: {error}");
        }
    }
}
