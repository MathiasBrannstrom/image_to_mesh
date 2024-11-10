use image_to_mesh::{create_and_save_mesh_from_image, Params};
use std::env;
use std::fs;
use std::path::Path;
use std::error::Error;

fn process_image(image_path: &Path) -> Result<(), Box<dyn Error>> {
    let save_path = image_path.with_extension("obj");
    let img = image::open(image_path)?;
    create_and_save_mesh_from_image(&img, save_path.to_str().unwrap(), Params::default())?;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <image_path_or_directory>", args[0]);
        std::process::exit(1);
    }

    let input_path = Path::new(&args[1]);

    if input_path.is_dir() {
        for entry in fs::read_dir(input_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("png") {
                if let Err(e) = process_image(&path) {
                    eprintln!("Error processing {}: {}", path.display(), e);
                }
            }
        }
    } else if input_path.is_file() {
        if input_path.extension().and_then(|s| s.to_str()) == Some("png") {
            if let Err(e) = process_image(input_path) {
                eprintln!("Error processing {}: {}", input_path.display(), e);
                std::process::exit(1);
            }
        } else {
            eprintln!("Error: The file is not a PNG image.");
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: The path is neither a file nor a directory.");
        std::process::exit(1);
    }
}
