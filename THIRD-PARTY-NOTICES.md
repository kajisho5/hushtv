# Third-party notices

## cleanyourtv

The per-model lists in `data/lists/` and the safety rules in `crates/hushtv-core/src/safety.rs` and `record.rs` are taken from https://github.com/Gathin23/cleanyourtv (commit recorded in `data/UPSTREAM.json`).

```
MIT License

Copyright (c) 2026 Gathin

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

---

The Outfit typeface in launcher/app/src/main/res/font/outfit.ttf is copyright
The Outfit Project Authors and licensed under the SIL Open Font License 1.1,
see launcher/FONT-LICENSE-OFL.txt.
```

## adb_client

https://github.com/cocool97/adb_client — MIT License, Copyright (c) Corentin LIAUD.

## libadb

`crates/hushtv-adb/src/pair.rs` (the SPAKE2 part of 6-digit pairing) follows https://github.com/tarigo/libadb `src/pairing/spake2.rs` — MIT License, Copyright (c) 2026 Igor Tarasov. It was checked line by line against BoringSSL's `crypto/curve25519/spake25519.cc` and against an independent textbook implementation.

## Inter (font)

`crates/hushtv-card/fonts/` holds a Latin subset of Inter, Copyright 2016-2023 The Inter Project Authors, licensed under the SIL Open Font License 1.1 (full text in `crates/hushtv-card/fonts/OFL.txt`).

## Rust crates

Other dependencies are listed in `Cargo.lock`; each keeps its own license (MIT, Apache-2.0 or both).
