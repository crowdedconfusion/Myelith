# MYL-SERVER: einrichten, Schlüssel, Portweiterleitung

Diese Anleitung führt von einem frischen NixOS zu einem laufenden,
erreichbaren Knoten. Sie setzt voraus, dass das Modul `services.myl-server`
eingebunden ist (siehe `beispiel-configuration.nix`).

> ⚠️ **Ungeprüft auf NixOS.** Die Optionsnamen und Schritte stammen aus
> der Dokumentation, nicht aus einem Lauf. Wer diese Schritte zuerst geht,
> berichtigt hier, was abweicht.

---

## 1. Der Reihe nach

1. **Modul einbinden** und die Beispielkonfiguration als Vorbild nehmen.
2. **Schlüssel erzeugen** (einmalig, Abschnitt 2) und mit den richtigen
   Rechten ablegen.
3. **Bauen und aktivieren:** `nixos-rebuild switch`.
4. **Portweiterleitung** im Router setzen (Abschnitt 3).
5. **Erreichbarkeit prüfen** (Abschnitt 4).

---

## 2. Schlüssel: das Wichtigste zuerst

⛔️ **Schlüssel gehören nicht in die `configuration.nix` und nicht in den
Nix-Store.** Der Store ist für jeden lokalen Nutzer lesbar. Deshalb liegt
der Schlüssel in einer eigenen Datei, die Root gehört, und das Modul
reicht sie dem Dienst über systemd-Credentials.

**Einmalig erzeugen** (der Knoten legt beim ersten Start selbst einen an,
wenn die Datei fehlt; wer ihn vorher erzeugen und sichern will, tut es an
einem sicheren Ort und kopiert ihn dann):

```
sudo install -d -m 0700 /etc/myl-server
# Identitaetsschluessel: legt der Knoten beim ersten Start an, wenn er
# fehlt. Danach sichern und die Rechte pruefen:
sudo chmod 0400 /etc/myl-server/identitaet.key
sudo chown root:root /etc/myl-server/identitaet.key
```

⛔️ **Für einen stimmberechtigten Knoten** (Governance) zusätzlich der
**BLS-Konsensschlüssel**, getrennt von der Identität, `/etc/myl-server/konsens.key`,
ebenfalls 0400 und Root. Er ist das Kronjuwel: Wer ihn hat, stimmt in
deinem Namen. Sichere ihn getrennt, und lass einen stimmberechtigten
Knoten am besten auf einer Maschine laufen, die sonst nichts tut.

**Warum zwei Dateien:** Ein Leck der einen Ebene soll nicht die andere
mitnehmen. Die Identität sagt, welche Maschine du bist; der
Konsensschlüssel, dass du mitstimmst.

---

## 3. Portweiterleitung im Router

⚑ **Genau ein Port.** Nach außen erreichbar sein muss nur das
Peer-to-Peer-Netz, also der `p2pPort` (Standard **4001**). **Tür,
Beobachtung und Ortsleitung bleiben auf der Rückschleife** und werden
**nicht** weitergeleitet: Die Tür ist dein privates Gateway, und sie offen
im Netz zu haben, macht aus einem Überlastangriff gegen sie einen gegen
die Lebendigkeit des Konsenses.

**Im Router:**

1. Gib dem Server im lokalen Netz eine **feste Adresse** (DHCP-Reservierung
   auf seine MAC), damit die Weiterleitung nicht ins Leere zeigt.
2. Lege eine **Portweiterleitung** an: extern Port **4001** → intern die
   feste Adresse des Servers, Port **4001**, **TCP und UDP** (UDP für
   QUIC).
3. ⛔️ **Kein UPnP.** Wenn dein Router UPnP anbietet, lass es **aus**: Es
   öffnet Ports ohne Nachfrage. Die eine Weiterleitung setzt du von Hand.

**Feste oder dynamische öffentliche Adresse:**

- **Fest** (Server im Rechenzentrum): trage sie unter
  `oeffentlicheAdressen` ein, z. B. `/dns4/mein-server.example/tcp/4001`.
- **Dynamisch** (Anschluss zu Hause): richte **DynDNS** ein (viele Router
  können das), sodass ein Name immer auf deine wechselnde Adresse zeigt,
  und trage den Namen unter `oeffentlicheAdressen` ein.

**Ohne Weiterleitung:** Der Knoten ist nicht ausgesperrt. libp2p kann
Löcher durch NAT stanzen und über Relais gehen; er ist dann nur schlechter
erreichbar. Wer erreichbar sein will (und ein Relais anderen anbieten
will), setzt die Weiterleitung und `rolle = "relais"`.

---

## 4. Erreichbarkeit prüfen

- Der Dienst läuft: `systemctl status myl-server`.
- Der Port horcht lokal: `ss -tulpen | grep 4001`.
- Von außen erreichbar: von einem anderen Netz aus den Port prüfen (ein
  Freund, ein Mobilfunknetz), **nicht** aus dem eigenen Netz (viele Router
  spiegeln die eigene Adresse nicht zurück).
- Die Tür ist **nicht** von außen erreichbar (das ist gewollt): Ein Versuch
  auf den Türport von außen muss scheitern.
- Diagnose: der Beobachtungsendpunkt bleibt auf der Rückschleife. Wer ihn
  sehen will, tunnelt ihn über SSH:
  `ssh -L 9100:127.0.0.1:9100 betreiber@server` und öffnet ihn lokal.

---

## 5. Aktualisieren

Die MYL-Version hängt an der gepinnten Flake-Eingabe. Aktualisieren heißt:
die Eingabe auf eine neue, geprüfte Revision setzen und
`nixos-rebuild switch`. ⚑ **Pinnen, nicht auf einem wandernden Zweig
stehen:** Ein Server, der bei jedem Rebuild etwas anderes zieht, ist nicht
reproduzierbar, und im Konsens ist Reproduzierbarkeit alles.
