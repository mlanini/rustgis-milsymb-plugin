# Military Symbols Library (milsymb) — RustGIS plugin

APP-6D (MIL-STD-2525D) SIDC tagging and schematic affiliation styling for RustGIS vector
layers. Built as a WebAssembly module, so it runs in the **desktop app, `rustgis-cli` and the
web app**.

## What it does

- **Tag Layer With APP-6D SIDC** (tool): parses a 20-digit APP-6D SIDC and writes its
  semantics (version, context, affiliation, symbol set, status, amplifiers) as feature
  properties. Output layers are plain GeoJSON, saved with the project and import/export-able.
- **Style by Affiliation** (command): applies Unique Values symbology on the `affiliation`
  field and colours each class with its APP-6 standard colour (friend blue, hostile red,
  neutral green, unknown yellow). Run **after** tagging. Needs the `edit` permission.
- **About** (command): one-line summary.

### Rendering note

RustGIS point symbology has only five marker shapes (circle, square, triangle, diamond, star)
and colour — it has **no image/SVG point symbols**. True milsymbol icons therefore cannot be
drawn on the map by any plugin. This plugin delivers a *schematic* APP-6 look (affiliation
colour) plus full SIDC semantics in the data.

## Current impasse (23 October 2026)

The project is currently blocked against the originally requested outcome.

- Confirmed requirement: real APP-6D/milsymbol icons placed directly on map layers (desktop,
  CLI, web), with plugin-only scope (no RustGIS core changes).
- Verified platform limit: RustGIS vector point symbology currently exposes only marker shape
  (`circle|square|triangle|diamond|star`) and colour. There is no plugin API for per-feature
  SVG/image point symbols.
- Practical consequence: a plugin can parse/store SIDC and apply affiliation styling, but cannot
  render true APP-6D symbol frames/icons as map markers.

### What this means for this repository

- The implemented path remains useful for SIDC attribution and APP-6 affiliation colour styling.
- Full visual parity with KADAS-style milsymbol rendering is not achievable in the current
  RustGIS plugin API surface.

### Unblock options

1. Extend RustGIS core/map symbology to support SVG/image point markers (or equivalent sprite
   pipeline) for vector layers.
2. Accept the current schematic mode (affiliation colour + basic marker shapes) as the final
   scope.
3. Move full-fidelity military symbol rendering to another platform that already supports it.

## Build (CI, no local Rust needed)

This workspace has no Rust toolchain. Push to GitHub and the `CI` workflow builds the WASM
module, runs the tests, and uploads `plugin.json` + `plugin.wasm` + the `.rgplugin` package as
a build artifact.

To build locally instead, install [Rust](https://rustup.rs), then:

```sh
rustup target add wasm32-unknown-unknown
cargo test
cargo build --release --target wasm32-unknown-unknown
```

## Try it in RustGIS

Download the CI artifact, then in the desktop app open **Plugins › Plugin Manager › Settings ›
Add Plugin Folder…** and pick the folder with `plugin.json` and `plugin.wasm`.

Workflow: run **Tag Layer With APP-6D SIDC** on a point layer (give a 20-digit default SIDC or
a SIDC field), then select the output layer and run **Style by Affiliation**.

## Credits & dependencies

- [**milsymbol**](https://github.com/spatialillusions) by Måns Beckman / spatialillusions —
  the reference implementation and documentation of the APP-6 / MIL-STD-2525 symbology that
  inspired this plugin and guided the APP-6D SIDC parsing. (milsymbol is MIT-licensed; its
  rendering engine is not embedded here — RustGIS has no SVG point symbols.)
- [**rustgis-plugin**](https://github.com/opengeos/rustgis-plugins) — the RustGIS plugin SDK
  (host bindings, GeoJSON helpers, WASM ABI).
- [**RustGIS**](https://rustgis.app/) and the
  [plugin template](https://github.com/opengeos/rustgis-plugin-template) by the opengeos
  community.
- [**serde_json**](https://github.com/serde-rs/json) — JSON handling (via the SDK).
- **APP-6D (NATO)** / **MIL-STD-2525D (US DoD)** — the symbology standards implemented.

## License

MIT. See [LICENSE](LICENSE).
