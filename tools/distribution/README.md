# Author tool release

The author binary is built from the existing publisher package; only the author
executable and LICENSE are distributed. Its package version owns the release tag.
No signing credential, publisher configuration or private database enters the build.

PRs affecting distribution/publisher code build and smoke-test both supported
platforms. These builds upload workflow artifacts only. After review and merge,
create `author-v<package-version>` at the merged commit and push the tag. The tag
workflow verifies both the version and main-branch ancestry, builds the two archives
and their SHA-256 files, uploads a draft release, then makes it public. Only that
release job has contents-write permission. Failed draft uploads require inspection;
do not replace the bytes of an already published release. Fixes use a new version.

Local packaging: `AUTHOR_VERSION=0.1.0 bash tools/distribution/package-author.sh`.
Outputs go to ignored `dist/author/`. Compare the downloaded checksum, extract into
a fresh directory, and run `lenso-marketplace-author --version` and `--help`.
The macOS archive is not notarized. Linux binaries target Ubuntu 24.04/glibc; they
are not advertised as portable to musl or older glibc systems.

Before announcing a release, verify both uploaded archives and checksums, the tool
version, and the issue form on the default branch. Deployment of Developer guide
changes is a separate Marketplace release.
