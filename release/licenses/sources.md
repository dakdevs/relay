# License texts for dependencies without a license file

`scripts/third-party.sh` writes `THIRD-PARTY-LICENSES.md` for each release
archive. Most crates ship their own license files, which are copied verbatim.
For the rest, in order:

1. **Upstream files** (`upstream/`, indexed by `upstream.tsv`): the license
   files from the crate's repository at the commit the version was published
   from (`.cargo_vcs_info.json`), or its version tag if no commit is recorded.
   The search starts in the crate's folder and moves up to the first folder
   with a LICENSE, COPYING or UNLICENSE file; NOTICE files passed on the way
   are kept as supplements, but a NOTICE alone doesn't count as a license.
   They keep the real copyright notices. Two crates, `xim-ctext` 0.3.0 and
   `xim-parser` 0.2.1, record neither, so their file comes from the default
   branch; the output says it may differ from the file current at release.
   Each file is stored under its SHA-256, which is verified on every run.
   Refresh with `scripts/vendor-licenses.sh` (needs `gh` and network access)
   whenever dependencies change; a release build whose new dependency lacks a
   license file shows it as a gap until then.
2. **Gaps**: crates with no license file in the package or upstream. Their
   entry states that no copyright notice is available, lists the Cargo.toml
   authors as authors (not as copyright holders), and the standard SPDX text of
   the declared license is appended once. Known gaps on 2026-10-18:

   | Crate                       | Declared       | Upstream finding                                                              |
   | --------------------------- | -------------- | ----------------------------------------------------------------------------- |
   | `block` 0.1.6               | MIT            | No license file at tag `0.1.6` (macOS only)                                   |
   | `malloc_buf` 0.0.6          | MIT            | No license file at tag `0.0.6`; one was added in a later version (macOS only) |
   | `objc_exception` 0.1.2      | MIT            | No license file at the published commit (macOS only)                          |
   | `pathfinder_geometry` 0.5.1 | MIT/Apache-2.0 | Published commit `a5e98fa` is not in `servo/pathfinder`                       |
   | `seahash` 4.1.0             | MIT            | `gitlab.redox-os.org` returned HTTP 502; not checked                          |
   | `simd_helpers` 0.1.0        | MIT            | No license file at the published commit                                       |

## Standard texts

Unmodified SPDX texts from
[spdx/license-list-data](https://github.com/spdx/license-list-data) release
`v3.29.0`, commit `31ba1a50e5397e00a304dbadc76531740e89ee48`, files
`text/<id>.txt`, retrieved 2026-10-18. Checksums are in `SHA256SUMS`.

| File               | SPDX id      |
| ------------------ | ------------ |
| `Apache-2.0.txt`   | Apache-2.0   |
| `BSD-2-Clause.txt` | BSD-2-Clause |
| `BSD-3-Clause.txt` | BSD-3-Clause |
| `ISC.txt`          | ISC          |
| `MIT.txt`          | MIT          |
| `Zlib.txt`         | Zlib         |

This is a best-effort record of what the dependencies publish, not legal
advice or a compliance guarantee.
