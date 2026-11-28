# The compiled-in cutout model

The downloaded models are listed in THIRD_PARTY_NOTICES.md at the
repository root and named in `src/models.rs`.

`selfie-segmentation.onnx` is Google's MediaPipe Selfie Segmentation model
(the general, 256 × 256 variant), converted to ONNX and published by the
ONNX Community at
<https://huggingface.co/onnx-community/mediapipe_selfie_segmentation>.
Both the model and the conversion are licensed under the Apache License,
Version 2.0; see THIRD_PARTY_NOTICES.md at the repository root.

The file is compiled into `concat-vision` with `include_bytes!`, so a
cutout needs no download and works the same on a desk and on a phone. The
model takes one RGB picture as `[1, 3, 256, 256]` floats in `0..=1` and
answers with `[1, 1, 256, 256]` probabilities that each pixel is a person.

`yunet.onnx` is OpenCV's YuNet face detector (face_detection_yunet_2023mar,
https://github.com/opencv/opencv_zoo, MIT), as claude-video-effects carries
it (SHA-256 `8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4`).
Input `[1, 3, 640, 640]` BGR 0..255; outputs per-anchor scores and boxes at
strides 8, 16 and 32, decoded in `src/faces.rs`.
