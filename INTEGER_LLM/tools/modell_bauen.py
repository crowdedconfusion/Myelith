#!/usr/bin/env python3
"""
**Ein Modell aus einem Repositorium bauen, benennen und lokal einpflegen.**

Fuer versierte Nutzer mit Admin-Rechten auf ihrer Maschine (seit 2026-10-19).
Aufgerufen ueber `INTEGER_LLM/scripts/modell_bauen.sh`, das die Umgebung
einrichtet:

    bash INTEGER_LLM/scripts/modell_bauen.sh                       # fragt alles ab
    bash INTEGER_LLM/scripts/modell_bauen.sh --repo https://huggingface.co/<org>/<modell> --probe
    bash INTEGER_LLM/scripts/modell_bauen.sh --lokal <ordner mit config.json und safetensors>

`--repo` nimmt den Link auf Hugging Face (auch mit `/tree/<revision>`) oder
`<org>/<modell>`.

Der Ablauf:

1. **Quelle waehlen**: ein Repositorium auf Hugging Face (die Revision wird
   festgehalten) oder ein Ordner, der schon auf der Platte liegt.
2. **Architektur ermitteln und freigeben**: `config.json` holen, die Bauart
   erkennen (`calibrate/src/bauarten.py`), den Exporteintrag erzeugen und
   **Feld fuer Feld gegen die Quelle halten** (`konfigabgleich`), seine
   Merkmale gegen das, was die Laufzeit rechnet. Nur wenn alles traegt, gibt
   das Skript frei; sonst endet es mit dem Grund.
3. **Namen waehlen**: Vorschlaege nach dem Schema
   `myelith-<gesamt>b[-a<aktiv>b][-int4]`, gerechnet aus den Koepfen der
   Gewichtsdateien (Sichtturm, MTP-Kopf und N-Gramm-Tabelle zaehlen nicht mit).
4. **Bauen**: Gewichte laden, kalibrieren und in int8 exportieren
   (Korpusdurchlauf oder ebenenweise, je nach Bauart), bei einem
   Expertengemisch auf Wunsch die Experten in vier Bit; danach eine Ladeprobe.
5. **Einpflegen**: Eintrag im lokalen Katalog `MODELS/llm/KATALOG.lokal.json`.
   Konsole und Fenster zeigen das Modell danach in der Modellwahl.

⛔️ **Was hier entsteht, ist privat und lokal.** Jedes Artefakt traegt
`betrieb = "lokal"` in `model_config.json`: Der Netzbetrieb laedt es nicht
(`load_model_fuer_netz`), und der Klient bietet es nicht im Netz an. Fuer das
Netz zugelassen ist nur, was dieses Projekt in `MODELS/llm/KATALOG.json` als
Netzmodell fuehrt. Die Lizenz der Gewichte gilt fuer das Artefakt weiter.

⚑ **Erst der kurze Lauf** (`--probe`): alles bis zur Freigabe und zum Namen,
die kleinen Dateien, und bei ebenenweisem Bau die erste Ebene auf den echten
Gewichten. Geschrieben wird nichts.

Die letzte Zeile ist `Fertig` oder beginnt mit `FEHLER`.
"""

import argparse
import datetime
import json
import os
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

LLM = Path(__file__).resolve().parent.parent
WURZEL = LLM.parent
sys.path.insert(0, str(LLM / "calibrate"))

from src.bauarten import erkennen, BAUARTEN  # noqa: E402
from src.konfigabgleich import flach, feldabgleich  # noqa: E402
from src import model_configs  # noqa: E402

KATALOG = WURZEL / "MODELS" / "llm" / "KATALOG.json"
KATALOG_LOKAL = WURZEL / "MODELS" / "llm" / "KATALOG.lokal.json"
QUELLEN = WURZEL / "MODELS" / "llm"
ARTEFAKTE = LLM / "artifacts"
LADER = LLM / "runtime" / "src" / "loader.rs"
GB = 1_000_000_000

# Was beim Zaehlen der Parameter nicht mitzaehlt: der Sichtturm und der MTP-Kopf
# werden nicht exportiert, die N-Gramm-Tabelle wird nur nachgeschlagen.
NICHT_GEZAEHLT = ("visual.", "vision_tower", "mtp.")
TABELLE = "ngram_embedding"


def fehler(text: str):
    print(f"FEHLER: {text}", flush=True)
    sys.exit(1)


def frage(text: str, vorgabe: str, ja: bool) -> str:
    """Eine Rueckfrage mit Vorgabe; mit `--ja` gilt die Vorgabe."""
    if ja:
        print(f"{text} [{vorgabe}]: {vorgabe}")
        return vorgabe
    try:
        antwort = input(f"{text} [{vorgabe}]: ").strip()
    except EOFError:
        antwort = ""
    return antwort or vorgabe


