# MacPorts

`sysutils/tracr/Portfile` installs the prebuilt universal command line tool from the GitHub release (no compiling). Not yet tested with `port` (MacPorts is not installed on the dev machine) and not yet submitted to `macports-ports`.

On each release: set `version`, then replace the three checksums (`shasum -a 256`, `openssl dgst -rmd160`, `stat -f%z` of `tracr-<version>-universal-apple-darwin.tar.gz`).

Test on a Mac with MacPorts: `sudo port -vst install` from a local port source (`file:///path/to/packaging/macports` in `sources.conf`, then `portindex`), then `tracr --version`.
