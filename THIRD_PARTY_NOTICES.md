# Third-party notices

Terminal Effects distributions contain third-party source, compiled code,
fonts, and a pinned Electron/Chromium runtime. Exact JavaScript and Rust package
versions are pinned in `pnpm-lock.yaml` and `Cargo.lock` respectively.

## Imported renderer source

The renderer incorporates portions of
[terminal-browser](https://github.com/zenbu-labs/terminal-browser) tag `v0.5.8`
at commit `9265a06fb875e5ed810359bbf007bd46d1156a3a`, copyright 2026
Zenbu Labs, Inc., under the MIT license. The retained source is maintained in
`apps/renderer/`, and the complete license is in
`apps/renderer/LICENSE.terminal-browser` and release archives.

## JavaScript runtime packages

The editor and terminal renderer bundle the following production packages.
Type-only and build-only packages are not included in this table.

| Package | Version | License |
| --- | --- | --- |
| `js-tokens` | 4.0.0 | MIT; copyright 2014-2018 Simon Lydell |
| `loose-envify` | 1.4.0 | MIT; copyright 2015 Andres Suarez |
| `lucide-react` | 1.33.0 | ISC; copyright 2026 Lucide Icons and Contributors; selected Feather-derived icons are MIT, copyright 2013-present Cole Bemis |
| `react` | 18.3.1, 19.2.8 | MIT; copyright Facebook, Inc. and its affiliates / Meta Platforms, Inc. and affiliates |
| `react-dom` | 19.2.8 | MIT; copyright Meta Platforms, Inc. and affiliates |
| `react-reconciler` | 0.29.2 | MIT; copyright Facebook, Inc. and its affiliates |
| `scheduler` | 0.23.2, 0.27.0 | MIT; copyright Facebook, Inc. and its affiliates / Meta Platforms, Inc. and affiliates |
| `zustand` | 5.0.15 | MIT; copyright 2019 Paul Henschel |

## Rust runtime packages

The `te` executable and the renderer's `pixel.node` native library statically
link third-party Rust crates. Their complete supported-platform dependency graph
is pinned in `Cargo.lock`; package manifests provide the corresponding SPDX
license expressions. The graph is licensed under one or more of MIT,
Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, Zlib,
0BSD, Unlicense, and Unicode-3.0.

Direct runtime crates are `anyhow`, `arboard`, `base64`, `clap`, `fontdue`,
`image`, `libc`, `miniz_oxide`, `napi`, `napi-derive`, `rayon`, `rustix`,
`serde`, `serde_json`, `taffy`, `tiny_http`, `tiny-skia`, and `uuid`, together
with their transitive dependencies. In particular, `arrayref` is BSD-2-Clause;
`tiny-skia` and `tiny-skia-path` are BSD-3-Clause; `foldhash` and `slotmap` are
Zlib; and `unicode-ident` additionally includes Unicode-3.0 terms. Crates that
offer a choice including MIT or Apache-2.0 are used under those permissive
terms.

## Electron and Chromium

Renderer packages contain Electron 43.3.0 and Chromium 150.0.7871.212.
Electron is distributed under the MIT license. Chromium contains components
under several open-source licenses. Release archives preserve Electron's
`electron/LICENSE` and Chromium's complete `electron/LICENSES.chromium.html`
inventory next to the runtime. Terminal Effects packages this verified prebuilt
runtime; it does not build Electron or Chromium from source.

## Fonts

The renderer embeds and/or bundles these fonts under the SIL Open Font License
1.1:

- Inter Variable 4.001, copyright 2016 The Inter Project Authors.
- JetBrains Mono Regular 2.305, copyright 2020 The JetBrains Mono Project
  Authors.

Their complete license texts are stored beside the fonts as
`LICENSE.Inter.txt` and `LICENSE.JetBrainsMono.txt` and are included in release
archives.
