# SYSTEM

**Was der Rechner braucht, damit Myelith läuft.** Kein Teil des Netzes,
keine Komponente, nichts mit einer eigenen Fassung: die Unterlage.

> Angelegt am 2026-09-24 auf Festlegung des Projektinhabers. Vorher lag
> all das lose in der Wurzel, und die Wurzel las sich wie eine Mischung
> aus Bauplan und Werkstatt.

| Ordner | Was drin liegt | eingecheckt |
|---|---|---|
| `full-build/` | wohin alle 26 Crates bauen, gemeinsam statt je Crate einmal | nein, erzeugt |
| `crates-vorrat/` | 760 `.crate`-Archive, alles, was die 26 Crates zum Bauen brauchen | **ja**, 117 MB |
| `crates-lager/` | dieselben Archive ausgepackt, dazu die Cargo-Konfiguration | nein, erzeugt |
| `golemos/` | die Linux-Distribution, die nur Myelith bedient, mit Einrichtungsassistent (die 26. Kiste) und freigegebenen Abbildern | **ja**, seit 2026-09-25; draußen bleiben `aus/` und `bau/` |
| `install/` | drei Einrichtungsskripte, je eines für macOS, NixOS und Windows | ja |
| `logs/` | Laufprotokolle des Knotens | ja |

⚑ **Warum Vorrat und Lager zwei Ordner sind.** Der **Vorrat** ist die
Zusage: Aus ihm baut ein frischer Klon ohne Netz. Das **Lager** ist das
Ausgepackte, das jeder Rechner selbst herstellt. Ein einziger Ordner
hiesse entweder, 117 MB Archive **und** 900 MB Auspackung einzuchecken,
oder gar nichts, und dann wäre die Zusage weg.

⚠️ **`full-build/` wächst und wird nie kleiner.** Cargo räumt es nicht
auf; hier stehen gerade 27 GB. Es wird gelegentlich geleert und einmal
voll gebaut, und ein frischer Vollbau aller Crates liegt bei rund 6 GB.
Wer deutlich darüber liegt, hat Altlasten und keine Abhängigkeiten.

📌 **Bis zum 2026-09-24 hiess es `target-shared/` und lag in der
Wurzel.** Ältere Berichte und Changelog-Einträge nennen deshalb noch
den alten Namen; sie halten den Stand ihrer Fassung und sind nicht
nachgezogen worden.

```sh
python3 SYSTEM/install/vorrat.py pruefen     # jede Datei gegen die Sperrdateien
python3 SYSTEM/install/vorrat.py auspacken   # Vorrat wird Lager
```

## ⛔️ Was hier NICHT liegt, und warum

**`flake.nix` bleibt in der Wurzel.** `nix develop` sucht sie dort und
nirgends sonst; ein Flake unter `SYSTEM/` wäre eines, das niemand
findet. 📌 Beim Umzug lag sie kurz falsch, und aufgefallen ist es nicht
beim Ausprobieren, sondern an einer Probe, die genau diesen Satz als
Begründung trug.

**Die Gewichte und die Artefakte** liegen unter `MODELS/` und
`INTEGER_LLM/artifacts/`. Sie wiegen 77 bis 165 GB; was diese
Grössenordnung hat, kommt über ein Skript und nicht über ein
Verzeichnis.

## ⚠️ Die Falle, die dieser Ordner mitbringt

**Jedes Skript hier liegt zwei Ebenen unter der Wurzel**, nicht mehr
eine. Wer eines verschiebt oder ein neues anlegt, rechnet mit
`../..` und nicht mit `..`.

📌 **Beim Umzug selbst ist genau das passiert**, obwohl in derselben
Datei drei Zeilen darüber die Warnung aus dem vorigen Umzug stand.
⚑ **Eine Lehre im Kommentar schützt nicht den, der verschiebt, sondern
den, der danach sucht.** Deshalb steht sie trotzdem da, und hier noch
einmal.
