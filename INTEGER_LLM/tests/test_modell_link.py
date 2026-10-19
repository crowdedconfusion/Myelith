#!/usr/bin/env python3
"""
**Der Link auf Hugging Face im Bauskript** (`tools/modell_bauen.py`,
`repo_aus_link`).

Die Konsole (`/build artifact`) reicht den Link unveraendert durch; das
Bauskript ist die einzige Stelle, die ihn liest. Diese Probe haelt fest, was
als Modell gilt (mit und ohne Revision im Pfad) und was abgelehnt wird
(fremde Seite, Datensatz, Space, ein Pfad hinter dem Modell, der keine
Revision nennt).

    INTEGER_LLM/calibrate/.venv/bin/python3 INTEGER_LLM/tests/test_modell_link.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import modell_bauen as mb  # noqa: E402

GUT = {
    "https://huggingface.co/Qwen/Qwen3.8-Flash-Next": ("Qwen/Qwen3.8-Flash-Next", ""),
    "huggingface.co/Qwen/Qwen3-4B/tree/main": ("Qwen/Qwen3-4B", "main"),
    "https://hf.co/Qwen/Qwen3-0.6B/commit/c1899de289a0": ("Qwen/Qwen3-0.6B", "c1899de289a0"),
    "https://huggingface.co/Qwen/Qwen3-4B/blob/abc/config.json": ("Qwen/Qwen3-4B", "abc"),
    "https://huggingface.co/Qwen/Qwen3-4B?library=transformers": ("Qwen/Qwen3-4B", ""),
    "  https://www.huggingface.co/Qwen/Qwen3-4B/  ": ("Qwen/Qwen3-4B", ""),
    "Qwen/Qwen3-4B": ("Qwen/Qwen3-4B", ""),
}

SCHLECHT = [
    "https://github.com/Qwen/Qwen3",
    "https://huggingface.co/datasets/Salesforce/wikitext",
    "https://huggingface.co/spaces/a/b",
    "https://huggingface.co/Qwen",
    "https://huggingface.co/Qwen/Qwen3-4B/discussions",
    "",
]


def main() -> int:
    fehler = 0
    for link, soll in GUT.items():
        ist = mb.repo_aus_link(link)
        if ist != soll:
            print(f"[modell_link] FEHLER: {link!r} gab {ist}, erwartet {soll}")
            fehler += 1
    for link in SCHLECHT:
        try:
            mb.repo_aus_link(link)
            print(f"[modell_link] FEHLER: {link!r} wurde nicht abgelehnt")
            fehler += 1
        except SystemExit:
            pass
    if fehler:
        print(f"[modell_link] FEHLGESCHLAGEN ({fehler})")
        return 1
    print(f"[modell_link] ⚑ BESTANDEN ({len(GUT)} gelesen, {len(SCHLECHT)} abgelehnt)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
