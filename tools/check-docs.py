#!/usr/bin/env python3
# The English and Russian documents are one text in two languages: README, CHANGELOG, docs/CONFIG,
# docs/SECURITY, fork/README and their .ru.md twins. Run by tools/check.sh (CI) and tools/release.sh.
# Each pair has to keep the same skeleton: the same headings (version headings and file names
# verbatim), paragraphs, list items and table rows in the same order, the same `code` in every
# block (a <placeholder> may be translated), the same code blocks (comments may be translated),
# the same links (a .ru.md twin in place of .md). The Russian side has to be Russian where the
# English one has words, the English side free of Russian. Every relative link and #anchor must
# resolve, the Russian files link the Russian docs, each file links its twin near the top, and the
# newest release section of the changelog is VERSION.
#   python3 -I tools/check-docs.py [-v]
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAIRS = ["README.md", "CHANGELOG.md", "docs/CONFIG.md", "docs/SECURITY.md", "docs/PACKAGING.md", "fork/README.md"]
# headings that are a name, not words: the same in both languages
SAME = re.compile(r"^(\d+\.\d+\.\d+|[\w.]*[.\-][\w.\-]*|yami|Nix)$")
HEADS = {"Unreleased": "Не выпущено"}
CYR = re.compile("[а-яёА-ЯЁ]")
ITEM = re.compile(r"\s*([-*]|\d+\.) ")
errors = []


def twin(rel):
    return rel[:-3] + ".ru.md"


def err(where, msg):
    errors.append(f"{where}: {msg}")


def blocks(text):
    """блоки markdown по порядку: (вид, номер строки, данные)"""
    lines, out, i = text.split("\n"), [], 0
    while i < len(lines):
        ln = lines[i]
        if ln.startswith("```"):
            j = i + 1
            while j < len(lines) and not lines[j].startswith("```"):
                j += 1
            out.append(("code", i + 1, (ln[3:].strip(), lines[i + 1 : j])))
            i = j + 1
        elif not ln.strip():
            i += 1
        elif m := re.match(r"(#{1,6}) (.*)", ln):
            out.append(("heading", i + 1, (len(m.group(1)), m.group(2).strip())))
            i += 1
        elif ln.startswith("|"):
            start, rows = i, []
            while i < len(lines) and lines[i].startswith("|"):
                rows.append(lines[i])
                i += 1
            out.append(("table", start + 1, rows))
        else:
            start, buf = i, [ln]
            i += 1
            item = bool(ITEM.match(ln))
            while i < len(lines) and lines[i].strip() and not lines[i].startswith(("#", "|", "```")) and not ITEM.match(lines[i]):
                buf.append(lines[i])
                i += 1
            out.append(("item" if item else "para", start + 1, " ".join(x.strip() for x in buf)))
    return out


def spans(t):
    """`код` блока; <заполнитель> внутри можно переводить"""
    return sorted(re.sub(r"<[^<>`]*>", "<>", s) for s in re.findall(r"`([^`]+)`", t))


def skeleton(s):
    return re.sub(r"[^\W\d_]+", "a", s)


def span_diff(ta, tb):
    """`код`, который есть только с одной стороны. Русский `код` с кириллицей — переведённый
    пример (`/имя`, `*.bak-ДАТА`): годится, если у английского есть такой же по форме"""
    a, b = spans(ta), spans(tb)
    for x in list(a):
        if x in b:
            a.remove(x)
            b.remove(x)
    for y in [y for y in b if CYR.search(y)]:
        m = next((x for x in a if skeleton(x) == skeleton(y)), None)
        if m is not None:
            a.remove(m)
            b.remove(y)
    return a, b


def links(t):
    """адреса ссылок и картинок по порядку; у своих — без языка и якоря"""
    out = []
    for u in re.findall(r"\]\(([^)\s]+)\)", t) + re.findall(r'(?:src|href)="([^"]+)"', t):
        if re.match(r"[a-z]+:", u):
            out.append(u)
        elif u.startswith("#"):
            out.append("#")
        else:
            # свой двойник и картинки на языке страницы (features-en.png / features-ru.png)
            out.append(re.sub(r"-(en|ru)(\.\w+)$", r"-xx\2", re.sub(r"\.ru\.md$", ".md", u.split("#")[0])))
    return out


def prose(t):
    """текст без кода, адресов, подписей картинок и тегов"""
    t = re.sub(r"`[^`]*`", " ", t)
    t = re.sub(r"!\[[^\]]*\]", " ", t)
    t = re.sub(r"\]\([^)]*\)", "]", t)
    t = re.sub(r"<[^>]*>", " ", t)
    return re.sub(r"https?://\S+", " ", t)


