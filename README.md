# Image Viewer

Image Viewer is a Mac application for looking through a folder of pictures and movies. You can open one file large on the screen, play a movie or an animation, and make a few simple changes to a picture.

A saved change is a new file beside the original. The original is left as it was.

It runs on macOS 13 or later. You install it yourself from this page.

## Install

You do this once. You do not need to know how to program.

1. Install the free [Rust](https://rustup.rs) tools. The page gives you one command to paste into Terminal. You only do this the first time.
2. Download this project. In Terminal:

   ```bash
   git clone https://github.com/schleising/rust-image-viewer.git
   cd rust-image-viewer
   ```

3. Build and install the application:

   ```bash
   ./scripts/install-app.sh
   ```

The first run needs an internet connection. It downloads the pieces the application is built from, and it can take several minutes.

When it finishes, **Image Viewer** is in your Applications folder. Open it from there, the same way you open any other application.

If the Mac says it cannot be opened because the developer cannot be verified, right-click Image Viewer, choose Open, and then choose Open again.

To install a newer copy later, quit Image Viewer first. An application that is already open keeps running the old copy. Run the script again, then open Image Viewer.

## Using it

Starting Image Viewer with nothing selected opens your Pictures folder.

You can also double-click a picture or movie in Finder, or choose Image Viewer from Open With. That file opens straight away.

### The folder

The window shows a grid, eight files across.

- A blue folder icon is a folder. Click it to look inside. The first item, when there is one, is the folder you came from.
- A picture fills in once it has been read. Until then the square shows a faint loading mark.
- A movie shows a frame from a little way into the clip, with a small play mark in the corner. An animated GIF or WebP has the same mark. An ordinary picture does not.
- Click an item, or highlight it with the arrow keys and press Return, to open it.

### A picture or a movie

- The scroll wheel zooms towards the place under the pointer. Drag to move a zoomed picture.
- Double-click, or press Return, for full screen. Press Return again to come back to the window.
- The Up arrow goes back to the folder.
- The Left and Right arrows go to the previous and next file.
- Escape quits. If a panel is open, Escape closes the panel instead.
- The Space bar pauses and resumes a movie or an animation.

On a movie, move the pointer and a progress bar appears along the bottom. Click it to jump to that part of the movie. It fades shortly after the pointer stops. Hold Shift and press Left or Right to skip a few seconds. Hold Shift and press Up or Down to change the volume. Shift-M mutes the sound.

Hold Control and drag to draw a box on a picture. The Show menu can zoom to that box, or crop the picture to it.

### Changes

The Image menu has adjustments such as colour, contrast, brightness, and sharpen. Adjust Image opens sliders for colour, contrast, and brightness together.

A star at the end of the window title means the picture has been changed and not saved. Choose File → Save. The new file is placed next to the original, with a name like `Holiday - Modified 26-10-18 14.03.09.png`. An animation is saved as a GIF. Movies are for watching; the Image menu does not edit them.

Command-Z undoes the last change.

## If something goes wrong

A short message appears in the window, and Image Viewer keeps running. A longer note is written to:

```text
~/Library/Logs/ImageViewer/ImageViewer.log
```

In Finder, choose Go → Go to Folder and paste that path. You can also open the file in Console.

## Changing the application

How Image Viewer is supposed to behave, and how it is built, is written in [Design/RustPort.md](Design/RustPort.md). Start there if you want to contribute.
