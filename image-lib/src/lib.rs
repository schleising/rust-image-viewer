use std::error::Error;
use std::path::{Path, PathBuf};
use std::io;

// Create error type for NoParentPath, NoFileName and ParentPathDoesNotExist
#[derive(Debug)]
pub enum ImageError {
    NoParentPath,
    NoFileName,
    ParentPathDoesNotExist,
    InvalidFileExtension,
}

// Implement the Error trait for the ImageError type
impl Error for ImageError {}

// Implement the Display trait for the ImageError type
impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ImageError::NoParentPath => write!(f, "No parent path found"),
            ImageError::NoFileName => write!(f, "No file name found"),
            ImageError::ParentPathDoesNotExist => write!(f, "Parent path does not exist"),
            ImageError::InvalidFileExtension => write!(f, "Invalid file extension"),
        }
    }
}

// Allowed file extensions
const ALLOWED_FILE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "gif"];

// Function to create a thumbnial path from a given path
// The function returns the path to the thumbnail
fn create_thumbnail_path(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    // Create a variable to store the parent path
    let parent_path: &Path = path.parent()
        .ok_or(ImageError::NoParentPath)?;

    // Check if the parent path exists
    if !parent_path.exists() {
        return Err(Box::new(ImageError::ParentPathDoesNotExist));
    }

    // Add a new folder called thumbnails to the parent path
    let thumbnail_path = parent_path.join("thumbnails");

    // Create the thumbnail path
    std::fs::create_dir_all(&thumbnail_path)?;

    // Return the thumbnail path
    Ok(thumbnail_path)
}

// Function which takes a path to an image file and resizes it to a thnumbnail storing it in a new folder called thumbnails
// The function returns the path to the thumbnail
pub fn create_thumbnail(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    // Check if the path exists
    if !path.exists() {
        return Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "Path does not exist")));
    }

    // Check if the path is a file
    if !path.is_file() {
        return Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "Path is not a file")));
    }

    // Check if the file extension is allowed
    match path.extension() {
        Some(file_extension) => {
            if !ALLOWED_FILE_EXTENSIONS.contains(&file_extension.to_str().unwrap()) {
                return Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "File extension not allowed")));
            }
        },
        None => return Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "No file extension found"))),
    };

    // Get the file name from the path
    let file_name = path.file_name()
        .ok_or(io::Error::new(io::ErrorKind::NotFound, "No file name found"))?;

    // Call the create_thumbnail_path function
    let thumbnail_path = create_thumbnail_path(&path)?;

    // Join the thumbnail path with the file name
    let thumbnail_path_with_file_name = thumbnail_path.join(file_name);

    // Open the image
    let image = image::open(&path)?;

    // Resize the image to a thumbnail
    let thumbnail = image.thumbnail(128, 128);

    // Save the thumbnail
    thumbnail.save(&thumbnail_path_with_file_name)?;

    // Return the thumbnail path as a string
    Ok(thumbnail_path_with_file_name)
}

// Test the create_thumbnail_path function with a good path and a bad path
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_thumbnail_path() {
        // Create a path to a file which exists
        let path = Path::new("../test-image/22272455.png");

        // Call the create_thumbnail_path function
        let thumbnail_path = create_thumbnail_path(&path).unwrap();

        // Check if the thumbnail path is correct
        assert_eq!(thumbnail_path, Path::new("../test-image/thumbnails"));
    }

    // Create a test for the create_thumbnail function where the parent path does not exist
    #[test]
    fn test_create_thumbnail_bad_parent_path() {
        // Create a path to a file which does not exist
        let path = Path::new("../path_which_does_not_exist/test2.jpg");

        // Call the create_thumbnail_path function
        let thumbnail_path = create_thumbnail_path(&path);

        //  Check the error text is correct
        assert_eq!(thumbnail_path.unwrap_err().to_string(), "Parent path does not exist");
    }
    
    // Create a test for the create_thumbnail function where the file name does not exist
    #[test]
    fn test_create_thumbnail_bad_file_name() {
        // Create a path to a file which does not exist
        let path = Path::new("../test-image/");

        // Call the create_thumbnail_path function
        let thumbnail_path = create_thumbnail(&path);

        //  Check the error text is correct
        assert_eq!(thumbnail_path.unwrap_err().to_string(), "Path is not a file");
    }

    // Create a test for the create_thumbnail function where the file name does exist
    #[test]
    fn test_create_thumbnail_good_file_name() {
        // Create a path to a file which does not exist
        let path = Path::new("../test-image/22272455.png");

        // Call the create_thumbnail function
        let thumbnail_path = create_thumbnail(&path).unwrap();

        //  Check the error text is correct
        assert_eq!(thumbnail_path, Path::new("../test-image/thumbnails/22272455.png"));
    }
}
