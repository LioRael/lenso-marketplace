#!/usr/bin/env bash
set -euo pipefail
: "${AUTHOR_VERSION:?Set AUTHOR_VERSION to the package version}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
package_version="$(cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="lenso-marketplace-publisher"))')"
[[ "$AUTHOR_VERSION" == "$package_version" ]] || { echo 'Release version does not match Cargo package' >&2; exit 1; }
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64|Linux-x86_64) platform="$(uname -s)-$(uname -m)" ;;
  *) echo 'Prebuilt distribution currently supports Darwin-arm64 and Linux-x86_64' >&2; exit 1 ;;
esac
cargo build --locked --manifest-path "$root/Cargo.toml" -p lenso-marketplace-publisher --bin lenso-marketplace-author --release
build_dir="$(cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
cp "$build_dir/release/lenso-marketplace-author" "$staging/"
"$staging/lenso-marketplace-author" --help
cp "$root/LICENSE" "$staging/"
output="$root/dist/author"
mkdir -p "$output"
archive="lenso-marketplace-author-$platform.tar.gz"
COPYFILE_DISABLE=1 tar -czf "$output/$archive" -C "$staging" lenso-marketplace-author LICENSE
(cd "$output" && shasum -a 256 "$archive" > "$archive.sha256")
mkdir "$staging/extracted"
tar -xzf "$output/$archive" -C "$staging/extracted"
test "$("$staging/extracted/lenso-marketplace-author" --version)" = "lenso-marketplace-author $AUTHOR_VERSION"
(cd "$output" && shasum -a 256 -c "$archive.sha256")
