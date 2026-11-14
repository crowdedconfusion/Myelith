# Woher die shims kommen

⛔️ **Fremde Binärdateien, unverändert, und das ist der Zweck.** Ein shim
startet auf einem Rechner mit Secure Boot nur, weil Microsoft es signiert
hat; wer es neu übersetzt, verliert die Signatur. Diese Dateien stammen
deshalb Byte für Byte aus den signierten Paketen von Debian 13 (trixie).

## Wozu

Der Rechner startet `shim`, das Microsoft signiert hat. `shim` startet
GRUB nur, wenn GRUB von Debian oder mit einem Schlüssel signiert ist, den
der Rechner eingetragen hat (MOK). GolemOS signiert GRUB und jeden Kern mit
seinem eigenen Schlüssel **Golem-Emet**; den trägt ein Rechner beim ersten
Start einmal über den MokManager ein, danach startet GolemOS mit Secure
Boot.

## Die Dateien

| Datei | aus Paket | Was |
|---|---|---|
| `x86_64/shimx64.efi` | `shim-signed` 1.51~1+deb13u1+16.1-2~deb13u1, amd64 | shim 16.1 |
| `x86_64/mmx64.efi` | `shim-helpers-amd64-signed` 1+16.1+2~deb13u1, amd64 | MokManager |
| `aarch64/shimaa64.efi` | `shim-signed` 1.51~1+deb13u1+16.1-2~deb13u1, arm64 | shim 16.1 |
| `aarch64/mmaa64.efi` | `shim-helpers-arm64-signed` 1+16.1+2~deb13u1, arm64 | MokManager |

Im Paket heißen sie `/usr/lib/shim/<name>.efi.signed`. Die Prüfsummen
stehen in `SHA256SUMS`, die der Pakete hier:

| Paket | SHA-256 |
|---|---|
| `shim-signed_1.51~1+deb13u1+16.1-2~deb13u1_amd64.deb` | `3c802fa303c0e6bf126adee74028d1042e3120360d159c2c05181dc9b2f61005` |
| `shim-signed_1.51~1+deb13u1+16.1-2~deb13u1_arm64.deb` | `e228a68b298865e0f1b35f93dac8fc9644a47bde3487419efe305777e2d4a1f0` |
| `shim-helpers-amd64-signed_1+16.1+2~deb13u1_amd64.deb` | `7e950d75d5a40ecc0a99f727452ce35780b47a04d9a2cc3b1ff7c2bb316c033e` |
| `shim-helpers-arm64-signed_1+16.1+2~deb13u1_arm64.deb` | `0563a90af1f9990a592a79f52845747daff51c1d345769adc07830d2547da325` |

Geholt am 2026-11-14 mit `apt-get download` aus `deb.debian.org`, trixie;
apt prüft dabei die Signatur der Paketliste und die Prüfsumme jedes Pakets.

## Was nachgesehen ist

- **Beide shims tragen zwei Signaturen von Microsoft** (`sbverify --list`):
  eine unter der *Microsoft Corporation UEFI CA 2011*, eine unter der
  *Microsoft UEFI CA 2023*. Sie starten also auf Rechnern, die nur eine der
  beiden kennen. ⚠️ Debian kennt Firmware, die eine doppelte Signatur
  ablehnt.
- **Der MokManager ist von Debian signiert** (*Debian Secure Boot Signer
  2022*); shim prüft ihn selbst.
- **SBAT** der shims: `shim,4` und `shim.debian,1`. Ihre eingebaute
  Sperrliste verlangt von GRUB mindestens `grub,5`.
- Kein NX-Kennzeichen im Kopf der shims; sie verlangen es also auch nicht
  von GRUB.

## Lizenz

shim steht unter BSD-2-Clause (`COPYRIGHT-shim`, aus dem Paket
`shim-signed`); es darf als Binärdatei weitergegeben werden, mit diesem
Hinweis.

## ⚠️ Was das kostet

Ein shim wird über die Sperrlisten von Microsoft (SBAT, `dbx`) irgendwann
gesperrt, etwa nach einer Lücke. Dann startet ein Stick mit diesem shim auf
Rechnern mit aktueller Sperrliste nicht mehr. **Neue Fassungen von Debian
gehören deshalb hierher nachgezogen**, mit neuem Eintrag in dieser Datei.
