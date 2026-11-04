#!/usr/bin/env bash
# Cut a release: VERSION and every crate version, the changelog section, tools/check.sh, a commit
# «release: X.Y.Z» and a tag vX.Y.Z (signed when a signing key is set, refused unsigned when
# keys/allowed_signers already holds keys, since clients would reject it), then push. Users get it
# through settings → updates or `yami update`.
# With gh logged in, the push also publishes the GitHub release (the changelog section + install).
#   tools/release.sh X.Y.Z [--no-push]   release
#   tools/release.sh --publish X.Y.Z     (re)publish the GitHub release of an existing tag
#   tools/release.sh --keygen            make an ssh signing key and trust it in keys/allowed_signers
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
RU=0
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in ru*) RU=1 ;; esac
t() { if ((RU)); then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '\e[38;2;196;22;28m▪\e[0m %s\n' "$*"; }
die() { printf '\e[31m✕ %s\e[0m\n' "$*" >&2; exit 1; }
KEYS=keys/allowed_signers
has_keys() { grep -qvE '^\s*(#|$)' "$KEYS" 2>/dev/null; }

# the GitHub release of tag vX.Y.Z: its CHANGELOG section, how to install, how to check the tag
publish() { # ver
  local v=$1 notes
  command -v gh >/dev/null && gh auth status >/dev/null 2>&1 || {
    say "$(t "gh не вошёл — релиз на GitHub потом: tools/release.sh --publish $v" "gh is not logged in: publish later with tools/release.sh --publish $v")"
    return 0
  }
  git rev-parse -q --verify "refs/tags/v$v" >/dev/null || die "$(t "нет тега v$v" "no tag v$v")"
  notes="$(git show "v$v:CHANGELOG.md" | awk -v h="## $v" '$0 == h { on = 1; next } on && /^## / { exit } on' | sed -e '/./,$!d')"
  [[ -n $notes ]] || notes="- $(t "изменения — в истории коммитов" "see the commit history")"
  local body
  body="$notes

### Install

\`\`\`sh
bash <(curl -fsSL https://raw.githubusercontent.com/Trendorin/yami/v$v/install.sh)
\`\`\`

Already on yami: settings → updates → **update**, or \`yami update apply\`.

Built from source on your machine (Arch, Fedora, Debian/Ubuntu, openSUSE, Void, Alpine, Gentoo, NixOS). The tag is signed; check it with
\`git -c gpg.ssh.allowedSignersFile=keys/allowed_signers verify-tag v$v\`.

Описание и установка по-русски: [README.ru.md](https://github.com/Trendorin/yami/blob/v$v/README.ru.md)."
  # the newest version tag is «latest»; an older one republished stays below it
  local newest latest=false
  newest="$(git tag -l 'v[0-9]*' --sort=-v:refname | head -n1)"
  [[ $newest == "v$v" ]] && latest=true
  if gh release view "v$v" >/dev/null 2>&1; then
    gh release edit "v$v" --title "yami $v" --notes "$body" --latest="$latest" >/dev/null
  else
    gh release create "v$v" --verify-tag --title "yami $v" --notes "$body" --latest="$latest" >/dev/null
  fi
  say "$(t "релиз на GitHub: yami $v" "GitHub release: yami $v")"
}

if [[ ${1:-} == --publish ]]; then
  [[ ${2:-} =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "$(t "как: tools/release.sh --publish X.Y.Z" "usage: tools/release.sh --publish X.Y.Z")"
  publish "$2"
  exit 0
fi

if [[ ${1:-} == --keygen ]]; then
  key="$HOME/.ssh/yami-release"
  [[ -f $key ]] || ssh-keygen -t ed25519 -C "yami release" -f "$key"
  git config gpg.format ssh
  git config user.signingkey "$key.pub"
  who="$(git config user.email || git config user.name)"
  grep -qF "$(cut -d' ' -f2 "$key.pub")" "$KEYS" ||
    printf '%s namespaces="git" %s\n' "$who" "$(cut -d' ' -f1,2 "$key.pub")" >>"$KEYS"
  say "$(t "ключ: $key (храни его; без него выпуски не подписать)" "key: $key (keep it safe; releases cannot be signed without it)")"
  say "$(t "закоммить $KEYS: с версии, где он есть, ставятся только подписанные выпуски" "commit $KEYS: from the version that has it, only signed releases are installed")"
  exit 0
fi

ver="${1:-}"
push=1
[[ ${2:-} == --no-push ]] && push=0
[[ $ver =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "$(t "как: tools/release.sh X.Y.Z [--no-push] | --keygen" "usage: tools/release.sh X.Y.Z [--no-push] | --keygen")"
cur="$(cat VERSION)"
[[ "$(printf '%s\n%s\n' "$cur" "$ver" | sort -V | tail -n1)" == "$ver" && $ver != "$cur" ]] ||
  die "$(t "$ver не новее $cur" "$ver is not newer than $cur")"
[[ "$(git rev-parse --abbrev-ref HEAD)" == main ]] || die "$(t "выпуск — только с main" "release from main only")"
[[ -z "$(git status --porcelain --untracked-files=no)" ]] || die "$(t "есть незакоммиченные правки" "uncommitted changes")"
git rev-parse -q --verify "refs/tags/v$ver" >/dev/null && die "$(t "тег v$ver уже есть" "tag v$ver exists")"
signed=0
[[ -n "$(git config user.signingkey || true)" ]] && signed=1
((signed)) || ! has_keys || die "$(t "в $KEYS есть ключи, а подписывать нечем: клиенты не поставят выпуск (tools/release.sh --keygen)" "$KEYS has keys but no signing key is set: clients would refuse the release (tools/release.sh --keygen)")"

# changelog: «Unreleased» becomes the release; empty — commit subjects since the last tag
last="$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)"
python3 -I - "$ver" "$last" <<'PY'
import re, subprocess, sys
from pathlib import Path
ver, last = sys.argv[1], sys.argv[2]
p = Path("CHANGELOG.md")
s = p.read_text()
if f"\n## {ver}\n" in s:
    sys.exit(0)
m = re.search(r"\n## Unreleased\n(.*?)(?=\n## |\Z)", s, re.S)
body = m.group(1).strip() if m else ""
if not body:
    rng = f"{last}..HEAD" if last else "HEAD"
    subj = subprocess.run(["git", "log", "--no-merges", "--format=- %s", rng], capture_output=True, text=True).stdout.strip()
    body = subj or "- maintenance"
sec = f"\n## {ver}\n\n{body}\n"
if m:
    s = s[: m.start()] + "\n## Unreleased\n" + sec + s[m.end():]
else:
    i = s.find("\n## ")
    s = s + sec if i < 0 else s[:i] + sec + s[i:]
p.write_text(s)
PY
if [[ -t 0 && -n ${EDITOR:-} ]]; then "$EDITOR" CHANGELOG.md; fi

say "$(t "версия" "version") $cur → $ver"
echo "$ver" >VERSION
crates=(shell/Cargo.toml shell/*/Cargo.toml)
for f in "${crates[@]}"; do
  # only the [package] version (the first `version =` line of the file)
  sed -i "0,/^version = \".*\"/s//version = \"$ver\"/" "$f"
done
cargo update --workspace --offline --manifest-path shell/Cargo.toml >/dev/null 2>&1 || cargo update --workspace --manifest-path shell/Cargo.toml
tools/check.sh --fast | tail -n 3
tools/check.sh --fast >/dev/null 2>&1 || die "$(t "tools/check.sh не прошёл" "tools/check.sh failed")"

git add VERSION CHANGELOG.md shell/Cargo.lock "${crates[@]}"
git commit -q -m "release: $ver"
if ((signed)); then
  git tag -s "v$ver" -m "yami $ver"
  say "$(t "тег v$ver подписан" "tag v$ver signed")"
else
  git tag -a "v$ver" -m "yami $ver"
  say "$(t "тег v$ver без подписи (подписывать: tools/release.sh --keygen)" "tag v$ver is unsigned (to sign: tools/release.sh --keygen)")"
fi
if ((push)); then
  git push -q origin main "v$ver"
  say "$(t "отправлено: у всех в настройках → обновления появится $ver" "pushed: $ver shows up in everyone's settings → updates")"
  publish "$ver"
else
  say "$(t "не отправлено: git push origin main v$ver" "not pushed: git push origin main v$ver")"
fi
