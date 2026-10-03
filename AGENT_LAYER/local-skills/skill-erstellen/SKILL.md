---
beschreibung: Einen neuen Skill anlegen, nach der Vorlage, damit er gefunden und befolgt wird.
stichworte: skill, neuer skill, anlegen, erstellen, vorlage, create skill, new skill, template, lernen, anleitung schreiben
---

# Einen Skill erstellen

## Wann

Wenn der Nutzer einen neuen Skill möchte, oder wenn du eine Arbeit
gelöst hast, die wiederkommen wird, und der Nutzer zustimmt, sie
festzuhalten.

## Vorgehen

1. **Name festlegen:** klein, Bindestriche, sagt die Tätigkeit
   (`rechnung-pruefen`, nicht `skill1`). Mit `search_skill` prüfen, dass
   es ihn noch nicht gibt.
2. **Vorlage holen:** `learn_skill` mit
   `skill-erstellen/vorlagen/SKILL.md`.
3. **Ausfüllen**, kurz und konkret:
   - `beschreibung`: ein Satz, wofür.
   - `stichworte`: wonach jemand suchen würde, deutsch und englisch,
     dazu typische Fehlermeldungen oder Begriffe.
   - **Wann:** die Lage, in der der Skill gilt.
   - **Vorgehen:** nummerierte Schritte, jeder prüfbar.
   - **Fallen:** was schiefgehen kann, und woran man es merkt.
4. **Schreiben** mit `write_file` nach `.AGENT/skills/<name>/SKILL.md`
   im Arbeitsordner.
5. **Prüfen:** `search_skill` mit einem der Stichworte muss ihn finden.

## Fallen

- Zu lang: Ein Skill steht beim Lernen im Kontext. Mehr als etwa
  sechzig Zeilen gehören nach `referenz/`.
- Zu allgemein: „sorgfältig arbeiten“ ist kein Schritt. Jeder Schritt
  nennt eine Handlung und ein Werkzeug.
- Ohne Stichworte wird er nur über den Namen gefunden.
