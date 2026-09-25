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
    <-- stueck\t<teil.wav>          (null- bis mehrmal je Satz, sobald ein Stueck klingt)
    <-- ok      oder   fehler: ...  (je Satz)

Schliesst die Standardeingabe, ist Schluss.

⚑ **Fassung 2 (2026-09-25): gestroemt, auf der GPU, mit gemerkter Stimme.**
Gemessen auf einem M5 Pro mit Fun-CosyVoice3-0.5B: Fassung 1 rechnete
einen Satz in doppelter Echtzeit (RTF 1,7 bis 2,2), der erste Ton kam 7
bis 9 s nach dem Satz, und zwischen den Saetzen blieben Pausen. Jetzt
kommt der erste Ton nach rund 2 s, und die Stimme laeuft dem Abspielen
voraus. Die Gruende stehen bei `beschleunigen` und `sprechen`.

Die Stuecke eines Satzes heissen `<ziel>.<n>.wav`. Ein Aufrufer, der
keine `stueck`-Zeilen kennt, bekommt am Ende trotzdem den ganzen Satz in
`<ziel.wav>`; so versteht ihn auch ein Client der Fassung 1.

Umgebung:
    MYL_COSYVOICE          Wurzel der CosyVoice-Installation (Pflicht)
    MYL_COSYVOICE_MODELL   Gewichte; ohne Angabe erst Fun-CosyVoice3-0.5B,
                           dann CosyVoice2-0.5B unter <Wurzel>/pretrained_models
    MYL_STIMMTEXT          Datei mit dem Text der Stimmprobe (fuer zero-shot)
    MYL_TTS_GERAET         `cpu` erzwingt die CPU; ohne Angabe rechnet der
                           Fluss auf der GPU, wenn es eine gibt
    MYL_FLUSSSCHRITTE      Schritte des Flusses; ohne Angabe 6 statt 10
    MYL_TTS_LM             `gpu` legt auch das Token-Sprachmodell auf die GPU.
                           ⚠️ Neben einem Hauptmodell gemessen schlechter
                           (erster Ton 4,8 statt 3,8 s, Luecken 13 statt
                           2 s), denn das rechnet dort mit; nur fuer einen
                           Rechner, der sonst nichts tut.

