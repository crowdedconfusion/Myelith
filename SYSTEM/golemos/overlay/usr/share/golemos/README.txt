GOLEM: DER DATENTEIL DEINES GOLEMOS-STICKS
==========================================

(English below.)

Diesen Teil des Sticks siehst du an jedem Rechner. Hierher kommt dein
Myelith-Ordner, damit GolemOS ihn benutzen kann.

SO GEHT ES

  1. Ziehe deinen GANZEN Myelith-Ordner hierher, also in "Golem",
     samt den Modellen unter INTEGER_LLM/artifacts. Der ganze Ordner,
     weil GolemOS daraus auch selbst neue Sticks schreibt.

     Weglassen darfst du nur, was ein frisch heruntergeladener Ordner
     gar nicht enthaelt:
       SYSTEM/full-build, SYSTEM/crates-lager   (entstehen beim Bauen)
       MODELS                                   (Rohgewichte; GolemOS
                                                 braucht die fertigen
                                                 Modelle)
       Ordner namens .venv                      (enthalten
                                                 Verknuepfungen, die
                                                 dieser Stick nicht
                                                 speichern kann)

  2. Wirf den Stick sicher aus (Finder: Auswerfen; Windows: "Hardware
     sicher entfernen").

  3. Starte den Rechner vom Stick. Melde dich mit "root" an (ohne
     Passwort). Der Assistent erscheint und bietet an:
       1  Myelith jetzt benutzen, ohne etwas zu installieren
       3  GolemOS dauerhaft auf eine Platte installieren
       4  Einen weiteren GolemOS-Stick schreiben

WELCHES MODELL PASST

  GolemOS waehlt selbst das groesste Modell, das in etwa sechs Zehntel
  des Arbeitsspeichers passt. Faustregel:
    myelith-0.6b   0,9 GB   ab 2 GB Arbeitsspeicher
    myelith-4b     4,5 GB   ab 8 GB
    myelith-8b     8,8 GB   ab 16 GB

HINWEISE

  - Der Myelith-Ordner kann auch auf einem zweiten Stick oder einer
    externen Platte liegen (FAT32 oder ext4); GolemOS sucht beim Start
    auf allen Laufwerken.
  - Beim ersten Start fragt der Assistent nach Sprache und Tastatur;
    aendern laesst sich beides spaeter mit Punkt 7.
  - Die ganze Anleitung steht im Myelith-Ordner unter
    SYSTEM/golemos/ANLEITUNG.md.
  - Diese Datei darfst du loeschen; sie kommt nicht wieder.


GOLEM: THE DATA PART OF YOUR GOLEMOS STICK
==========================================

Every computer can see this part of the stick. Put your Myelith folder
here so GolemOS can use it.

HOW IT WORKS

  1. Drag your WHOLE Myelith folder here, into "Golem", including the
     models under INTEGER_LLM/artifacts. The whole folder, because
     GolemOS also writes new sticks from it.

     You may leave out only what a freshly downloaded folder does not
     contain:
       SYSTEM/full-build, SYSTEM/crates-lager   (created when building)
       MODELS                                   (raw weights; GolemOS
                                                 needs the finished
                                                 models)
       folders named .venv                      (they contain links
                                                 this stick cannot
                                                 store)

  2. Eject the stick safely (Finder: Eject; Windows: "Safely Remove
     Hardware").

  3. Start the computer from the stick. Log in as "root" (no
     password). The assistant appears and offers:
       1  use Myelith right away, without installing anything
       3  install GolemOS permanently on a disk
       4  write another GolemOS stick

WHICH MODEL FITS

  GolemOS picks the largest model that fits into about six tenths of
  the memory. Rule of thumb:
    myelith-0.6b   0.9 GB   from 2 GB of memory
    myelith-4b     4.5 GB   from 8 GB
    myelith-8b     8.8 GB   from 16 GB

NOTES

  - The Myelith folder may also be on a second stick or an external
    disk (FAT32 or ext4); GolemOS searches all drives at startup.
  - On the first start the assistant asks for language and keyboard;
    item 7 changes both later.
  - The full guide is in the Myelith folder under
    SYSTEM/golemos/ANLEITUNG.en.md.
  - You may delete this file; it will not come back.
