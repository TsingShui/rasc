#!/usr/bin/env bash
set -euo pipefail

: "${RELEASE_VERSION:?RELEASE_VERSION is required}"
: "${GITHUB_REF:?GITHUB_REF is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"

if [[ "$GITHUB_REF" != refs/heads/main ]]; then
  echo "Run the release workflow on the main branch." >&2
  exit 1
fi
if [[ ! "$RELEASE_VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "Version must be a stable X.Y.Z version without a v prefix." >&2
  exit 1
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "The release checkout must be clean." >&2
  exit 1
fi

tag="rasc-v${RELEASE_VERSION}"
if release_state="$(gh api "repos/${GITHUB_REPOSITORY}/releases/tags/${tag}" --jq .draft 2>&1)"; then
  if [[ "$release_state" != true ]]; then
    echo "Release ${tag} is already published; choose a new version." >&2
    exit 1
  fi
elif [[ "$release_state" != *"(HTTP 404)"* ]]; then
  echo "Could not check release ${tag}: ${release_state}" >&2
  exit 1
fi

git fetch --tags origin '+refs/heads/main:refs/remotes/origin/main'
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse refs/remotes/origin/main)" ]]; then
  echo "main changed since this workflow was started; run it again." >&2
  exit 1
fi

if git show-ref --verify --quiet "refs/tags/${tag}"; then
  # A previous run may have created the tag but failed before publishing.
  commit="$(git rev-parse "refs/tags/${tag}^{commit}")"
  if ! git merge-base --is-ancestor "$commit" refs/remotes/origin/main; then
    echo "Existing tag ${tag} is not on main; refusing to release it." >&2
    exit 1
  fi
  tag_version="$(git show "${commit}:Cargo.toml" | python3 -c 'import sys, tomllib; print(tomllib.loads(sys.stdin.read())["package"]["version"])')"
  if [[ "$tag_version" != "$RELEASE_VERSION" ]]; then
    echo "Existing tag ${tag} has Cargo version ${tag_version}." >&2
    exit 1
  fi
  echo "Retrying unpublished tag ${tag} at ${commit}."
else
  python3 tools/release/prepare_version.py "$RELEASE_VERSION"
  cargo update --workspace
  git add Cargo.toml Cargo.lock crates/dexdec/Cargo.toml crates/rusty-dex/Cargo.toml
  if ! git diff --cached --quiet; then
    git config user.name 'github-actions[bot]'
    git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
    git commit -m "chore(release): prepare ${tag}"
    # A non-fast-forward push fails rather than overwriting a concurrent change.
    git push origin HEAD:refs/heads/main
  fi
  commit="$(git rev-parse HEAD)"
fi

{
  echo "version=${RELEASE_VERSION}"
  echo "tag=${tag}"
  echo "commit=${commit}"
} >> "$GITHUB_OUTPUT"