# ----------------------------------------------------------------------------
# Quelle
# ----------------------------------------------------------------------------

def lizenz_aus_karte(readme: Path) -> str:
    """Die Lizenz aus dem Kopf einer Modellkarte (`license:`, bei `other` der
    `license_name:`), wie Hugging Face sie fuehrt; leer, wenn keiner da ist."""
    if not readme.is_file():
        return ""
    zeilen = readme.read_text(encoding="utf-8", errors="replace").splitlines()
    if not zeilen or zeilen[0].strip() != "---":
        return ""
    kopf = {}
    for z in zeilen[1:]:
        if z.strip() == "---":
            break
        k, _, v = z.partition(":")
        kopf[k.strip()] = v.strip().strip("'\"")
    lizenz = kopf.get("license", "")
    return kopf.get("license_name") or lizenz if lizenz == "other" else lizenz


HF_HOSTS = ("huggingface.co", "www.huggingface.co", "hf.co")


def repo_aus_link(text: str) -> tuple:
    """**Repositorium und Revision aus einem Link auf Hugging Face**
    (`https://huggingface.co/<org>/<modell>`, auch mit `/tree/<revision>`,
    `/commit/<revision>` oder `/blob/<revision>/...`), oder aus `<org>/<modell>`.
    Gibt `(repo, revision)`; die Revision ist leer, wenn der Link keine nennt.

    ⚑ **Die einzige Stelle, die einen Link liest**: Die Konsole
    (`/build artifact`) reicht ihn unveraendert durch.
    """
    t = text.strip()
    rest = t
    for schema in ("https://", "http://"):
        if rest.startswith(schema):
            rest = rest[len(schema):]
            break
    host, _, pfad = rest.partition("/")
    if host in HF_HOSTS:
        rest = pfad
    elif "://" in t or "." in host:
        fehler(f"{t} ist kein Link auf Hugging Face (erwartet https://huggingface.co/<org>/<modell>)")
    teile = [x for x in rest.split("?")[0].split("#")[0].split("/") if x]
    if teile[:1] in (["datasets"], ["spaces"]):
        fehler(f"{t} ist ein Datensatz oder Space, kein Modell")
    if len(teile) < 2:
        fehler(f"{t} nennt kein Modell (erwartet https://huggingface.co/<org>/<modell>)")
    repo = f"{teile[0]}/{teile[1]}"
    revision = ""
    if len(teile) >= 4 and teile[2] in ("tree", "commit", "blob", "resolve"):
        revision = teile[3]
    elif len(teile) > 2:
        fehler(f"{t}: unerwarteter Pfad hinter {repo}")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*", repo):
        fehler(f"{repo} ist kein gueltiger Name eines Repositoriums")
    return repo, revision


