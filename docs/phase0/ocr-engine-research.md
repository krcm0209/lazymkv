# OCR engine for lazymkv under D17 (research, 2026-10-06)

Which OCR engine the released app can ship inside its installer (decision D17: nothing is downloaded at run time; D8: Linux/macOS later) to read bitmap subtitles for experiment 14's markers (SDH sound descriptions, speaker labels, music notes) and, possibly, a language check. Produced by a research workflow: four researchers (sources cited per claim), eight adversarial verifications of the load-bearing claims, one synthesis. Facts marked *verified* were re-read from the primary source by a second agent; *researcher-sourced* facts were not; *unverified (load-bearing)* facts would change the recommendation if wrong and still need checking. Nothing here is legal advice.

## Recommendation of the synthesis

Plan to bundle Tesseract: a self-built, static, Apache-2.0 tesseract.exe or libtesseract with tessdata_fast eng/spa/fra/deu models inside the installer. Keep pure-Rust ocrs as the runner-up, and switch to it only if phase 0 shows that its ASCII-only output is good enough for the language check.

On every point except language coverage, ocrs is the best fit. The library is pure Rust with no C, C++, CMake or bindgen, and its CI covers windows-latest, so the weak J4125 has nothing native to build (verified). Its code is MIT OR Apache-2.0. Its models are about 12 MB and can be embedded with rten's Model::load_static_slice and include_bytes!. Only the ocrs CLI downloads models; the library never does, which satisfies D17 (verified). The deciding fact is that the default recognition model's alphabet is ASCII only. It has no é, ñ, ü, ß, ç, ¿ or ¡, and no U+266A/U+266B. The alphabet cannot be widened without retraining the recognition model, and the maintainer knows of no one who has done this (verified, ocrs lib.rs and issues #204 and #247). The brief asks for Spanish, French and German after English, plus a language check. Output without accents also throws away the unique-letter signals that Subtitle Edit's language detector relies on (ß/ü, ñ/¿/¡, ç/œ). So ocrs as shipped meets the English SDH-marker need and does not meet the brief.

Tesseract has the accented languages. Its code and all three model repos are Apache-2.0 (verified), and Leptonica is BSD-2. The duties are to ship the licence texts and reproduce the notices. The eng+spa+fra+deu fast models come to about 9 MB. It is also the engine the project already runs in experiment 14. Its cost is the native build. The Rust bindings expect vcpkg or a CMake build (unverified, load-bearing). tesseract-rs downloads sources at build time without checksums, and the UB-Mannheim exe's bundled MinGW DLLs have unclear licences. Both of those are unverified. The build cost is real on the J4125. My inference, not a measured fact, is that it is paid once: build a static tesseract with curl and libarchive turned off (the CMake options are unverified), cache the result or vendor it, then call it as a subprocess or link to it. Music notes are not a deciding factor. No stock engine outputs ♪: ocrs cannot, and Tesseract misreads it (unverified reports). Either way lazymkv needs a stand-in heuristic, as Subtitle Edit uses.

Three conditions would flip the choice to ocrs:
- phase 0 shows that ASCII-folded ocrs output still identifies es/fr/de correctly by stopwords,
- the speed on the J4125 is acceptable (no AVX2, so rten uses its generic path, verified),
- a reading of CC-BY-SA-4.0 clears bundling the models.

If the language check can use the track's language tag instead of OCR, ocrs becomes clearly preferable. I could not determine whether that is acceptable, so it is the owner's call.

## Runner-up

ocrs 0.13.1 (pure Rust, rten runtime) with the ~12 MB detection and recognition models embedded via include_bytes!. This is the right pick if lazymkv only needs ASCII SDH markers and the language check can work from unaccented text or from track tags. Before adopting it, measure speed on the J4125 and get a legal read of CC-BY-SA-4.0 for the models.

## Rejected options

- **ocrs with a custom-trained Latin-extended recognition model** — Training is a large undertaking. The maintainer knows of no existing accented model (verified, #204/#247). The training-code repo has no licence file, and the result would be a CC-BY-SA adaptation of HierText-trained work. Too much risk for a feature used only for markers.
- **PaddleOCR PP-OCRv5/v6 Latin models via ort (ONNX Runtime)** — The models are Apache-2.0 and include diacritics (researcher-sourced). However, pyke's prebuilt ONNX Runtime requires x86-64-v3 (unverified), while the J4125 is SSE4.2-only (verified via Intel ARK), so it probably will not run on the build box or on older user PCs. The alternative is building ONNX Runtime from source, which is the heavy C++ build this decision is trying to avoid. ort is also still 2.0.0-rc.13.
- **PP-OCR models via pure-Rust tract (pure_onnx_ocr)** — The crate is early-stage (about 1.7k downloads). An open tract bug (#2724) blocks official paddle2onnx exports unless the files are patched. Both are unverified. Revisit later as the pure-Rust multilingual path.
- **PP-OCR models on rten (ocrs's own runtime)** — Attractive: pure Rust and multilingual. But I found no ready Rust pipeline, and rten issue #1225 says PP-OCRv5 detection shape inference is incomplete. Lazymkv would have to write preprocessing and CTC decoding itself. Worth a later spike, not a plan.
- **Windows.Media.Ocr** — Recognises only OCR language packs installed on the user's machine, which the app cannot ship (unverified). Microsoft lists it as requiring package identity (unverified). Windows-only, which conflicts with D8.
- **Bundling the UB-Mannheim tesseract-ocr-w64 installer contents as-is** — It is built with MinGW and pulls in DLLs (curl, libarchive, icu, libgomp, libstdc++, etc.) whose licences are not collected anywhere (unverified). Not a clean redistributable; a self-built static tesseract is cleaner.
- **tesseract-rs 0.4.0** — Its build.rs downloads sources and tessdata from GitHub at build time with no checksum and no offline switch (unverified). That conflicts with the owner's no-fetch stance and repeats the C++ build on every clean build. docs.rs also failed to build 0.4.0.
- **kalosm-ocr, sceptre, ocr-rs, rusto-rs** — kalosm-ocr and sceptre download weights at run time by default. ocr-rs and rusto-rs need MNN C++/CMake builds. kalosm-ocr's language scope is not documented, and sceptre is slow (0.23 img/s).

## Risks

- The Tesseract build route is unproven on the J4125. The vcpkg dependency tree, the tesseract-sys and bindgen/libclang needs, and the CMake DISABLE_CURL/DISABLE_ARCHIVE options all come from unverified claims. Build time and the size of the static binary are not known.
- Build-time fetching: vcpkg downloads sources, and its hash verification was not checked. If the owner's no-fetch stance also covers the build machine, the sources must be vendored or mirrored with checksums. D17 itself covers only run time.
- Music notes: neither engine can output U+266A/U+266B. Tesseract's stock eng unicharset probably lacks them (unverified; the langdata_lstm sources contain none). The misread set ('P or D', 'O', '#', '&', 'J') comes from 2020 Tesseract 4 reports and is unverified for 5.x. Marker rules will rely on a stand-in heuristic, and nothing has measured its precision.
- The ocrs model licence is CC-BY-SA-4.0 (verified), with no LICENSE file in ocrs-models. ShareAlike and attribution duties for shipping the files inside a closed installer have had no legal analysis. It is also not stated that the older S3 models carry the same licence as the Hugging Face models.
- ocrs speed on the J4125 is unknown. It uses the generic non-SIMD path; the maintainer estimates AVX2 is 2-4x faster. Tesseract's per-call cost on the J4125 is also unmeasured outside experiment 14.
- No head-to-head accuracy data exists for any engine on PGS or VobSub frames, so the choice rests on capability (alphabet), not on measured accuracy.
- Whether ASCII-folded ocrs output can still drive a stopword language ID for es/fr/de has not been tested. It is the condition that would reverse this recommendation.
- The ocrs model must come from the same rten version ocrs depends on (0.26.x, not 0.27). That is a minor dependency-pinning hazard if ocrs is chosen.
- The subprocess integration facts (list-file input, stdout output, \f page separator) are unverified, although the project's scripts/phase0/ocr-subs.ps1 already uses the list-file route.

## Checks that would settle it (phase 0, mini PC)

- On the mini PC, build a tiny release-mode Rust bin that depends on ocrs 0.13.1 (default-features=false, 'onnx') and embeds text-detection.onnx and text-recognition.onnx via include_bytes! (place the files by hand; no CLI download). Record cargo build time, exe size, and that it runs on the J4125 without SIGILL.
- Run that bin on the experiment 14 frames (English SDH, Spanish, French, German). Record ms per frame, marker hit rate ([..], (..), NAME:), what accented glyphs and ♪ turn into, and whether a stopword-based language ID on the ASCII output names the right language. Compare with Tesseract on the same frames.
- On the mini PC (Tesseract 5.4.0 installed), run `combine_tessdata -u eng.traineddata eng.` and the same for spa/fra/deu (fast). Grep each lstm-unicharset for U+266A/U+266B and for é ñ ü ß ç ¿ ¡.
- Collect the actual ♪/♫ misread strings from both engines on frames with lyrics, to build and test the note stand-in heuristic.
- Time `vcpkg install tesseract:x64-windows-static` (or a hand CMake build with DISABLE_CURL=ON, DISABLE_ARCHIVE=ON, BUILD_TRAINING_TOOLS=OFF, against a minimal Leptonica) on the mini PC. Record wall time, peak RAM, resulting tesseract.exe or static lib size, and the full list of dependency licences.
- Run `dumpbin /dependents` on the installed tesseract.exe and list the DLLs in C:\Program Files\Tesseract-OCR, to size and licence-audit the subprocess payload.
- Compare tessdata_fast against tessdata_best accuracy on the experiment 14 frames, to fix the installer payload (about 9 MB fast vs about 41 MB best for eng+spa+fra+deu).
- Ask a human to read CC-BY-SA-4.0 for bundling the unmodified ocrs models (attribution text, ShareAlike scope). Only needed if ocrs stays in play.

## Facts the recommendation rests on

| Status | Fact | Source |
|---|---|---|
| verified | ocrs default recognition alphabet is ASCII only: digits, ASCII punctuation, A-Z, a-z, plus an 'E' in the training-time euro slot. No accented letters, no ♪/♫. | <https://github.com/robertknight/ocrs/blob/main/ocrs/src/lib.rs> |
| verified | Recognising extra characters with ocrs requires training a custom recognition model; a custom alphabet on the default model errors on size mismatch. | <https://github.com/robertknight/ocrs/issues/204> |
| verified | Maintainer says German umlauts and ß need a custom-trained model. | <https://github.com/robertknight/ocrs/issues/247> |
| verified | The ocrs library does no downloading; only the CLI fetches models from S3 into ~/.cache/ocrs. The library takes caller-loaded rten::Model values. | <https://github.com/robertknight/ocrs/blob/main/ocrs-cli/src/models.rs> |
| verified | rten Model::load_static_slice(&'static [u8]) is documented for models embedded via include_bytes (also in rten 0.26). ocrs 0.13.1 depends on rten ^0.26. | <https://docs.rs/rten/latest/rten/struct.Model.html> |
| verified | The ocrs library's dependency closure is pure Rust (no -sys/cc/cmake/bindgen), and CI builds on windows-latest. It was not built on the mini PC. | <https://github.com/robertknight/rten> |
| verified | ocrs models are CC-BY-SA-4.0 (Hugging Face card), trained on HierText (CC BY-SA 4.0). The ocrs-models repo has no LICENSE file. | <https://huggingface.co/robertknight/ocrs> |
| researcher-sourced, not independently verified | ocrs code licence is MIT OR Apache-2.0. | <https://crates.io/api/v1/crates/ocrs> |
| researcher-sourced, not independently verified | ocrs model files total about 12.2 MB (detection 2.5 MB + recognition 9.7 MB). | <https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx> |
| verified | rten's SIMD paths are AVX2(+FMA+F16C), AVX-512, Neon and WASM; otherwise it uses generic kernels. The maintainer estimates AVX2 is 2-4x faster. The J4125 lists SSE4.2 only. | <https://github.com/robertknight/ocrs/issues/52> |
| researcher-sourced, not independently verified | ocrs describes itself as an 'early preview'. No accuracy figures are published. Latest release 0.13.1 is dated 2026-09-13. | <https://github.com/robertknight/ocrs> |
| verified | All data in the tessdata, tessdata_fast and tessdata_best repos is Apache-2.0. | <https://github.com/tesseract-ocr/tessdata_fast> |
| researcher-sourced, not independently verified | Tesseract code is Apache-2.0 with no NOTICE file. Leptonica is BSD-2 (binary redistribution must reproduce the notice). | <https://github.com/tesseract-ocr/tesseract/blob/main/README.md> |
| researcher-sourced, not independently verified | tessdata_fast sizes: eng 4.1 MB, spa 2.3 MB, fra 1.1 MB, deu 1.5 MB. tessdata_best eng 15.4 MB. | <https://api.github.com/repos/tesseract-ocr/tessdata_fast/contents/eng.traineddata> |
| unverified (load-bearing) | tesseract-sys and leptess find libtesseract on Windows via vcpkg or env vars, and run bindgen at build time. | <https://github.com/ccouzens/tesseract-sys> |
| unverified (load-bearing) | The vcpkg tesseract port depends on curl, leptonica and libarchive; Leptonica pulls in image libraries; x64-windows-static gives DLL-free builds. | <https://raw.githubusercontent.com/microsoft/vcpkg/master/ports/tesseract/vcpkg.json> |
| unverified (load-bearing) | Tesseract CMake has DISABLE_CURL, DISABLE_ARCHIVE and BUILD_TRAINING_TOOLS options. | <https://raw.githubusercontent.com/tesseract-ocr/tesseract/main/CMakeLists.txt> |
| unverified (load-bearing) | tesseract-rs 0.4.0 build.rs downloads sources and tessdata from GitHub at build time without checksum verification. | <https://docs.rs/crate/tesseract-rs/0.4.0/source/build.rs> |
| unverified (load-bearing) | The UB-Mannheim installer is MinGW-built, bundles MSYS2 DLLs, and shows only the Apache-2.0 licence. | <https://raw.githubusercontent.com/UB-Mannheim/tesseract/master/nsis/build.sh> |
| unverified (load-bearing) | The tesseract CLI accepts a list file of images and writes to stdout; pages are separated by page_separator (default \f). | <https://github.com/tesseract-ocr/tesseract/blob/main/doc/tesseract.1.asc> |
| unverified (load-bearing) | The langdata_lstm eng training charset contains no U+266A/U+266B. The shipped traineddata was not inspected. | <https://raw.githubusercontent.com/tesseract-ocr/langdata_lstm/main/eng/desired_characters> |
| unverified (load-bearing) | Stock Tesseract 4.x misreads subtitle music notes (e.g. 'P or D', 'O'). | <https://github.com/SubtitleEdit/subtitleedit/issues/3909> |
| unverified (load-bearing) | Subtitle Edit replaces #, *, ¶ and mojibake with ♪ rather than trusting OCR for notes. | <https://github.com/SubtitleEdit/subtitleedit/blob/main/src/libse/Settings/ToolsSettings.cs> |
| researcher-sourced, not independently verified | Subtitle Edit's language detection uses word lists plus unique accented letters (üß, ñ¡¿, çœ). | <https://github.com/SubtitleEdit/subtitleedit/blob/main/src/libse/Common/LanguageAutoDetect.cs> |
| unverified (load-bearing) | pyke's prebuilt ONNX Runtime binaries (ort download-binaries) require an x86-64-v3 CPU. | <https://ort.pyke.io/misc/prebuilt-binaries> |
| unverified (load-bearing) | PaddleOCR is Apache-2.0; PP-OCRv6 covers 46 Latin-script languages with about 200 diacritical characters added. | <https://www.paddleocr.ai/latest/en/version3.x/algorithm/PP-OCRv6/PP-OCRv6.html> |
| unverified (load-bearing) | tract issue #2724 blocks official PaddleOCR ONNX exports (open). | <https://github.com/sonos/tract/issues/2724> |
| unverified (load-bearing) | Windows OCR recognises only installed OCR language packs, and OcrEngine is listed as requiring package identity. | <https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/winrt-api-desktop-app-support> |
| researcher-sourced, not independently verified | rten issue #1225: PP-OCRv5 detection runs, but shape inference is incomplete. | <https://github.com/robertknight/rten/issues/1225> |

## Verification record

- **confirmed** — The pre-trained models are published under CC-BY-SA-4.0 (Hugging Face model card), trained on HierText (CC BY-SA 4.0) and synthetic data. The ocrs-models GitHub repo does not state a model licence in its README and has no detected LICENSE file (GitHub API returns license null). ShareAlike/attribution obligations for bundling the unmodified models in an installer need a legal read; this was not analysed.
  - source: <https://huggingface.co/robertknight/ocrs>
  - evidence: I checked each part of the claim against its primary source.
(1) The raw Hugging Face model card (https://huggingface.co/robertknight/ocrs/raw/main/README.md) has front-matter 'license: cc-by-sa-4.0'. It says the files include PyTorch checkpoints, ONNX models and RTen models, and states 'Models are trained on HierText and synthetic data.'
(2) For the ocrs-models GitHub repo, the API (https://api.github.com/repos/robertknight/ocrs-models) returns "license": null. The root contents are .github, .gitignore, Makefile, README.md, docs, layout-scraper, mypy.ini, ocrs_models, poetry.lock and pyproject.toml, so there is no LICENSE file. The only licence mention in its README is in the Datasets section: 'HierText (CC-BY-SA 4.0)', with the note that models are trained on datasets that are 'open and ... have non-restrictive licenses'. The README states no licence for the models themselves.
(3) The HierText README (https://github.com/google-research-datasets/hiertext, License section) says the dataset is under the CC BY-SA 4.0 license.
(4) The ocrs main README (https://github.com/robertknight/ocrs) has no model licence either. It only points to ocrs-models.
The claim's final point, that ShareAlike/attribution obligations were not analysed, is the researcher's own caveat, not a sourced fact, and nothing contradicts it.
A side note relevant to D17, outside the claim: the ocrs README says the CLI 'will download the required models automatically' on first run into ~/.cache/ocrs. A bundled build would have to ship the .rten files itself and load them from disk.
- **confirmed** — The ocrs CLI downloads models at first run from an S3 bucket into ~/.cache/ocrs: https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx and .../text-recognition.onnx. The LIBRARY does no downloading; it takes already-loaded rten::Model values.
  - source: <https://github.com/robertknight/ocrs/blob/main/ocrs-cli/src/models.rs (also https://github.com/robertknight/ocrs/blob/main/ocrs-cli/src/main.rs, https://github.com/robertknight/ocrs/blob/main/ocrs/Cargo.toml, https://github.com/robertknight/ocrs/blob/main/ocrs/examples/hello_ocr.rs, https://github.com/robertknight/ocrs/blob/main/README.md)>
  - evidence: I read the source files on the main branch and they back the claim.

1. ocrs-cli/src/main.rs has `const DETECTION_MODEL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";` and `const RECOGNITION_MODEL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";`. These are the default sources. The `--detect-model` and `--rec-model` options replace them with local paths.

2. ocrs-cli/src/models.rs has `load_model(ModelSource::Url | ModelSource::Path)`. It downloads with `ureq::get(url).call()` into `~/.cache/ocrs/`, and only when the file is not already cached. On wasm32 it returns an error saying downloading is not supported.

3. ocrs-cli/Cargo.toml lists ureq under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`. It is not behind a feature flag.

4. The top-level README says that on first run the tool will "download the required models automatically and store them in ~/.cache/ocrs".

5. The library crate's Cargo.toml (ocrs/Cargo.toml) depends only on anyhow, rayon, rten, rten-imageproc, rten-tensor and thiserror, plus wasm-bindgen on wasm32. There is no HTTP client, so the library has no way to download anything.

6. ocrs/examples/hello_ocr.rs loads the models with `Model::load_file(...)` and passes them in through `OcrEngine::new(OcrEngineParams { detection_model: Some(..), recognition_model: Some(..), ..Default::default() })`. The library README tells you to get the .onnx models yourself with `./download-models.sh` in the examples directory, which is a separate step done by hand.

The claim leaves out two conditions, but they do not change it. The CLI downloads only when no local model path is given and the models are not cached yet, and it never downloads on wasm32.

What this means for D17: lazymkv can link the ocrs library, ship the two .onnx (or .rten) model files inside the installer, and load them from disk. No network access is involved at run time.
- **confirmed** — Embedding models in the executable is supported: rten::Model::load_static_slice(data: &'static [u8]) is documented as for models embedded via include_bytes; also Model::load(Vec<u8>) (auto-detects ONNX/RTen), Model::load_file(path), and load_mmap (needs 'mmap' feature, unsafe). ocrs API: OcrEngine::new(OcrEngineParams), prepare_input(ImageSource), get_text(&OcrInput) -> String, or detect_words / find_text_lines / recognize_text. ImageSource::from_bytes(raw_rgb, (w,h)). Enable ocrs 'onnx' feature (default in 0.13.1) to load .onnx models, or 'rten' feature for .rten.
  - source: <https://docs.rs/rten/latest/rten/struct.Model.html (also https://docs.rs/ocrs/0.13.1/ocrs/struct.OcrEngine.html, https://docs.rs/ocrs/0.13.1/ocrs/struct.ImageSource.html, https://docs.rs/crate/ocrs/0.13.1/source/Cargo.toml.orig)>
  - evidence: I read the docs.rs pages and Cargo.toml myself. The claim holds in substance, with a few signature details stated loosely.

rten (docs.rs rten 0.27.0, Model page):
- `pub fn load_static_slice(data: &'static [u8]) -> Result<Model, LoadError>`. Doc text: "This is useful for loading models embedded in the binary via include_bytes for example. The model can be in either ONNX or RTen format. The model type is detected automatically."
- `pub fn load(data: Vec<u8>) -> Result<Model, LoadError>`, which also detects ONNX or RTen on its own.
- `pub fn load_file<P: AsRef<Path>>(path: P)`, documented for `.onnx` or `.rten` files.
- `pub unsafe fn load_mmap<P: AsRef<Path>>(path)`. It "requires the mmap crate feature to be enabled". It is unsafe because a model file changed on disk while in use causes undefined behaviour.
- rten 0.26.0 has the same `load_static_slice` signature and the same include_bytes doc text (https://docs.rs/rten/0.26.0/rten/struct.Model.html).

ocrs 0.13.1 (docs.rs):
- `OcrEngine::new(params: OcrEngineParams) -> Result<OcrEngine>`
- `prepare_input(&self, image: ImageSource<'_>) -> Result<OcrInput>`
- `detect_words(&self, &OcrInput) -> Result<Vec<RotatedRect>>`
- `find_text_lines(...)` and `recognize_text(...)` exist.
- `get_text(&self, input: &OcrInput) -> Result<String>`
- `OcrEngineParams` fields: `detection_model: Option<Model>`, `recognition_model: Option<Model>`, `debug`, `decode_method`, `alphabet`, `allowed_chars`.
- `ImageSource::from_bytes(bytes: &'a [u8], dimensions: (u32, u32)) -> Result<ImageSource, ImageSourceError>` takes pixels in HWC order. `from_tensor` is also available.

ocrs 0.13.1 Cargo.toml.orig:
- `default = ["export-wasm", "rten", "onnx"]`
- `onnx = ["rten/onnx_format"]` ("Enables loading custom models in ONNX format")
- `rten = ["rten/rten_format"]` ("Enables loading custom models in rten format")
- The crates.io page lists the dependency as rten ^0.26.0.
- **confirmed** — The default recognition model's alphabet is ASCII only: ' 0123456789!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~' plus (a Euro-sign slot in training) plus A-Z a-z. So [ ] ( ) : ? ! - and digits are covered, but NO accented letters, no other currencies, and no music notes (U+266A/U+266B). README: 'ocrs currently recognizes the Latin alphabet only (eg. English)'.
  - source: <https://github.com/robertknight/ocrs/blob/main/ocrs/src/lib.rs>
  - evidence: I read the primary sources and could not refute the claim. (1) ocrs/src/lib.rs line 34 on main is: const DEFAULT_ALPHABET: &str = " 0123456789!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~EABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"; That is 96 characters: space, 10 digits, 32 ASCII punctuation marks, one extra 'E', then A-Z and a-z. The doc comment on the `alphabet` field says the default "matches the one used to train the original models" (https://github.com/robertknight/ocrs-models). (2) ocrs-models ocrs_models/datasets/hiertext.py defines the training DEFAULT_ALPHABET as the same punctuation, then chr(8364) (the euro sign), then A-Z and a-z. Its docstring says the alphabet "closely matches the English 'gen2' model from EasyOCR". Characters outside this alphabet are encoded as unknown_char="?" in training. So the 'E' slot in the Rust string sits where the euro sign was in training. Neither alphabet has accented letters, other currency signs or U+266A/U+266B. (3) README.md, "Language Support" section: "ocrs currently recognizes the Latin alphabet only (eg. English). Support for more languages is planned (issue #8)." (4) In issue #246, maintainer robertknight lists the same character set and says "this includes the dollar sign but no other currencies". His example output turned a euro sign into '?'. The most recent commit to lib.rs (2026-03-28) did not change the alphabet. One nuance the claim leaves out: the Rust decoder maps the euro slot to the letter 'E', not to a euro sign, so ocrs never outputs a real euro sign. This only strengthens the "ASCII only" point. It is also worth knowing for marker detection: a stray 'E' could stand in for an unknown symbol.
- **confirmed** — Extending beyond ASCII requires training a custom recognition model; the maintainer states no one he knows of has done it for Latin-extended languages, and a custom alphabet passed to the default model just errors on size mismatch. German umlauts/ß were asked about (issue #247) and answered: a custom model must be trained.
  - source: <https://github.com/robertknight/ocrs/issues/204>
  - evidence: I read the issue bodies and full comment threads through the GitHub API (gh api repos/robertknight/ocrs/issues/{204,247,8} and their /comments).

#204 "Full latin support?" (opened 2025-10-18 by richarddd, still open). The reporter, not the maintainer, posted the error: "output column count (97) does not match alphabet size (96)". It came from passing a custom alphabet for Swedish/Polish characters. robertknight replied on 2025-10-18: "The custom alphabet option in this library is tied to the text recognition model being used. To recognize additional characters you need to find or create a custom model and use an alphabet which matches. I'm not aware of anyone who has already done this for the languages you mentioned." He also said that if the training dataset (HierText) already has examples of those characters, "it might be enough to just change the alphabet used in the training process"; otherwise the dataset has to be expanded. Either way a model has to be retrained.

#247 "Language Support" (opened 2026-04-22, still open). In a comment on 2026-07-31, jb-alvarado asked about äöüÄÖÜ and ß. robertknight answered on 2026-08-01: "to recognize those characters you would need to train a custom model with an alphabet that includes those characters". He added that dataset generation and alphabet configuration "needs better documentation" and pointed to ocrs-models/docs/training.md.

#8 "Support for non-Latin characters" (opened by robertknight 2024-01-03, still open). The hard-coded-alphabet sub-task is checked (PR #100). "Create a proof-of-concept recognition model that recognizes a non-Latin alphabet" is still unchecked. So is "standard file format for model metadata".

Every quote in the claim matches the source. Two small points of precision:
(a) The maintainer's "not aware of anyone" covers "the languages you mentioned" (Swedish, Polish), not Latin-extended languages in general.
(b) The size-mismatch error is the reporter's observation, not a statement by the maintainer. His reply does explain it: the model's output classes are fixed by the alphabet it was trained on.

Neither point changes the substance. The default ocrs model only recognizes its ASCII-ish alphabet, and anything beyond it (umlauts, ß, Swedish/Polish letters, and by implication ♪) needs a retrained recognition model.
- **confirmed** — The ocrs library dependency set is pure Rust: ocrs depends on anyhow, rayon, rten, rten-imageproc, rten-tensor, thiserror (+ wasm-bindgen only on wasm32); rten's own README says 'End-to-end Rust. This project and all of its required dependencies are written in Rust.' The Cargo.lock closure from the ocrs library contains no -sys, cc, cmake or bindgen crates; the only 'cc' users in the lockfile are ring and wayland-backend, which belong to the CLI (ureq/clipboard), not the library. ocrs CI builds on windows-latest as well as ubuntu. No extra toolchain (CMake, LLVM) is needed by the library; I did not myself build it on Windows MSVC.
  - source: <https://github.com/robertknight/rten>
  - evidence: I cloned github.com/robertknight/ocrs and github.com/robertknight/rten (default branch, 2026-10-06) and checked each part of the claim. All parts hold.

(1) ocrs/ocrs/Cargo.toml (ocrs 0.13.1) has these [dependencies]: anyhow, rayon, rten, rten-imageproc, rten-tensor, thiserror. It adds wasm-bindgen only under [target.'cfg(target_arch = "wasm32")']. The workspace pins rten 0.26.0 with default-features = false, and the default "onnx" feature turns on rten/onnx_format.

(2) In the rten README "Goals" section, the text reads: "End-to-end Rust. This project and all of its required dependencies are written in Rust. This simplifies the build and deployment process."

(3) cargo is not installed on this Mac, so I parsed ocrs/Cargo.lock with a script and walked everything reachable from the "ocrs" package. The lockfile does not separate dev-dependencies, so this walk is a superset: it also picks up image, png, tiff, zune-jpeg, lexopt and fastrand. Even so, the closure has no crate whose name contains "sys" and no cc, cmake or bindgen crate. The non-Rust-looking entries are libc and hermit-abi, which are Rust binding crates and compile no C. Across the whole lockfile, only ring and wayland-backend depend on cc, and nothing depends on cmake or bindgen. Neither ring nor wayland-backend can be reached from the ocrs library package, so they belong to the CLI or the workspace.

(4) The matrix in ocrs/.github/workflows/ci.yml is os: [ubuntu-latest, windows-latest], and it runs `cargo build --features clipboard` and `just test` on both.

(5) In rten/Cargo.toml (master, 0.27.0), every listed dependency is a Rust crate (rten-* subcrates, rayon, num_cpus, flatbuffers, memmap2, and others), and nothing pulls in CMake or LLVM.

Minor caveats, none of which contradict the claim:
- rten master is 0.27.0 with rust-version 1.94, while ocrs pins rten 0.26. The mini PC's Rust 1.99 meets that minimum.
- The README's wording is about "required" dependencies.
- Like the researcher, I did not build the library on Windows MSVC myself. A green windows-latest CI job is evidence for that, not proof on this specific machine.

Sources:
- https://github.com/robertknight/ocrs/blob/main/ocrs/Cargo.toml
- https://github.com/robertknight/ocrs/blob/main/Cargo.lock
- https://github.com/robertknight/ocrs/blob/main/.github/workflows/ci.yml
- https://github.com/robertknight/rten/blob/main/README.md
- https://github.com/robertknight/rten/blob/main/Cargo.toml
- **confirmed** — rten supports SIMD via AVX2, AVX-512, Arm Neon and WASM SIMD only; on CPUs without AVX2 it uses a slower generic fallback. The maintainer says AVX2 is '2-4x faster' than the fallback, and a pre-AVX2 crash (SIGILL) was fixed in ocrs 0.6.0. The Celeron J4125 (Goldmont Plus) lists SSE4.2 and no AVX2, so it takes the slow path; no published timings exist for it, so the speed on the mini PC must be measured.
  - source: <https://github.com/robertknight/ocrs/issues/52>
  - evidence: The claim holds, with three small precision issues that do not change its conclusion.

(1) The rten README says, word for word: "It supports SIMD via AVX2, AVX-512, Arm Neon and WebAssembly SIMD." The current source agrees. rten-simd/src/arch/x86_64/ holds only avx2.rs and avx512.rs, and there is also a generic.rs. rten-gemm/src/kernels/ holds aarch64, x86_64, wasm, simd_generic and generic. There is no SSE backend, so an SSE4.2-only x86 CPU falls back to the generic path.

(2) In ocrs issue #52 (filed 2024-04-29: illegal instruction in a Proxmox VM), maintainer robertknight wrote: "the fallback without these CPU features will be somewhat slower". He reproduced SIGILL in rten_vecmath::exp::simd_sigmoid on a Nehalem (pre-AVX2) CPU under qemu. He said it was "fixed upstream by https://github.com/robertknight/rten/pull/131". Then he wrote: "ocrs-cli v0.6.0 has been released which should fix the issue. I would still recommend configuring your VM to use a newer CPU, as using one that supports AVX2 will be 2-4x faster." The reporter confirmed that 0.6.0 works.

(3) Intel ARK for the Celeron J4125 lists only "Intel SSE4.2" under instruction set extensions, with no AVX or AVX2. Its code name is Gemini Lake Refresh, which is a Goldmont Plus core.

I found no published J4125 timings for ocrs or rten. That absence is a gap I could not rule out, not proof that none exist.

Precision issues:
- In rten-simd/src/arch/x86_64/avx2.rs, the AVX2 path is chosen only when avx2, fma and f16c are all detected at run time. In the issue the maintainer said "AVX-2 / FMA".
- The release he named is ocrs-cli v0.6.0. The fix itself is in rten (PR #131, merged 2024-04-28, "Support AVX-512 for SIMD unary operators", which corrected #[inline]/#[target_feature] use).
- "2-4x" is the maintainer's informal estimate, given as advice about VM CPU configuration, not a published benchmark.
- **confirmed** — All model data in tessdata, tessdata_fast and tessdata_best repos is licensed Apache-2.0.
  - source: <https://github.com/tesseract-ocr/tessdata_fast (also https://github.com/tesseract-ocr/tessdata_best, https://github.com/tesseract-ocr/tessdata, and the raw README.md/LICENSE on the main branch of each)>
  - evidence: I tried to refute the claim and could not. I read the README.md and LICENSE files on the main branch of all three repos. Each README carries the same line: "All data in the repository are licensed under the Apache-2.0 License, see file LICENSE." It is at lines 32-33 in tessdata, lines 62-63 in tessdata_fast and lines 11-12 in tessdata_best. Each LICENSE file is the Apache License, Version 2.0, January 2004. None of the three READMEs mentions an exception, carve-out or any file under a different licence. One limit on this check: I did not inspect each .traineddata file for embedded licence metadata. Apache-2.0 also has conditions the claim leaves out: anyone who redistributes the files must include the licence text and any NOTICE. That is a duty that comes with bundling, not a mistake in the claim.

## Controller's reading (Claude, 2026-10-06)

The synthesis weighs a language check by OCR that the spec does not require: subtitle languages come from MakeMKV's
track attributes (experiment 11 tests that filter), and R5/R6 are about English tracks. What experiment 14 needs from
OCR is the markers, which are ASCII on every Latin-script disc (brackets, parentheses, capitals, "NAME:"); accents are
lost with ocrs but no marker depends on them, and neither engine reads the music note, so a glyph-shape stand-in is
needed whichever engine ships. On that reading ocrs meets the need and Tesseract is the safer superset at the price of
a one-time native build and a licence audit of its payload. Two cheap checks on the mini PC decide it: ocrs's speed
and marker hit rate on the experiment 14 frames (the J4125 has no AVX2), and whether the Tesseract eng unicharset has
U+266A at all. The CC-BY-SA-4.0 models are shipped unmodified, which needs attribution and the licence text (ShareAlike
binds adaptations); the owner should confirm that reading before ocrs is adopted.

## Trial (2026-10-06)

Built to answer the first two settling checks above on the mini PC; results to be filled in by the controller.

- `experiments/ocrs-trial/`: a standalone crate (its own workspace) with one binary, `ocrs-trial --detection <file>
  --recognition <file> --frames <dir> --out <jsonl> [--limit N]`. It pins ocrs =0.13.1, rten =0.26.0 (the only 0.26
  release, as ocrs requires), image =0.25.10 (png only), serde_json and sha1, loads both models from files with
  `rten::Model::load_file` (nothing is downloaded), reads every `*.png` of the folder in name order and writes one JSON
  line per frame (`frame`, `file`, `lines`, `ms` = decode + OCR) and a summary line (frames, ms per frame, model load
  time, ocrs version, SHA-1 of both models). Its README carries the model licence and attribution.
- Models: ocrs 0.13.1 documents `.onnx` files, not `.rten`: the CLI's defaults (`ocrs-cli/src/main.rs`) and the
  library's `examples/download-models.sh` both fetch `https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx`
  (2499479 bytes, SHA-1 709176816006063da8155f549aa0e44735ed0e9c) and `.../text-recognition.onnx` (9713177 bytes, SHA-1
  b16f361df884a9afaa067e4d6932567f6f652a81); both were re-uploaded on 2026-09-13, the day 0.13.1 was released. The
  `.rten` files at the same place date from 2024-01-01 and are not used. The models are CC-BY-SA-4.0 and are never
  committed; on the mini PC they live in `C:\lazymkv\phase0\models\`.
- `scripts/phase0/ocrs-trial.ps1 -Slug -Label [-Stream 0] [-Folder subs-ocr] [-Limit N] [-Show 20] [-Force]`: copies one
  stream's frames from the tuning dataset to C: (refused above 2 GB, always deleted), runs the exe under
  `lazymkv-capture record`, keeps the text as `ocrs-s<n>.jsonl` beside Tesseract's in the tuning dataset, and writes
  `ocrs-summary.json` (counts, ocr-subs.ps1's five marker counts, timing, model hashes; no text) to the recording.
  `tuning-fetch.ps1 -Text` now also fetches `ocrs-s<n>.jsonl`.
- `lazymkv-dev ocr-compare <sub-<n>.ocr.jsonl> <ocrs-s<n>.jsonl> [--rec <recording>/subs-ocr]` (a Python script of
  `scripts/phase0/` until the Rust port): pairs the frames, folds both texts with the subtitle text rule's fold and
  corrections (`core::subtitle::text`), and prints numbers only: frames with text in each,
  exact matches, character error rate, the text rule's marker counts, readability and stopword language per engine,
  the speed of each, and the 10 frames that differ most (lengths and CER).

How to run it (Mac, controller):

```
ssh -o BatchMode=yes USER@makemkv-box "if not exist C:\lazymkv\phase0\models mkdir C:\lazymkv\phase0\models"
scp -q -o BatchMode=yes MODELS/text-detection.onnx MODELS/text-recognition.onnx USER@makemkv-box:C:/lazymkv/phase0/models/
time scripts/box.sh cargo build --release --manifest-path experiments/ocrs-trial/Cargo.toml
scp -q -o BatchMode=yes USER@makemkv-box:C:/lazymkv/src/experiments/ocrs-trial/Cargo.lock experiments/ocrs-trial/
ssh -o BatchMode=yes USER@makemkv-box "dir C:\lazymkv\target\release\ocrs-trial.exe"
scripts/phase0/job.sh ocrs-trial.ps1 ocrs-SLUG-LABEL -Slug SLUG -Label LABEL -Stream N -Limit 50
scripts/phase0/wait.sh ocrs-SLUG-LABEL
scripts/phase0/job.sh ocrs-trial.ps1 ocrs-SLUG-LABEL -Slug SLUG -Label LABEL -Stream N -Force
scripts/phase0/wait.sh ocrs-SLUG-LABEL
scripts/phase0/pull-recording.sh SLUG
scripts/phase0/job.sh tuning-fetch.ps1 fetch-SLUG-LABEL -Slug SLUG -Label LABEL -Text
scripts/phase0/wait.sh fetch-SLUG-LABEL        # then the scp and the clean-up it prints
cargo build -q --release -p lazymkv-dev
target/release/lazymkv-dev ocr-compare TEXT/sub-N.ocr.jsonl TEXT/ocrs-sN.jsonl --rec corpus/owner/SLUG/LABEL/subs-ocr
```

Results (to be filled in): build time and exe size on the J4125; whether it runs without SIGILL; ms per frame for ocrs
and Tesseract; exact-match rate and CER; marker counts and stopword language per engine; what accented letters and
music notes became.

### Trial results (2026-10-06, Office S03E16 English PGS, 600 frames)

Same frames for both engines (the canvas render of `ocr-subs.ps1`, 1920x1080, inverted); Tesseract 5.4.0 `eng` psm 6
against ocrs 0.13.1 with the `.onnx` models (SHA-1 7091768... and b16f361...), run on the mini PC by
`scripts/phase0/ocrs-trial.ps1`, compared with phase 0's Python comparison, now `lazymkv-dev ocr-compare` (numbers
only; the text stays in the
tuning dataset: `lazymkv-tuning\the-office-2005-s3d3-bluray\e16-stereo\subs-ocr\ocrs-s0.jsonl`).

| Measure | Tesseract | ocrs |
|---|---|---|
| frames with text | 345 | 345 (the same 345) |
| exact match after ASCII folding | - | 213 of 345 (0.617) |
| character error rate (Levenshtein / longer text), mean / median | - | 0.019 / 0.000; above 0.20 on 2 frames |
| same line count | - | 313 of 345 (0.907) |
| CUE / LABEL / CHEV / NOTE / ODD | 10 / 7 / 0 / 0 / 0 | 10 / 7 / 0 / 0 / 1 |
| M / SO / CS, class | 17 / 9 / 10, sdh | 17 / 9 / 10, sdh |
| language (stopwords) | en 0.413 | en 0.409 |
| time per frame on the J4125 | about 95 ms (57 s for 600) | 933 ms (560 s for 600; model load 19 ms) |

Reading: every measure the experiment 14 rule decides on comes out identical. The engines differ in the kind of
error: Tesseract reads a standalone "I" as "|" (corrected by the text rule's corrections, `core::subtitle::text`), ocrs drops it, or misreads it in a
contraction (three frames), and reads a few other glyphs wrongly (a "p" read as "o", for one); neither error touches
brackets, capitals, colons or the stopwords. ocrs is about ten times slower on a full 1080p frame: a two-hour film's
1,700 frames would take about 26 minutes per track against 2.5 with Tesseract. The frame is almost entirely white;
the app, which decodes the PGS bitmap itself, would hand ocrs only the glyph box (typically under 1500 x 200), which
removes most of that cost (not measured here; the frames the trial used are kept in the tuning dataset for that
measurement). Open for the owner: the CC-BY-SA-4.0 reading for shipping the unmodified models with attribution, and
that the S3 files the ocrs tools download carry no licence of their own (the Hugging Face card does).

## Decision (owner, 2026-10-06)

ocrs is the engine (spec D22): identical rule measures to Tesseract on the trial, pure Rust, bundle-friendly, keeps `cargo install` seamless; the app OCRs only the glyph box to recover the speed; Tesseract is the fallback if that fails. The CC-BY-SA-4.0 attribution and licence text ship with the app; the owner reads the licence.

### Crop trial (2026-10-06, same 600 frames, `--crop 16`)

OCR only the glyph box (bounding box of the dark pixels, 16 px padding; mean box 611 x 143 px): markers identical
again (CUE 10, LABEL 7, upper 10; 345 text frames; 619 lines against 603 full-frame). Time per READ frame 854 ms
(full frame: about 930 ms including decode), of which detection 613 ms, recognition 240 ms, PNG decode 57 ms, box
search 4 ms; blank frames are skipped, so the mean over all 600 frames falls to 491 ms. The crop therefore recovers
little: ocrs 0.13.1's detection model takes a fixed 800 x 600 input (read from the ONNX file), so a crop is padded or
resized to the same size and the detector costs the same. The recovery the app needs is to skip ocrs's detector for
subtitles: the lines are found from the rows of dark pixels of the decoded bitmap and handed to `recognize_text`
directly, which is the 240 ms part (about 7 minutes for a two-hour film's text frames on the J4125, without AVX2).
Not built in phase 0; it is the first measurement of the phase 1 OCR work. Detection on a crop narrower than 600 px
is padded rather than scaled, which is why the crop's line count differs slightly from the full frame's.
