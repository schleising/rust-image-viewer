# Ideas

[RustPort.md](RustPort.md) is what Image Viewer does today. This note is a menu of ways to extend it. Nothing here is scheduled, and nothing here changes the current behaviour until it is chosen and written into the specification.

The application already has the hooks that make many of these small: a transform registry that builds the Image menu, a transition registry that Next and Previous already consult, two image layers that hold the previous picture until the next has loaded, and AVFoundation for movie frames. Prefer those hooks over a new subsystem.

A few items below reopen a decision that RustPort.md currently lists as a non-goal (a preferences window, redo, HEIC, saving an edited animation as WebP, notarization). They are here because they are still worth considering, not because that decision has changed.

## Looking through a folder

The grid is eight columns, sorted by name, with one highlight. That is enough for a small folder and awkward for a large one.

- **Remember the last folder.** Launch still opens Pictures when nothing is being handed over. Remembering the folder from the previous quit would make the application feel like it was left where you were. Folder memory already stores the highlight and the scroll position for folders you have visited in this session; this would keep one path across launches.
- **Sort by date, size, or kind.** Date taken is more useful than the file name for photographs. Kind would group stills, animations, and movies. The order should stay case-insensitive by name until the user asks for something else.
- **Type the first letters of a name.** Jump the highlight to the first label that starts with what was typed, the way Finder does. The labels are already the file stem.
- **A count in the title.** `Holiday — 12 of 40` while a file is open, and the number of pictures and movies while browsing. The title already carries the folder stem, the file stem, and the dirty star.
- **Change how many columns there are.** Eight is right for the opening window. A wider window, or a folder of portraits, wants fewer or more. The thumbnail pixel size is already the column width times the screen scale, so a different column count falls out of the layout.
- **Decode what is on screen first.** The thumbnail pool walks the folder from the top. In a folder of movies that means frames for files you cannot see yet. Prefer the rows inside the scroll view, then the rows just outside it.
- **Keep thumbnails on disk.** Reopening Pictures currently decodes every file again. A cache keyed by path, file size, modification time, and pixel size would make the second visit immediate. Leave the folder and the cache entry is still valid until the file changes. A failed movie frame should not be cached as success.
- **Show a folder by its contents.** The blue folder icon is clear, and it tells you nothing about what is inside. A small stack of the first few pictures in that folder, behind the folder shape, would help. Keep the blue folder when the directory is empty or unreadable.
- **Put the movie length on the badge.** The play mark says it is a movie. A short duration, such as `1:24`, in the same corner would say whether it is a clip or an hour. The duration is already known once the thumbnail frame has been read.
- **Hover a thumbnail to peek.** The grid does not play animations, which is right for a quiet folder. Hovering an animation or a movie for a moment could play a short loop in that cell only, and stop when the pointer leaves.
- **Filter the grid.** A text field that hides names which do not contain what you typed. Hidden files that already match a suffix stay included. Clearing the field restores the full list and the highlight.
- **Select more than one file.** Shift-click and Shift-arrows extend the highlight into a range. The useful commands on a range are slideshow, reveal in Finder, move to Trash, and "apply the last edit to each". Opening still opens one file.
- **Drag a folder or a file onto the window.** Dropping a folder lists it. Dropping a file opens it, the same way Finder already does. There is no drop handler today.
- **Recursive view.** One grid of every picture and movie under the current folder, still skipping directories whose names start with a dot. This needs the on-screen-first thumbnail rule, or a large tree will sit on loading marks for a long time.
- **Recent files and recent folders.** A short list on the File menu, separate from the grid. Useful when Pictures is not where the work is.

## A picture on screen

Fit, zoom, pan, and a single selection rectangle are in place. The two image layers already cross-fade in spirit: the old picture stays until the new one has loaded.

