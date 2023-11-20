use std::{error::Error, path::Path, path::PathBuf};

// Function to create a thumbnial path from a given path
// The function returns the path to the thumbnail
fn create_thumbnail_path(path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    // Create a variable to store the parent path
    let parent_path: &Path;

    // Get the parent path of the given path
    match path.parent() {
        Some(path) => parent_path = path,
        None => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "No parent path found"))),
    }

    // Check if the parent path exists
    if !parent_path.exists() {
        // Print an error message
        println!("Parent path does not exist");

        // Return an error
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Parent path does not exist")));
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
    // Create a variable to store the thumbnail path
    let thumbnail_path: PathBuf;

    // Call the create_thumbnail_path function
    match create_thumbnail_path(&path) {
        Ok(path) => thumbnail_path = path,
        Err(error) => return Err(error),
    }

    // Get the file name from the path
    let file_name = match path.file_name() {
        Some(file_name) => file_name,
        None => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "No file name found"))),
    };

    // Join the thumbnail path with the file name
    let thumbnail_path_with_file_name = thumbnail_path.join(file_name);

    // Create a variable to store the image
    let image: image::DynamicImage;

    // Open the image
    match image::open(&path) {
        Ok(img) => image = img,
        Err(error) => return Err(Box::new(error)),
    }

    // Create a variable to store the thumbnail
    let thumbnail: image::DynamicImage;

    // Resize the image to a thumbnail
    thumbnail = image.thumbnail(256, 256);

    // Save the thumbnail
    match thumbnail.save(&thumbnail_path_with_file_name) {
        Ok(_) => {},
        Err(error) => return Err(Box::new(error)),
    }

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
        let path = Path::new("../tests/test.jpg");

        // Call the create_thumbnail_path function
        let thumbnail_path = create_thumbnail_path(&path).unwrap();

        // Check if the thumbnail path is correct
        assert_eq!(thumbnail_path, Path::new("../tests/thumbnails"));
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
        let path = Path::new("../tests/test2.jpg");

        // Call the create_thumbnail_path function
        let thumbnail_path = create_thumbnail(&path);

        //  Check the error text is correct
        assert_eq!(thumbnail_path.unwrap_err().to_string(), "No file name found");
    }

    // Create a test for the create_thumbnail function where the file name does exist
    #[test]
    fn test_create_thumbnail_good_file_name() {
        // Create a path to a file which does not exist
        let path = Path::new("../tests/test.jpg");

        // Call the create_thumbnail function
        let thumbnail_path = create_thumbnail(&path).unwrap();

        //  Check the error text is correct
        assert_eq!(thumbnail_path, Path::new("../tests/thumbnails/test.jpg"));
    }
}
