# Pruefung eines geholten Stands gegen die Freigabeschluessel, bevor er
# eingespielt wird. Eingebunden von den Installationsskripten fuer macOS,
# Linux und NixOS (`. freigabe.sh`), nicht fuer sich aufzurufen.
#
# ⚑ **Warum** (Sicherheitsaudit vom 2026-11-18): `--aktualisieren` bewegt den
# Klon auf den Stand von GitHub und baut ihn. Wer den Zugang zum
# Repositorium hat oder ihn uebernimmt, liefert damit Code an jede Maschine,
# die aktualisiert. `git` traegt nur die Unversehrtheit der Uebertragung,
# nicht die Herkunft. Die Herkunft traegt erst eine Signatur, und zwar die
# eines Schluessels, den nicht der Zugang zu GitHub hat.
#
# ⚑ **Geprueft wird die Spitze des neuen Stands** (`FETCH_HEAD`): Ihr Hash
# bindet den ganzen Baum und die ganze Geschichte darunter. Die Signatur
# muss von einem Schluessel aus `SYSTEM/install/freigabeschluessel` stammen,
# **und zwar aus dem bisherigen Stand** (`HEAD`), nie aus dem neuen: Sonst
# truege der neue Stand den Schluessel, mit dem er sich selbst beglaubigt.
# Ein Wechsel der Schluessel kommt deshalb nur mit einer Signatur eines
# bisherigen herein.
#
# ⚠️ **Solange die Liste keinen Schluessel enthaelt**, wird nicht geprueft,
# und das steht laut da. Das ist der Stand vor den Freigabeschluesseln; mit
# dem ersten eingetragenen Schluessel wird jede weitere Aktualisierung ohne
# gueltige Signatur abgewiesen.
#
# Das Format der Liste ist das von `ssh-keygen` (ALLOWED SIGNERS), eine
# Zeile je Schluessel:
#   freigabe-a namespaces="git" ssh-ed25519 AAAA...
# `git verify-commit` liest sie ueber `gpg.ssh.allowedSignersFile`.

FREIGABE_LISTE=SYSTEM/install/freigabeschluessel

# freigabe_pruefen <commit>: 0, wenn der Stand eingespielt werden darf.
freigabe_pruefen() {
  ziel=$1
  if [ "$(git rev-parse "$ziel" 2>/dev/null)" = "$(git rev-parse HEAD 2>/dev/null)" ]; then
    return 0
  fi
  liste="$(git rev-parse --absolute-git-dir)/myelith-freigabeschluessel"
  if ! git show "HEAD:$FREIGABE_LISTE" > "$liste" 2>/dev/null \
     || ! grep -q '^[^#[:space:]]' "$liste"; then
    rm -f "$liste"
    echo "   ⚠️ Im bisherigen Stand steht kein Freigabeschluessel." >&2
    echo "      Der neue Stand wird NICHT auf eine Signatur geprueft." >&2
    return 0
  fi
  if git -c gpg.ssh.allowedSignersFile="$liste" verify-commit "$ziel" >/dev/null 2>&1; then
    rm -f "$liste"
    echo "   Signatur des neuen Stands: gueltig (Freigabeschluessel)"
    return 0
  fi
  rm -f "$liste"
  echo "   ⛔️ Der neue Stand traegt keine gueltige Signatur eines Freigabeschluessels." >&2
  echo "      Nichts eingespielt. Pruefen: git verify-commit FETCH_HEAD" >&2
  return 1
}
