# Die Quellen zu den GolemOS-Abbildern

Die Abbilder unter `abbild/fertig/` enthalten Software unter der GPL, darunter
den Kern Linux, BusyBox und GRUB. Wer sie bekommt, bekommt hier auch die
Quellen genau dieser Fassungen.

**Wo:** als Anhang der Freigabe **GolemOS: Quellen** auf GitHub,
<https://github.com/crowdedconfusion/Myelith/releases/tag/golemos-quellen>.

Die Freigabe trägt bewusst keine Fassung: Mehrere Abbilder teilen sich
dieselben Quellen, solange kein Paket sich ändert, und ihre Beschreibung
nennt, zu welchen Fassungen von GolemOS die Archive gehören (heute v0.6.1
und v0.7.0). Ändert sich ein Paket, kommen die neuen Archive mit eigenem
Namen dazu; die alten bleiben, solange ein Abbild aus der Geschichte des
Repositoriums sie braucht.

| Datei | Inhalt |
|---|---|
| `quellen-x86_64.tar`, `quellen-aarch64.tar` | die Quellen aller Pakete im Abbild und in der Oberflächen-Schicht, dazu Buildroot selbst, je rund 0,7 GB |
| `lizenzen-x86_64.tar.xz`, `lizenzen-aarch64.tar.xz` | das Verzeichnis aller Pakete mit Fassung und Lizenz und die Lizenztexte |
| je eine `.sha256` daneben | die Prüfsumme |

Prüfen nach dem Herunterladen:

```sh
shasum -a 256 -c quellen-x86_64.tar.sha256
```

Die Bauanleitung, aus der Abbild und Archive entstehen, liegt im
Repositorium selbst: `SYSTEM/golemos/bauen.sh` mit den Konfigurationen unter
`SYSTEM/golemos/konfigurationen/`; `auskunft.sh` erzeugt die Archive neu.

⚠️ **Die Archive selbst liegen nicht im Repositorium.** Eine Datei darf dort
höchstens 100 MiB haben, und jede Freigabe liesse jeden Klon für immer
wachsen. `auskunft.sh` legt sie samt Lizenzauskunft und Prüfsummen in die
Unterordner `x86_64/` und `aarch64/` hier (ignoriert); von dort werden sie
hochgeladen, und `freigeben.sh` liest sie dort.
