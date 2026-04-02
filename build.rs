use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};

fn main() {
    println!("cargo:rerun-if-changed=.github/assets/icon.svg");
    println!("cargo:rerun-if-changed=Cargo.toml");

    if cfg!(target_os = "windows") {
        if let Err(error) = compile_windows_resources() {
            panic!("failed to compile Windows resources: {error}");
        }
    }
}

fn compile_windows_resources() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let svg_path = manifest_dir.join(".github").join("assets").join("icon.svg");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let icon_path = out_dir.join("usb_mirror_sync.ico");
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());

    write_icon_from_svg(&svg_path, &icon_path)?;

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(
        icon_path
            .to_str()
            .ok_or("generated icon path was not valid UTF-8")?,
    );
    resource.set("FileDescription", "USB Mirror Sync");
    resource.set("InternalName", "usb_mirror_sync");
    resource.set("OriginalFilename", "usb_mirror_sync.exe");
    resource.set("ProductName", "USB Mirror Sync");
    resource.set("CompanyName", "Rad");
    resource.set("LegalCopyright", "Copyright (C) 2026 Rad");
    resource.set("FileVersion", &version);
    resource.set("ProductVersion", &version);
    resource.compile()?;
    Ok(())
}

fn write_icon_from_svg(svg_path: &Path, icon_path: &Path) -> Result<(), Box<dyn Error>> {
    let svg_data = fs::read(svg_path)?;
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &options)?;
    let size = tree.size();

    let mut icon_dir = IconDir::new(ResourceType::Icon);
    for side in [16u32, 24, 32, 48, 64, 128, 256] {
        let mut pixmap = Pixmap::new(side, side).ok_or("failed to allocate pixmap")?;
        let transform =
            Transform::from_scale(side as f32 / size.width(), side as f32 / size.height());

        let mut pixmap_mut = pixmap.as_mut();
        resvg::render(&tree, transform, &mut pixmap_mut);

        let image = IconImage::from_rgba_data(side, side, pixmap.take());
        icon_dir.add_entry(IconDirEntry::encode(&image)?);
    }

    let mut file = File::create(icon_path)?;
    icon_dir.write(&mut file)?;
    Ok(())
}
