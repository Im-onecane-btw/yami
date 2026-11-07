#!/usr/bin/env python3
"""Static i18n check for every crate (shell, files, view, play, greet): every Russian string literal must be translatable.

A literal passes when it is a key of PAIRS in i18n.rs, the Russian part of tf!("…", "…", "…"),
or sits on a line marked `// i18n-ok` (text that is Russian on purpose). In a file marked
`// i18n: inline pairs` (bilingual tables such as keys.rs) a Russian literal also passes when
the next literal on the line is its English twin. Brazilian Portuguese: every English value of
PAIRS and of an inline pair has its line in i18n_pt_br.rs, and every tf! keeps the English
template's placeholders in the Portuguese one. Also catches duplicate keys and Cyrillic left in
English or Portuguese text. The runtime half is `shot --lang en` and `--lang pt-BR` of every app
(strings built at draw time). Exit 1 on any problem.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "shell"
CRATES = [ROOT / "src", ROOT / "files" / "src", ROOT / "view" / "src", ROOT / "play" / "src", ROOT / "greet" / "src"]
CYR = re.compile(r"[а-яА-ЯёЁ]")
LIT = re.compile(r'"((?:[^"\\]|\\.)*)"')
NATIVE = {"Русский"}
S = r'"((?:[^"\\]|\\.)*)"'
PAIR = re.compile(r"^\s*\(" + S + r", " + S + r"\),", re.M)
TEMPLATE = re.compile(r"tf!\(\s*" + S + r"\s*,\s*" + S + r"\s*,\s*" + S)
FIELDS = re.compile(r"(?<!\{)\{(?!\{)[^{}]*\}(?!\})")
INLINE_PAIR = re.compile(r"(?=" + S + r"\s*,\s*" + S + r")")


def code_part(line: str) -> str:
    """the line without its // comment (string contents are kept)"""
    out, instr, i = [], False, 0
    while i < len(line):
        if not instr and line.startswith("//", i):
            break
        if line[i] == '"' and (i == 0 or line[i - 1] != "\\"):
            instr = not instr
        out.append(line[i])
        i += 1
    return "".join(out)


def check(SRC: Path) -> tuple:
    table = (SRC / "i18n.rs").read_text()
    pairs = PAIR.findall(table)
    problems = []
    seen = {}
    for ru, en in pairs:
        if ru in seen:
            # тот же перевод дважды — мусор в таблице, другой — неясно, какой победит
            problems.append(f"{SRC.name}/i18n.rs: duplicate key «{ru}»: «{seen[ru]}» / «{en}»")
        seen[ru] = en
        if CYR.search(en) and en not in NATIVE:
            problems.append(f"i18n.rs: English value of «{ru}» is Cyrillic: «{en}»")
    rel = SRC.relative_to(ROOT.parent)
    portuguese = {}
    for en, pt in PAIR.findall((SRC / "i18n_pt_br.rs").read_text()):
        if en in portuguese:
            problems.append(f"{rel}/i18n_pt_br.rs: duplicate key «{en}»")
        portuguese[en] = pt
        if CYR.search(pt) and pt not in NATIVE:
            problems.append(f"{rel}/i18n_pt_br.rs: Portuguese of «{en}» is Cyrillic: «{pt}»")
        if FIELDS.findall(en) != FIELDS.findall(pt):
            problems.append(f"{rel}/i18n_pt_br.rs: placeholders differ for «{en}»")
    for en in sorted(set(seen.values())):
        if en not in portuguese:
            problems.append(f"{rel}/i18n_pt_br.rs: no Portuguese for «{en}»")
    for f in sorted(SRC.glob("*.rs")):
        if f.name in ("i18n.rs", "i18n_pt_br.rs"):
            continue
        text = f.read_text()
        inline = "// i18n: inline pairs" in text
        for n, line in enumerate(text.splitlines(), 1):
            if "i18n-ok" in line:
                continue
            code = code_part(line)
            if inline:
                # a tf! carries its own Portuguese template: only the other pairs need the catalog
                own = {(m.group(1), m.group(2)) for m in TEMPLATE.finditer(code)}
                for ru, en in INLINE_PAIR.findall(code):
                    if CYR.search(ru) and en and not CYR.search(en) and en not in portuguese and (ru, en) not in own:
                        problems.append(f"{rel}/{f.name}:{n}: no Portuguese for the inline «{en}»")
            for m in TEMPLATE.finditer(code):
                _, en, pt = m.groups()
                if FIELDS.findall(en) != FIELDS.findall(pt):
                    problems.append(f"{rel}/{f.name}:{n}: tf! placeholders differ between English and Portuguese")
                if CYR.search(pt):
                    problems.append(f"{rel}/{f.name}:{n}: the Portuguese tf! template is Cyrillic")
            lits = list(LIT.finditer(code))
            for j, m in enumerate(lits):
                s = m.group(1)
                if not CYR.search(s) or s in seen:
                    continue
                if inline and j + 1 < len(lits) and not CYR.search(lits[j + 1].group(1)) and code[m.end():lits[j + 1].start()].strip() == ",":
                    continue
                if re.search(r"tf!\(\s*$", code[: m.start()]):
                    continue
                problems.append(f"{SRC.relative_to(ROOT.parent)}/{f.name}:{n}: no translation for «{s}»")
    return problems, len(seen)


def main() -> int:
    problems, n = [], 0
    for c in CRATES:
        p, k = check(c)
        problems += p
        n += k
    seen = range(n)
    for p in problems:
        print(p)
    if problems:
        print(f"check-i18n: {len(problems)} problem(s). Add pairs to PAIRS in that crate's i18n.rs, "
              "use tf!(ru, en, pt, …), give each English text its line in i18n_pt_br.rs, "
              "or mark intentional Russian with // i18n-ok", file=sys.stderr)
        return 1
    print(f"check-i18n: ok ({len(seen)} pairs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
