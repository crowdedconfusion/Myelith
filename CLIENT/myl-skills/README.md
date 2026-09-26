# myl-skills: die mitgelieferten Skills

Ein **Skill** ist eine kurze Anleitung für eine wiederkehrende Art von
Arbeit. Der Agent trägt Skills nicht im Kontext, sondern schlägt sie
nach:

- `search_skill` sucht nach den Worten einer Aufgabe oder eines Problems
  (ein leerer Text nennt alle). Das Modell soll es von selbst rufen,
  sobald es unsicher ist oder nicht weiterkommt.
- `learn_skill` liefert die Anleitung eines Skills, der das Modell für
  den Rest der Aufgabe folgt. Das ist der Weg für „lerne skill …“ und
  „learn skill …“.

In der deutschen Ansage heißen die Werkzeuge `skill_suchen` und
`skill_lernen`. Beide liegen in jeder Werkzeugkiste, auch in `Base`.

## Drei Orte

| Ort | Was dort liegt | Vorrang |
|---|---|---|
| `<Arbeitsordner>/.AGENT/skills/` | was zu **diesem Projekt** gehört; der Agent kann hier selbst Skills anlegen | 1 |
| neben den Einstellungen (`myl skills` zeigt den Pfad) | **eigene** Skills, die überall gelten | 2 |
| `CLIENT/myl-skills/` (dieser Ordner) | die **mitgelieferten** Grundskills und die Vorlage | 3 |

Bei gleichem Namen gewinnt der Ort mit dem kleineren Rang. Wer einen
mitgelieferten Skill anpassen will, legt einen gleichnamigen unter den
eigenen an, statt diesen Ordner zu ändern.

## Aufbau eines Skills

```
<name>/
  SKILL.md        die Anleitung, mit Kopf (Pflicht)
  referenz/       Einzelheiten, die nur manchmal gebraucht werden (frei)
  vorlagen/       Dateien zum Abschreiben oder Ausfüllen (frei)
```

`SKILL.md` beginnt mit einem Kopf zwischen zwei `---`:

```
---
beschreibung: Ein Satz: wofür dieser Skill da ist.
stichworte: deutsch, englisch, Synonyme, typische Fehlermeldungen
---
```

- **Der Ordnername ist der Name**, klein und mit Bindestrichen, so wie
  ihn `learn_skill` erwartet.
- **Die Beschreibung** steht in jeder Trefferliste; sie entscheidet, ob
  das Modell den Skill lernt.
- **Die Stichworte** sind das, wonach gesucht wird. Sie wiegen bei der
  Suche mehr als die Anleitung, also lohnen sich Synonyme und die
  englischen Wörter.
- Ein Ordner, dessen Name mit `_` beginnt, wird übergangen; so liegt ein
  Entwurf bereit, ohne gefunden zu werden.

Weitere Dateien nennt `learn_skill` am Ende der Anleitung. Das Modell
öffnet eine davon mit `learn_skill` und `name/datei`, etwa
`bericht-schreiben/vorlagen/bericht.md`.

## Einen eigenen Skill anlegen

- **Von Hand:** `myl skills neu <name>` kopiert die Vorlage unter die
  eigenen Skills. Dann die Vorlage ausfüllen.
- **Durch den Agenten:** „lerne skill skill-erstellen“ und dann
  beschreiben, was der neue Skill können soll. Er legt ihn unter
  `.AGENT/skills/` im Arbeitsordner an.

Die Vorlage liegt unter `skill-erstellen/vorlagen/SKILL.md`.

## Die mitgelieferten Skills

| Skill | Wofür |
|---|---|
| `skill-erstellen` | einen neuen Skill anlegen, mit der Vorlage |
| `fehlersuche` | einen Fehler systematisch finden, statt zu raten |
| `aufgabe-zerlegen` | ein großes Ziel in prüfbare Schritte teilen, auch im Loop |
| `bericht-schreiben` | Ergebnisse als Markdown-Bericht festhalten |
| `datei-sicher-aendern` | bestehende Dateien ändern, ohne etwas zu zerstören |
