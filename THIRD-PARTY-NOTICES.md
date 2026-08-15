# Third party notices

The crates that go into the binary, at the versions pinned in `Cargo.lock`.
Crates that the lockfile carries for other targets are left out, since they are
never built here.

## Linked into the binary

| Component | Version | License |
|---|---|---|
| [hidapi](https://crates.io/crates/hidapi) (Rust bindings) | 2.6.6 | MIT |
| [HIDAPI](https://github.com/libusb/hidapi) (bundled C library) | as vendored by the crate | BSD-Style, see `LICENSE-hidapi-bsd.txt` |
| [libc](https://crates.io/crates/libc) | 0.2.189 | MIT OR Apache-2.0, used here under Apache-2.0 |
| [cfg-if](https://crates.io/crates/cfg-if) | 1.0.4 | MIT OR Apache-2.0, used here under Apache-2.0 |

HIDAPI offers a choice of three licenses: the GNU General Public License
version 3, a BSD-Style License, or the original HIDAPI license. **The BSD-Style
License is the one chosen here**; the GPL is expressly not used. Its terms
require the copyright notice, the list of conditions and the disclaimer to
accompany a binary distribution, which is why `LICENSE-hidapi-bsd.txt` is part
of this repository and must be shipped with any binary release.

## Build time only

These run during the build and leave no code in the resulting binary:

| Component | Version | License |
|---|---|---|
| [cc](https://crates.io/crates/cc) | 1.4.2 | MIT OR Apache-2.0 |
| [pkg-config](https://crates.io/crates/pkg-config) | 0.3.33 | MIT OR Apache-2.0 |
| [shlex](https://crates.io/crates/shlex) | 2.0.1 | MIT OR Apache-2.0 |

## MIT License

Applies to the `hidapi` crate. The dual licensed crates above are used under
Apache-2.0, whose text is in `LICENSE`.

```
Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

To regenerate this list after a dependency change:

```
cargo tree --edges normal --prefix none
```
