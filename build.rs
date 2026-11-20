//! Embeds the icon and the version info into the exe on Windows.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    embed_icon()?;
    Ok(())
}

#[cfg(windows)]
fn embed_icon() -> Result<(), Box<dyn std::error::Error>> {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("FileDescription", "Veritas");
    res.set("ProductName", "Veritas");
    res.compile()
        .map_err(|err| format!("failed to embed assets/icon.ico: {err}"))?;
    Ok(())
}
