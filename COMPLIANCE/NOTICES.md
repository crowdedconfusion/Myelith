# NOTICES

> **Erzeugt, nicht bearbeiten.** Quelle: `COMPLIANCE/werkzeuge/notices.py`
> aus `cargo metadata`, `MODELS/llm/KATALOG.json` und
> `COMPLIANCE/fremdkomponenten.json`.
>
> **Generated, do not edit.** Source: `COMPLIANCE/werkzeuge/notices.py`
> from `cargo metadata`, `MODELS/llm/KATALOG.json` and
> `COMPLIANCE/fremdkomponenten.json`.

Myelith selbst steht unter der PolyForm Shield License 1.0.0 (siehe
`LICENSE.md`). Diese Datei nennt alles Fremde, was Myelith benutzt,
mitliefert oder auf den Rechner des Nutzers holt, mit der Lizenz, die
sein Anbieter angibt. Sie ist eine Auskunft und keine Rechtsberatung.

Myelith itself is licensed under the PolyForm Shield License 1.0.0
(see `LICENSE.md`). This file lists every third-party component that
Myelith uses, ships or fetches onto the user's machine, with the licence
stated by its provider. It is information, not legal advice.

## 1. Sprachmodelle / Language models

Die Artefakte sind ganzzahlig umgerechnete Bearbeitungen der Grundgewichte.
The artefacts are integer-converted derivative works of the base weights.

