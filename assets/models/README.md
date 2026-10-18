# Super-resolution models

The application looks for these files next to this note:

- `fsrcnn_x2.onnx`
- `fsrcnn_x3.onnx`
- `fsrcnn_x4.onnx`

They are the FSRCNN graphs from <https://github.com/Saafke/FSRCNN_Tensorflow>, converted once with `tf2onnx`. The application does not convert them and does not ship the `.pb` files.

Until those files are added, Super Resolution logs an error and leaves the image unchanged. The crate still builds.

Each graph is the OpenCV FSRCNN path: one luminance plane, not a colour image.

- Input `y`: `float32`, NCHW `[1, 1, height, width]`. The value is the Y channel of YCrCb, divided by 255, so the range is 0–1.
- Output `output`: `float32`, NCHW `[1, 1, height × scale, width × scale]`, the same Y channel in 0–1.
- Cr and Cb are resized with bilinear interpolation and combined with the network output, then converted back to RGB.
- Alpha is resampled with nearest neighbour. A flat colour stays within 24 levels of the source.
