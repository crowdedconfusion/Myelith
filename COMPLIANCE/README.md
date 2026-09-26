# COMPLIANCE

Alles, was Myelith gegenüber dem Recht zusagt und nachweist: die Pflichten
aus der KI-Verordnung (EU) 2024/1689, die Lizenzen der Fremdkomponenten
und die ethischen Selbstbindungen des Projekts.

Everything Myelith commits to and documents with regard to the law: the
obligations under the AI Act (EU) 2024/1689, the licences of third-party
components, and the project's ethical commitments.

> ⚠️ Selbsteinschätzung des Anbieters, keine Rechtsberatung.
> Provider's self-assessment, not legal advice.

## Dokumente / Documents

| Deutsch (maßgeblich) | English | Worum es geht / What it covers |
|---|---|---|
| [Selbsteinschätzung](de/Selbsteinschaetzung.md) | [Self-Assessment](en/Self-Assessment.md) | Jeder relevante Artikel mit Status / every relevant article with status |
| [Zweckbestimmung](de/Zweckbestimmung.md) | [Intended Use Policy](en/Intended-Use-Policy.md) | Wofür Myelith gedacht ist, was verboten ist / intended and prohibited uses (Art. 5, Annex III) |
| [GPAI-Dokumentation](de/GPAI-Dokumentation.md) | [GPAI Documentation](en/GPAI-Documentation.md) | Technische Dokumentation der Modelle / technical documentation of the models (Art. 53(1)(a), (b)) |
| [Trainingsdaten](de/Trainingsdaten.md) | [Training Data](en/Training-Data.md) | Öffentliche Zusammenfassung / public summary (Art. 53(1)(d)) |
| [Urheberrecht](de/Urheberrecht.md) | [Copyright Policy](en/Copyright-Policy.md) | Urheberrechtsstrategie / copyright policy (Art. 53(1)(c)) |
| [NOTICES](NOTICES.md) | [NOTICES](NOTICES.md) | Fremdkomponenten und ihre Lizenzen / third-party components and their licences |

## Ethik / Ethics

[`ethics/`](ethics/README/README.md): das Ethik-Manifest, der
Ausschlusskatalog, die Risikoklassen der Werkzeuge, die Lizenzprüfung der
Grundmodelle und die erzeugte Modellkarte. / The ethics manifesto, the
exclusion catalogue, the risk classes of tools, the licence review of the
base models and the generated model card.

## Systemprompt / System prompt

[`systemprompt/`](systemprompt/): der fest vorgegebene Systemprompt, unter
dem jeder Lauf von Myelith steht, deutsch und englisch, mit
`pruefsummen.txt`. Er ist eingebaut und nicht einstellbar; vor jedem Lauf
wird er gegen seine Prüfsumme gehalten, und passt sie nicht, läuft kein
Agent. Jeder Lauf trägt den vollen SHA-256 der Fassung ins
Aktionsprotokoll. Hier stehen künftig auch die Vorgaben aus Compliance
und Ethik, die das Modell kennen muss. / The fixed system prompt every
Myelith run operates under, in German and English, with
`pruefsummen.txt`. It is built in and cannot be configured; before every
run it is checked against its checksum, and if it does not match, no agent
runs. Every run records the full SHA-256 of the version in the action log.

```
cd COMPLIANCE/systemprompt && shasum -a 256 -c pruefsummen.txt
```

⚠️ **Wer den Text ändert, ändert die Summe mit** (`shasum -a 256 de.md
en.md > pruefsummen.txt`); sonst schlägt die Probe
`die_pruefsummen_stimmen` fehl, und kein Agent läuft. / Whoever changes the
text updates the checksum too; otherwise the probe fails and no agent runs.

## Werkzeuge / Tools

| Befehl / command | Wofür / purpose |
|---|---|
| `python3 COMPLIANCE/werkzeuge/notices.py [--pruefe]` | erzeugt oder prüft `NOTICES.md` / generates or checks `NOTICES.md` |
| `myl kennzeichen <datei.wav>` | prüft die KI-Kennzeichnung einer Tondatei / checks the AI marking of an audio file |
| `myl protokoll [anzahl]` | zeigt das Aktionsprotokoll / shows the action log |

## Kontakt / Contact

Hinweise, Beschwerden von Rechteinhabern und Meldungen von Missbrauch über
die Issues des Repositoriums. / Notices, complaints by rightholders and
reports of misuse via the repository's issues.
