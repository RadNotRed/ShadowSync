use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use png::{BitDepth, ColorType, Encoder};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg,
};

const WINDOWS_ICON_SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed=.github/assets/icon.svg");
    println!("cargo:rerun-if-changed=Cargo.toml");

    if let Err(error) = generate_release_assets() {
        panic!("failed to generate release assets: {error}");
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if let Err(error) = compile_windows_resources() {
            panic!("failed to compile Windows resources: {error}");
        }
    }
}

fn generate_release_assets() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let target = env::var("TARGET")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let svg_path = manifest_dir.join(".github").join("assets").join("icon.svg");
    let output_dir = manifest_dir
        .join("target")
        .join("generated-assets")
        .join(target);
    fs::create_dir_all(&output_dir)?;

    let svg_data = fs::read(&svg_path)?;
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &options)?;
    let size = tree.size();

    for side in [16u32, 32, 64, 128, 256, 512, 1024] {
        let rgba = render_svg(&tree, side, size.width(), size.height())?;
        let png_path = output_dir.join(format!("icon_{side}x{side}.png"));
        write_png(&png_path, side, side, &rgba)?;
        if side == 256 {
            let runtime_icon_path = out_dir.join("wizard_icon_256.png");
            write_png(&runtime_icon_path, side, side, &rgba)?;
        }
    }

    let icon_path = output_dir.join("shadowsync.ico");
    write_icon_from_svg(&svg_path, &icon_path)?;

    Ok(())
}

fn compile_windows_resources() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let svg_path = manifest_dir.join(".github").join("assets").join("icon.svg");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let icon_path = out_dir.join("shadowsync.ico");
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());

    write_icon_from_svg(&svg_path, &icon_path)?;

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(
        icon_path
            .to_str()
            .ok_or("generated icon path was not valid UTF-8")?,
    );
    resource.set("FileDescription", "ShadowSync");
    resource.set("InternalName", "shadowsync");
    resource.set("OriginalFilename", "shadowsync.exe");
    resource.set("ProductName", "ShadowSync");
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
    for side in WINDOWS_ICON_SIZES {
        let rgba = render_svg(&tree, side, size.width(), size.height())?;
        let image = IconImage::from_rgba_data(side, side, rgba);
        icon_dir.add_entry(IconDirEntry::encode(&image)?);
    }

    let mut file = File::create(icon_path)?;
    icon_dir.write(&mut file)?;
    Ok(())
}

fn render_svg(
    tree: &usvg::Tree,
    side: u32,
    width: f32,
    height: f32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut pixmap = Pixmap::new(side, side).ok_or("failed to allocate pixmap")?;
    let transform = Transform::from_scale(side as f32 / width, side as f32 / height);

    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(tree, transform, &mut pixmap_mut);
    Ok(pixmap.take())
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let mut encoder = Encoder::new(file, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    Ok(())
}