def words(t):
    return re.findall(r"\b[A-Za-z]{3,}\b", prose(t))


# служебные слова английского: в названиях программ (Arch, polkit, mpv) их не бывает, так что
# два таких слова в русском блоке — оставленная английская фраза; «…» (заголовки issue) не в счёт
STOP = set("the and with from that when into this are its only every each not which while after before "
           "under over than then also both their there where what was by of to in on or it as at an you your".split())


def english(t):
    t = re.sub(r"«[^»]*»", " ", prose(t))
    return [w for w in re.findall(r"[A-Za-z]+(?:-[A-Za-z]+)*", t) if w.lower() in STOP]


def uncomment(lines):
    """строки кода без комментариев (их можно переводить)"""
    return [re.sub(r"(^|\s)#.*$", "", x).rstrip() for x in lines]


def slugs(text):
    """якоря заголовков, как их делает GitHub"""
    seen, out = {}, set()
    for kind, _, d in blocks(text):
        if kind != "heading":
            continue
        h = re.sub(r"\]\([^)]*\)", "]", d[1]).lower()
        h = "".join(c for c in h if c.isalnum() or c in " -_" or unicodedata.category(c).startswith("M")).replace(" ", "-")
        n = seen.get(h, 0)
        seen[h] = n + 1
        out.add(h if n == 0 else f"{h}-{n}")
    return out


def compare(en_rel, ru_rel, en, ru):
    a, b = blocks(en), blocks(ru)
    for k, ((ka, la, da), (kb, lb, db)) in enumerate(zip(a, b)):
        at = f"{en_rel}:{la} ↔ {ru_rel}:{lb}"
        if ka != kb:
            err(at, f"{ka} in English, {kb} in Russian: the files part here (a paragraph, item or row added on one side?)")
            return
        if ka == "heading":
            (lv_a, ha), (lv_b, hb) = da, db
            if lv_a != lv_b:
                err(at, f"heading level {lv_a} vs {lv_b}")
            if ha in HEADS and hb != HEADS[ha]:
                err(at, f"«{ha}» is «{HEADS[ha]}» in Russian, not «{hb}»")
            elif SAME.match(ha) and ha != hb:
                err(at, f"heading «{ha}» is a name and stays the same, not «{hb}»")
            if any(span_diff(ha, hb)):
                err(at, f"code in heading differs: {spans(ha)} vs {spans(hb)}")
            if not (SAME.match(ha) or ha in HEADS) and len(words(ha)) and not CYR.search(prose(hb)):
                err(at, f"heading not translated: «{hb}»")
        elif ka == "code":
            (lang_a, ca), (lang_b, cb) = da, db
            if lang_a != lang_b:
                err(at, f"code block language {lang_a!r} vs {lang_b!r}")
            ua, ub = uncomment(ca), uncomment(cb)
            same = lambda x, y: x == y or (CYR.search(y) and skeleton(x) == skeleton(y))
            if len(ua) != len(ub) or not all(same(x, y) for x, y in zip(ua, ub)):
                d = next((i for i, (x, y) in enumerate(zip(ua, ub)) if not same(x, y)), min(len(ua), len(ub)))
                err(at, f"code block differs (comments aside) at its line {d + 1}: {ua[d] if d < len(ua) else '∅'!r} vs {ub[d] if d < len(ub) else '∅'!r}")
        elif ka == "table":
            if len(da) != len(db):
                err(at, f"table has {len(da)} rows in English, {len(db)} in Russian")
                continue
            for r, (ra, rb) in enumerate(zip(da, db)):
                rat = f"{en_rel}:{la + r} ↔ {ru_rel}:{lb + r}"
                cells_a, cells_b = ra.strip("|").split(" | "), rb.strip("|").split(" | ")
                if len(cells_a) != len(cells_b):
                    err(rat, f"{len(cells_a)} cells vs {len(cells_b)}")
                one(rat, ra, rb)
        else:
            if ka == "item" and ITEM.match(da).group(1)[-1:] != ITEM.match(db).group(1)[-1:]:
                err(at, "bullet vs numbered item")
            one(at, da, db)
    if len(a) != len(b):
        longer, rel, blk = (en_rel, a[len(b)]) if len(a) > len(b) else (ru_rel, b[len(a)])
        err(f"{longer}:{blk[1]}", f"{abs(len(a) - len(b))} more block(s) than its twin from here")


