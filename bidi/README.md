# wezterm-bidi - a pure Rust bidi implementation

This crate provides an implementation of the *The Unicode Bidirectional
Algorithm (UBA)* in Rust.

This crate was developed for use in wezterm but does not depend on
other code in wezterm.

The focus for this crate is conformance.

## Status

This crate resolves embedding levels and can reorder line ranges.

The implementation conformant with 100% of the BidiTest.txt and
BidiCharacterTest.txt test cases (approx 780,000 test cases).

## License

MIT compatible License
Copyright © 2022-Present Wez Furlong.

Portions of the code in this crate were derived from the bidiref reference
implementation of the UBA which is:

Copyright © 1991-2022 Unicode, Inc. All rights reserved.

See [LICENSE.md](LICENSE.md) for the full text of the license.
