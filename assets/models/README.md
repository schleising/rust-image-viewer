# Super-resolution models

The application looks for these files next to this note:

- `fsrcnn_x2.onnx`
- `fsrcnn_x3.onnx`
- `fsrcnn_x4.onnx`

They are the FSRCNN graphs from <https://github.com/Saafke/FSRCNN_Tensorflow>, converted once with `tf2onnx`. The application does not convert them and does not ship the `.pb` files.

Until those files are added, Super Resolution logs an error and leaves the image unchanged. The crate still builds.

When a file is present, inference uses `tract-onnx`:

- Samples are `f32` in the range 0–255 (blob scale 1.0). If the network returns values that all sit at 1.5 or below, they are treated as 0–1 and multiplied by 255.
- Channels are BGR, matching the OpenCV path with no channel swap. The loader swaps RGB to BGR on the way in and back on the way out.
- Layout is tried as NHWC `[1, height, width, 3]`, then NCHW `[1, 3, height, width]`. The attempt whose output is `scale` times the input in each dimension is kept.
- Alpha is not part of the network. It is resampled with nearest neighbour onto the upscaled image.

Record the tensor names and the tolerance here after conversion, and the FSRCNN test will compare a small image against that tolerance. While this note does not name an output tensor, the test stays skipped.