def one(at, ta, tb):
    """один абзац, пункт или строка таблицы в двух языках"""
    only_a, only_b = span_diff(ta, tb)
    if only_a or only_b:
        err(at, f"`code` differs: only English {only_a}, only Russian {only_b}")
    if links(ta) != links(tb):
        err(at, f"links differ: {links(ta)} vs {links(tb)}")
    if len(words(ta)) >= 3 and not CYR.search(prose(tb)):
        err(at, f"not translated: {tb[:90]!r}")
    elif len(english(tb)) >= 2:
        err(at, f"English left in the Russian text ({' '.join(english(tb)[:6])}): {tb[:90]!r}")


def link_targets(rel, text, own):
    """каждая относительная ссылка и якорь существуют; русские файлы ведут на русские двойники"""
    base = (ROOT / rel).parent
    body = "\n".join(x for k, _, d in blocks(text) if k != "code" for x in ([d] if isinstance(d, str) else d if k == "table" else [d[1]]))
    ru = rel.endswith(".ru.md")
    for u in re.findall(r"\]\(([^)\s]+)\)", body) + re.findall(r'(?:src|href)="([^"]+)"', body):
        if re.match(r"[a-z]+:", u):
            continue
        path, _, anchor = u.partition("#")
        tgt = (base / path).resolve() if path else (ROOT / rel)
        if not tgt.exists():
            err(rel, f"link to a missing file: {u}")
            continue
        if anchor and tgt.suffix == ".md" and anchor not in slugs(tgt.read_text()):
            err(rel, f"link to a missing anchor: {u}")
        if not path or tgt.suffix != ".md":
            continue
        trel = tgt.relative_to(ROOT).as_posix() if tgt.is_relative_to(ROOT) else ""
        if trel == own:
            continue
        if ru and trel in PAIRS:
            err(rel, f"links the English {trel}; the Russian file links {twin(trel)}")
        if not ru and trel.endswith(".ru.md"):
            err(rel, f"links the Russian {trel}; the English file links {trel[:-6]}.md")


def main():
    verbose = "-v" in sys.argv[1:]
    for en_rel in PAIRS:
        ru_rel = twin(en_rel)
        ep, rp = ROOT / en_rel, ROOT / ru_rel
        if not ep.exists() or not rp.exists():
            err(en_rel, f"the pair {en_rel} / {ru_rel} is incomplete")
            continue
        en, ru = ep.read_text(), rp.read_text()
        compare(en_rel, ru_rel, en, ru)
        for rel, text, other in ((en_rel, en, ru_rel), (ru_rel, ru, en_rel)):
            name = Path(other).name
            if not re.search(rf"\]\({re.escape(name)}\)", "\n".join(text.split("\n")[:20])):
                err(rel, f"no link to {name} in its first 20 lines")
            link_targets(rel, text, other)
        # английский файл без русского текста (кроме `кода` и подписи ссылки на двойник)
        for kind, ln, d in blocks(en):
            if kind in ("para", "item", "table"):
                t = "\n".join(d) if kind == "table" else d
                t = re.sub(r"\[[^\]]*\]\(" + re.escape(Path(ru_rel).name) + r"[^)]*\)", " ", t)
                if CYR.search(prose(t)):
                    err(f"{en_rel}:{ln}", f"Russian text in the English file: {prose(t).strip()[:90]!r}")
        if verbose:
            print(f"  {en_rel} ↔ {ru_rel}: {len(blocks(en))} blocks")
    # Русские файлы без пары
    for p in ROOT.rglob("*.ru.md"):
        rel = p.relative_to(ROOT).as_posix()
        if not rel.startswith(("shell/target/", ".git/")) and rel[:-6] + ".md" not in PAIRS:
            err(rel, "a Russian document outside the checked pairs (add its English twin to PAIRS)")
    # самый новый выпуск в CHANGELOG — это VERSION
    ver = (ROOT / "VERSION").read_text().strip()
    for rel in ("CHANGELOG.md", "CHANGELOG.ru.md"):
        p = ROOT / rel
        if p.exists():
            vs = re.findall(r"^## (\d+\.\d+\.\d+)$", p.read_text(), re.M)
            if vs and vs[0] != ver:
                err(rel, f"the newest section is {vs[0]}, VERSION is {ver}: new lines go under «Unreleased», tools/release.sh makes the section")
            if vs != sorted(vs, key=lambda v: tuple(map(int, v.split("."))), reverse=True) or len(set(vs)) != len(vs):
                err(rel, "release sections are not newest first, or one repeats")
    for e in errors:
        print(f"docs: {e}")
    if errors:
        print(f"docs: {len(errors)} problem(s); the pairs are {', '.join(PAIRS)} and their .ru.md twins")
        return 1
    print(f"docs: {len(PAIRS)} pairs in step")
    return 0


if __name__ == "__main__":
    sys.exit(main())
