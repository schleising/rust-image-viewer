use std::{fs, path::PathBuf};
use image_lib::create_thumbnail;
use rayon::prelude::*; // Import the necessary traits from the rayon crate

fn main() {
    // Create an empty vector to store the paths to the images
    let mut image_paths: Vec<PathBuf> = Vec::new();

    // Iterate through the entries in the tests folder
    if let Ok(entries) = fs::read_dir("../tests") {
        // Iterate through the entries
        for entry in entries {
            // Unwrap the entry
            if let Ok(entry) = entry {
                // Get the path
                let path = entry.path();
                // Check if the path is a file
                if path.is_file() {
                    // Check if the file is a jpg
                    if let Some("jpg") = path.extension().and_then(std::ffi::OsStr::to_str) {
                        // Add the path to the vector
                        image_paths.push(path);
                    }
                }
            }
        }
    }

    // Map the image paths to thumbnails in a parallel fashion
    let thumbnails: Vec<PathBuf> = image_paths.par_iter().map(|path| create_thumbnail(path).unwrap_or(PathBuf::new())).collect();

    // Print the thumbnails
    println!("Thumbnails: {:?}", thumbnails);

    // Print the number of thumbnails
    println!("Number of thumbnails: {}", thumbnails.len());
}