- **Crossfade.** The transition registry ships an instant cut, and the two layers are already mounted. A crossfade is an opacity animation between them. Video and folder changes stay hard cuts, as the specification already says. A later setting would choose the transition; until there is a preferences window, a menu item under Show is enough.
- **Other transitions.** A wipe or a blend is a function from two frames and a time to one frame, served through the same image address. The registry is the place to add them. Keep them short. A slow transition makes Left and Right feel broken, and Next is ignored while one is running.
- **Slideshow.** Advance on a timer, with the chosen transition, and pause on Space. Esc or Up leaves it. The interval is the only new number.
- **Actual size.** A command that sets the zoom so one image pixel is one screen pixel, then the existing pan. Reset Zoom already returns to fit.
- **Fit width, fit height.** Useful for a long screenshot or a portrait. Fit-to-window remains the default when a file opens.
- **Pinch to zoom.** The scroll wheel already zooms around the pointer. A trackpad pinch should use the same anchor.
- **Swipe to the next file.** A horizontal trackpad swipe moves through the folder. It has to be distinct from a pan, so it should only start when the picture is still fitted, or when the swipe begins at the edge.
- **A zoom figure.** A small number, such as `140%`, that appears while the wheel is moving and fades with the same patience as the movie progress bar. Relative to the fitted size, which is what the viewport already stores.
- **Rotate and flip as edits.** Opening a JPEG already honours the camera orientation, so phone pictures come in upright. Rotate left, rotate right, and flip are still missing as edits. They belong in the transform registry, they apply to every frame of an animation, and they are one undo step. A lossless JPEG rotate that rewrites the file in place is a different, later command; the normal path is pixels and Save.
- **Straighten.** A small angle in the adjust dialog, or its own slider, cropping to the upright rectangle. Preview works the way colour, contrast, and brightness already preview.
- **Hold a key to see the original.** While the key is down, show the saved frame list instead of the edited one. The undo stack already has the history; the saved list is the `clean` arc. Releasing the key returns to the edit. This is the fastest way to judge denoise or super-resolution.
- **Before and after, side by side.** The same two lists, both fitted, same zoom. Heavier than the hold-key, and worth it for super-resolution because the size has changed.
- **A loupe.** A circle that follows the pointer and shows the pixels underneath at actual size. It should not steal the drag that pans.
- **Copy the picture.** Command-C puts the current frame on the clipboard as an image. Crop to Rect already knows the rectangle; copy can use the selection when there is one, and the whole frame otherwise.
- **Reveal in Finder, rename, duplicate, move to Trash.** These are file commands, not pixel commands. Trash needs a confirm step. Rename should refuse to leave the supported suffixes, or it should update the suffix and the kind together. None of these should run while a save is in progress.
- **Ask before quitting a dirty picture.** Escape quits even when the title has a star. A confirm step, with Save, Discard, and Cancel, would match what people expect. Cancel stays on the picture.
- **The position in the folder as an overlay.** `12 / 40` in a corner, fading like the progress bar, so full screen still tells you where you are.

## Editing

The Image menu is a registry. A new edit is a struct and one line. The ideas below are edits that people reach for and that the menu does not have yet.

- **Levels and curves.** The adjust dialog is three factors. Levels (black point, white point, midpoint per channel) cover the cases auto-contrast is too blunt for. Curves can wait until levels exist.
- **Temperature and tint.** A white-balance pair next to colour, contrast, and brightness. Preview on the current frame, commit on every frame, one undo step. That is the adjust-dialog rule already.
- **Resize.** A dialog with a pixel width, the height following the aspect unless it is unlinked. Super-resolution is the quality path for making an image larger; resize is the plain path for making one smaller, or for an exact size.
- **Choose the super-resolution scale.** The menu always asks for 4. The models for 2 and 3 are already accepted by the loader. A submenu, or a dialog, would let a large photograph use 2 without a 16-times pixel count. Show a busy state while it runs. The stage currently refuses another edit and does not say why.
- **Cancel a long edit.** Denoise and super-resolution block further edits until they finish. A Cancel item, or Escape while busy, should drop the result if it arrives late, the same way a stale thumbnail is dropped. The generation counter is the tool for that.
- **Progress for a long edit.** A thin bar or a short message, not a dimmed stage. "Super-resolution, 4×" is enough. The log already records the failure.
- **Sharpen with a radius.** Unsharp mask has fixed numbers. The same dialog pattern as Adjust Image, with radius, amount, and threshold, would make it useful on both a small thumbnail-sized crop and a large photograph.
- **Vignette, grain, and a simple border.** Small registry entries. A border changes the canvas size and should clear the selection and return to fit, the way crop already does.
- **Red-eye.** Needs a selection or a click. Easy to do badly. Only worth it if it is limited to a small radius around a click and one undo step.
- **Save as, with a format.** Save currently writes a PNG for a still and a GIF for an animation, under a new name. A format choice would add JPEG with a quality slider, and would keep PNG when the picture has transparency. The name pattern can stay.
- **Save animated WebP.** Playback of animated WebP works. A saved edit becomes a GIF because the encoder in use writes one frame. A real animated-WebP writer would round-trip an edited WebP without leaving the format. This is an explicit non-goal until that encoder exists.
- **Redo.** Undo keeps sixteen frame lists and there is no redo. Command-Shift-Z is the usual key. A redo stack clears when a new edit is made. This is an explicit non-goal today; it is still the first thing people try after Undo.
- **Apply the last edit to other files.** After a denoise or a resize that came out well, run that same command on the rest of the selection, or on every picture in the folder. Each file gets its own saved copy. Skip movies. Show a count of how many were written and how many failed, and keep going after one failure.
- **Crop handles.** Crop to Rect uses the selection as it was drawn. Handles on the selection, and a rule-of-thirds overlay while dragging, would make the crop deliberate. The rectangle stays in image pixels.