class Quelle:
    """Ein Repositorium auf Hugging Face oder ein Ordner auf der Platte."""

    def __init__(self, repo: str = "", revision: str = "", ordner: str = ""):
        self.repo, self.revision = repo, revision
        self.lizenz = ""
        if ordner:
            self.ordner = Path(ordner).resolve()
            if not (self.ordner / "config.json").is_file():
                fehler(f"{self.ordner} enthaelt kein config.json")
            self.repo = f"lokal/{self.ordner.name}"
            self.revision = "lokal"
            ziel = QUELLEN / self.ordner.name
            if ziel.exists() and ziel.resolve() != self.ordner:
                fehler(f"{ziel} gibt es schon und zeigt nicht auf {self.ordner}")
            self.lizenz = lizenz_aus_karte(self.ordner / "README.md")
        else:
            from huggingface_hub import HfApi
            info = HfApi().model_info(repo, revision=revision or None)
            self.revision = info.sha
            karte = getattr(info, "card_data", None) or {}
            lizenz = karte.get("license") if hasattr(karte, "get") else None
            name = karte.get("license_name") if hasattr(karte, "get") else None
            self.lizenz = name if lizenz == "other" and name else (lizenz or "")
            self.ordner = QUELLEN / repo.split("/")[-1]

    @property
    def lokal(self) -> bool:
        return self.revision == "lokal"

    def verweisen(self):
        """⚑ Die Kalibrierung laedt nur aus `MODELS/llm/<name>` (keine zweite
        Ablage, `paths.local_model_dir`); ein Ordner anderswo bekommt dort
        einen Verweis. Erst beim Bau: Eine Erkundung hinterlaesst nichts."""
        ziel = QUELLEN / self.ordner.name
        if self.lokal and not ziel.exists():
            ziel.symlink_to(self.ordner, target_is_directory=True)

    def kleine_dateien(self):
        """config.json, Tokenizer, Lizenz, Verzeichnis der Gewichte."""
        if self.lokal:
            return
        from huggingface_hub import snapshot_download
        snapshot_download(self.repo, revision=self.revision, local_dir=self.ordner,
                          allow_patterns=["*.json", "*.jinja", "*.txt", "LICENSE*", "*.md"])

    def config(self) -> dict:
        return json.loads((self.ordner / "config.json").read_text(encoding="utf-8"))

    def tensorkoepfe(self) -> dict:
        """Name zu `(dtype, form)` fuer jeden Tensor, ohne Gewichte zu laden."""
        aus = {}
        if self.lokal:
            for datei in sorted(self.ordner.glob("*.safetensors")):
                with open(datei, "rb") as f:
                    laenge = struct.unpack("<Q", f.read(8))[0]
                    for n, e in json.loads(f.read(laenge)).items():
                        if n != "__metadata__":
                            aus[n] = (e["dtype"], e["shape"])
            return aus
        from huggingface_hub import get_safetensors_metadata
        meta = get_safetensors_metadata(self.repo, revision=self.revision)
        for datei in meta.files_metadata.values():
            for n, t in datei.tensors.items():
                aus[n] = (t.dtype, list(t.shape))
        return aus

    def bytes_gesamt(self, koepfe: dict) -> int:
        groesse = {"BF16": 2, "F16": 2, "F32": 4, "I64": 8, "F8_E4M3": 1, "I8": 1, "U8": 1}
        summe = 0
        for dtype, form in koepfe.values():
            n = 1
            for d in form:
                n *= d
            summe += n * groesse.get(dtype, 2)
        return summe

    def gewichte(self, nur: list = None):
        """Die Gewichtsdateien holen (ganz oder nur die genannten)."""
        if self.lokal:
            return
        from huggingface_hub import snapshot_download
        snapshot_download(self.repo, revision=self.revision, local_dir=self.ordner,
                          allow_patterns=nur)


# ----------------------------------------------------------------------------
# Freigabe
# ----------------------------------------------------------------------------

def gekonnte_merkmale() -> set:
    """Was dieser Bau der Laufzeit rechnen kann, aus `GEKONNTE_MERKMALE`."""
    quelle = LADER.read_text(encoding="utf-8")
    anfang = quelle.index("pub const GEKONNTE_MERKMALE")
    ende = quelle.index("];", anfang)
    teil = "\n".join(z.split("//")[0] for z in quelle[anfang:ende].splitlines())
    return set(re.findall(r'"([a-z0-9_]+)"', teil))


def freigeben(roh: dict, quelle: Quelle, variant: str):
    """**Die Bauart erkennen und nur freigeben, was traegt.** Zurueck kommen
    Bauart und Exporteintrag, sonst endet das Skript mit dem Grund."""
    bauart = erkennen(roh)
    typen = sorted({roh.get("model_type"), (roh.get("text_config") or {}).get("model_type")} - {None})
    if bauart is None:
        bekannt = ", ".join(f"{b.name} ({'/'.join(b.model_types)})" for b in BAUARTEN if b.freigegeben)
        fehler(f"Architektur {typen} wird nicht unterstuetzt. Unterstuetzt: {bekannt}")
    print(f"[bauen] Architektur: {roh.get('architectures')} ({', '.join(typen)}) -> {bauart.name}")
    if not bauart.freigegeben:
        fehler(f"{bauart.name} ist erkannt, aber nicht freigegeben: {bauart.grund}")
    datum = datetime.date.today().isoformat()
    herkunft = f"{quelle.repo} config.json@{quelle.revision[:8]}, Feldabgleich bestanden am {datum}"
    try:
        eintrag = bauart.erzeuger(roh, variant, herkunft, quelle.repo)
    except ValueError as e:
        fehler(f"{bauart.name}: {e}")
    befund = feldabgleich(eintrag, flach(roh))
    probleme = [f"{a[0]}: Quelle {a[1]!r}, Eintrag {a[2]!r}" for a in befund["abweichend"]]
    probleme += befund["abgeleitet"] + [f"ungelesen: {u}" for u in befund["ungelesen"]]
    if probleme:
        fehler("nicht freigegeben, die Quelle hat Felder, die der Erzeuger nicht abbildet:\n  "
               + "\n  ".join(probleme))
    eintrag["betrieb"] = "lokal"
    merkmale = set(model_configs.merkmale_ableiten(eintrag))
    fehlend = merkmale - gekonnte_merkmale()
    if fehlend:
        fehler(f"nicht freigegeben, die Laufzeit rechnet diese Merkmale nicht: {sorted(fehlend)}")
    print(f"[bauen] ⚑ freigegeben: jedes Feld der Quelle abgebildet, Merkmale {sorted(merkmale)} gerechnet")
    return bauart, eintrag


