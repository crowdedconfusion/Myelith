#!/usr/bin/env python3
"""Sprechen ueber CosyVoice, fuer Myelith.

Diese Datei wird von `myl-senses` mitgebracht und beim Einrichten nach
`<Heimat>/bin/sprechen-cosyvoice.py` geschrieben. **Danach gehoert sie
dir**: Sie wird nie ueberschrieben, solange sie dort liegt.

Zwei Betriebsarten, beide mit demselben Ergebnis:

    python3 sprechen-cosyvoice.py <text.txt> <ziel.wav> [<probe.wav>]
    python3 sprechen-cosyvoice.py --dauer [<probe.wav>]

⚑ Die zweite ist die wichtige. CosyVoice laedt je Aufruf ein halbes
Milliardenmodell; satzweise zu sprechen waere langsamer als gar nicht zu
streamen, wenn jeder Satz einen neuen Prozess braeuchte. Im Dauerbetrieb
laedt es **einmal** und spricht dann Satz fuer Satz:

    <-- bereit                      (einmal, wenn das Modell steht)
    --> <text.txt>\t<ziel.wav>      (je Satz)
    <-- ok      oder   fehler: ...  (je Satz)

Schliesst die Standardeingabe, ist Schluss.

Umgebung:
    MYL_COSYVOICE          Wurzel der CosyVoice-Installation (Pflicht)
    MYL_COSYVOICE_MODELL   Gewichte; ohne Angabe erst Fun-CosyVoice3-0.5B,
                           dann CosyVoice2-0.5B unter <Wurzel>/pretrained_models
    MYL_STIMMTEXT          Datei mit dem Text der Stimmprobe (fuer zero-shot)

⚠️ Ohne MYL_STIMMTEXT wird `inference_cross_lingual` genommen. Das geht
ohne den Text der Probe und trifft die Stimme etwas schlechter.
"""

import contextlib
import os
import sys


def fehler(satz):
    print(f"fehler: {satz}", flush=True)


@contextlib.contextmanager
def stdout_nach_stderr():
    """⛔️ **stdout ist der Protokollkanal und sonst nichts.**

    modelscope, tqdm und torch schreiben beim Laden nach stdout, und der
    Aufrufer liest dort seine Zeilen (`bereit`, `ok`, `fehler: ...`).
    Eine Fortschrittszeile mitten im Handschlag sieht fuer ihn aus wie
    eine falsche Antwort. Also geht waehrend des Ladens und waehrend des
    Rechnens alles nach stderr, wo es hingehoert.
    """
    echt = sys.stdout
    try:
        sys.stdout = sys.stderr
        yield
    finally:
        sys.stdout = echt


def modellordner(wurzel):
    """Welche Gewichte genommen werden, und warum in dieser Reihenfolge.

    ⛔️ **Deutsch kann erst CosyVoice 3.** Die 2.0-Gewichte decken
    Chinesisch und Englisch ab; eine deutsche Stimmprobe ergibt damit
    Laute, die wie Deutsch klingen und keines sind. Gemessen am
    2026-09-17: whisper las aus dem Ergebnis „dies Go! Ist die Örsteck
    ist proschein" statt „dies ist die erste gesprochene Antwort".
    Deshalb gewinnt 3, wenn es da ist.
    """
    eigen = os.environ.get("MYL_COSYVOICE_MODELL", "").strip()
    if eigen:
        return eigen
    for name in ("Fun-CosyVoice3-0.5B", "CosyVoice2-0.5B"):
        pfad = os.path.join(wurzel, "pretrained_models", name)
        if os.path.isdir(pfad):
            return pfad
    raise SystemExit(
        "keine Gewichte gefunden unter {}/pretrained_models".format(wurzel)
    )


def laden():
    """Laedt die Gewichte und sagt dazu, ob es CosyVoice 3 ist.

    ⚑ **`AutoModel` waehlt die Klasse selbst**, anhand der Konfiguration
    im Modellordner. Von Hand zu waehlen hiesse, dieselbe Entscheidung
    ein zweites Mal zu treffen, und die zweite meldet sich nicht, wenn
    CosyVoice eine vierte Fassung bekommt.
    """
    wurzel = os.environ.get("MYL_COSYVOICE", "").strip()
    if not wurzel:
        raise SystemExit("MYL_COSYVOICE ist nicht gesetzt: es zeigt auf die CosyVoice-Installation")
    if not os.path.isdir(wurzel):
        raise SystemExit(f"MYL_COSYVOICE zeigt auf nichts: {wurzel}")
    # CosyVoice erwartet, aus seinem eigenen Baum importiert zu werden,
    # samt der Drittkiste Matcha-TTS.
    sys.path.insert(0, wurzel)
    sys.path.insert(0, os.path.join(wurzel, "third_party", "Matcha-TTS"))

    modell = modellordner(wurzel)
    if not os.path.isdir(modell):
        raise SystemExit(f"die Gewichte fehlen: {modell}")

    from cosyvoice.cli.cosyvoice import AutoModel  # noqa: E402

    ist_drei = os.path.isfile(os.path.join(modell, "cosyvoice3.yaml"))
    return AutoModel(model_dir=modell), ist_drei


