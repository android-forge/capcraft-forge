# Models: Depth Anything V2 and MediaPipe against what Forge runs

*Status: analysis, written 27 November 2026 against Forge 0.0.8. Nothing replaced yet; see [Verdict](#verdict).*

Two questions:

1. Is [Depth Anything V2](https://github.com/DepthAnything/Depth-Anything-V2) better than Forge's depth model?
2. Do the [MediaPipe solutions](https://developers.google.com/edge/mediapipe/solutions) have models better than the ones
   Forge uses, with better Android support?

## Summary

- **Depth: Forge already runs Depth Anything V2.** Relight and Atmosphere's light use Depth Anything V2 **Small**
  (int8 ONNX, 27 MB, `concat-vision/src/models.rs`, `capcraft-android/src/depthlight.rs`). Small is the only V2 size
  under Apache-2.0. Base, Large and Giant are **CC-BY-NC-4.0**: non-commercial, so a free AGPL app cannot ship them or
  let users download them through it. **No change.** One candidate for later: *Video Depth Anything* (Small, Apache-2.0)
  for depth that doesn't flicker from frame to frame on video. Today Forge reads depth from one still, so it doesn't flicker.
- **MediaPipe: no model Forge runs today has a clearly better MediaPipe replacement.** Forge already ships one MediaPipe
  model, Selfie Segmentation (built into `concat-vision`, as ONNX). For detection, video matting, object cutout, inpainting,
  OCR, depth and speech, Forge's models are either more accurate or have no MediaPipe counterpart.
- **Where MediaPipe would clearly help is two tools that use no model today:**
  - **Retouch** finds skin by its colour. The **Selfie Multiclass** segmenter gives a real face-skin and body-skin mask.
  - **Auto reframe** follows the centre of the person's mask. **BlazeFace** (Face Detector) gives the face and its eyes,
    in about 3 ms on a Pixel 6's CPU.

  Both are new uses, not replacements, so they are proposed below rather than made here.
- **Runtime.** Forge runs every model on ONNX Runtime with NNAPI on Android. Google deprecated NNAPI in Android 15, so
  on new phones the models mostly run on the CPU. MediaPipe runs on LiteRT with its own GPU delegate, the path Google
  now supports. Adding MediaPipe Tasks means a second runtime in the APK. The cheaper route, which Forge already took for
  Selfie Segmentation, is converting the `.tflite` model to ONNX and keeping one runtime.

## Model by model

| Forge tool | Forge's model (licence, size) | MediaPipe counterpart | Better? |
|---|---|---|---|
| Relight, Atmosphere light | Depth Anything V2 Small, int8 (Apache-2.0, 27 MB) | none | Already the requested model. V2 Base/Large are CC-BY-NC-4.0, so not shippable. |
| Cutout fallback, Body effects | MediaPipe Selfie Segmentation 256×256 (Apache-2.0, 0.46 MB, compiled in) | Selfie Segmenter square/landscape (same model family); Selfie Multiclass 256×256 | **Same model.** Multiclass is 217.8 ms CPU / 71.2 ms GPU on a Pixel 6, against about 34 ms for the selfie model, so not as the fallback. |
| Remove BG on video | Robust Video Matting, MobileNetV3 (GPL-3.0, 15 MB) | none (segmenters only, with no temporal matting or hair-level alpha) | **No.** RVM keeps hair edges and is stable over time. |
| Remove BG on objects | IS-Net general (Apache-2.0, 179 MB) | DeepLab-V3 (21 classes, 257×257) | **No.** DeepLab's edges are coarse and it only knows its classes. |
| Custom removal brush | SlimSAM-77 (Apache-2.0, 40 MB) | Interactive Segmenter, MagicTouch (int8, 768×768): 208 ms CPU / 581 ms GPU on a Pixel 10 | **Not clearly.** SAM segments any object from a point. MagicTouch is smaller, but slower on GPU than CPU. Worth a side-by-side on the test phone before changing anything. |
| Remove objects (finding them) | YOLOX-S (Apache-2.0, 36 MB, COCO mAP 40.5) | EfficientDet-Lite0/Lite2 (COCO mAP about 25.7 / 34.0; 29–61 ms CPU on a Pixel 6) | **No.** Smaller and faster, but finds noticeably less. |
| Remove objects (filling) | MI-GAN 512 (MIT, 30 MB) | none | No counterpart. |
| Text tools (OCR) | PP-OCRv3 det + PP-OCRv5 rec (Apache-2.0, 10 MB) | none in MediaPipe Tasks | No counterpart. |
| Enhance | Real-ESRGAN general x4v3 (BSD-3, 5 MB) | none | No counterpart. |
| Captions, filler words | whisper.cpp tiny/base/small (MIT) | Audio Transcriber (new; no model or accuracy given) | **No evidence it is better.** Whisper's word timings are what Filler words is built on. |
| Isolate voice / Reduce noise | GTCRN (MIT) | none | No counterpart. |
| Text to speech | Kokoro, Pocket, Supertonic (sherpa-onnx) | Text-to-Speech (new, 24 kHz; voices and languages not listed) | Revisit when its voices and languages are documented. |
| **Retouch** | *no model: skin found by its colour* | Selfie Multiclass: face-skin and body-skin classes | **Yes.** It stops smoothing skin-coloured walls, wood and sand, and leaves eyes, lips and hair alone. |
| **Auto reframe** | *no model of its own: the centre of RVM's person mask* | Face Detector, BlazeFace short-range: 2.94 ms CPU on a Pixel 6, box plus 6 keypoints | **Yes.** It frames on the face rather than on the body's centre, which drifts with arms and clothes. |
| Light people, Face light | skin brightness read by colour | Face Landmarker (478 points, BlazeFace inside) | **Probably.** It finds the face exactly; use it after Retouch, once the skin mask is in. |

Sources: the MediaPipe Tasks pages for Image Segmenter, Face Detector, Face Landmarker, Object Detector and Interactive
Segmenter, read on 27 November 2026 (latencies as they print them); the Depth Anything V2 repository's licence section;
the EfficientDet and YOLOX papers for COCO mAP.

## Verdict

- **Depth Anything V2:** already used, at the only size Forge may ship. Nothing to replace.
- **MediaPipe:** nothing Forge runs should be swapped. Two tools should *gain* a MediaPipe model:
  1. **Retouch with a skin mask** from Selfie Multiclass (Apache-2.0), converted to ONNX the way Selfie Segmentation
     was, so it runs on Forge's one runtime. It's computed once per clip and cached, like Remove BG's masks, so its
     217 ms CPU cost is paid at analysis time, not during playback. The retouch shader reads the mask where it reads
     colour today.
  2. **Auto reframe on faces** with BlazeFace short-range (0.2 MB, about 3 ms). It runs at the same two samples a
     second as today, falls back to the person mask when no face is found, and adds a "Follow: face / body" choice in
     the panel.
- **Runtime:** stay on ONNX Runtime, with converted models. Test ORT's XNNPACK provider in place of NNAPI on Android 15+
  phones, since NNAPI is deprecated there. Add MediaPipe Tasks (LiteRT, GPU delegate) only if a model can't be
  converted or the converted one is clearly slower than LiteRT on the test phone.

## To check before building

- **Licences:** the model cards on Hugging Face (`litert-community`) for Selfie Multiclass and BlazeFace. The MediaPipe
  pages don't state them, and Google's model cards for these have been Apache-2.0.
- **Conversion:** `tf2onnx` on both `.tflite` files, comparing ONNX and LiteRT outputs on the same frames.
- **Phone timings:** Selfie Multiclass on the test phone through ORT CPU, ORT XNNPACK and ORT NNAPI.
