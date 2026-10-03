# Third-party code and assets

The root MIT license applies to original Mashup work. It does not replace the
licenses of dependencies, vendored code, fonts or player-owned game content.

## Current dependencies

The direct dependency is [Bevy](https://github.com/bevyengine/bevy), licensed
MIT OR Apache-2.0. The exact dependency graph is recorded in `Cargo.lock`;
each transitive dependency retains its declared license. The starter UI uses
Bevy's bundled Fira Mono font, licensed under SIL OFL-1.1. Its copyright and
full license are preserved in
[`third_party/bevy/FiraMono-LICENSE`](third_party/bevy/FiraMono-LICENSE).

Run `cargo deny --locked check licenses` before accepting dependency changes.
The allowlist in `deny.toml` is a conservative project policy, not a relicensing
of any dependency. Check upstream notices and license obligations in addition
to package metadata, especially for bundled fonts, shaders and other assets.

## Existing third-party source

No game source or other vendored implementation is included in this scaffold.
For future vendoring, create `third_party/<name>/` with the original license,
copyright and attribution notices, plus a provenance document containing:

- Upstream project and source URL.
- Exact version or commit and files used.
- License and relevant asset or exception terms.
- Local modifications and their authorship.
- Required release notices.

Copied, adapted and translated implementations belong to this category even
when they are rewritten in Rust or stored under a game-specific namespace.
Keep their licensing explicit at the file boundary. Independently written
behavior-compatible code is original work and uses the root MIT license.

## Release packaging

Ship `LICENSE` and a notice bundle containing all required dependency and
vendored license texts, copyright notices, and applicable upstream NOTICE files
for the exact shipped build, including bundled assets. Generate that bundle
from the locked graph and verify it against upstream files before distribution.
This document is the policy and inventory starting point, not that notice bundle.

Never include installed-game files, extracted source assets, converted game
assets, `assets/imported/`, or `user_data/` in source archives or binary releases.
Git ignores and Cargo exclusions are guardrails; release packaging must still
inspect its inputs. No automatic release packaging exists yet.
