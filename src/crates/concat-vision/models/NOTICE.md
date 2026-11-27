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

`selfie-multiclass.onnx` is Google's MediaPipe Selfie Multiclass
Segmentation model (256 × 256), from
<https://huggingface.co/litert-community/MediaPipe-ImageSegmenter-Selfie-Multiclass>
at commit `c7b7e9e0f1e73c10a12114379077b5257e60c25c`
(`selfie_multiclass_256x256.tflite`, SHA-256
`c6748b1253a99067ef71f7e26ca71096cd449baefa8f101900ea23016507e0e0`), licensed
under the Apache License, Version 2.0. CapCraft Forge converted it to ONNX
with tf2onnx (opset 17) and stored its weights as half floats with
onnxconverter-common, keeping float inputs and outputs; the conversion is
under the same licence. It takes `[1, 256, 256, 3]` RGB floats in `0..=1`
(named `input_29`) and answers `[1, 256, 256, 6]` scores (`Identity`) for
background, hair, body skin, face skin, clothes and others; Retouch keeps
body skin and face skin. Against the original on a test portrait, the
converted model's face-skin answer differs from MediaPipe's own by 0.4 % on
average. SHA-256 of the converted file:
`823a491190a9fe3bdae8473b94cdcd574e7070b6c648766fb398f580d7b1148a`.
