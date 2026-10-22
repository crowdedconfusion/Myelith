#!/usr/bin/env python3
"""
**Die Erzeuger der Bauarten gegen die von Hand verifizierten Eintraege.**

Fuer jedes Modell, dessen `config.json` lokal liegt: Der Erzeuger seiner
Bauart (`calibrate/src/bauarten.py`) muss aus dem `config.json` genau den
Eintrag in `model_configs.py` machen, bis auf die Herkunftsfelder. Dazu
der Feldabgleich (`konfigabgleich`): keine Abweichung, kein ungelesenes Feld.

⚑ **Das ist der Beleg, der einen Erzeuger zur Zusage macht.** Ohne ihn waere
das Bauskript eine Behauptung ueber fremde Konfigurationen.

Gegenprobe: Ein verfaelschtes Feld im Config muss der Abgleich melden.

Aufruf: `python3 INTEGER_LLM/tests/test_bauarten.py` (mit der
Kalibrier-Umgebung; Modelle ohne lokales `config.json` werden uebersprungen,
Flash-Next braucht transformers ab 5.18 nicht, nur den Erzeuger).
"""
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
LLM = HIER.parent
REPO = LLM.parent
sys.path.insert(0, str(LLM / "calibrate"))

from src.bauarten import erkennen  # noqa: E402
from src.konfigabgleich import flach, feldabgleich  # noqa: E402
from src.model_configs import MODEL_CONFIGS  # noqa: E402

HERKUNFT = {"verified", "hf_model_id", "variant", "betrieb"}


def main() -> int:
    fehler = 0
    geprueft = 0
    for name, eintrag in sorted(MODEL_CONFIGS.items()):
        # ⚑ Die ternaeren Pakete sind ein eigener Bauweg (gedrehte Gewichte,
        #   RoPE-Skalierung) und keine Bauart, die das Bauskript anbietet.
        if "hf_model_id" not in eintrag or "family" not in eintrag \
                or any(k in eintrag for k in ("paket", "gedreht", "rope_art")):
            continue
        pfad = REPO / "MODELS" / "llm" / eintrag["hf_model_id"].split("/")[-1] / "config.json"
        if not pfad.is_file():
            print(f"[bauarten] {name}: uebersprungen, {pfad.relative_to(REPO)} fehlt")
            continue
        roh = json.loads(pfad.read_text(encoding="utf-8"))
        bauart = erkennen(roh)
        if bauart is None or not bauart.freigegeben:
            print(f"[bauarten] ⛔️ {name}: Bauart nicht erkannt oder nicht freigegeben")
            fehler += 1
            continue
        try:
            erzeugt = bauart.erzeuger(roh, eintrag["variant"], eintrag["verified"], eintrag["hf_model_id"])
        except (ValueError, ImportError) as e:
            print(f"[bauarten] ⛔️ {name}: {e}")
            fehler += 1
            continue
        geprueft += 1
        soll = {k: v for k, v in eintrag.items() if k not in HERKUNFT}
        ist = {k: v for k, v in erzeugt.items() if k not in HERKUNFT}
        anders = sorted(k for k in set(soll) | set(ist) if soll.get(k) != ist.get(k))
        befund = feldabgleich(erzeugt, flach(roh))
        probleme = anders + [f"abweichend {a[0]}" for a in befund["abweichend"]] + befund["abgeleitet"] + \
            [f"ungelesen {u}" for u in befund["ungelesen"]]
        if probleme:
            fehler += 1
            print(f"[bauarten] ⛔️ {name} ({bauart.name}):")
            for k in anders:
                print(f"     {k}: Eintrag {soll.get(k)!r}, erzeugt {ist.get(k)!r}")
            for p in probleme[len(anders):]:
                print(f"     {p}")
        else:
            print(f"[bauarten] ⚑ {name} ({bauart.name}): erzeugt genau den Eintrag, alle Felder gelesen")

        # Gegenprobe: ein verfaelschtes Feld muss auffallen.
        if name == "pigeon-4b-int8":
            falsch = dict(flach(roh))
            falsch["num_hidden_layers"] = 35
            if not feldabgleich(erzeugt, falsch)["abweichend"]:
                print("[bauarten] ⛔️ Gegenprobe: eine falsche Ebenenzahl fiel nicht auf")
                fehler += 1
    if geprueft == 0:
        print("[bauarten] ⛔️ kein einziges Modell geprueft")
        return 1
    print(f"[bauarten] {'⚑ BESTANDEN' if not fehler else f'⛔️ FEHLGESCHLAGEN: {fehler}'} ({geprueft} geprueft)")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