def probe_laden(pfad):
    """Die Stimmprobe und, wenn es ihn gibt, ihr Text.

    ⛔️ Die Probe wird **als Pfad** weitergereicht und nicht geladen.
    `frontend_zero_shot` ruft `load_wav` selbst, und zwar zweimal: bei
    16 kHz fuer die Sprecherkennung und bei 24 kHz fuer die Merkmale.
    Wer hier einen Tensor uebergibt, bekommt `TypeError: Invalid file:
    tensor([[...]])` aus soundfile, und zwar erst nach anderthalb Minuten
    Modellladen.
    """
    if not pfad:
        return None, None
    ton = pfad
    textdatei = os.environ.get("MYL_STIMMTEXT", "").strip()
    text = None
    if textdatei and os.path.isfile(textdatei):
        with open(textdatei, encoding="utf-8") as f:
            text = f.read().strip() or None
    return ton, text


# ⛔️ **CosyVoice 3 will diesen Vorspann, und ohne ihn zerfaellt es.**
# Ohne das Sondertoken steigt es mit `Kernel size can't be greater than
# actual input size` aus einer conv1d aus, und zwar erst nach dem
# Modellladen. Es steht so in `example.py` des Projekts.
VORSPANN = "You are a helpful assistant.<|endofprompt|>"


def sprechen(tts, text, ziel, probe, probentext, ist_drei=False):
    import torchaudio  # noqa: E402

    if probe is None:
        # ⚠️ CosyVoice2 ist ein Zero-Shot-Modell und bringt nicht
        # zwangslaeufig eingebaute Stimmen mit. Ohne Probe geht es nur,
        # wenn welche da sind, und sonst sagt es das.
        spks = tts.list_available_spks()
        if not spks:
            raise RuntimeError(
                "dieses CosyVoice-Modell hat keine eingebaute Stimme; "
                "es braucht eine Stimmprobe (Einstellungsseite: Stimme hochladen)"
            )
        stuecke = tts.inference_sft(text, spks[0], stream=False)
    elif probentext:
        # ⚑ Mit dem Text der Probe trifft CosyVoice die Stimme am besten.
        vorne = VORSPANN if ist_drei else ""
        stuecke = tts.inference_zero_shot(text, vorne + probentext, probe, stream=False)
    else:
        # Ohne ihn: der sprachuebergreifende Weg, der ohne auskommt.
        # ⚑ Hier traegt der **Sprechtext** den Vorspann, nicht der
        # Probentext, denn einen Probentext gibt es ja nicht.
        stuecke = tts.inference_cross_lingual(
            (VORSPANN if ist_drei else "") + text, probe, stream=False
        )

    teile = [s["tts_speech"] for s in stuecke]
    if not teile:
        raise RuntimeError("CosyVoice hat nichts erzeugt")
    import torch  # noqa: E402

    torchaudio.save(ziel, torch.cat(teile, dim=1), tts.sample_rate)


def main():
    args = sys.argv[1:]
    dauer = bool(args) and args[0] == "--dauer"
    if dauer:
        args = args[1:]

    if dauer:
        probe_pfad = args[0] if args else None
    else:
        if len(args) < 2:
            raise SystemExit("Aufruf: <text.txt> <ziel.wav> [<probe.wav>]  oder  --dauer [<probe.wav>]")
        probe_pfad = args[2] if len(args) > 2 else None

    with stdout_nach_stderr():
        tts, ist_drei = laden()
        probe, probentext = probe_laden(probe_pfad)

    if not dauer:
        with open(args[0], encoding="utf-8") as f:
            text = f.read().strip()
        with stdout_nach_stderr():
            sprechen(tts, text, args[1], probe, probentext, ist_drei)
        return

    # ⚑ Erst jetzt `bereit`: Der Aufrufer wartet darauf, und ein zu
    # frueher Ruf schickte den ersten Satz in ein Modell, das noch laedt.
    print("bereit", flush=True)
    for zeile in sys.stdin:
        zeile = zeile.rstrip("\n")
        if not zeile:
            continue
        teile = zeile.split("\t")
        if len(teile) != 2:
            fehler("erwartet wird <text.txt>\\t<ziel.wav>")
            continue
        try:
            with open(teile[0], encoding="utf-8") as f:
                text = f.read().strip()
            with stdout_nach_stderr():
                sprechen(tts, text, teile[1], probe, probentext, ist_drei)
            print("ok", flush=True)
        except Exception as f:  # noqa: BLE001
            # ⚠️ Ein Satz, der schiefgeht, beendet den Laeufer nicht:
            # Der naechste kann gelingen, und ein Vorleser, der beim
            # ersten Stolpern verstummt, ist schlimmer als einer, der
            # eine Luecke laesst.
            fehler(str(f).replace("\n", " "))


if __name__ == "__main__":
    main()