# ----------------------------------------------------------------------------
# Name
# ----------------------------------------------------------------------------

def parameter(koepfe: dict, eintrag: dict):
    """Gesamt und aktiv in Milliarden, dazu die Groesse der N-Gramm-Tabelle."""
    gesamt = experten = tabelle = 0
    for n, (_, form) in koepfe.items():
        if any(t in n for t in NICHT_GEZAEHLT):
            continue
        # 📌 **Eine gebundene Einbettung zaehlt einmal** (2026-10-19): Qwen3-0.6B
        #   legt `lm_head.weight` trotz `tie_word_embeddings` als eigene Kopie
        #   ab, und der Vorschlag hiess `myelith-0.8b` statt `myelith-0.6b`.
        if eintrag.get("tie_word_embeddings") and n.endswith("lm_head.weight"):
            continue
        k = 1
        for d in form:
            k *= d
        if TABELLE in n:
            tabelle += k
            continue
        gesamt += k
        if ".mlp.experts." in f".{n}." or n.endswith(("experts.gate_up_proj", "experts.down_proj")):
            experten += k
    aktiv = gesamt
    if eintrag.get("num_experts"):
        aktiv = gesamt - experten + experten * eintrag["num_experts_per_tok"] / eintrag["num_experts"]
    return gesamt / 1e9, aktiv / 1e9, tabelle / 1e9


def groesse(milliarden: float) -> str:
    """`0.6`, `4`, `126`: unter einer Milliarde eine Stelle nach dem Komma,
    unter einer Zehntel so viele, dass eine Ziffer ungleich null steht."""
    if milliarden >= 1:
        return str(round(milliarden))
    stellen = 1
    while stellen < 6 and round(milliarden, stellen) == 0:
        stellen += 1
    return f"{milliarden:.{stellen}f}"


def namensvorschlaege(gesamt: float, aktiv: float, gemisch: bool) -> list:
    stamm = f"myelith-{groesse(gesamt)}b"
    if gemisch:
        stamm += f"-a{max(1, round(aktiv))}b"
    return [f"{stamm}-int4", stamm] if gemisch else [stamm]


NAMENSMUSTER = re.compile(r"^myelith-[0-9][0-9.]*b(-a[0-9]+b)?(-[a-z0-9]+)*$")


def name_pruefen(name: str) -> str:
    if not re.fullmatch(r"[a-z0-9][a-z0-9.-]{1,63}", name):
        return "nur Kleinbuchstaben, Ziffern, Punkt und Bindestrich, 2 bis 64 Zeichen"
    vergeben = set(model_configs.MODEL_CONFIGS)
    for k in (KATALOG, KATALOG_LOKAL):
        if k.is_file():
            vergeben |= set(json.loads(k.read_text(encoding="utf-8")))
    if name in vergeben or (ARTEFAKTE / name).exists():
        return "der Name ist schon vergeben"
    return ""


def projektschluessel(repo: str) -> str:
    """**Ist das Repositorium ein Modell dieses Projekts?** Dann der Schluessel
    seines int8-Eintrags in `model_configs.py` (der kuerzeste unter denen mit
    diesem `hf_model_id`), sonst leer."""
    treffer = [k for k, v in model_configs.MODEL_CONFIGS.items() if v.get("hf_model_id") == repo]
    return min(treffer, key=len) if treffer else ""


def name_befund(name: str, gemisch_vierbit: bool, projekt: str = "") -> tuple:
    """Grund gegen den Namen (leer, wenn er geht), ob er in vier Bit baut, und
    der Name des int8-Artefakts darunter."""
    vierbit = name.endswith("-int4") and gemisch_vierbit
    int8_name = quelle_des_viers(name) if vierbit else name
    if projekt:
        if int8_name != projekt:
            return f"ein Projektmodell heisst {projekt} oder {projekt}-int4", vierbit, int8_name
        if (ARTEFAKTE / name / "model_config.json").is_file():
            return "liegt schon da", vierbit, int8_name
        return "", vierbit, int8_name
    grund = name_pruefen(name) or (name_pruefen(int8_name) if vierbit else "")
    return grund, vierbit, int8_name


def quelle_des_viers(name: str) -> str:
    """Der Name des int8-Artefakts, aus dem das in vier Bit entsteht."""
    return name[: -len("-int4")] if name.endswith("-int4") else f"{name}-int8"


# ----------------------------------------------------------------------------
# Bau und Eintrag
# ----------------------------------------------------------------------------

