# GolemOS

Werkzeuge, mit denen der Agent auf **GolemOS** die Oberfläche und den
Knoten bedient. Auf jedem anderen System sagen sie „Das ist kein GolemOS“
und tun nichts.

| Werkzeug | was |
|---|---|
| `golemos_status` | Oberfläche und Knoten: geholt, eingerichtet, läuft; ändert nichts |
| `oberflaeche_holen` | die Oberflächen-Schicht von einem Laufwerk oder per https holen und prüfen |
| `oberflaeche_starten` | das Myelith-Fenster im Vollbild starten |
| `oberflaeche_beim_start` | ob die Oberfläche beim Hochfahren von selbst kommt |
| `knoten_einrichten` | diesen Rechner als Myelith-Knoten einrichten (Name, Rolle) |
| `knoten_schalten` | den Knoten starten, stoppen, beim Hochfahren an oder aus |

⚑ **Die Werkzeuge rufen nur die Befehle der Basis** (`golem-schicht`,
`golem-knoten`) und bringen keine eigene Logik mit. Was dort geprüft wird
(Prüfsumme, erlaubte Werte, Rechte), gilt also auch hier, und was ein
Mensch an der Konsole tippt, tut dasselbe.

⚑ **Den Ablauf erklärt der Skill `golemos-einrichten`**: wo die Schicht
liegt, in welcher Reihenfolge, und was vor dem Einrichten eines Knotens
zu fragen ist.

⚠️ **Die Werkzeuge verändern das System**: Sie starten Dienste und legen
fest, was beim Hochfahren startet. Wie jedes Werkzeug aus einer Kiste
brauchen sie die Schreiberlaubnis, und im `manual mode` wird jeder Aufruf
vorgelegt; für diese Kiste ist der `manual mode` die Empfehlung.