## Movies

Playback, pause, skip, volume, mute, and the progress bar are in place. The movie stays on screen while the next one loads.

- **Step one frame.** Comma and period, or the arrow keys with a modifier that is still free, move a paused movie by one frame. Shift-Left and Shift-Right already jump five seconds, so they should stay as they are.
- **Playback speed.** 0.5× and 2× from the Video menu. Pitch can stay unchanged or can follow; unchanged is less surprising for a voice.
- **Save the current frame.** Write a PNG of the frame on screen, using the same name pattern as Save. This is the bridge from a movie to the still-image edits.
- **Pick the thumbnail frame.** The grid uses 15% of the duration, which avoids a black leader and is sometimes the wrong moment. A command, while the movie is paused, stores "use this time for the thumbnail" beside the cache entry. Until that exists, 15% remains the rule.
- **Scrub with the keyboard.** Left and Right move between files. While the progress bar is visible, a modifier plus Left and Right can seek by a small fraction of the duration. The five-second skip stays on Shift.
- **J, K, and L.** The usual shuttle: J plays backwards, K pauses, L plays forwards, and repeating L speeds up. This is only worth it if people are using the application to review clips. It should not steal J, K, and L from anything else; nothing uses them today.
- **Remember volume and mute per movie, and a default.** Volume starts at 1 and mute follows the document. A folder of quiet clips wants the last choice to stick.
- **Loudness on the thumbnail.** Optional, and easy to overdo. A single number, or nothing. The duration text is the more useful mark.
- **Subtitles.** If a `.srt` or a `.vtt` sits beside the movie with the same stem, offer it. The `<video>` element can take a track. Do not fetch subtitles from the network.
- **A loop region.** Set an in point and an out point, and play that span. Useful for an animation study or a short gesture in a long file. Clearing the points returns to the whole movie.
- **Keep the progress bar while the pointer is over it.** It hides 500 ms after the pointer stops, including when the pointer is resting on the bar you are about to click. If the pointer is over the bar, leave it up.

## Animations

GIF and animated WebP play, Space pauses them, and an edit runs on every frame. The grid shows the first frame.

- **Step frames while paused.** The same idea as a movie, on the frame list that is already in memory.
- **A frame counter.** `4 / 24` while an animation is open, in the same fading corner as the zoom figure.
- **Change one frame's delay.** The info panel lists the total duration. Editing a single delay, then saving, is how a flicker gets fixed. The undo entry is the whole frame list, as it is for every other edit.
- **Delete a frame, or export one frame.** Delete drops that frame and keeps the rest. Export writes one PNG. Both are awkward once the animation is over the memory limit and has already been reduced to a still; in that case the commands should explain that only the first frame is loaded.
- **Onion skin.** While paused, show the previous and next frames faintly over the current one. It is a viewing aid for checking motion, not an edit.

## Information

Image Information lists format, description, size, mode, and, for an animation, frame count, loop, and duration.

- **When the photograph was taken, and with what.** Date, camera, lens, focal length, aperture, shutter, ISO, from the file's metadata. Show "—" when the field is absent. Do not invent a value.
- **Where it was taken.** If the file has a location, show it as numbers and offer "Open in Maps". Offer "Remove location from the saved copy" on Save, off by default, so a shared PNG does not carry a house address by surprise.
- **File size and the path.** The info panel describes the image. The file size and the folder path answer a different question and belong there too.
- **The colour profile.** Most files are sRGB in practice. When a profile is present and it is not sRGB, say so. Converting for display is a larger step; naming the profile is the small one.
- **A histogram in the adjust dialog.** One luminance histogram, updated as the sliders move, using the preview that is already computed. It tells you when contrast has clipped.

## Kinds of file

The suffixes today are GIF, JPEG, PNG, WebP, MP4, and MOV.

- **HEIC and HEIF.** This is the format an iPhone writes. It is an explicit non-goal. On a Mac the practical reader is ImageIO, which is already next door to the AVFoundation code. Treat a burst or a live photo as the still, and offer the paired movie when one is sitting beside it.
- **RAW.** DNG, and the common camera raw types, via the system raw decoder. Open as a still, edits on the decoded RGB, Save writes a PNG or JPEG. Do not claim to write the raw file back.
- **AVIF.** Worth it once HEIC works, because the decoder path is similar and the files are showing up in downloads.
- **TIFF and BMP.** TIFF shows up in scans. BMP is rare and cheap. Both are stills.
- **SVG.** Render at the window's pixel size, and again when the window changes size, so zoom stays sharp. Save of an edited SVG is a PNG of that rendering, not a rewritten SVG.
- **A PDF page.** Open the first page as a still. A page control, like the frame counter, can move through the rest. This is a viewer, not a PDF editor.
- **Drop a list of files from another application.** The existing Open With path handles one file. A list can open the first and keep the rest as the Previous and Next order, even when they do not share a folder.