def python_der(umgebung: str) -> Path:
    """Der Python der Umgebung einer Bauart; fehlt sie, wird sie eingerichtet
    (`python_einrichten.sh`, fuer Flash-Next mit `--flashnext`)."""
    py = LLM / "calibrate" / umgebung / "bin" / "python3"
    if not py.is_file():
        schalter = ["--flashnext"] if umgebung.endswith("flashnext") else []
        print(f"[bauen] Umgebung {umgebung} einrichten")
        laufen(["bash", LLM / "scripts" / "python_einrichten.sh", *schalter])
        if not py.is_file():
            fehler(f"die Umgebung {umgebung} fehlt nach dem Einrichten")
    return py


def korpus_sichern(py: Path):
    """**Der WikiText-2-Testsplit fuer den Kalibrierkorpus.** Ohne ihn bricht
    die Kalibrierung ab (mit Absicht: Ein Artefakt mit schmalerer Grundlage
    waere mit den uebrigen nicht vergleichbar). Im frischen Klon fehlt er."""
    if not (WURZEL / "BENCHMARKS" / "Inferenz" / "datasets" / "wikitext2_test.txt").is_file():
        print("[bauen] WikiText-2-Testsplit fuer den Kalibrierkorpus holen")
        laufen([py, WURZEL / "BENCHMARKS" / "Inferenz" / "wikitext_common.py"])