⚠️ Ohne MYL_STIMMTEXT wird `inference_cross_lingual` genommen. Das geht
ohne den Text der Probe und trifft die Stimme etwas schlechter.
"""

import contextlib
import os
import sys

# ⚑ Woran der Client erkennt, welche Fassung er vor sich hat.
FASSUNG = 2


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


def _hin(x, geraet):
    """Tensoren, auch in Listen und Woerterbuechern, auf ein Geraet."""
    import torch  # noqa: E402

    if torch.is_tensor(x):
        return x.to(geraet)
    if isinstance(x, dict):
        return {k: _hin(v, geraet) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return type(x)(_hin(v, geraet) for v in x)
    return x


def beschleunigen(tts):
    """Legt die Teile des Modells dorthin, wo sie am schnellsten rechnen.

    Gemessen an einem Satz von 101 Zeichen (5,6 s Ton), je Stufe:

    | Stufe | CPU | GPU (MPS) |
    |---|---|---|
    | Token-Sprachmodell | 3,0 s | 4,2 bis 5,0 s |
    | Fluss (10 Schritte) | 5,4 bis 6,3 s | 1,8 s |
    | Vocoder | 0,3 s | geht nicht (float64) |

    ⚑ **Nur der Fluss wandert auf die GPU.** Das Sprachmodell erzeugt
    Token fuer Token, und je Token sind die Rechnungen so klein, dass
    der Start auf der GPU mehr kostet, als sie spart. Der Fluss rechnet
    grosse Bloecke, dort gewinnt sie das Dreieinhalbfache. CosyVoice
    selbst kennt nur CUDA oder CPU; hier wird der Fluss verschoben und
    seine Ein- und Ausgaben an der Grenze umgeladen, ohne den Baum von
    CosyVoice anzufassen.

    ⚑ **Sechs Flussschritte statt zehn.** Gemessen an drei Saetzen und
    drei Stimmproben: Whisper verstand bei 10, 6 und 4 Schritten jeden
    Satz vollstaendig, und die Aehnlichkeit zur Stimmprobe (Sprecher-
    einbettung, Kosinus) lag bei allen zwischen 0,80 und 0,89, also
    innerhalb der Streuung. Sechs sind die vorsichtige Wahl.

    ⚑ **Zehn Faeden fuer die CPU.** Mit fuenf, zehn und fuenfzehn
    gemessen war zehn am schnellsten; die Effizienzkerne helfen dem
    Sprachmodell kaum, stoeren aber auch nicht.

    Rueckgabe: ein Satz fuer das Protokoll auf stderr.
    """
    import torch  # noqa: E402

    torch.set_num_threads(int(os.environ.get("MYL_TTS_FAEDEN", "10")))
    m = tts.model
    schritte = int(os.environ.get("MYL_FLUSSSCHRITTE", "6"))
    decoder = m.flow.decoder.forward

    def mit_schritten(*a, **k):
        if "n_timesteps" in k:
            k["n_timesteps"] = schritte
        return decoder(*a, **k)

    m.flow.decoder.forward = mit_schritten

    will = os.environ.get("MYL_TTS_GERAET", "").strip().lower()
    if will == "cpu" or not (hasattr(torch.backends, "mps") and torch.backends.mps.is_available()):
        return f"Fluss auf der CPU, {schritte} Schritte"
    gpu = torch.device("mps")
    cpu = torch.device("cpu")
    fluss = m.flow.inference

    def fluss_auf_gpu(*a, **k):
        return _hin(fluss(*_hin(a, gpu), **_hin(k, gpu)), cpu)

    m.flow.to(gpu)
    m.flow.inference = fluss_auf_gpu
    wo = f"Fluss auf der GPU, {schritte} Schritte"
    if os.environ.get("MYL_TTS_LM", "").strip().lower() == "gpu":
        sprachmodell = m.llm.inference

        def sprachmodell_auf_gpu(*a, **k):
            yield from sprachmodell(*_hin(a, gpu), **_hin(k, gpu))

        m.llm.to(gpu)
        m.llm.inference = sprachmodell_auf_gpu
        wo += ", Sprachmodell auf der GPU"
    return wo


def zurueck_auf_cpu(tts):
    """Der Rueckweg, falls die GPU beim Aufwaermen scheitert."""
    import torch  # noqa: E402

    m = tts.model
    m.flow.to(torch.device("cpu"))
    m.flow.inference = type(m.flow).inference.__get__(m.flow)


# ⛔️ **CosyVoice 3 will diesen Vorspann, und ohne ihn zerfaellt es.**
# Ohne das Sondertoken steigt es mit `Kernel size can't be greater than
# actual input size` aus einer conv1d aus, und zwar erst nach dem
# Modellladen. Es steht so in `example.py` des Projekts.
VORSPANN = "You are a helpful assistant.<|endofprompt|>"


# ⚑ Unter dieser Kennung merkt sich CosyVoice die aufbereitete Stimmprobe.
STIMME = "myelith"


def stimme_merken(tts, probe, probentext, ist_drei):
    """Bereitet die Stimmprobe **einmal** auf, statt je Satz.

    ⚑ Fassung 1 gab die Probe je Satz als Pfad mit, und CosyVoice las
    sie jedes Mal neu: Sprechereinbettung, Sprachtoken, Merkmale. Gemerkt
    spart das je Satz rund eine Sekunde Rechnung (gemessen 9,8 auf
    8,1 s fuer denselben Satz, noch auf der CPU).

    Rueckgabe: die Art, wie gesprochen wird, oder None ohne Probe.
    """
    if probe is None:
        return None
    vorne = VORSPANN if ist_drei else ""
    if probentext:
        # ⚑ Mit dem Text der Probe trifft CosyVoice die Stimme am besten.
        tts.add_zero_shot_spk(vorne + probentext, probe, STIMME)
        return "zero_shot"
    # Ohne ihn: der sprachuebergreifende Weg; der Vorspann gehoert dann
    # vor den Sprechtext (siehe `sprechen`).
    tts.add_zero_shot_spk("", probe, STIMME)
    return "cross_lingual"


def stuecke_erzeugen(tts, text, art, ist_drei, strom):
    """Die Tonstuecke eines Satzes, als Erzeuger."""
    if art is None:
        # ⚠️ CosyVoice2 ist ein Zero-Shot-Modell und bringt nicht
        # zwangslaeufig eingebaute Stimmen mit. Ohne Probe geht es nur,
        # wenn welche da sind, und sonst sagt es das.
        spks = tts.list_available_spks()
        if not spks:
            raise RuntimeError(
                "dieses CosyVoice-Modell hat keine eingebaute Stimme; "
                "es braucht eine Stimmprobe (Einstellungsseite: Stimme hochladen)"
            )
        return tts.inference_sft(text, spks[0], stream=strom)
    if art == "zero_shot":
        return tts.inference_zero_shot(text, "", "", zero_shot_spk_id=STIMME, stream=strom)
    return tts.inference_cross_lingual(
        (VORSPANN if ist_drei else "") + text, "", zero_shot_spk_id=STIMME, stream=strom
    )


def sprechen(tts, text, ziel, art, ist_drei, strom=False, melde=None):
    """Spricht einen Satz nach `ziel`, im Strom zusaetzlich in Stuecken.

    ⚑ **Im Strom rechnen Sprachmodell und Fluss zugleich**: das eine auf
    der CPU, der andere auf der GPU, jedes Stueck von rund einer Sekunde,
    sobald seine Token da sind. Die Zeit bis zum ersten Ton haengt damit
    am ersten Stueck und nicht am ganzen Satz. Gemessen mit einer
    Stimmprobe von 7,1 s: erster Ton nach 2,0 bis 2,3 s, und jedes
    weitere Stueck war da, bevor das vorige ausgeklungen war.

    ⛔️ **`token_hop_len` wird vor jedem Satz zurueckgesetzt.** CosyVoice
    verdoppelt die Stueckgroesse waehrend des Stroms auf dem Modell
    selbst und setzt sie nie zurueck; ab dem zweiten Satz kam alles in
    einem Stueck, also ohne jeden Vorteil (gemessen: erstes Stueck gleich
    ganzer Satz).
    """
    import torch  # noqa: E402
    import torchaudio  # noqa: E402

    m = tts.model
    if strom and hasattr(m, "token_hop_len"):
        m.token_hop_len = getattr(m, "_myl_hop_anfang", m.token_hop_len)
    teile = []
    stamm = ziel[:-4] if ziel.lower().endswith(".wav") else ziel
    for n, s in enumerate(stuecke_erzeugen(tts, text, art, ist_drei, strom)):
        teil = s["tts_speech"]
        teile.append(teil)
        if melde is not None:
            pfad = f"{stamm}.{n}.wav"
            torchaudio.save(pfad, teil, tts.sample_rate)
            melde(pfad)
    if not teile:
        raise RuntimeError("CosyVoice hat nichts erzeugt")
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
        wo = beschleunigen(tts)
        art = stimme_merken(tts, probe, probentext, ist_drei)
        m = tts.model
        if hasattr(m, "token_hop_len"):
            m._myl_hop_anfang = m.token_hop_len
        # ⚑ **Einmal aufwaermen, bevor `bereit` kommt.** Die GPU uebersetzt
        # ihre Kerne beim ersten Lauf; ohne das zahlte der erste echte
        # Satz diese Zeit. Scheitert die GPU hier, rechnet der Fluss auf
        # der CPU weiter, langsamer, aber er rechnet.
        probe_ziel = os.path.join(os.path.dirname(os.path.abspath(__file__)), ".aufwaermen.wav")
        try:
            sprechen(tts, "Guten Tag.", probe_ziel, art, ist_drei)
        except Exception as f:  # noqa: BLE001
            zurueck_auf_cpu(tts)
            wo = f"Fluss auf der CPU (GPU scheiterte: {f})"
        finally:
            with contextlib.suppress(OSError):
                os.remove(probe_ziel)
        print(f"Fassung {FASSUNG}: {wo}", file=sys.stderr, flush=True)

    if not dauer:
        with open(args[0], encoding="utf-8") as f:
            text = f.read().strip()
        with stdout_nach_stderr():
            sprechen(tts, text, args[1], art, ist_drei)
        return

    # ⚑ Erst jetzt `bereit`: Der Aufrufer wartet darauf, und ein zu
    # frueher Ruf schickte den ersten Satz in ein Modell, das noch laedt.
    print("bereit", flush=True)
    echt = sys.stdout

    def melde(pfad):
        print(f"stueck\t{pfad}", file=echt, flush=True)

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
                sprechen(tts, text, teile[1], art, ist_drei, strom=True, melde=melde)
            print("ok", flush=True)
        except Exception as f:  # noqa: BLE001
            # ⚠️ Ein Satz, der schiefgeht, beendet den Laeufer nicht:
            # Der naechste kann gelingen, und ein Vorleser, der beim
            # ersten Stolpern verstummt, ist schlimmer als einer, der
            # eine Luecke laesst.
            fehler(str(f).replace("\n", " "))


if __name__ == "__main__":
    main()