| Artefakt / artefact | Grundgewichte / base weights | Lizenz der Gewichte / weights licence | Lizenz des Artefakts / artefact licence |
|---|---|---|---|
| `myelith-0.6b` | [Qwen/Qwen3-0.6B](https://huggingface.co/Qwen/Qwen3-0.6B) | Apache-2.0 | PolyForm Shield License 1.0.0 |
| `myelith-30b-a3b` | [Qwen/Qwen3-30B-A3B](https://huggingface.co/Qwen/Qwen3-30B-A3B) | Apache-2.0 | PolyForm Shield License 1.0.0 |
| `myelith-35b-a3b` | [Qwen/Qwen3.6-35B-A3B](https://huggingface.co/Qwen/Qwen3.6-35B-A3B) | Apache-2.0 | PolyForm Shield License 1.0.0 |
| `myelith-4b` | [Qwen/Qwen3-4B](https://huggingface.co/Qwen/Qwen3-4B) | Apache-2.0 | PolyForm Shield License 1.0.0 |
| `myelith-8b` | [Qwen/Qwen3-8B](https://huggingface.co/Qwen/Qwen3-8B) | Apache-2.0 | PolyForm Shield License 1.0.0 |

## 2. Weitere Modelle / Further models

| Modell / model | Zweck / purpose | Lizenz / licence | Weg / how it arrives | geprüft / checked |
|---|---|---|---|---|
| [Fun-CosyVoice3-0.5B-2512](https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512) | Sprechen (Text zu Sprache, Stimmnachbildung) | **Apache-2.0** | vom Einrichtungsskript geholt / fetched by the setup script | 2026-09-25, Hugging-Face-API (license=apache-2.0) |
| [Whisper large-v3-turbo (ggml, q5_0)](https://huggingface.co/ggerganov/whisper.cpp) | Hoeren (Sprache zu Text) | **MIT** | vom Einrichtungsskript geholt / fetched by the setup script | 2026-09-25, Hugging-Face-API (openai/whisper-large-v3-turbo: license=mit; ggerganov/whisper.cpp: license=mit) |
| [SmolVLM2-2.2B-Instruct (GGUF, Q4_K_M)](https://huggingface.co/ggml-org/SmolVLM2-2.2B-Instruct-GGUF) | Sehen, schnelle Stufe (Bild zu Text) | **Apache-2.0** | vom Einrichtungsskript geholt / fetched by the setup script | 2026-09-25, Hugging-Face-API (HuggingFaceTB/SmolVLM2-2.2B-Instruct und ggml-org: license=apache-2.0) |
| [Qwen3-VL-4B-Instruct (GGUF, Q4_K_M)](https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF) | Sehen, genaue Stufe (Bild zu Text) | **Apache-2.0** | vom Einrichtungsskript geholt / fetched by the setup script | 2026-09-25, Hugging-Face-API (Qwen/Qwen3-VL-4B-Instruct und Qwen/Qwen3-VL-4B-Instruct-GGUF: license=apache-2.0) |

- **Fun-CosyVoice3-0.5B-2512:** Erzeugt synthetische Sprache; die Ausgabe wird als KI-erzeugt gekennzeichnet.

- **Qwen3-VL-4B-Instruct (GGUF, Q4_K_M):** Seit 2026-09-25 statt Qwen2.5-VL-3B-Instruct, dessen Original unter der Qwen Research License steht (Fund 468). Das Einrichtungsskript ersetzt eine alte Datei.

## 3. Programme und Systembestandteile / Programs and system components

| Programm / program | Zweck / purpose | Lizenz / licence | Weg / how it arrives |
|---|---|---|---|
| [llama.cpp (llama-mtmd-cli)](https://github.com/ggml-org/llama.cpp) | fuehrt die Sehmodelle aus | MIT | vom Nutzer installiert / installed by the user |
| [whisper.cpp (whisper-cli)](https://github.com/ggml-org/whisper.cpp) | fuehrt das Hoermodell aus | MIT | vom Nutzer installiert / installed by the user |
| [CosyVoice (Quelltext des Sprechmodells)](https://github.com/FunAudioLLM/CosyVoice) | fuehrt das Sprechmodell aus | Apache-2.0 | vom Einrichtungsskript geholt / fetched by the setup script |
| [FFmpeg](https://ffmpeg.org) | wandelt Tonaufnahmen um | LGPL-2.1-or-later (je nach Bau GPL-2.0-or-later) | vom Nutzer installiert / installed by the user |
| [piper](https://github.com/rhasspy/piper) | Rueckfall fuer das Sprechen, ohne Stimmnachbildung | MIT (rhasspy/piper) oder GPL-3.0-or-later (Nachfolger piper1-gpl), je nach Installation; die Stimmen tragen je eigene Lizenzen | vom Nutzer installiert / installed by the user |
| [Python](https://www.python.org) | Laufzeit fuer Kalibrierung und Sprechmodell | PSF-2.0 | vom Nutzer installiert / installed by the user |

## 4. Python-Pakete / Python packages

| Paket / package | Zweck / purpose | Lizenz / licence | Anforderung / requirement file | Weg / how it arrives |
|---|---|---|---|---|
| `accelerate` | Kalibrierung | Apache-2.0 | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `datasets` | Kalibrierung, Messung | Apache-2.0 | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `huggingface_hub` | Gewichte herunterladen | Apache-2.0 | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `numpy` | Kalibrierung | BSD-3-Clause | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `pypdf` | Trainingskorpus aus PDF (Rad liegt unter TRAINING/korpus/raeder) | BSD-3-Clause | `TRAINING/korpus/requirements.txt` | mitgeliefert / shipped |
| `safetensors` | Gewichte lesen und schreiben | Apache-2.0 | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `torch` | Kalibrierung, Sprechmodell | BSD-3-Clause | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `transformers` | Kalibrierung (Gleitkomma-Referenz) | Apache-2.0 | `INTEGER_LLM/calibrate/requirements.txt` | vom Einrichtungsskript geholt / fetched by the setup script |
| `typing_extensions` | Abhaengigkeit von pypdf (Rad liegt unter TRAINING/korpus/raeder) | PSF-2.0 | `TRAINING/korpus/requirements.txt` | mitgeliefert / shipped |

## 5. Rust-Kisten / Rust crates

760 Pakete aus 26 Kisten, offline aus `SYSTEM/crates-vorrat`. In die
Freigabebündel gehen nur die, die das jeweilige Programm zieht.

760 packages from 26 crates, offline from `SYSTEM/crates-vorrat`.
Release bundles contain only those the respective program pulls in.

| Lizenz / licence | Pakete / packages |
|---|---|
| MIT OR Apache-2.0 | 340 |
| MIT | 177 |
| Apache-2.0 OR MIT | 81 |
| Unicode-3.0 | 32 |
| MIT/Apache-2.0 | 31 |
| Zlib OR Apache-2.0 OR MIT | 19 |
| Apache-2.0 | 9 |
| Unlicense OR MIT | 9 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 6 |
| BSD-3-Clause | 6 |
| MPL-2.0 | 6 |
| Apache-2.0/MIT | 5 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 4 |
| ISC | 4 |
| MIT OR Apache-2.0 OR Zlib | 3 |
| Zlib | 3 |
| Apache-2.0 OR MIT OR Zlib | 2 |
| BSD-3-Clause OR MIT OR Apache-2.0 | 2 |
| MIT OR Apache-2.0 OR LGPL-2.1-or-later | 2 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| Unlicense/MIT | 2 |
| (MIT OR Apache-2.0) AND Apache-2.0 | 1 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| Apache-2.0 / MIT | 1 |
| Apache-2.0 AND ISC | 1 |
| Apache-2.0 AND MIT | 1 |
| Apache-2.0 OR BSL-1.0 | 1 |
| Apache-2.0 OR ISC OR MIT | 1 |
| Apache-2.0 WITH LLVM-exception | 1 |
| BSD-2-Clause | 1 |
| BSD-3-Clause AND MIT | 1 |
| BSD-3-Clause/MIT | 1 |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |
| MIT OR Apache-2.0 OR BSD-1-Clause | 1 |
| MIT OR BSD-3-Clause | 1 |

| Paket / package | Version | Lizenz / licence | Quelle / source |
|---|---|---|---|
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | https://github.com/oyvindln/adler2 |
| `aead` | 0.5.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `aes` | 0.8.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-ciphers |
| `aes-gcm` | 0.10.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs |
| `ahash` | 0.8.12 | MIT OR Apache-2.0 | https://github.com/tkaitchuck/ahash |
| `aho-corasick` | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick |
| `alloc-no-stdlib` | 2.0.4 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| `alloc-stdlib` | 0.2.4 | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib |
| `android_system_properties` | 0.1.6 | MIT OR Apache-2.0 | https://github.com/nical/android_system_properties |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| `arrayref` | 0.3.9 | BSD-2-Clause | https://github.com/droundy/arrayref |
| `asn1-rs` | 0.7.2 | MIT OR Apache-2.0 | https://github.com/rusticata/asn1-rs.git |
| `asn1-rs-derive` | 0.6.0 | MIT OR Apache-2.0 | https://github.com/rusticata/asn1-rs.git |
| `asn1-rs-impl` | 0.2.0 | MIT/Apache-2.0 | https://github.com/rusticata/asn1-rs.git |
| `async-channel` | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-channel |
| `async-io` | 2.6.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-io |
| `async-trait` | 0.1.92 | MIT OR Apache-2.0 | https://github.com/dtolnay/async-trait |
| `asynchronous-codec` | 0.7.0 | MIT | https://github.com/mxinden/asynchronous-codec |
| `atk` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `atk-sys` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `atomic-waker` | 1.1.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/atomic-waker |
| `attohttpc` | 0.30.1 | MPL-2.0 | https://github.com/sbstp/attohttpc |
| `autocfg` | 1.5.1 | Apache-2.0 OR MIT | https://github.com/cuviper/autocfg |
| `base-x` | 0.2.11 | MIT | https://github.com/OrKoN/base-x-rs |
| `base256emoji` | 1.0.2 | MIT | https://github.com/Jorropo/base256emoji/ |
| `base45` | 3.2.0 | MIT | https://github.com/opendevtools/base45 |
| `base64` | 0.13.1 | MIT/Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| `base64` | 0.21.7 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| `base64` | 0.22.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| `base64ct` | 1.8.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| `bit-set` | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-set |
| `bit-vec` | 0.8.0 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-vec |
| `bitflags` | 1.3.2 | MIT/Apache-2.0 | https://github.com/bitflags/bitflags |
| `bitflags` | 2.13.1 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| `blake2` | 0.10.6 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `block-buffer` | 0.12.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `block2` | 0.6.2 | MIT | https://github.com/madsmtm/objc2 |
| `blst` | 0.3.17 | Apache-2.0 | https://github.com/supranational/blst |
| `borsh` | 1.8.1 | MIT OR Apache-2.0 | https://github.com/near/borsh-rs |
| `borsh-derive` | 1.8.1 | Apache-2.0 | https://github.com/near/borsh-rs |
| `brotli` | 8.0.4 | BSD-3-Clause AND MIT | https://github.com/dropbox/rust-brotli |
| `brotli-decompressor` | 5.0.3 | BSD-3-Clause/MIT | https://github.com/dropbox/rust-brotli-decompressor |
| `bs58` | 0.5.1 | MIT/Apache-2.0 | https://github.com/Nullus157/bs58-rs |
| `bumpalo` | 3.20.3 | MIT OR Apache-2.0 | https://github.com/fitzgen/bumpalo |
| `bytemuck` | 1.25.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck |
| `byteorder` | 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| `bytes` | 1.12.1 | MIT | https://github.com/tokio-rs/bytes |
| `cairo-rs` | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `cairo-sys-rs` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `camino` | 1.2.5 | MIT OR Apache-2.0 | https://github.com/camino-rs/camino |
| `cargo-platform` | 0.1.9 | MIT OR Apache-2.0 | https://github.com/rust-lang/cargo |
| `cargo_metadata` | 0.19.2 | MIT | https://github.com/oli-obk/cargo_metadata |
| `cargo_toml` | 0.22.3 | Apache-2.0 OR MIT | https://gitlab.com/lib.rs/cargo_toml |
| `castaway` | 0.2.4 | MIT | https://github.com/sagebind/castaway |
| `cc` | 1.4.2 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `cc` | 1.4.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `cc` | 1.4.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `cc` | 1.4.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `cesu8` | 1.1.0 | Apache-2.0/MIT | https://github.com/emk/cesu8-rs |
| `cfb` | 0.7.3 | MIT | https://github.com/mdsteele/rust-cfb |
| `cfg-expr` | 0.15.8 | MIT OR Apache-2.0 | https://github.com/EmbarkStudios/cfg-expr |
| `cfg-if` | 1.0.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| `cfg_aliases` | 0.2.2 | MIT | https://github.com/katharostech/cfg_aliases |
| `chacha20` | 0.10.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/stream-ciphers |
| `chacha20` | 0.9.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/stream-ciphers |
| `chacha20poly1305` | 0.10.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs/tree/master/chacha20poly1305 |
| `chrono` | 0.4.45 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| `cipher` | 0.4.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `cmov` | 0.5.4 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| `combine` | 4.6.8 | MIT | https://github.com/Marwes/combine |
| `compact_str` | 0.9.1 | MIT | https://github.com/ParkMyCar/compact_str |
| `concurrent-queue` | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/concurrent-queue |
| `console` | 0.15.11 | MIT | https://github.com/console-rs/console |
| `const-oid` | 0.10.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| `const-oid` | 0.9.6 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/const-oid |
| `const-str` | 0.4.3 | MIT | https://github.com/Nugine/const-str |
| `convert_case` | 0.10.0 | MIT | https://github.com/rutrum/convert-case |
| `cookie` | 0.18.2 | MIT OR Apache-2.0 | https://github.com/SergioBenitez/cookie-rs |
| `core-foundation` | 0.10.1 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| `core-foundation` | 0.9.4 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| `core-foundation-sys` | 0.8.7 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| `core-graphics` | 0.25.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| `core-graphics-types` | 0.2.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `cpufeatures` | 0.3.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `cpufeatures` | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `crc32fast` | 1.5.0 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| `crc32fast` | 1.5.1 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| `critical-section` | 1.2.0 | MIT OR Apache-2.0 | https://github.com/rust-embedded/critical-section |
| `crossbeam-channel` | 0.5.16 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-channel` | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-deque` | 0.8.7 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-deque` | 0.8.8 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-epoch` | 0.9.20 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-epoch` | 0.9.21 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-utils` | 0.8.22 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossbeam-utils` | 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| `crossterm` | 0.29.0 | MIT | https://github.com/crossterm-rs/crossterm |
| `crossterm_winapi` | 0.9.1 | MIT | https://github.com/crossterm-rs/crossterm-winapi |
| `crunchy` | 0.2.4 | MIT | https://github.com/eira-fransham/crunchy |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `crypto-common` | 0.2.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `cssparser` | 0.36.0 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| `cssparser-macros` | 0.6.1 | MPL-2.0 | https://github.com/servo/rust-cssparser |
| `ctor` | 0.8.0 | Apache-2.0 OR MIT | https://github.com/mmastrac/rust-ctor |
| `ctor-proc-macro` | 0.0.7 | Apache-2.0 OR MIT | https://github.com/mmastrac/rust-ctor |
| `ctr` | 0.9.2 | MIT OR Apache-2.0 | https://github.com/RustCrypto/block-modes |
| `ctutils` | 0.4.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| `curve25519-dalek` | 4.1.3 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/curve25519-dalek |
| `curve25519-dalek-derive` | 0.1.1 | MIT/Apache-2.0 | https://github.com/dalek-cryptography/curve25519-dalek |
| `darling` | 0.20.11 | MIT | https://github.com/TedDriggs/darling |
| `darling` | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| `darling_core` | 0.20.11 | MIT | https://github.com/TedDriggs/darling |
| `darling_core` | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| `darling_macro` | 0.20.11 | MIT | https://github.com/TedDriggs/darling |
| `darling_macro` | 0.24.1 | MIT | https://github.com/TedDriggs/darling |
| `dary_heap` | 0.3.9 | MIT OR Apache-2.0 | https://github.com/hanmertens/dary_heap |
| `data-encoding` | 2.11.1 | MIT | https://github.com/ia0/data-encoding |
| `data-encoding-macro` | 0.1.21 | MIT | https://github.com/ia0/data-encoding |
| `data-encoding-macro-internal` | 0.1.19 | MIT | https://github.com/ia0/data-encoding |
| `dbus` | 0.9.12 | Apache-2.0/MIT | https://github.com/diwic/dbus-rs |
| `defmt` | 1.1.1 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| `defmt-macros` | 1.1.1 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| `defmt-parser` | 1.0.0 | MIT OR Apache-2.0 | https://github.com/knurling-rs/defmt |
| `der` | 0.7.10 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/der |
| `der` | 0.8.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| `der-parser` | 10.0.0 | MIT OR Apache-2.0 | https://github.com/rusticata/der-parser.git |
| `deranged` | 0.5.8 | MIT OR Apache-2.0 | https://github.com/jhpratt/deranged |
| `derive_builder` | 0.20.2 | MIT OR Apache-2.0 | https://github.com/colin-kiegel/rust-derive-builder |
| `derive_builder_core` | 0.20.2 | MIT OR Apache-2.0 | https://github.com/colin-kiegel/rust-derive-builder |
| `derive_builder_macro` | 0.20.2 | MIT OR Apache-2.0 | https://github.com/colin-kiegel/rust-derive-builder |
| `derive_more` | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| `derive_more-impl` | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| `digest` | 0.10.7 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `digest` | 0.11.3 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `dirs` | 6.0.0 | MIT OR Apache-2.0 | https://github.com/soc/dirs-rs |
| `dirs-sys` | 0.5.0 | MIT OR Apache-2.0 | https://github.com/dirs-dev/dirs-sys-rs |
| `dispatch2` | 0.3.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `displaydoc` | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc |
| `dlopen2` | 0.8.2 | MIT | https://github.com/OpenByteDev/dlopen2 |
| `dlopen2_derive` | 0.4.3 | MIT | https://github.com/OpenByteDev/dlopen2 |
| `document-features` | 0.2.12 | MIT OR Apache-2.0 | https://github.com/slint-ui/document-features |
| `dom_query` | 0.27.0 | MIT | https://github.com/niklak/dom_query |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit |
| `dtoa` | 1.0.11 | MIT OR Apache-2.0 | https://github.com/dtolnay/dtoa |
| `dtoa-short` | 0.3.5 | MPL-2.0 | https://github.com/upsuper/dtoa-short |
| `dtor` | 0.3.0 | Apache-2.0 OR MIT | https://github.com/mmastrac/rust-ctor |
| `dtor-proc-macro` | 0.0.6 | Apache-2.0 OR MIT | https://github.com/mmastrac/rust-ctor |
| `dunce` | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://gitlab.com/kornelski/dunce |
| `dyn-clone` | 1.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/dyn-clone |
| `ed25519` | 2.2.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/signatures/tree/master/ed25519 |
| `ed25519-dalek` | 2.2.0 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/ed25519-dalek |
| `either` | 1.17.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/either |
| `either` | 1.18.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/either |
| `embed-resource` | 3.0.11 | MIT | https://github.com/nabijaczleweli/rust-embed-resource |
| `embed_plist` | 1.2.2 | MIT OR Apache-2.0 | https://github.com/nvzqz/embed-plist-rs |
| `encode_unicode` | 1.0.0 | Apache-2.0 OR MIT | https://github.com/tormol/encode_unicode |
| `enum-as-inner` | 0.6.1 | MIT/Apache-2.0 | https://github.com/bluejekyll/enum-as-inner |
| `equivalent` | 1.0.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/equivalent |
| `erased-serde` | 0.4.10 | MIT OR Apache-2.0 | https://github.com/dtolnay/erased-serde |
| `errno` | 0.3.14 | MIT OR Apache-2.0 | https://github.com/lambda-fairy/rust-errno |
| `esaxx-rs` | 0.1.10 | Apache-2.0 | https://github.com/Narsil/esaxx-rs |
| `event-listener` | 5.4.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/event-listener |
| `event-listener-strategy` | 0.5.4 | Apache-2.0 OR MIT | https://github.com/smol-rs/event-listener-strategy |
| `fastbloom` | 0.17.0 | MIT OR Apache-2.0 | https://github.com/tomtomwombat/fastbloom/ |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/fastrand |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate |
| `fiat-crypto` | 0.2.9 | MIT OR Apache-2.0 OR BSD-1-Clause | https://github.com/mit-plv/fiat-crypto |
| `field-offset` | 0.3.6 | MIT OR Apache-2.0 | https://github.com/Diggsey/rust-field-offset |
| `find-msvc-tools` | 0.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `find-msvc-tools` | 0.1.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `find-msvc-tools` | 0.1.12 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs |
| `fnv` | 1.0.7 | Apache-2.0 / MIT | https://github.com/servo/rust-fnv |
| `foldhash` | 0.1.5 | Zlib | https://github.com/orlp/foldhash |
| `foldhash` | 0.2.0 | Zlib | https://github.com/orlp/foldhash |
| `foreign-types` | 0.5.0 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| `foreign-types-macros` | 0.2.4 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| `foreign-types-shared` | 0.3.1 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types |
| `form_urlencoded` | 1.2.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| `futures` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-bounded` | 0.2.4 | MIT | https://github.com/thomaseizinger/rust-futures-bounded |
| `futures-channel` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-core` | 0.3.33 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-executor` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-io` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-lite` | 2.6.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/futures-lite |
| `futures-macro` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-rustls` | 0.26.0 | MIT/Apache-2.0 | https://github.com/quininer/futures-rustls |
| `futures-sink` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-task` | 0.3.33 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-timer` | 3.0.4 | MIT/Apache-2.0 | https://github.com/async-rs/futures-timer |
| `futures-util` | 0.3.33 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| `gdk` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `gdk-pixbuf` | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `gdk-pixbuf-sys` | 0.18.0 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `gdk-sys` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `gdkwayland-sys` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `gdkx11` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `gdkx11-sys` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `generic-array` | 0.14.7 | MIT | https://github.com/fizyk20/generic-array.git |
| `getrandom` | 0.2.17 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| `getrandom` | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| `ghash` | 0.5.1 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| `gio` | 0.18.4 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `gio-sys` | 0.18.1 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `glib` | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `glib-macros` | 0.18.5 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `glib-sys` | 0.18.1 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `glob` | 0.3.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/glob |
| `gobject-sys` | 0.18.0 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `gtk` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `gtk-sys` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `gtk3-macros` | 0.18.2 | MIT | https://github.com/gtk-rs/gtk3-rs |
| `h2` | 0.4.15 | MIT | https://github.com/hyperium/h2 |
| `h2` | 0.4.19 | MIT | https://github.com/hyperium/h2 |
| `hashbrown` | 0.12.3 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| `hashbrown` | 0.14.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| `hashbrown` | 0.15.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| `hashlink` | 0.10.0 | MIT OR Apache-2.0 | https://github.com/kyren/hashlink |
| `hashlink` | 0.9.1 | MIT OR Apache-2.0 | https://github.com/kyren/hashlink |
| `heck` | 0.4.1 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| `heck` | 0.5.0 | MIT OR Apache-2.0 | https://github.com/withoutboats/heck |
| `hermit-abi` | 0.5.2 | MIT OR Apache-2.0 | https://github.com/hermit-os/hermit-rs |
| `hermit-abi` | 0.5.3 | MIT OR Apache-2.0 | https://github.com/hermit-os/hermit-rs |
| `hex` | 0.4.3 | MIT OR Apache-2.0 | https://github.com/KokaKiwi/rust-hex |
| `hex_fmt` | 0.3.0 | MIT/Apache-2.0 | https://github.com/poanetwork/hex_fmt |
| `hickory-proto` | 0.25.2 | MIT OR Apache-2.0 | https://github.com/hickory-dns/hickory-dns |
| `hickory-resolver` | 0.25.2 | MIT OR Apache-2.0 | https://github.com/hickory-dns/hickory-dns |
| `hkdf` | 0.12.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/KDFs/ |
| `hmac` | 0.12.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/MACs |
| `html5ever` | 0.38.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| `http` | 1.5.0 | MIT OR Apache-2.0 | https://github.com/hyperium/http |
| `http-body` | 1.1.0 | MIT | https://github.com/hyperium/http-body |
| `http-body-util` | 0.1.5 | MIT | https://github.com/hyperium/http-body |
| `httparse` | 1.10.1 | MIT OR Apache-2.0 | https://github.com/seanmonstar/httparse |
| `hybrid-array` | 0.4.14 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hybrid-array |
| `hyper` | 1.11.0 | MIT | https://github.com/hyperium/hyper |
| `hyper` | 1.11.1 | MIT | https://github.com/hyperium/hyper |
| `hyper-util` | 0.1.20 | MIT | https://github.com/hyperium/hyper-util |
| `iana-time-zone` | 0.1.65 | MIT OR Apache-2.0 | https://github.com/strawlab/iana-time-zone |
| `iana-time-zone-haiku` | 0.1.2 | MIT OR Apache-2.0 | https://github.com/strawlab/iana-time-zone |
| `ico` | 0.5.0 | MIT | https://github.com/mdsteele/rust-ico |
| `icu_collections` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_collections` | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_locale_core` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_locale_core` | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_normalizer` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_normalizer` | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_normalizer_data` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_normalizer_data` | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_properties` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_properties` | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_properties_data` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_properties_data` | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_provider` | 2.2.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `icu_provider` | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `ident_case` | 1.0.1 | MIT/Apache-2.0 | https://github.com/TedDriggs/ident_case |
| `idna` | 1.1.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| `idna_adapter` | 1.2.2 | Apache-2.0 OR MIT | https://github.com/hsivonen/idna_adapter |
| `if-addrs` | 0.15.0 | MIT OR BSD-3-Clause | https://github.com/messense/if-addrs |
| `if-watch` | 3.2.2 | MIT OR Apache-2.0 | https://github.com/libp2p/if-watch |
| `igd-next` | 0.16.2 | MIT | https://github.com/dariusc93/rust-igd |
| `indexmap` | 1.9.3 | Apache-2.0 OR MIT | https://github.com/bluss/indexmap |
| `indexmap` | 2.14.0 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| `indexmap` | 2.14.1 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| `indicatif` | 0.17.11 | MIT | https://github.com/console-rs/indicatif |
| `infer` | 0.19.0 | MIT | https://github.com/bojand/infer |
| `inout` | 0.1.4 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `ipconfig` | 0.3.4 | MIT/Apache-2.0 | https://github.com/liranringel/ipconfig |
| `ipnet` | 2.12.1 | MIT OR Apache-2.0 | https://github.com/krisprice/ipnet |
| `ipnet` | 2.12.2 | MIT OR Apache-2.0 | https://github.com/krisprice/ipnet |
| `itertools` | 0.13.0 | MIT OR Apache-2.0 | https://github.com/rust-itertools/itertools |
| `itertools` | 0.14.0 | MIT OR Apache-2.0 | https://github.com/rust-itertools/itertools |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| `javascriptcore-rs` | 1.1.2 | MIT | https://github.com/tauri-apps/javascriptcore-rs |
| `javascriptcore-rs-sys` | 1.1.1 | MIT | https://github.com/tauri-apps/javascriptcore-rs |
| `jiff` | 0.2.35 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| `jiff-core` | 0.1.0 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| `jiff-static` | 0.2.35 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| `jiff-tzdb` | 0.1.8 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| `jiff-tzdb-platform` | 0.1.3 | Unlicense OR MIT | https://github.com/BurntSushi/jiff |
| `jni` | 0.21.1 | MIT/Apache-2.0 | https://github.com/jni-rs/jni-rs |
| `jni-sys` | 0.3.1 | MIT OR Apache-2.0 | https://github.com/jni-rs/jni-sys |
| `jni-sys` | 0.4.1 | MIT OR Apache-2.0 | https://github.com/jni-rs/jni-sys |
| `jni-sys-macros` | 0.4.1 | MIT OR Apache-2.0 | https://github.com/jni-rs/jni-sys |
| `js-sys` | 0.3.104 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys |
| `js-sys` | 0.3.105 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys |
| `json-patch` | 3.0.1 | MIT/Apache-2.0 | https://github.com/idubrov/json-patch |
| `jsonptr` | 0.6.3 | MIT OR Apache-2.0 | https://github.com/chanced/jsonptr |
| `keccak` | 0.2.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/sponges |
| `kem` | 0.3.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits |
| `keyboard-types` | 0.7.0 | MIT OR Apache-2.0 | https://github.com/pyfisch/keyboard-types |
| `lazy_static` | 1.5.0 | MIT OR Apache-2.0 | https://github.com/rust-lang-nursery/lazy-static.rs |
| `libappindicator` | 0.9.0 | Apache-2.0 OR MIT |  |
| `libappindicator-sys` | 0.9.0 | Apache-2.0 OR MIT |  |
| `libc` | 0.2.189 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc |
| `libdbus-sys` | 0.2.7 | Apache-2.0/MIT | https://github.com/diwic/dbus-rs |
| `libloading` | 0.7.4 | ISC | https://github.com/nagisa/rust_libloading/ |
| `libm` | 0.2.16 | MIT | https://github.com/rust-lang/compiler-builtins |
| `libp2p` | 0.56.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-allow-block-list` | 0.6.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-autonat` | 0.15.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-connection-limits` | 0.6.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-core` | 0.43.2 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-dcutr` | 0.14.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-dns` | 0.44.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-gossipsub` | 0.49.5 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-identify` | 0.47.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-identity` | 0.2.14 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-kad` | 0.48.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-mdns` | 0.48.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-metrics` | 0.17.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-noise` | 0.46.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-ping` | 0.47.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-quic` | 0.13.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-relay` | 0.21.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-request-response` | 0.29.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-swarm` | 0.47.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-swarm-derive` | 0.35.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-tcp` | 0.44.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-tls` | 0.6.2 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-upnp` | 0.5.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libp2p-yamux` | 0.47.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `libredox` | 0.1.23 | MIT | https://gitlab.redox-os.org/redox-os/libredox.git |
| `linux-raw-sys` | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/sunfishcode/linux-raw-sys |
| `litemap` | 0.8.2 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `litemap` | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `litrs` | 1.0.0 | MIT OR Apache-2.0 | https://github.com/LukasKalbertodt/litrs |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| `log` | 0.4.33 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| `log` | 0.4.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| `lru-slab` | 0.1.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/Ralith/lru-slab |
| `macro_rules_attribute` | 0.2.3 | Apache-2.0 OR MIT OR Zlib | https://github.com/danielhenrymantilla/macro_rules_attribute-rs |
| `macro_rules_attribute-proc_macro` | 0.2.3 | Apache-2.0 OR MIT OR Zlib | https://github.com/danielhenrymantilla/macro_rules_attribute-rs |
| `markup5ever` | 0.38.0 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| `match-lookup` | 0.1.2 | MIT | https://github.com/mriise/smol-base-x |
| `memchr` | 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| `memmap2` | 0.9.11 | MIT OR Apache-2.0 | https://github.com/RazrFalcon/memmap2-rs |
| `memoffset` | 0.9.1 | MIT | https://github.com/Gilnaa/memoffset |
| `mime` | 0.3.17 | MIT OR Apache-2.0 | https://github.com/hyperium/mime |
| `minimal-lexical` | 0.2.1 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/minimal-lexical |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| `mio` | 1.2.2 | MIT | https://github.com/tokio-rs/mio |
| `mio` | 1.2.3 | MIT | https://github.com/tokio-rs/mio |
| `ml-kem` | 0.3.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/KEMs |
| `module-lattice` | 0.2.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/KEMs |
| `moka` | 0.12.16 | (MIT OR Apache-2.0) AND Apache-2.0 | https://github.com/moka-rs/moka |
| `monostate` | 0.1.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/monostate |
| `monostate-impl` | 0.1.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/monostate |
| `muda` | 0.19.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/muda |
| `multiaddr` | 0.18.2 | MIT | https://github.com/multiformats/rust-multiaddr |
| `multibase` | 0.9.3 | MIT | https://github.com/multiformats/rust-multibase |
| `multihash` | 0.19.5 | MIT | https://github.com/multiformats/rust-multihash |
| `multistream-select` | 0.13.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `ndk` | 0.9.0 | MIT OR Apache-2.0 | https://github.com/rust-mobile/ndk |
| `ndk-sys` | 0.6.0+11769913 | MIT OR Apache-2.0 | https://github.com/rust-mobile/ndk |
| `netlink-packet-core` | 0.8.2 | MIT | https://github.com/rust-netlink/netlink-packet-core |
| `netlink-packet-route` | 0.28.0 | MIT | https://github.com/rust-netlink/netlink-packet-route |
| `netlink-proto` | 0.12.1 | MIT | https://github.com/rust-netlink/netlink-proto |
| `netlink-proto` | 0.12.2 | MIT | https://github.com/rust-netlink/netlink-proto |
| `netlink-sys` | 0.8.8 | MIT | https://github.com/rust-netlink/netlink-sys |
| `new_debug_unreachable` | 1.0.6 | MIT | https://github.com/mbrubeck/rust-debug-unreachable |
| `nix` | 0.30.1 | MIT | https://github.com/nix-rust/nix |
| `nohash-hasher` | 0.2.0 | Apache-2.0 OR MIT | https://github.com/paritytech/nohash-hasher |
| `nom` | 7.1.3 | MIT | https://github.com/Geal/nom |
| `num-bigint` | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-num/num-bigint |
| `num-conv` | 0.2.2 | MIT OR Apache-2.0 | https://github.com/jhpratt/num-conv |
| `num-integer` | 0.1.47 | MIT OR Apache-2.0 | https://github.com/rust-num/num-integer |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits |
| `num_cpus` | 1.17.0 | MIT OR Apache-2.0 | https://github.com/seanmonstar/num_cpus |
| `num_enum` | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | https://github.com/illicitonion/num_enum |
| `num_enum_derive` | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | https://github.com/illicitonion/num_enum |
| `number_prefix` | 0.4.0 | MIT | https://github.com/ogham/rust-number-prefix |
| `objc2` | 0.6.4 | MIT | https://github.com/madsmtm/objc2 |
| `objc2-app-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-cloud-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-core-data` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-core-foundation` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-core-graphics` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-core-image` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-core-location` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-core-text` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-encode` | 4.1.0 | MIT | https://github.com/madsmtm/objc2 |
| `objc2-exception-helper` | 0.1.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-foundation` | 0.3.2 | MIT | https://github.com/madsmtm/objc2 |
| `objc2-io-surface` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-metal` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-quartz-core` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-ui-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-user-notifications` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `objc2-web-kit` | 0.3.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/madsmtm/objc2 |
| `oid-registry` | 0.8.1 | MIT OR Apache-2.0 | https://github.com/rusticata/oid-registry.git |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| `onig` | 6.5.3 | MIT | https://github.com/iwillspeak/rust-onig |
| `onig_sys` | 69.9.3 | MIT | https://github.com/rust-onig/rust-onig |
| `opaque-debug` | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| `option-ext` | 0.2.0 | MPL-2.0 | https://github.com/soc/option-ext.git |
| `pango` | 0.18.3 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `pango-sys` | 0.18.0 | MIT | https://github.com/gtk-rs/gtk-rs-core |
| `parking` | 2.2.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/parking |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| `paste` | 1.0.15 | MIT OR Apache-2.0 | https://github.com/dtolnay/paste |
| `pastey` | 0.2.3 | MIT OR Apache-2.0 | https://github.com/as1100k/pastey |
| `pem` | 3.0.6 | MIT | https://github.com/jcreekmore/pem-rs.git |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| `phf` | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| `phf_codegen` | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| `phf_generator` | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| `phf_macros` | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| `phf_shared` | 0.13.1 | MIT | https://github.com/rust-phf/rust-phf |
| `pin-project` | 1.1.13 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project |
| `pin-project-internal` | 1.1.13 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project-lite |
| `pkcs8` | 0.10.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/pkcs8 |
| `pkcs8` | 0.11.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| `pkg-config` | 0.3.33 | MIT OR Apache-2.0 | https://github.com/rust-lang/pkg-config-rs |
| `pkg-config` | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/pkg-config-rs |
| `plist` | 1.10.1 | MIT | https://github.com/ebarnard/rust-plist/ |
| `png` | 0.17.16 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| `png` | 0.18.1 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| `polling` | 3.11.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/polling |
| `poly1305` | 0.8.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| `polyval` | 0.6.2 | Apache-2.0 OR MIT | https://github.com/RustCrypto/universal-hashes |
| `portable-atomic` | 1.15.0 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic |
| `portable-atomic-util` | 0.2.8 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic-util |
| `potential_utf` | 0.1.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `potential_utf` | 0.1.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `powerfmt` | 0.2.0 | MIT OR Apache-2.0 | https://github.com/jhpratt/powerfmt |
| `ppv-lite86` | 0.2.21 | MIT OR Apache-2.0 | https://github.com/cryptocorrosion/cryptocorrosion |
| `precomputed-hash` | 0.1.1 | MIT | https://github.com/emilio/precomputed-hash |
| `proc-macro-crate` | 1.3.1 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| `proc-macro-crate` | 2.0.0 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| `proc-macro-crate` | 3.5.0 | MIT OR Apache-2.0 | https://github.com/bkchr/proc-macro-crate |
| `proc-macro-error` | 1.0.4 | MIT OR Apache-2.0 | https://gitlab.com/CreepySkeleton/proc-macro-error |
| `proc-macro-error-attr` | 1.0.4 | MIT OR Apache-2.0 | https://gitlab.com/CreepySkeleton/proc-macro-error |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| `prometheus-client` | 0.23.1 | Apache-2.0 OR MIT | https://github.com/prometheus/client_rust |
| `prometheus-client-derive-encode` | 0.4.2 | Apache-2.0 OR MIT | https://github.com/prometheus/client_rust |
| `prost` | 0.14.4 | Apache-2.0 | https://github.com/tokio-rs/prost |
| `prost-derive` | 0.14.4 | Apache-2.0 | https://github.com/tokio-rs/prost |
| `quick-protobuf` | 0.8.1 | MIT | https://github.com/tafia/quick-protobuf |
| `quick-protobuf-codec` | 0.3.1 | MIT | https://github.com/libp2p/rust-libp2p |
| `quick-xml` | 0.42.0 | MIT | https://github.com/tafia/quick-xml |
| `quinn` | 0.11.11 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| `quinn-proto` | 0.11.16 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| `quinn-proto` | 0.11.17 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| `quinn-udp` | 0.5.15 | MIT OR Apache-2.0 | https://github.com/quinn-rs/quinn |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| `r-efi` | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | https://github.com/r-efi/r-efi |
| `r-efi` | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | https://github.com/r-efi/r-efi |
| `rand` | 0.10.2 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand` | 0.8.7 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand` | 0.9.5 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand_chacha` | 0.3.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand_chacha` | 0.9.0 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand_core` | 0.10.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand_core |
| `rand_core` | 0.6.4 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand_core` | 0.9.5 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| `rand_pcg` | 0.10.2 | MIT OR Apache-2.0 | https://github.com/rust-random/rngs |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| `rayon` | 1.12.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/rayon |
| `rayon-cond` | 0.4.0 | Apache-2.0/MIT | https://github.com/cuviper/rayon-cond |
| `rayon-core` | 1.13.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/rayon |
| `rcgen` | 0.13.2 | MIT OR Apache-2.0 | https://github.com/rustls/rcgen |
| `redox_syscall` | 0.5.18 | MIT | https://gitlab.redox-os.org/redox-os/syscall |
| `redox_users` | 0.5.2 | MIT | https://gitlab.redox-os.org/redox-os/users |
| `ref-cast` | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| `ref-cast-impl` | 1.0.27 | MIT OR Apache-2.0 | https://github.com/dtolnay/ref-cast |
| `regex` | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| `regex-automata` | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| `regex-syntax` | 0.8.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| `reqwest` | 0.13.4 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| `resolv-conf` | 0.7.6 | MIT OR Apache-2.0 | https://github.com/hickory-dns/resolv-conf |
| `rfd` | 0.16.0 | MIT | https://github.com/PolyMeilex/rfd |
| `ring` | 0.17.14 | Apache-2.0 AND ISC | https://github.com/briansmith/ring |
| `rtnetlink` | 0.20.0 | MIT | https://github.com/rust-netlink/rtnetlink |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash |
| `rustc_version` | 0.4.1 | MIT OR Apache-2.0 | https://github.com/djc/rustc-version-rs |
| `rusticata-macros` | 4.1.0 | MIT/Apache-2.0 | https://github.com/rusticata/rusticata-macros.git |
| `rustix` | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/rustix |
| `rustix` | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/rustix |
| `rustls` | 0.23.45 | Apache-2.0 OR ISC OR MIT | https://github.com/rustls/rustls |
| `rustls-pki-types` | 1.15.1 | MIT OR Apache-2.0 | https://github.com/rustls/pki-types |
| `rustls-webpki` | 0.103.14 | ISC | https://github.com/rustls/webpki |
| `rustls-webpki` | 0.103.15 | ISC | https://github.com/rustls/webpki |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | https://github.com/dtolnay/rustversion |
| `rw-stream-sink` | 0.4.0 | MIT | https://github.com/libp2p/rust-libp2p |
| `ryu` | 1.0.23 | Apache-2.0 OR BSL-1.0 | https://github.com/dtolnay/ryu |
| `same-file` | 1.0.6 | Unlicense/MIT | https://github.com/BurntSushi/same-file |
| `schemars` | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| `schemars` | 0.9.0 | MIT | https://github.com/GREsau/schemars |
| `schemars` | 1.2.2 | MIT | https://github.com/GREsau/schemars |
| `schemars_derive` | 0.8.22 | MIT | https://github.com/GREsau/schemars |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bluss/scopeguard |
| `selectors` | 0.36.1 | MPL-2.0 | https://github.com/servo/stylo |
| `semver` | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| `serde-untagged` | 0.1.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-untagged |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| `serde_derive_internals` | 0.29.1 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| `serde_repr` | 0.1.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/serde-repr |
| `serde_spanned` | 0.6.9 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `serde_spanned` | 1.1.1 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `serde_with` | 3.23.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| `serde_with_macros` | 3.23.0 | MIT OR Apache-2.0 | https://github.com/jonasbb/serde_with/ |
| `serialize-to-javascript` | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| `serialize-to-javascript-impl` | 0.1.2 | MIT OR Apache-2.0 | https://github.com/chippers/serialize-to-javascript |
| `servo_arc` | 0.4.3 | MIT OR Apache-2.0 | https://github.com/servo/stylo |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| `sha3` | 0.11.0 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 | https://github.com/comex/rust-shlex |
| `signal-hook` | 0.3.18 | Apache-2.0/MIT | https://github.com/vorner/signal-hook |
| `signal-hook-mio` | 0.2.5 | MIT OR Apache-2.0 | https://github.com/vorner/signal-hook |
| `signal-hook-registry` | 1.4.8 | MIT OR Apache-2.0 | https://github.com/vorner/signal-hook |
| `signature` | 2.2.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/traits/tree/master/signature |
| `simd-adler32` | 0.3.10 | MIT | https://github.com/mcountryman/simd-adler32 |
| `siphasher` | 1.0.3 | MIT/Apache-2.0 | https://github.com/jedisct1/rust-siphash |
| `slab` | 0.4.12 | MIT | https://github.com/tokio-rs/slab |
| `smallvec` | 1.15.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| `smallvec` | 1.16.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| `snow` | 0.9.6 | Apache-2.0 OR MIT | https://github.com/mcginty/snow |
| `socket2` | 0.5.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 |
| `socket2` | 0.6.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/socket2 |
| `softbuffer` | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-windowing/softbuffer |
| `soup3` | 0.5.0 | MIT | https://gitlab.gnome.org/World/Rust/soup3-rs |
| `soup3-sys` | 0.5.0 | MIT | https://gitlab.gnome.org/World/Rust/soup3-rs |
| `spki` | 0.7.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats/tree/master/spki |
| `spki` | 0.8.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/formats |
| `spm_precompiled` | 0.1.4 | Apache-2.0 | https://github.com/huggingface/spm_precompiled |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 | https://github.com/nvzqz/static-assertions-rs |
| `string_cache` | 0.9.0 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| `string_cache_codegen` | 0.6.1 | MIT OR Apache-2.0 | https://github.com/servo/string-cache |
| `strsim` | 0.11.1 | MIT | https://github.com/rapidfuzz/strsim-rs |
| `subtle` | 2.6.1 | BSD-3-Clause | https://github.com/dalek-cryptography/subtle |
| `swift-rs` | 1.0.8 | MIT OR Apache-2.0 | https://github.com/Brendonovich/swift-rs |
| `syn` | 1.0.109 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| `syn` | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| `syn` | 3.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| `syn` | 3.0.4 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| `syn` | 3.0.5 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| `sync_wrapper` | 1.0.2 | Apache-2.0 | https://github.com/Actyx/sync_wrapper |
| `synstructure` | 0.13.2 | MIT | https://github.com/mystor/synstructure |
| `system-configuration` | 0.7.0 | MIT OR Apache-2.0 | https://github.com/mullvad/system-configuration-rs |
| `system-configuration-sys` | 0.6.0 | MIT OR Apache-2.0 | https://github.com/mullvad/system-configuration-rs |
| `system-deps` | 6.2.2 | MIT OR Apache-2.0 | https://github.com/gdesmott/system-deps |
| `tagptr` | 0.2.0 | MIT/Apache-2.0 | https://github.com/oliver-giersch/tagptr.git |
| `tao` | 0.35.3 | Apache-2.0 | https://github.com/tauri-apps/tao |
| `tao-macros` | 0.1.4 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tao |
| `target-lexicon` | 0.12.16 | Apache-2.0 WITH LLVM-exception | https://github.com/bytecodealliance/target-lexicon |
| `tauri` | 2.11.5 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-build` | 2.6.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-codegen` | 2.6.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-macros` | 2.6.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-plugin` | 2.6.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-plugin-dialog` | 2.7.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| `tauri-plugin-fs` | 2.5.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| `tauri-runtime` | 2.11.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-runtime-wry` | 2.11.4 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-utils` | 2.9.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| `tauri-winres` | 0.3.6 | MIT | https://github.com/tauri-apps/winres |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile |
| `tendril` | 0.5.1 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| `thiserror` | 2.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| `thiserror-impl` | 2.0.20 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| `threadpool` | 1.8.1 | MIT/Apache-2.0 | https://github.com/rust-threadpool/rust-threadpool |
| `time` | 0.3.55 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| `time-core` | 0.1.9 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| `time-macros` | 0.2.32 | MIT OR Apache-2.0 | https://github.com/time-rs/time |
| `tinystr` | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `tinystr` | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `tinyvec` | 1.12.0 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| `tinyvec` | 1.13.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| `tinyvec_macros` | 0.1.1 | MIT OR Apache-2.0 OR Zlib | https://github.com/Soveu/tinyvec_macros |
| `tokenizers` | 0.21.4 | Apache-2.0 | https://github.com/huggingface/tokenizers |
| `tokio` | 1.53.1 | MIT | https://github.com/tokio-rs/tokio |
| `tokio-macros` | 2.7.2 | MIT | https://github.com/tokio-rs/tokio |
| `tokio-util` | 0.7.19 | MIT | https://github.com/tokio-rs/tokio |
| `toml` | 0.8.23 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml` | 0.9.12+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml` | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_datetime` | 0.6.11 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_datetime` | 0.7.5+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_datetime` | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_edit` | 0.19.15 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_edit` | 0.20.7 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_edit` | 0.22.27 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_edit` | 0.25.13+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_parser` | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_write` | 0.1.2 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `toml_writer` | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| `tower` | 0.5.3 | MIT | https://github.com/tower-rs/tower |
| `tower-http` | 0.6.11 | MIT | https://github.com/tower-rs/tower-http |
| `tower-layer` | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| `tower-service` | 0.3.3 | MIT | https://github.com/tower-rs/tower |
| `tracing` | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| `tracing-attributes` | 0.1.31 | MIT | https://github.com/tokio-rs/tracing |
| `tracing-core` | 0.1.36 | MIT | https://github.com/tokio-rs/tracing |
| `tray-icon` | 0.24.2 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tray-icon |
| `try-lock` | 0.2.5 | MIT | https://github.com/seanmonstar/try-lock |
| `typeid` | 1.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/typeid |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 | https://github.com/paholg/typenum |
| `uint` | 0.10.1 | MIT OR Apache-2.0 | https://github.com/paritytech/parity-common |
| `unic-char-property` | 0.9.0 | MIT/Apache-2.0 | https://github.com/open-i18n/rust-unic/ |
| `unic-char-range` | 0.9.0 | MIT/Apache-2.0 | https://github.com/open-i18n/rust-unic/ |
| `unic-common` | 0.9.0 | MIT/Apache-2.0 | https://github.com/open-i18n/rust-unic/ |
| `unic-ucd-ident` | 0.9.0 | MIT/Apache-2.0 | https://github.com/open-i18n/rust-unic/ |
| `unic-ucd-version` | 0.9.0 | MIT/Apache-2.0 | https://github.com/open-i18n/rust-unic/ |
| `unicode-ident` | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| `unicode-normalization-alignments` | 0.1.12 | MIT/Apache-2.0 | https://github.com/n1t0/unicode-normalization |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-segmentation |
| `unicode-width` | 0.2.2 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-width |
| `unicode_categories` | 0.1.1 | MIT OR Apache-2.0 | https://github.com/swgillespie/unicode-categories |
| `universal-hash` | 0.5.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/traits |
| `unsigned-varint` | 0.7.2 | MIT | https://github.com/paritytech/unsigned-varint |
| `unsigned-varint` | 0.8.0 | MIT | https://github.com/paritytech/unsigned-varint |
| `untrusted` | 0.9.0 | ISC | https://github.com/briansmith/untrusted |
| `url` | 2.5.8 | MIT OR Apache-2.0 | https://github.com/servo/rust-url |
| `urlpattern` | 0.3.0 | MIT | https://github.com/denoland/rust-urlpattern |
| `utf8_iter` | 1.0.4 | Apache-2.0 OR MIT | https://github.com/hsivonen/utf8_iter |
| `uuid` | 1.24.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| `uuid` | 1.25.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| `uuid` | 1.26.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| `version-compare` | 0.2.1 | MIT | https://gitlab.com/timvisee/version-compare |
| `version_check` | 0.9.5 | MIT/Apache-2.0 | https://github.com/SergioBenitez/version_check |
| `vswhom` | 0.1.0 | MIT | https://github.com/nabijaczleweli/vswhom.rs |
| `vswhom-sys` | 0.1.3 | MIT | https://github.com/nabijaczleweli/vswhom-sys.rs |
| `walkdir` | 2.5.0 | Unlicense/MIT | https://github.com/BurntSushi/walkdir |
| `want` | 0.3.1 | MIT | https://github.com/seanmonstar/want |
| `wasi` | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/wasi |
| `wasip2` | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/wasi-rs |
| `wasm-bindgen` | 0.2.127 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen |
| `wasm-bindgen` | 0.2.128 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen |
| `wasm-bindgen-futures` | 0.4.78 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/futures |
| `wasm-bindgen-macro` | 0.2.127 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro |
| `wasm-bindgen-macro` | 0.2.128 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro |
| `wasm-bindgen-macro-support` | 0.2.127 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro-support |
| `wasm-bindgen-macro-support` | 0.2.128 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/macro-support |
| `wasm-bindgen-shared` | 0.2.127 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared |
| `wasm-bindgen-shared` | 0.2.128 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared |
| `wasm-streams` | 0.5.0 | MIT OR Apache-2.0 | https://github.com/MattiasBuelens/wasm-streams/ |
| `web-sys` | 0.3.105 | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/web-sys |
| `web-time` | 1.1.0 | MIT OR Apache-2.0 | https://github.com/daxpedda/web-time |
| `web_atoms` | 0.2.6 | MIT OR Apache-2.0 | https://github.com/servo/html5ever |
| `webkit2gtk` | 2.0.2 | MIT | https://github.com/tauri-apps/webkit2gtk-rs |
| `webkit2gtk-sys` | 2.0.2 | MIT | https://github.com/tauri-apps/webkit2gtk-rs |
| `webview2-com` | 0.38.2 | MIT | https://github.com/wravery/webview2-rs |
| `webview2-com-macros` | 0.8.1 | MIT | https://github.com/wravery/webview2-rs |
| `webview2-com-sys` | 0.38.2 | MIT | https://github.com/wravery/webview2-rs |
| `widestring` | 1.2.1 | MIT OR Apache-2.0 | https://github.com/VoidStarKat/widestring-rs |
| `winapi` | 0.3.9 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| `winapi-i686-pc-windows-gnu` | 0.4.0 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| `winapi-util` | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util |
| `winapi-x86_64-pc-windows-gnu` | 0.4.0 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| `window-vibrancy` | 0.6.0 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri-plugin-vibrancy |
| `windows` | 0.61.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows` | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-collections` | 0.2.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-collections` | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-core` | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-core` | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-future` | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-future` | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-implement` | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-interface` | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-link` | 0.1.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-numerics` | 0.2.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-numerics` | 0.3.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-registry` | 0.6.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-result` | 0.3.4 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-result` | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-strings` | 0.4.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-strings` | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-sys` | 0.45.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-sys` | 0.52.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-sys` | 0.59.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-sys` | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-targets` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-targets` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-targets` | 0.53.5 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-threading` | 0.1.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-threading` | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows-version` | 0.1.7 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_aarch64_gnullvm` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_aarch64_gnullvm` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_aarch64_gnullvm` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_aarch64_msvc` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_aarch64_msvc` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_aarch64_msvc` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_gnu` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_gnu` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_gnu` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_gnullvm` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_gnullvm` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_msvc` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_msvc` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_i686_msvc` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnu` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnu` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnu` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnullvm` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnullvm` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_gnullvm` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_msvc` | 0.42.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_msvc` | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `windows_x86_64_msvc` | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| `winnow` | 0.5.40 | MIT | https://github.com/winnow-rs/winnow |
| `winnow` | 0.7.15 | MIT | https://github.com/winnow-rs/winnow |
| `winnow` | 1.0.4 | MIT | https://github.com/winnow-rs/winnow |
| `winreg` | 0.55.0 | MIT | https://github.com/gentoo90/winreg-rs |
| `wit-bindgen` | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | https://github.com/bytecodealliance/wit-bindgen |
| `writeable` | 0.6.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `writeable` | 0.6.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `wry` | 0.55.1 | Apache-2.0 OR MIT | https://github.com/tauri-apps/wry |
| `x11` | 2.21.0 | MIT | https://github.com/AltF02/x11-rs.git |
| `x11-dl` | 2.21.0 | MIT | https://github.com/AltF02/x11-rs.git |
| `x25519-dalek` | 2.0.1 | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/x25519-dalek |
| `x509-parser` | 0.17.0 | MIT OR Apache-2.0 | https://github.com/rusticata/x509-parser.git |
| `xml-rs` | 0.8.29 | MIT | https://github.com/kornelski/xml-rs |
| `xmltree` | 0.10.3 | MIT | https://github.com/eminence/xmltree-rs |
| `yamux` | 0.12.1 | Apache-2.0 OR MIT | https://github.com/paritytech/yamux |
| `yamux` | 0.13.10 | Apache-2.0 OR MIT | https://github.com/paritytech/yamux |
| `yasna` | 0.5.2 | MIT OR Apache-2.0 | https://github.com/qnighy/yasna.rs |
| `yoke` | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `yoke-derive` | 0.8.2 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerocopy` | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| `zerocopy` | 0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| `zerocopy-derive` | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| `zerocopy-derive` | 0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| `zerofrom` | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerofrom-derive` | 0.1.7 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zeroize` | 1.9.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| `zeroize_derive` | 1.5.0 | Apache-2.0 OR MIT | https://github.com/RustCrypto/utils |
| `zerotrie` | 0.2.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerotrie` | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerovec` | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerovec` | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerovec-derive` | 0.11.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zerovec-derive` | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| `zlib-rs` | 0.6.7 | Zlib | https://github.com/trifectatechfoundation/zlib-rs |
| `zmij` | 1.0.23 | MIT | https://github.com/dtolnay/zmij |

## 6. Daten / Data

| Datensatz / dataset | Zweck / purpose | Lizenz / licence | Weg / how it arrives | geprüft / checked |
|---|---|---|---|---|
| [WikiText-2 (wikitext-2-raw-v1, Testsplit)](https://huggingface.co/datasets/Salesforce/wikitext) | Kalibrierung der ganzzahligen Skalen und Messung der Perplexitaet; kein Training | CC-BY-SA-3.0 oder GFDL | vom Einrichtungsskript geholt / fetched by the setup script | 2026-09-25, Hugging-Face-API (license=cc-by-sa-3.0, gfdl) |

## 7. Medien / Media

| Inhalt / content | Urheber / author | Lizenz / licence | Weg / how it arrives |
|---|---|---|---|
| Symbole und Banner (CLIENT/myl-oberflaeche/icons, README/Grafiken) | Projektinhaber | PolyForm-Shield-1.0.0 (wie das Repositorium) | mitgeliefert / shipped |
| Schrift der Wortmarke (eingebetteter Ausschnitt, nur die Buchstaben von MYELITH) | Adam Nerland, Insanitype Font Design (2001) | ungeklaert | mitgeliefert / shipped |

- **Schrift der Wortmarke (eingebetteter Ausschnitt, nur die Buchstaben von MYELITH):** ⚠️ Die Schrift traegt kein Lizenzfeld. Ist sie nur zur privaten Nutzung frei, gehoert der Ausschnitt aus dem Stilblatt heraus. Entscheidung des Projektinhabers offen.
