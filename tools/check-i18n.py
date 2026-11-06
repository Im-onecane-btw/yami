#!/usr/bin/env python3
"""Static i18n check for every crate (shell, files, view, play): every Russian string literal must be translatable.

A literal passes when it is a key of PAIRS in i18n.rs, the Russian half of tf!("…", "…"),
or sits on a line marked `// i18n-ok` (text that is Russian on purpose). In a file marked
`// i18n: inline pairs` (bilingual tables such as keys.rs) a Russian literal also passes when
the next literal on the line is its English twin. Also catches
duplicate keys and Cyrillic left in English values. The runtime half of the check is
`yami-shell shot --lang en` (strings built at draw time). Exit 1 on any problem.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "shell"
CRATES = [ROOT / "src", ROOT / "files" / "src", ROOT / "view" / "src", ROOT / "play" / "src"]
CYR = re.compile(r"[а-яА-ЯёЁ]")
LIT = re.compile(r'"((?:[^"\\]|\\.)*)"')
NATIVE = {"Русский"}


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
    pairs = re.findall(r'^\s*\("((?:[^"\\]|\\.)*)", "((?:[^"\\]|\\.)*)"\),', table, re.M)
    problems = []
    seen = {}
    for ru, en in pairs:
        if ru in seen:
            # тот же перевод дважды — мусор в таблице, другой — неясно, какой победит
            problems.append(f"{SRC.name}/i18n.rs: duplicate key «{ru}»: «{seen[ru]}» / «{en}»")
        seen[ru] = en
        if CYR.search(en) and en not in NATIVE:
            problems.append(f"i18n.rs: English value of «{ru}» is Cyrillic: «{en}»")
    for f in sorted(SRC.glob("*.rs")):
        if f.name == "i18n.rs":
            continue
        text = f.read_text()
        inline = "// i18n: inline pairs" in text
        for n, line in enumerate(text.splitlines(), 1):
            if "i18n-ok" in line:
                continue
            code = code_part(line)
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
              "use tf!(ru, en, …), or mark intentional Russian with // i18n-ok", file=sys.stderr)
        return 1
    print(f"check-i18n: ok ({len(seen)} pairs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