def laufen(befehl: list, umgebung: dict = None, wo: Path = LLM):
    """Ein Schritt mit durchgereichter Ausgabe.

    📌 **Das „Fertig“ eines Schritts ist nicht das des Bauskripts**
    (2026-10-19): Kalibrierung, WikiText und Vierbitbau enden selbst mit
    „Fertig“, und ein Waechter, der darauf wartet, schlug beim ersten Schritt
    an. Ihre Schlusszeile heisst deshalb hier anders; „Fertig“ allein auf
    einer Zeile schreibt nur das Bauskript, als letzte.
    """
    print(f"[bauen] $ {' '.join(str(b) for b in befehl)}", flush=True)
    p = subprocess.Popen([str(b) for b in befehl], cwd=wo, env={**os.environ, **(umgebung or {})},
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
    for zeile in p.stdout:
        if zeile.strip() == "Fertig":
            zeile = "[bauen]   Schritt beendet\n"
        print(zeile, end="", flush=True)
    if p.wait() != 0:
        fehler(f"Schritt endete mit {p.returncode}: {befehl[0]}")


def ladeprobe(name: str, quelle: Quelle):
    """Laedt das Artefakt mit der Laufzeit und rechnet vier Token."""
    laufen(["cargo", "build", "--release", "--offline", "--features", "cpu-simd",
            "--bin", "flashnext_vergleich", "--quiet"], wo=LLM / "runtime")
    from tokenizers import Tokenizer
    tok = Tokenizer.from_file(str(ARTEFAKTE / name / "tokenizer.json"))
    ids = tok.encode("Die Hauptstadt von Frankreich ist", add_special_tokens=False).ids[:4]
    r = subprocess.run([str(WURZEL / "SYSTEM/full-build/release/flashnext_vergleich"),
                        str(ARTEFAKTE / name), json.dumps(ids)], capture_output=True, text=True)
    if r.returncode != 0:
        fehler(f"{name} laedt oder rechnet nicht: {r.stderr[-1500:]}")
    print(f"[bauen] ⚑ {name} laedt und rechnet")


def ordnergroesse(pfad: Path) -> int:
    return sum(p.stat().st_size for p in pfad.rglob("*") if p.is_file())


def einpflegen(name: str, quelle: Quelle, bauart, eintrag: dict, gesamt: float, aktiv: float,
               tabelle: float, bauweg: str, aus: str = ""):
    """**Der Eintrag im lokalen Katalog.** Ohne `anzeigename`: Den bildet der
    Klient aus dem Namen nach dem Schema, an genau einer Stelle."""
    katalog = json.loads(KATALOG_LOKAL.read_text(encoding="utf-8")) if KATALOG_LOKAL.is_file() else {}
    groesse_bytes = ordnergroesse(ARTEFAKTE / name)
    gemisch = bool(eintrag.get("num_experts"))
    speicher = groesse_bytes / GB / 1.5 if gemisch else groesse_bytes / GB + 6
    def mrd(x: float) -> str:
        return groesse(x).replace(".", ",") + " Mrd."
    teile = [mrd(gesamt) + (f" gesamt, rund {mrd(aktiv)} aktiv je Token" if gemisch else "")]
    if tabelle:
        teile.append(f"dazu {mrd(tabelle)} N-Gramm-Tabelle, nur nachgeschlagen")
    katalog[name] = {
        "betrieb": "lokal",
        "status": "erprobt",
        "reihung": round(gesamt, 1),
        "hardware": f"ab {max(4, round(speicher))} GB Arbeitsspeicher, {groesse_bytes / GB:.0f} GB Platte",
        "hf_repo": quelle.repo,
        "hf_verzeichnis": quelle.ordner.name,
        "hf_revision": quelle.revision,
        "lizenz_gewichte": quelle.lizenz or "unbekannt, vor jeder Weitergabe pruefen",
        "lizenz_artefakt": "privat, nur lokal; die Lizenz der Gewichte gilt weiter",
        "parameter": "; ".join(teile),
        "artefakt_bytes": groesse_bytes,
        "bauart": bauart.name,
        "bauweg": bauweg,
        "gebaut_am": datetime.date.today().isoformat(),
    }
    if aus:
        katalog[name]["quelle"] = aus
    KATALOG_LOKAL.write_text(json.dumps(katalog, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"[bauen] eingetragen in {KATALOG_LOKAL.relative_to(WURZEL)}: {name} (privat, nur lokal)")


def main():
    ap = argparse.ArgumentParser(description="Ein Modell bauen, benennen und lokal einpflegen.")
    ap.add_argument("--repo", help="Link auf Hugging Face (https://huggingface.co/<org>/<modell>) oder <org>/<modell>")
    ap.add_argument("--revision", default="", help="Revision (Vorgabe: die neueste, festgehalten)")
    ap.add_argument("--lokal", default="", help="statt --repo: ein Ordner mit config.json und safetensors")
    ap.add_argument("--name", default="", help="Name des Artefakts (sonst Vorschlag)")
    ap.add_argument("--format", choices=["int4", "int8"], default="",
                    help="bei einem Expertengemisch: int4 (Experten in vier Bit) oder int8; sonst gefragt")
    ap.add_argument("--probe", action="store_true", help="der kurze Lauf: nichts bauen, nichts schreiben")
    ap.add_argument("--projekt", action="store_true",
                    help="ein Modell dieses Projekts (Eintrag in model_configs.py) bauen, ohne lokalen Katalog")
    ap.add_argument("--ja", action="store_true", help="keine Rueckfragen, die Vorgaben gelten")
    ap.add_argument("--quelle-loeschen", action="store_true",
                    help="die geladenen Gewichte loeschen, sobald das int8-Artefakt laedt (senkt den Platzbedarf)")
    ap.add_argument("--erkunden", action="store_true",
                    help="nur erkunden: Freigabe, Parameter, Namen, Platz als eine Zeile ERKUNDUNG <json>; baut nichts")
    a = ap.parse_args()

    if a.projekt:
        print("[bauen] Projektmodell: Eintrag aus model_configs.py, kein lokaler Katalog.")
    else:
        print("[bauen] ⛔️ Was hier entsteht, ist privat und lokal: nicht fuer den Netzbetrieb zugelassen.")
    # ⚑ Zwei Wege und keine Vorgabe: ein Link auf Hugging Face oder ein Ordner.
    if not a.repo and not a.lokal:
        if a.ja:
            fehler("--ja braucht --repo <link> oder --lokal <ordner>")
        a.repo = frage("Link des Repositoriums auf Hugging Face (leer: ein Ordner auf diesem Rechner)", "", False)
        if not a.repo:
            a.lokal = frage("Pfad des Ordners mit config.json und safetensors", "", False)
            if not a.lokal:
                fehler("weder Link noch Ordner")
    if a.repo:
        a.repo, aus_link = repo_aus_link(a.repo)
        if aus_link and a.revision and aus_link != a.revision:
            fehler(f"Revision {aus_link} im Link, {a.revision} mit --revision")
        a.revision = a.revision or aus_link
    quelle = Quelle(a.repo, a.revision, a.lokal) if a.repo else Quelle(ordner=a.lokal)
    print(f"[bauen] Quelle: {quelle.repo} @ {quelle.revision[:12]}, Lizenz der Gewichte: {quelle.lizenz or 'nicht angegeben'}")
    quelle.kleine_dateien()
    roh = quelle.config()

    # Freigabe mit vorlaeufiger Variante; die endgueltige folgt aus dem Namen.
    bauart, eintrag = freigeben(roh, quelle, "vorlaeufig")
    koepfe = quelle.tensorkoepfe()
    gesamt, aktiv, tabelle = parameter(koepfe, eintrag)
    gemisch = bool(eintrag.get("num_experts"))
    print(f"[bauen] Parameter: {gesamt:.2f} Mrd." + (f", aktiv {aktiv:.2f} Mrd." if gemisch else "")
          + (f", N-Gramm-Tabelle {tabelle:.1f} Mrd." if tabelle else ""))

    gemisch_vierbit = gemisch and bauart.vierbit
    projekt = projektschluessel(quelle.repo)
    if a.projekt and not projekt:
        fehler(f"--projekt: {quelle.repo} ist kein Modell aus model_configs.py")
    if projekt and not a.projekt:
        print(f"[bauen] ⚑ {quelle.repo} ist das Projektmodell {projekt}; mit --projekt baut es unter seinem Namen")
    vorschlaege = namensvorschlaege(gesamt, aktiv, gemisch_vierbit)
    if a.projekt or (a.erkunden and projekt):
        vorschlaege = [f"{projekt}-int4", projekt] if gemisch_vierbit else [projekt]

    if a.erkunden:
        # ⚑ **Eine Zeile fuer die Konsole** (`/build artifact`): Sie baut daraus
        #   die Wahl, statt Saetze zu lesen. Eine Erkundung schreibt nichts.
        bf16 = quelle.bytes_gesamt(koepfe)
        int8 = gesamt * GB + tabelle * GB
        aus = {
            "repo": quelle.repo, "revision": quelle.revision, "lokal": quelle.lokal,
            "ordner": str(quelle.ordner), "lizenz": quelle.lizenz,
            "architektur": roh.get("architectures") or [], "bauart": bauart.name, "bauweg": bauart.bauweg,
            "gemisch": gemisch, "vierbit": gemisch_vierbit,
            "gesamt": round(gesamt, 3), "aktiv": round(aktiv, 3), "tabelle": round(tabelle, 3),
            "projekt": projekt,
            "vorschlaege": [{"name": v, "vierbit": name_befund(v, gemisch_vierbit, projekt)[1],
                             "grund": name_befund(v, gemisch_vierbit, projekt)[0]} for v in vorschlaege],
            "quelle_bytes": bf16, "frei_bytes": shutil.disk_usage(ARTEFAKTE).free,
            # Der Hoehepunkt des Platzes: beim int8-Bau Quelle und int8 zugleich;
            # mit vier Bit kommt der Bau in vier Bit dazu, ausser die Quelle wird
            # vorher geloescht (`--quelle-loeschen`), dann bleibt `noetig_bytes`.
            "noetig_bytes": int(bf16 + int8), "noetig_vierbit_bytes": int(bf16 + int8 + gesamt * 0.9 * GB),
        }
        if a.name:
            aus["name"] = a.name
            aus["name_grund"] = name_befund(a.name, gemisch_vierbit, projekt)[0]
        print("ERKUNDUNG " + json.dumps(aus, ensure_ascii=False))
        print("Fertig")
        return

    # ⚑ **Erst das Format, dann der Name** (Wunsch des Projektinhabers,
    #   2026-10-19): bei einem Gemisch, das in vier Bit gehen kann, gewaehlt;
    #   die Vorschlaege danach nur in diesem Format. Ein Name auf `-int4` baut
    #   in vier Bit, jeder andere in int8.
    if gemisch_vierbit and not a.name:
        fmt = a.format or frage("Format: int4 (Experten in vier Bit, kleiner) oder int8", "int4", a.ja)
        if fmt not in ("int4", "int8"):
            fehler(f"Format {fmt}: int4 oder int8")
        vorschlaege = [v for v in vorschlaege if v.endswith("-int4") == (fmt == "int4")]
    elif a.format == "int4" and not gemisch_vierbit:
        fehler("int4 gibt es nur fuer die Experten eines Gemischs; diese Bauart baut int8")
    print("[bauen] Namensvorschlaege nach dem Schema myelith-<gesamt>b[-a<aktiv>b][-int4]:")
    for i, v in enumerate(vorschlaege):
        art = "Experten in vier Bit (kleiner, empfohlen)" if v.endswith("-int4") else "int8"
        print(f"    {i + 1}. {v}  ({art})")
    while True:
        name = a.name or frage("Name des Artefakts", vorschlaege[0], a.ja)
        if name.isdigit() and 1 <= int(name) <= len(vorschlaege):
            name = vorschlaege[int(name) - 1]
        grund, vierbit, int8_name = name_befund(name, gemisch_vierbit, projekt if a.projekt else "")
        if a.projekt and grund == "liegt schon da":
            grund = ""
        if not grund:
            break
        if a.name or a.ja:
            fehler(f"Name {name}: {grund}")
        print(f"[bauen] {name}: {grund}")
    if not NAMENSMUSTER.match(name):
        print(f"[bauen] ⚠️ {name} folgt nicht dem Schema; Konsole und Fenster zeigen den Ordnernamen")
    eintrag["variant"] = int8_name.removeprefix("myelith-")

    # Platz
    bf16 = quelle.bytes_gesamt(koepfe)
    noetig = bf16 + gesamt * GB + tabelle * GB + (gesamt * 0.9 * GB if vierbit else 0)
    # ⚑ Mit `--quelle-loeschen` ist der Hoehepunkt der int8-Bau: Quelle und
    #   int8 zugleich. Der Bau in vier Bit braucht danach weniger als die
    #   Quelle, die dann schon weg ist.
    if a.quelle_loeschen and not quelle.lokal:
        noetig = bf16 + gesamt * GB + tabelle * GB
    frei = shutil.disk_usage(ARTEFAKTE).free
    print(f"[bauen] Platz: rund {noetig / GB:.0f} GB gebraucht (Quelle {bf16 / GB:.0f} GB), frei {frei / GB:.0f} GB")

    if a.projekt:
        eintrag = model_configs.get_export_model_config(int8_name)
    else:
        eintrag_pfad = model_configs.LOKALE_EINTRAEGE / f"{int8_name}.json"

    quelle.verweisen()
    if a.probe:
        if bauart.bauweg == "ebenenweise":
            py = python_der(bauart.umgebung)
            karte = json.loads((quelle.ordner / "model.safetensors.index.json").read_text())["weight_map"] \
                if (quelle.ordner / "model.safetensors.index.json").is_file() else {}
            noetig_dateien = sorted({d for n, d in karte.items() if "embed_tokens" in n or ".layers.0." in n})
            quelle.gewichte(noetig_dateien or None)
            korpus_sichern(py)
            vorlaeufig = model_configs.LOKALE_EINTRAEGE / f"{int8_name}.json"
            if not a.projekt:
                vorlaeufig.parent.mkdir(parents=True, exist_ok=True)
                vorlaeufig.write_text(json.dumps(eintrag, indent=1, ensure_ascii=False))
            try:
                laufen([py, "-m", "calibrate.src.ebenenweise"],
                       {"INTEGER_LLM_MODEL": int8_name, "INTEGER_LLM_EBENEN_PROBE": "1"})
            finally:
                if not a.projekt:
                    vorlaeufig.unlink(missing_ok=True)
        print(f"[bauen] PROBE bestanden; der lange Lauf baut {name}" + (f" ueber {int8_name}" if vierbit else ""))
        print("Fertig")
        return

    if frei < noetig and not quelle.lokal:
        fehler(f"zu wenig Platz: rund {noetig / GB:.0f} GB gebraucht, frei {frei / GB:.0f} GB")
    if not a.projekt:
        eintrag_pfad.parent.mkdir(parents=True, exist_ok=True)
        eintrag_pfad.write_text(json.dumps(eintrag, indent=1, ensure_ascii=False), encoding="utf-8")

    if not (ARTEFAKTE / int8_name / "theta_v.json").is_file():
        print("[bauen] Gewichte laden")
        quelle.gewichte()
        py = python_der(bauart.umgebung)
        korpus_sichern(py)
        modul = "calibrate.src.ebenenweise" if bauart.bauweg == "ebenenweise" else "calibrate.src.main"
        print(f"[bauen] Kalibrieren und in int8 exportieren ({bauart.bauweg})")
        laufen([py, "-m", modul], {"INTEGER_LLM_MODEL": int8_name})
    ladeprobe(int8_name, quelle)
    if not a.projekt:
        einpflegen(int8_name, quelle, bauart, eintrag, gesamt, aktiv, tabelle, bauart.bauweg)
    # ⛔️ Nur geladene Gewichte, nie ein Ordner, den der Nutzer mitgebracht hat
    #   (`--lokal`), und erst, wenn das int8-Artefakt geladen und gerechnet hat.
    if a.quelle_loeschen and not quelle.lokal:
        weg = sorted(quelle.ordner.glob("*.safetensors"))
        for d in weg:
            d.unlink()
        print(f"[bauen] Quelle geloescht: {len(weg)} Gewichtsdateien unter {quelle.ordner}")
    if vierbit:
        print("[bauen] Experten in vier Bit")
        python_der(".venv")
        laufen(["bash", LLM / "scripts" / "build_vierbit.sh", int8_name, name])
        ladeprobe(name, quelle)
        if not a.projekt:
            einpflegen(name, quelle, bauart, eintrag, gesamt, aktiv, tabelle, "vierbit", aus=int8_name)
    if a.projekt:
        print("[bauen] ⚑ Projektmodell: den Status in MODELS/llm/KATALOG.json von Hand nachtragen")
    privat = ", privat" if eintrag.get("betrieb") == "lokal" else ""
    print(f"[bauen] {name} ist gebaut und lokal nutzbar (Modellwahl in Konsole und Fenster{privat})")
    print("Fertig")


if __name__ == "__main__":
    main()