## The window

- **A preferences window.** An explicit non-goal. The settings that would justify it are the startup folder, the slideshow interval, the transition, the column count, whether Trash asks, and whether a dirty quit asks. Until those exist, a menu item is a smaller home for each one. If a preferences window does arrive, store the file in the usual Application Support location and keep the window optional.
- **Full screen on the Show menu.** Return and double-click are the gestures. A menu item with the same command would show the shortcut beside it, which is how people discover Return. The specification currently has no such item on purpose.
- **A short help panel.** The shortcuts that are easy to forget: Return, Up, Left, Right, Space, Control-drag, the Shift keys on a movie. One screen, Esc closes it. Not a manual.
- **Confirm the window size and the last display.** The window always opens at 1024×768 near the top-left. Restoring the last frame, on the display that held it, is the ordinary Mac behaviour.
- **Two files at once.** A second window, or a split in this window, showing two pictures from the folder with the same zoom. Useful for picking between two shots. The single-window rule is part of the current design; a split keeps that rule, a second window breaks it on purpose.
- **Quick Look from the grid.** Space on a highlighted cell shows a large preview and another Space returns to the grid, without the full viewer. Return still opens the file for editing and for movies. This depends on Space being free in the browser; it is.

## Making it faster and calmer

- **Open a huge photograph without decoding all of it.** A 40 megapixel JPEG does not need every pixel to fit a laptop window. Decode a smaller version for the first view, and decode the full size when the user zooms past fitted, or when they edit. Edits must run on the full pixels, or the saved file would be the small one.
- **Do not hold every animation frame if the user has not asked to edit.** Playback can stay as it is. The 256 MiB rule already collapses a huge animation to its first frame. A middle path is to keep the frames on disk and decode a window of them around the one on screen. That is a large change. The small change is a clear message when the collapse happens, not only a line in the log.
- **Say when the application is busy.** A long denoise looks like a freeze because further edits are refused and the picture does not change. The message area already exists. Use it.
- **Thumbnail failure in the cell.** A picture that cannot be read already shows the error. A movie that cannot be read falls back to the video icon, which looks like success. A small mark on that icon, or the error text the pictures use, would stop a broken file from looking like a normal movie.
- **Keep the grid scroll stable when thumbnails arrive.** The layout should not jump as each cell replaces the loading mark. The cells already have a fixed square, so this is a check rather than a redesign.
- **Warm the next file.** While a picture is on screen, decode the next file's fitted view in the background. Left and Right would then cut to a picture that is already ready. The hold-until-loaded behaviour stays for the case where the user is faster than the decode.

## Sharing it with other people

The README asks a visitor to install Rust and run the script. That is honest, and it is a tall step for someone who only wants the application.

- **A signed, notarized build on the GitHub releases page.** Download, move to Applications, open. No Rust tools. This reopens notarization, which is a non-goal of the project as specified. It is also the change that makes the README's audience able to install Image Viewer without a compiler.
- **A short release note.** What changed, in the same plain language as the README. The design notes stay the specification.
- **A way to report a bad file.** The log path is in the README. A menu item that reveals the log in Finder would save the Go to Folder step. Attach the file only if the person chooses to; do not upload it.

## Small polish

- The play badge could show a pause glyph while an animation is playing in the viewer, if the hover-peek idea lands. In the grid it stays a play mark.
- The progress bar's click target is taller than the painted line. That is correct. The same generosity should apply if keyboard scrubbing is added, so a near miss still seeks.
- Middle-elision of long names is already there. A tooltip with the full file name would cover the cases the elision hides.
- The parent row could show the parent folder's own thumbnail treatment once folders can show their contents, so going up looks like the folder you are going to.
- Saving could reveal the new file in the grid, highlighted, when you return to the folder. Today the new file appears in name order on the next listing.
- The About window already has the version. A "what's new" line there is enough if full release notes feel too heavy.

## What to pick first

If only a few of these are built, the ones that change daily use the most, and that fit the code that is already there, are:

1. Rotate and flip, as registry edits.
2. A visible busy message, and Cancel, for denoise and super-resolution.
3. Thumbnails for the rows on screen first, then a disk cache.
4. Crossfade, using the two layers and the transition registry.
5. Date, camera, and file size in Image Information.
6. Reveal in Finder, and move to Trash with a confirm step.
7. A signed download, if other people besides the author are going to install it.
