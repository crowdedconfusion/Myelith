// ⚑ Kein Bauwerkzeug, kein Rahmenwerk. Die Oberflaeche ruft dieselben
// Unterbefehle wie `myl` und hat keine eigene Logik; dafuer braucht es
// weder einen Buendler noch eine Node-Werkzeugkette.
//
// # Der Aufbau, seit dem 2026-09-09
//
// Ein Gespraechsfenster: links eine Seitenleiste mit Modus und
// Gespraechen, oben rechts das Zahnrad. Davor waren es drei
// untereinanderliegende Abschnitte.
//
// 📌 Hier stand bis zum 2026-09-09 „oben mittig der Ort (lokal oder
// Netz)". Der Schalter ist entfallen, der Satz nicht, und ein
// Kommentar, der einen Bedienteil beschreibt, den es nicht gibt,
// schickt den Naechsten suchen.
//
// ⚑ **Der Modus steht ueber den Gespraechen, weil er bestimmt, WAS ein
// Gespraech ist.** Im Chat antwortet das Modell ohne Werkzeuge; beim
// Agenten hat jeder Auftrag Werkzeuge, ein eigenes Schrittbudget und
// eine eigene Belegkette. 📌 Bis zum 2026-09-14 stand hier ausserdem,
// dass beim Agenten jeder Auftrag fuer sich steht (Entscheidung C2).
// Seitdem tragen beide Modi ihr Gespraech mit; siehe `kontext_von`.

import { vorhangStarten } from "./vorhang.js";

const { invoke } = window.__TAURI__.core;
const $ = (k) => document.getElementById(k);

// --- Vorschaltbild ------------------------------------------------------

// ⚑ **Das Vorschaltbild geht erst weg, wenn die Einstellungen da
// sind**, und mindestens nach anderthalb Sekunden. Beides zusammen:
// Wer sofort verschwindet, hat nichts gezeigt; wer auf eine feste Zeit
// wartet, obwohl er fertig ist, haelt den Nutzer auf.
const VORHANG_MINDESTENS = 1500;
const vorhang = $("vorhang");
const vorhangAnhalten = vorhangStarten($("vorhangbild"));
const start = performance.now();

async function vorhangWeg() {
  const rest = VORHANG_MINDESTENS - (performance.now() - start);
  if (rest > 0) await new Promise((r) => setTimeout(r, rest));
  vorhang.classList.add("weg");
  // ⚑ Erst nach der Blende anhalten, sonst friert das Bild sichtbar
  // ein, waehrend es noch durchscheint.
  setTimeout(() => {
    vorhangAnhalten();
    vorhang.remove();
  }, 700);
}

// --- Modi ---------------------------------------------------------------

// ⚑ Die Liste steht hier und nicht im HTML: Jeder Modus traegt einen
// Satz darueber, was er tut, und der gehoert zu ihm und nicht in eine
// Auszeichnungssprache.
// --- Die Sprache des Fensters -------------------------------------------
//
// ⚑ **Zwei Sprachen, eine Tabelle** (Festlegung des Projektinhabers,
// 2026-09-10). Deutsch ist die Sprache dieses Projekts und bleibt die
// Vorgabe; Englisch steht daneben, weil ein Netz, an dem jeder
// teilnehmen soll, nicht auf Deutsch bedient wird.
//
// ⚑ **Die Beschriftungen der Einstellungsfelder stehen NICHT hier.**
// Sie kommen aus `myl-client`, zusammen mit dem Feld selbst und in der
// eingestellten Sprache. Zwei Orte fuer denselben Satz waeren zwei
// Orte, an denen die naechste Sprache vergessen werden kann.
//
// 📌 **Und was ein Programm vergleicht, wird nie uebersetzt:**
// Feldnamen (`agent.wurzel`), Pfade, Modellnamen, die Kennung `netz`,
// die Marken im Verlauf. Ein uebersetzter Schluessel ist kein
// Schluessel mehr.
const TEXTE = {
  de: {
    "vorhang.stand": "wird geladen",
    "menue.umbenennen": "Umbenennen",
    "menue.ausgeben": "Exportieren",
    "menue.loeschen": "Löschen",
    "kontext.label": "Kontext",
    "kontext.zahl": (belegt, grenze, prozent) => `Kontext ${prozent} % · ${belegt} von ${grenze} Token`,
    "kontext.titel": (ansage, gespraech, n) =>
      `Werkzeugansage ${ansage} Token, Gespräch ${gespraech} Token in ${n} Nachrichten. Klicken zum Verdichten.`,
    "kontext.verdichten": "Kontext verdichten",
    "kontext.neu": "Neues Gespräch",
    "kontext.laeuft": "Kontext wird verdichtet …",
    "kontext.leer": "Das Gespräch ist leer; es gibt nichts zu verdichten.",
    "kontext.verdichtet": (vorher, nachher) =>
      `⚑ Kontext verdichtet: ${vorher} → ${nachher} Token. Ab hier sieht das Modell eine Zusammenfassung des Gesprächs davor.`,
    "kontext.verdichtet_kurz": (vorher, nachher) => `verdichtet ${vorher} → ${nachher}`,
    "kontext.im_lauf": "Kontext verdichtet",
    "kontext.fehler": (f) => `Verdichten fehlgeschlagen: ${f}`,
    "leiste.label": "Gespräche und Betriebsart",
    "leiste.modus": "Modus",
    "leiste.modell": "Modell",
    "leiste.schalten": "Seitenleiste ein- und ausblenden",
    "knopf.laden": "Modell laden",
    "kopf.einstellungen": "Einstellungen",
    "eingabe.platz": "Frag etwas, oder gib einen Auftrag.",
    "eingabe.label": "Eingabe",
    "knopf.senden": "Senden",
    "knopf.anhang": "Datei anhängen",
    "lauf.laeuftschon": "Es läuft noch ein Auftrag; einen Augenblick.",
    "sprachmodus.halten": "Zum Sprechen gedrückt halten",
    "anhang.weg": "Anhang entfernen",
    "knopf.sprachmodus": "Sprachmodus",
    "knopf.sprechtaste": "Gedrückt halten und sprechen",
    "knopf.vorlesen": "Antworten vorlesen",
    "sinne.titel": "Sinne",
    "sinne.stimmewaehlen": "Stimme hochladen",
    "notaus.knopf": "Notaus",
    "notaus.gemeldet": "Notaus: Der Auftrag wurde angehalten. Das Gespräch bleibt, wie es ist.",
    "sinne.einwilligung": "Die Aufnahme ist meine eigene Stimme, oder die Person hat eingewilligt.",
    "protokoll.titel": "Aktionsprotokoll",
    "protokoll.satz": "Jede Handlung des Agenten, ohne Klartext: Eingaben und Ergebnisse stehen nur als Fingerabdruck darin. Nach 30 Tagen wird gelöscht.",
    "protokoll.zeigen": "Protokoll anzeigen",
    "protokoll.leer": "Noch keine Einträge.",
    "protokoll.ort": (o) => `Ablage: ${o}`,
    "protokoll.zeit": "Zeit",
    "protokoll.art": "Art",
    "protokoll.werkzeug": "Werkzeug",
    "protokoll.entscheidung": "Entscheidung",
    "protokoll.ergebnis": "Ergebnis",
    "protokoll.dauer": "Dauer",
    "sinne.stimmeweg": "Stimme entfernen",
    "sinne.einrichten": "Läufer anlegen",
    "sinne.hoert": "🎙 Aufnahme läuft, zum Beenden loslassen",
    "sinne.schreibtmit": "Wird mitgeschrieben …",
    "sinne.keinestimme": "keine eigene Stimme",
    "sinne.stimmeliegt": "eigene Stimme hinterlegt",
    "anhang.wirdangesehen": "⏳ Ein anderes Modell sieht sie gerade an …",
    "anhang.angesehen": "Ein anderes Modell hat sie angesehen; das ist alles, was es dazu gibt:",
    "anhang.geaendert": "Geändert:",
    "anhang.speichern": "Speichern unter …",
    "anhang.gespeichert": "Gespeichert nach",
    "seite.zurueck": "Zurück zum Gespräch",
    "seite.modelle": "Modelle",
    "seite.freigabe": "Was dieser Rechner hergibt",

    "modus.chat.name": "Chat",
    "modus.chat.was": "ohne Werkzeuge",
    "modus.chat.leer":
      "Ein Gespräch mit dem lokalen Modell. Der Verlauf wird mitgegeben, das Modell sieht also, was im Verlauf gesagt wurde.",
    "modus.agent.name": "Agent",
    "modus.agent.was": "mit Werkzeugen",
    "modus.agent.leer":
      "Der Agent erledigt deinen Auftrag mit den Werkzeugen in seinem Werkzeugkoffer. Das Gespräch geht von Auftrag zu Auftrag mit. In den Einstellungen kannst du Anpassungen vornehmen …",
    "modus.knoten.name": "Knoten",
    "modus.knoten.was": "Mining",
    "modus.knoten.warum":
      "Der Knoten rechnet für das Netz und verdient daran. Dem Klienten fehlen Knotenadresse und Vollmacht.",
    "modus.wallet.name": "Wallet",
    "modus.wallet.was": "Guthaben",
    "modus.wallet.warum":
      "Guthaben, Überweisungen und die Belege dazu. Braucht dieselbe Netzhälfte wie der Knoten.",
    "modus.terminal.name": "Terminal",
    "modus.terminal.was": "Befehle",
    "modus.terminal.leer":
      "Tipp einen Befehl und drück Enter. Dies ist deine Kommandozeile, nicht die des Modells: Was hier läuft, hat deine Rechte und keine Einhängegrenze. Der Agent kommt hier nicht heran.",
    "terminal.eingabe": "Befehl",
    "terminal.laeuft": "läuft …",
    "terminal.gekuerzt": "[Ausgabe gekürzt]",
    "terminal.abgebrochen": "[abgebrochen, die Frist von 300 s ist abgelaufen]",

    "wort.chat.eines": "Gespräch",
    "wort.chat.viele": "Gespräche",
    "wort.chat.neu": "Neues Gespräch",
    "wort.agent.eines": "Prozess",
    "wort.agent.viele": "Prozesse",
    "wort.agent.neu": "Neuer Prozess",

    "speicher.voll": (was) => `Das ${was} ließ sich nicht merken; der Speicher des Fensters ist zu.`,
    "wurzel.warum":
      "Der Agent hat noch keinen Arbeitsordner. Ohne einen gibt es die Dateiwerkzeuge nicht, und er antwortet nur aus dem, was er weiß. Wähle den Ordner, in dem er arbeiten soll.",
    "reichweite.kein": "kein Verzeichnis eingehängt",
    "reichweite.hinweis": "Ohne Arbeitsordner hat der Agent keine Werkzeuge. In den Einstellungen setzen.",
    "reichweite.gesperrt": ", Werkzeuge durch die Betriebsart gesperrt",
    "reichweite.pfadmarke": "Einhängepfad:",
    "reichweite.wechseln": "Verzeichnis wechseln",
    "reichweite.kistemarke": "Lokale Werkzeugkiste:",
    "reichweite.andereKiste": "andere Werkzeugkiste",
    "reichweite.kistehinweis": "Der Ordner, aus dem die Werkzeuge kommen. Weiterschalten zwischen den Kisten.",
    "werkzeuge.zeigen": "Werkzeuge anzeigen",
    "werkzeuge.verbergen": "Werkzeuge verbergen",
    "werkzeuge.aus": (kiste, n) => `Aus der Kiste „${kiste}“, ${n} Stück:`,
    "reichweite.fehler": (f) => `Ging nicht: ${f}`,
    "md.beitraege": (n) => `- Beiträge: ${n}`,
    "geloescht": (was, titel) => `${was} „${titel}“ gelöscht.`,
    "abgelegt": (was, wo) => `${was} abgelegt: ${wo}`,
    "ausgabe.warum": (was) =>
      `Bevor ein ${was} ausgegeben werden kann, braucht es einen Ordner dafür. Trage ihn hier ein; danach landet jedes ausgegebene ${was} als Markdown darin.`,
    "ausgabe.fehler": (f) => `Fehler beim Ausgeben: ${f}`,
    "umbenennen.label": (was) => `${was} umbenennen`,

    "denken.mal": (n) => (n === 1 ? "1 Mal nachgedacht" : `${n} Mal nachgedacht`),
    "denken.jetzt": " · denkt gerade nach …",
    "befehl.laufend": (getan) =>
      getan === 0
        ? "Befehl wird ausgeführt …"
        : `${getan} ${getan === 1 ? "Befehl" : "Befehle"} ausgeführt · einer läuft …`,
    "befehl.vorhaben": "Vorhaben",
    "befehl.befehl": "Befehl",
    "befehl.antwort": "Antwort",
    "befehl.aussteht": "Antwort steht noch aus…",
    "befehl.ohne": "Keine Antwort erhalten",
    "befehl.unlesbar": "Nicht lesbarer Vorschlag",
    "befehl.ohne_aufruf": "Antwort ohne erkannten Befehl",
    "befehl.keine": "Keine Befehle ausgeführt",
    "befehl.eins": "1 Befehl ausgeführt",
    "befehl.viele": (n) => `${n} Befehle ausgeführt`,
    "lauf.arbeitet": "arbeitet",
    "antwort.keine": "(keine Schlussantwort)",

    "meldung.schliessen": "Meldung schließen",
    "ordner.waehlen": "Ordner wählen",
    "budget.nicht": "nicht denken",
    "budget.frei": "unbegrenzt",
    "budget.token": (n) => `${n} Token`,
    "ordner.waehlenFuer": (titel) => `Ordner für ${titel} wählen`,
    "fehler": (f) => `Fehler: ${f}`,
    "fehler.start": (f) => `Fehler beim Start: ${f}`,
    "gesetzt": (titel) => `${titel} gesetzt.`,

    "loeschen": "Löschen",
    "loeschen.frage": "Wirklich löschen?",
    "loeschen.laeuft": "löscht …",
    "loeschen.fehler": (f) => `Löschen ging nicht: ${f}`,

    "bau.hinweis": "Herunterladen und Kalibrieren dauert Minuten bis Stunden und braucht Gigabyte.",
    "bau.knopf": "Download und kalibrieren",
    "bau.knopf.nurbauen": "kalibrieren",
    "bau.fertig": (n) => `${n} ist fertig kalibriert.`,
    "bau.fehler": (n) => `${n} ist fehlgeschlagen.`,
    "katalog.fehler": (f) => `Katalog nicht lesbar: ${f}`,

    "platte.frei": (g) => `${g} frei auf der Platte`,
    "platte.haelt": (g) => ` Myelith hält davon ${g}`,
    "platte.reserviert": (g) => ` und reserviert ${g}`,

    "modell.keins": "kein Modell geladen",
    "modell.nichtGeladen": (name, pfad) => `${name} (${pfad}) nicht geladen`,
    "modell.geladen": (name, pfad, zusatz) => `${name} (${pfad}) geladen${zusatz}`,
    "modell.ladefrist": (s) => `, in ${s} s`,
    "modell.laedt": "lädt",
    "modell.gewechselt": "Modell gewechselt; es wird beim nächsten Auftrag geladen.",
    "modell.wechselfehler": (f) => `Wechsel ging nicht: ${f}`,
    "modell.ladefehler": (f) => `Laden ging nicht: ${f}`,
    "modelle.fehler": (f) => `Modelle nicht lesbar: ${f}`,
    "modell.artefaktKlammer": (a) => `${a} (nicht geladen)`,

    "akt.titel": "Updates",
    "akt.pruefen": "Nach Updates suchen",
    "akt.einspielen": "Updates installieren",
    "akt.sieht": "sucht …",
    "akt.laeuft": "installiert …",
    "akt.aktuell": (f) => `Fassung ${f}. Der Klon ist auf dem neuesten Stand.`,
    "akt.hinterher": (f, n) => `Fassung ${f}. ${n === 1 ? "Eine Änderung liegt" : `${n} Änderungen liegen`} bereit.`,
    "akt.neueste": (m, wann) => ` Neueste Freigabe: ${m}${wann ? ` vom ${wann.slice(0, 10)}` : ""}.`,
    "akt.kein": (f) => `Fassung ${f}.`,
    "akt.fehler": (g) => `Nachsehen ging nicht: ${g}`,
    "akt.fertig": "Eingespielt. Der neue Stand läuft ab dem nächsten Start.",
    "lauf.agent": "der Agent fährt",
    "lauf.antwort": "das Modell antwortet",
    "lauf.nichtgeladen": "das Modell ist nicht geladen",
  },

  en: {
    "vorhang.stand": "loading",
    "menue.umbenennen": "Rename",
    "menue.ausgeben": "Export",
    "menue.loeschen": "Delete",
    "kontext.label": "Context",
    "kontext.zahl": (belegt, grenze, prozent) => `Context ${prozent} % · ${belegt} of ${grenze} tokens`,
    "kontext.titel": (ansage, gespraech, n) =>
      `Tool listing ${ansage} tokens, conversation ${gespraech} tokens in ${n} messages. Click to compress.`,
    "kontext.verdichten": "Compress context",
    "kontext.neu": "New conversation",
    "kontext.laeuft": "Compressing context …",
    "kontext.leer": "The conversation is empty; there is nothing to compress.",
    "kontext.verdichtet": (vorher, nachher) =>
      `⚑ Context compressed: ${vorher} → ${nachher} tokens. From here on the model sees a summary of the conversation before.`,
    "kontext.verdichtet_kurz": (vorher, nachher) => `compressed ${vorher} → ${nachher}`,
    "kontext.im_lauf": "context compressed",
    "kontext.fehler": (f) => `Compressing failed: ${f}`,
    "leiste.label": "Conversations and mode",
    "leiste.modus": "Mode",
    "leiste.modell": "Model",
    "leiste.schalten": "Show or hide the sidebar",
    "knopf.laden": "Load model",
    "kopf.einstellungen": "Settings",
    "eingabe.platz": "Ask something, or give a task.",
    "eingabe.label": "Input",
    "knopf.senden": "Send",
    "knopf.anhang": "Attach a file",
    "lauf.laeuftschon": "A run is still going; one moment.",
    "sprachmodus.halten": "Hold to speak",
    "anhang.weg": "Remove attachment",
    "knopf.sprachmodus": "Voice mode",
    "knopf.sprechtaste": "Hold and speak",
    "knopf.vorlesen": "Read answers aloud",
    "sinne.titel": "Senses",
    "sinne.stimmewaehlen": "Upload a voice",
    "notaus.knopf": "Stop",
    "notaus.gemeldet": "Emergency stop: the task was halted. The conversation stays as it is.",
    "sinne.einwilligung": "The recording is my own voice, or the person has consented.",
    "protokoll.titel": "Action log",
    "protokoll.satz": "Every agent action, without plain text: inputs and results appear only as fingerprints. Deleted after 30 days.",
    "protokoll.zeigen": "Show log",
    "protokoll.leer": "No entries yet.",
    "protokoll.ort": (o) => `Stored in: ${o}`,
    "protokoll.zeit": "Time",
    "protokoll.art": "Kind",
    "protokoll.werkzeug": "Tool",
    "protokoll.entscheidung": "Decision",
    "protokoll.ergebnis": "Result",
    "protokoll.dauer": "Duration",
    "sinne.stimmeweg": "Remove the voice",
    "sinne.einrichten": "Create the runner",
    "sinne.hoert": "🎙 Recording, release to stop",
    "sinne.schreibtmit": "Transcribing …",
    "sinne.keinestimme": "no custom voice",
    "sinne.stimmeliegt": "custom voice stored",
    "anhang.wirdangesehen": "⏳ A different model is examining it …",
    "anhang.angesehen": "A different model examined it; this is all there is:",
    "anhang.geaendert": "Changed:",
    "anhang.speichern": "Save as …",
    "anhang.gespeichert": "Saved to",
    "seite.zurueck": "Back to the conversation",
    "seite.modelle": "Models",
    "seite.freigabe": "What this machine offers",

    "modus.chat.name": "Chat",
    "modus.chat.was": "without tools",
    "modus.chat.leer":
      "A conversation with the local model. The history is passed along, so the model sees what was said earlier in it.",
    "modus.agent.name": "Agent",
    "modus.agent.was": "with tools",
    "modus.agent.leer":
      "The agent carries out your task with the tools in its toolbox. The conversation carries over from task to task. You can adjust things in the settings …",
    "modus.knoten.name": "Node",
    "modus.knoten.was": "Mining",
    "modus.knoten.warum":
      "The node computes for the network and earns from it. The client is missing a node address and a mandate.",
    "modus.wallet.name": "Wallet",
    "modus.wallet.was": "Balance",
    "modus.wallet.warum":
      "Balance, transfers and the receipts for them. Needs the same network half as the node.",
    "modus.terminal.name": "Terminal",
    "modus.terminal.was": "Commands",
    "modus.terminal.leer":
      "Type a command and press Enter. This is your command line, not the model's: what runs here has your rights and no mount boundary. The agent cannot reach it.",
    "terminal.eingabe": "Command",
    "terminal.laeuft": "running …",
    "terminal.gekuerzt": "[output truncated]",
    "terminal.abgebrochen": "[aborted, the 300 s limit expired]",

    "wort.chat.eines": "Conversation",
    "wort.chat.viele": "Conversations",
    "wort.chat.neu": "New conversation",
    "wort.agent.eines": "Process",
    "wort.agent.viele": "Processes",
    "wort.agent.neu": "New process",

    "speicher.voll": (was) =>
      `The ${was.toLowerCase()} could not be remembered; the window's storage is closed.`,
    "wurzel.warum":
      "The agent has no working folder yet. Without one there are no file tools at all, and it answers only from what it knows. Pick the folder it should work in.",
    "reichweite.kein": "no folder mounted",
    "reichweite.hinweis": "Without a working folder the agent has no tools. Set one in the settings.",
    "reichweite.gesperrt": ", tools locked by the operating mode",
    "reichweite.pfadmarke": "Mount path:",
    "reichweite.wechseln": "Change folder",
    "reichweite.kistemarke": "Local toolbox:",
    "reichweite.andereKiste": "other toolbox",
    "reichweite.kistehinweis": "The folder the tools come from. Cycle through the boxes.",
    "werkzeuge.zeigen": "Show tools",
    "werkzeuge.verbergen": "Hide tools",
    "werkzeuge.aus": (kiste, n) => `From the “${kiste}” box, ${n} of them:`,
    "reichweite.fehler": (f) => `Did not work: ${f}`,
    "md.beitraege": (n) => `- Messages: ${n}`,
    "geloescht": (was, titel) => `${was} “${titel}” deleted.`,
    "abgelegt": (was, wo) => `${was} exported to: ${wo}`,
    "ausgabe.warum": (was) =>
      `Before a ${was.toLowerCase()} can be exported it needs a folder. Enter it here; every exported ${was.toLowerCase()} then lands there as Markdown.`,
    "ausgabe.fehler": (f) => `Export failed: ${f}`,
    "umbenennen.label": (was) => `Rename ${was.toLowerCase()}`,

    "denken.mal": (n) => (n === 1 ? "Thought once" : `Thought ${n} times`),
    "denken.jetzt": " · thinking …",
    "befehl.laufend": (getan) =>
      getan === 0
        ? "Running a command …"
        : `${getan} ${getan === 1 ? "command" : "commands"} run · one running …`,
    "befehl.vorhaben": "Intent",
    "befehl.befehl": "Command",
    "befehl.antwort": "Response",
    "befehl.aussteht": "Response still pending…",
    "befehl.ohne": "No response received",
    "befehl.unlesbar": "Unreadable proposal",
    "befehl.ohne_aufruf": "Response without a recognised command",
    "befehl.keine": "No commands run",
    "befehl.eins": "1 command run",
    "befehl.viele": (n) => `${n} commands run`,
    "lauf.arbeitet": "working",
    "antwort.keine": "(no final answer)",

    "meldung.schliessen": "Dismiss message",
    "ordner.waehlen": "Choose folder",
    "budget.nicht": "no thinking",
    "budget.frei": "unlimited",
    "budget.token": (n) => `${n} tokens`,
    "ordner.waehlenFuer": (titel) => `Choose folder for ${titel}`,
    "fehler": (f) => `Error: ${f}`,
    "fehler.start": (f) => `Error at start: ${f}`,
    "gesetzt": (titel) => `${titel} set.`,

    "loeschen": "Delete",
    "loeschen.frage": "Really delete?",
    "loeschen.laeuft": "deleting …",
    "loeschen.fehler": (f) => `Delete failed: ${f}`,

    "bau.hinweis": "Downloading and calibrating takes minutes to hours and needs gigabytes.",
    "bau.knopf": "Download and calibrate",
    "bau.knopf.nurbauen": "calibrate",
    "bau.fertig": (n) => `${n} is calibrated.`,
    "bau.fehler": (n) => `${n} failed.`,
    "katalog.fehler": (f) => `Catalogue not readable: ${f}`,

    "platte.frei": (g) => `${g} free on disk`,
    "platte.haelt": (g) => ` Myelith holds ${g} of it`,
    "platte.reserviert": (g) => ` and reserves ${g}`,

    "modell.keins": "no model loaded",
    "modell.nichtGeladen": (name, pfad) => `${name} (${pfad}) not loaded`,
    "modell.geladen": (name, pfad, zusatz) => `${name} (${pfad}) loaded${zusatz}`,
    "modell.ladefrist": (s) => `, in ${s} s`,
    "modell.laedt": "loading",
    "modell.gewechselt": "Model changed; it will be loaded with the next task.",
    "modell.wechselfehler": (f) => `Change failed: ${f}`,
    "modell.ladefehler": (f) => `Loading failed: ${f}`,
    "modelle.fehler": (f) => `Models not readable: ${f}`,
    "modell.artefaktKlammer": (a) => `${a} (not loaded)`,

    "akt.titel": "Update",
    "akt.pruefen": "Check for updates",
    "akt.einspielen": "Install updates",
    "akt.sieht": "checking …",
    "akt.laeuft": "installing …",
    "akt.aktuell": (f) => `Version ${f}. The clone is up to date.`,
    "akt.hinterher": (f, n) => `Version ${f}. ${n === 1 ? "One change is" : `${n} changes are`} ready.`,
    "akt.neueste": (m, wann) => ` Latest release: ${m}${wann ? ` from ${wann.slice(0, 10)}` : ""}.`,
    "akt.kein": (f) => `Version ${f}.`,
    "akt.fehler": (g) => `Checking failed: ${g}`,
    "akt.fertig": "Installed. The new version runs from the next start.",
    "lauf.agent": "the agent is running",
    "lauf.antwort": "the model is answering",
    "lauf.nichtgeladen": "the model is not loaded",
  },
};

/// Die Sprache, in der das Fenster gerade spricht.
///
/// ⚑ **Der Ruecken sagt sie, nicht der Browser.** `navigator.language`
/// waere die Sprache des Betriebssystems und nicht die gewaehlte; wer
/// auf einem englischen System Deutsch einstellt, meint das.
let sprache = "de";

/// Ein Text in der gewaehlten Sprache.
///
/// ⚑ **Ein fehlender Schluessel faellt auf Deutsch zurueck und nicht
/// auf den Schluessel selbst.** Ein Fenster, in dem „modell.laedt"
/// steht, ist kaputt; eines, in dem an einer Stelle Deutsch steht, ist
/// unvollstaendig uebersetzt, und das ist der kleinere Schaden.
///
/// 📌 **Und ein Text kann eine Funktion sein.** Wer einen Wert
/// einsetzen muss, setzt ihn in der Sprache ein, in der er steht: Im
/// Deutschen steht die Zahl vor dem Wort, im Englischen auch, aber die
/// naechste Sprache tut es vielleicht nicht.
function t(schluessel, ...werte) {
  const eintrag = (TEXTE[sprache] || TEXTE.de)[schluessel] ?? TEXTE.de[schluessel];
  if (eintrag === undefined) return "";
  return typeof eintrag === "function" ? eintrag(...werte) : eintrag;
}

/// **Beschriftet alles, was fest im HTML steht.**
///
/// ⚑ **Ueber Attribute und nicht ueber eine Liste von Kennungen.** Eine
/// Liste im Skript waere eine zweite Stelle, an der jedes neue Element
/// nachgetragen werden muss, und genau das ist Fund 271. Was ein
/// `data-t` traegt, wird beschriftet; was keines traegt, nicht.
function beschriften() {
  document.documentElement.lang = sprache;
  for (const el of document.querySelectorAll("[data-t]")) {
    el.textContent = t(el.dataset.t);
  }
  for (const el of document.querySelectorAll("[data-t-marke]")) {
    el.setAttribute("aria-label", t(el.dataset.tMarke));
  }
  for (const el of document.querySelectorAll("[data-t-platz]")) {
    el.setAttribute("placeholder", t(el.dataset.tPlatz));
  }
}

/// **Setzt Erscheinungsbild und Schriftgroesse an der Wurzel.**
///
/// ⚑ **Zwei Attribute und keine Klassen.** Das Stilblatt haengt seine
/// Themenblöcke an `:root[data-thema=…]` und `:root[data-schrift=…]`;
/// damit steht die ganze Umschaltung dort, wo die Farben stehen, und
/// nicht hier. **Dieses Skript weiss von keiner einzigen Farbe.**
///
/// ⚑ **Ohne eigene Liste der erlaubten Werte.** Geprueft wird im
/// Setzer der Kiste, und zwar dort allein: Was hier ankommt, ist schon
/// eine gueltige Kennung. **Eine zweite Liste hier waere die Stelle,
/// an der ein drittes Erscheinungsbild vergessen wuerde**, und sie
/// faende es lautlos: Das Attribut stuende da, das Stilblatt kennte es
/// nicht, und es saehe aus wie die Vorgabe.
///
/// Der Rueckfall gilt nur einer Ablage ohne das Feld.
function bild_setzen(thema, schrift) {
  const w = document.documentElement;
  w.dataset.thema = thema || "dunkel";
  w.dataset.schrift = schrift || "normal";
}

/// Uebernimmt eine Sprache und zeichnet alles neu.
async function sprache_setzen(k) {
  if (k !== "de" && k !== "en") return;
  if (k === sprache) return;
  sprache = k;
  beschriften();
  alles_zeichnen();
  await modellzeile_schreiben();
}

const MODI = [
  {
    // ⚑ Der Modus heisst „Chat" und der Befehl im Ruecken `frage`, und
    // das ist kein Versehen: Der Befehl fragt das Modell **einmal**,
    // der Modus ist ein Gespraech, weil das Fenster den Verlauf
    // mitfuehrt und bei jedem Zug wieder mitgibt. Das eine ist der
    // Zug, das andere die Partie.
    // ⚑ **Hier stehen nur noch Kennung und Zustand.** Name, Kurzsatz
    // und Leertext stehen in `TEXTE` unter `modus.<id>.…`: Was ein
    // Mensch liest, steht in der Sprachtabelle, was ein Programm
    // vergleicht, hier.
    id: "chat",
  },
  {
    id: "agent",
  },
  // ⚠️ **Zwei Modi, die es geben WIRD und heute nicht gibt.** Sie
  // stehen gedaempft da und sagen beim Anfassen, was fehlt. Ein Modus,
  // der still nichts taete, waere schlimmer als keiner; einer, der
  // ganz fehlt, verschweigt, wohin der Client geht.
  {
    id: "knoten",
    offen: false,
  },
  {
    id: "wallet",
    offen: false,
  },
  // ⛔️ **Deine Kommandozeile, nicht die des Modells** (Auftrag des
  //    Projektinhabers, 2026-09-23, unter Wallet).
  //
  //    Jede andere Schranke im Client schuetzt vor einem **Modell**, das
  //    sich irrt oder das ein fremder Text in die Irre fuehrt. Ein
  //    Mensch, der ein Terminal oeffnet, hat genau das gewollt. Der
  //    Befehl dahinter steht in keinem Werkzeugkasten und keine
  //    Agentenschleife kann ihn rufen; siehe den Block vor
  //    `terminal_ausfuehren` im Ruecken.
  //
  //    ⚑ **Und er ist die Voraussetzung dafuer, dass dieses Fenster auf
  //    einem System ohne Desktop allein genuegt:** kein Dateimanager,
  //    kein Editor, keine zweite Anwendung noetig.
  {
    id: "terminal",
  },
];

// --- Was eine Zeile in der Liste IST ------------------------------------
//
// ⚑ **Im Agentenmodus heisst es Prozess und nicht Gespraech**
// (Festlegung des Projektinhabers, 2026-09-10), und das ist keine
// Geschmacksfrage: Im Chat traegt eine Zeile einen **Verlauf**, jeder
// Zug sieht die vorigen. Beim Agenten ist jeder Auftrag ein Lauf mit
// Werkzeugen, eigenem Schrittbudget und eigener Belegkette. **Zwei
// verschiedene Dinge unter einem Wort sind eine Behauptung, dass sie
// dasselbe seien.** 📌 Bis zum 2026-09-14 stand hier auch, dass beim
// Agenten jeder Auftrag fuer sich steht; seitdem geht das Gespraech auch
// dort mit (Entscheidung C2), und der Unterschied bleibt der der Laeufe.
const wort = (modus) => {
  const m = modus || modus_jetzt();
  const art = m === "agent" ? "agent" : "chat";
  return {
    eines: t(`wort.${art}.eines`),
    viele: t(`wort.${art}.viele`),
    neu: t(`wort.${art}.neu`),
    frisch: t(`wort.${art}.neu`),
  };
};

/// Der eingerastete Modus, unabhaengig davon, ob etwas offen ist.
///
/// 📌 **Er hing bis zum 2026-09-10 am offenen Gespraech**, und daraus
/// folgte ein Fehler, den der Projektinhaber gemeldet hat: Ein
/// Moduswechsel musste dann **etwas anlegen**, um den Modus ueberhaupt
/// festhalten zu koennen. Wer zwischen Chat und Agent hin und her
/// klickte, hinterliess bei jedem Klick ein leeres Gespraech in der
/// Liste.
///
/// ⚑ **Angelegt wird jetzt nur noch auf zwei Wege**: Knopf gedrueckt,
/// oder etwas abgeschickt. **Ein Modus ist eine Ansicht, und eine
/// Ansicht legt nichts an.**
let modus = MODI[0].id;

/// **Welcher Modus gerade gilt.**
const modus_jetzt = () => modus;

// --- Gespraeche ---------------------------------------------------------

// 📌 **Sie liegen im Browserspeicher des Fensters und nicht in einer
// Datei.** Das ist eine bewusste Zwischenloesung und keine Ablage: Sie
// ueberlebt einen Neustart, sie liegt aber nur auf dieser Maschine, sie
// wird nicht gesichert, und niemand kann sie ausserhalb des Fensters
// lesen. Eine richtige Ablage waere ein Format, ein Ort und ein Befehl
// im Ruecken; das ist eine Entscheidung des Projektinhabers und keine,
// die eine Oberflaeche nebenbei trifft.
const SPEICHER = "myelith.gespraeche.v1";
let gespraeche = [];
let offen = null;

function laden_aus_speicher() {
  try {
    const roh = localStorage.getItem(SPEICHER);
    gespraeche = roh ? JSON.parse(roh) : [];
    if (!Array.isArray(gespraeche)) gespraeche = [];
  } catch {
    // Ein privates Fenster oder geleerte Ablage: dann eben ohne.
    gespraeche = [];
  }
  // ⚑ **Ein Modus, den es nicht mehr gibt, wird zum ersten.** Der
  // Modus „frage" hiess bis zum 2026-09-09 so und heisst jetzt „chat";
  // ohne diese Zeile stuende ein altes Gespraech in einem Zustand, den
  // keine Schaltflaeche zeigt, und liesse sich nicht mehr einordnen.
  // Die Regel gilt allgemein und nicht nur fuer diese eine Umbenennung.
  for (const g of gespraeche) {
    if (!MODI.some((m) => m.id === g.modus)) g.modus = MODI[0].id;
  }
}

function sichern() {
  try {
    localStorage.setItem(SPEICHER, JSON.stringify(gespraeche));
  } catch {
    melden(t("speicher.voll", wort().eines));
  }
}

const jetzt = () => new Date().toISOString();

/// **Die Warnung vor dem Agentenbetrieb, einmal je Sitzung.**
///
/// ⚑ **Sie steht vor der Benutzung, nicht beim Start** (Auftrag des
/// Projektinhabers, 2026-09-15): Wer im Chat bleibt, bekommt sie nie zu
/// sehen, denn dort handelt niemand auf seinem Rechner.
///
/// ⚑ **Der Text kommt aus der Kiste**, nicht von hier. Die Konsole sagt
/// denselben; zweimal getippt liefe er auseinander, sobald einer ihn
/// verbessert.
///
/// ⚠️ **Sie ist keine Schranke.** Einhängegrenze, Schreiberlaubnis und
/// Betriebsart wirken unabhängig davon, ob jemand gelesen hat.
// --- Der Hinweis beim Start -------------------------------------------
//
// ⛔️ **Bei jedem Start, aktiv zu bestaetigen, ohne Ausweg** (Art. 50
// Abs. 1 KI-Verordnung; Festlegung des Projektinhabers, 2026-09-25). Der
// Knopf geht erst mit dem Haekchen, Escape und ein Klick daneben tun
// nichts, und alles darunter ist `inert`, solange er steht. Der Text
// kommt aus der Kiste (`kennzeichnung::starthinweis`).
//
// ⚑ **Die Marken kommen aus derselben Antwort**: das Wort im Kopf und die
// Zeile unter jeder Antwort. Bis sie da ist, gilt `KI_MARKE`.
let KI_MARKE = "KI-generiert";
async function starthinweis_zeigen() {
  const h = await invoke("starthinweis");
  KI_MARKE = h.marke;
  $("kimarke").textContent = h.kurz;
  $("kimarke").title = h.titel;
  $("kihinweistitel").textContent = h.titel;
  const liste = $("kihinweispunkte");
  liste.replaceChildren(
    ...h.punkte.map((p) => {
      const li = document.createElement("li");
      li.textContent = p;
      return li;
    }),
  );
  $("kihinweisverweistitel").textContent = h.verweis_titel;
  $("kihinweisverweis").textContent = h.verweis;
  $("kihinweisbestaetigung").textContent = h.bestaetigung;
  const haken = $("kihinweishaken");
  const weiter = $("kihinweisweiter");
  weiter.textContent = h.weiter;
  haken.checked = false;
  weiter.disabled = true;
  haken.addEventListener("change", () => (weiter.disabled = !haken.checked));

  const darunter = [$("haupt"), $("seitenleiste")].filter(Boolean);
  darunter.forEach((e) => (e.inert = true));
  const kasten = $("kihinweis");
  kasten.hidden = false;
  haken.focus();
  // Alles neu zeichnen, damit schon gezeichnete Antworten die Marke in
  // der richtigen Sprache tragen.
  alles_zeichnen();
  await new Promise((fertig) => {
    weiter.onclick = () => {
      if (!haken.checked) return;
      kasten.hidden = true;
      darunter.forEach((e) => (e.inert = false));
      fertig();
    };
  });
}

let warnung_gezeigt = false;
async function agentenwarnung_zeigen() {
  if (warnung_gezeigt) return;
  let w;
  try {
    w = await invoke("agentenwarnung");
  } catch {
    return;
  }
  // `null` heisst: zugestimmt und Haekchen gesetzt.
  if (!w) {
    warnung_gezeigt = true;
    return;
  }
  warnung_gezeigt = true;

  $("warnungtitel").textContent = w.achtung;
  $("warnungkern").textContent = w.kern;
  $("warnunganleitungtitel").textContent = w.anleitung_titel;
  $("warnungnichtwiedertext").textContent = w.nicht_wieder;
  $("warnungok").textContent = w.zustimmung;

  const liste = $("warnungregeln");
  liste.replaceChildren();
  for (const [regel, grund] of w.regeln) {
    const li = document.createElement("li");
    const r = document.createElement("span");
    r.className = "regel";
    r.textContent = regel;
    // ⚑ Der Grund steht dabei: Eine Regel ohne Grund wird beim ersten
    // Mal befolgt und beim zweiten umgangen.
    li.append(r, document.createTextNode(` ${grund}`));
    liste.append(li);
  }

  const haken = $("warnungnichtwieder");
  haken.checked = false;
  const kasten = $("warnung");
  kasten.hidden = false;
  $("warnungok").focus();

  $("warnungok").onclick = async () => {
    if (haken.checked) {
      try {
        await invoke("setzen", { feld: "agent.warnung", wert: "aus" });
      } catch (f) {
        melden(t("fehler", f));
      }
    }
    kasten.hidden = true;
  };
}

/// **Stellt das Modell ein, mit dem dieses Gespraech zuletzt lief.**
///
/// ⚑ **Eingestellt und nicht geladen** (Auftrag des Projektinhabers,
/// 2026-09-15): Wer ein altes Gespraech aufschlaegt, will sehen, womit
/// es gefuehrt wurde, und nicht minutenlang auf ein Artefakt warten, das
/// er vielleicht nur nachlesen wollte. Geladen wird beim ersten Auftrag,
/// wie sonst auch.
///
/// 📌 **Das alte wird dabei entladen**, genau wie beim Wechsel ueber die
/// Modellwahl. Ohne das faehrt der naechste Auftrag mit dem alten
/// Modell, waehrend die Anzeige das neue nennt; die Lehre steht beim
/// `modellwahl`-Rueckruf und gilt hier genauso.
///
/// ⚠️ **Steht schon dasselbe Modell, geschieht nichts.** Sonst wuerde
/// jedes Umschalten zwischen zwei Gespraechen desselben Modells das
/// geladene wegwerfen, und das ist das Gegenteil von hilfreich.
async function modell_dem_gespraech_folgen(g) {
  if (!g || !g.modell) return;
  try {
    const e = await invoke("einstellungen");
    if (e.werte["modell.artefakt"] === g.modell) return;
    await invoke("setzen", { feld: "modell.artefakt", wert: g.modell });
    geladen = false;
    try {
      await invoke("modell_entladen");
    } catch {
      /* dann eben nicht */
    }
    modellstand = null;
    await kopf_zeichnen();
    await modellzeile_schreiben();
  } catch {
    // ⚑ Ein Gespraech laesst sich auch ohne sein Modell oeffnen: Das
    // Artefakt kann inzwischen geloescht sein, und dann ist die
    // Einstellung eine Unbequemlichkeit und kein Grund, nichts zu
    // zeigen.
  }
}

/// **Der Einhaengepfad des offenen Prozesses**, oder nichts.
///
/// ⚑ **Je Prozess gespeichert** (Auftrag des Projektinhabers,
/// 2026-09-15). Er liegt beim Gespraech im Speicher des Fensters, nicht
/// in den Einstellungen: Wer einen Prozess in einem Ordner fuehrt, fuehrt
/// ihn dort weiter, und der naechste Prozess erbt das nicht.
///
/// ⚠️ **`null` heisst „was die Einstellungen sagen"**, nicht „keiner".
/// Nur so bleibt unterscheidbar, ob jemand fuer diesen Prozess etwas
/// gewaehlt hat; die Rust-Seite legt dieselbe Reihenfolge an: Prozess,
/// dann Einstellung, dann die Vorgabe `WORK_DIR`.
function prozesspfad() {
  return (offen && offen.wurzel) || null;
}

function neues_gespraech(modus) {
  const art = modus || modus_jetzt();
  const g = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    titel: wort(art).frisch,
    modus: art,
    beitraege: [],
    wann: jetzt(),
  };
  gespraeche.unshift(g);
  offen = g;
  modus = art;
  sichern();
  return g;
}

/// **Holt ein Gespraech in der Leiste nach ganz oben.**
///
/// ⚑ **Auftrag des Projektinhabers, 2026-09-12:** Womit man gerade
/// arbeitet, steht oben. Bis dahin stand die Leiste in der Reihenfolge
/// der **Anlage**: Ein Gespraech, das man seit Wochen fuehrt, rutschte
/// mit jedem neuen weiter nach unten, bis man es suchen musste.
///
/// ⚑ **Ausgeloest von jeder Beruehrung**, nicht nur vom Schreiben: Wer
/// einen Auftrag anstoesst, arbeitet damit. Beides geht durch
/// `senden`, und dort steht der Aufruf.
///
/// ⚠️ **Nicht beim blossen Oeffnen.** Wer die Leiste durchsieht, um
/// etwas wiederzufinden, wuerde sie sonst beim Lesen umsortieren, und
/// die Zeile, auf die er als Naechstes klicken wollte, waere
/// weggerutscht. **Umordnen ist eine Folge von Arbeit, nicht von
/// Hinsehen.**
///
/// ⚑ **Die Reihenfolge ueberlebt einen Neustart ohne ein weiteres
/// Feld.** `sichern` legt das Feld `gespraeche` als Ganzes ab, also
/// samt seiner Reihenfolge, und `laden_aus_speicher` liest es
/// unveraendert zurueck.
///
/// ⚠️ **`wann` wird dabei ausdruecklich NICHT angefasst.** Es steht im
/// Markdown-Export als „Begonnen", und das ist eine Aussage ueber den
/// Anfang. Wer es hier fortschriebe, machte daraus stillschweigend
/// „zuletzt benutzt", und der Export sagte etwas anderes, als dort
/// steht. Ein zweites Feld dafuer liest heute niemand, und ein
/// ungelesenes Feld ist eine Einladung, ihm spaeter etwas anderes zu
/// unterstellen.
function nach_oben(g) {
  if (!g) return;
  const i = gespraeche.findIndex((x) => x.id === g.id);
  // Nicht gefunden oder schon oben: nichts tun, damit kein
  // ueberfluessiges Zeichnen ausgeloest wird.
  if (i <= 0) return;
  gespraeche.splice(i, 1);
  gespraeche.unshift(g);
}

// ⚑ Der Titel kommt aus dem ersten Beitrag und wird nicht erfragt. Ein
// Dialog, der nach einem Namen fragt, bevor man weiss, worum es geht,
// ist eine Huerde vor dem ersten Satz.
/// ⚑ **Ein Agentengespraech ohne Arbeitsverzeichnis fuehrt zur
/// Einstellung, statt ohne Werkzeuge loszulaufen.**
///
/// # 📌 Warum das kein Hinweis ist, sondern ein Weg
///
/// Ohne gesetzte Wurzel gibt es die Dateiwerkzeuge **gar nicht**, und
/// der Agent arbeitet dann ohne sie. Das steht zwar in der
/// Seitenleiste, aber wer gerade ein Agentengespraech aufmacht, liest
/// dort nicht nach: Er tippt einen Auftrag und wundert sich, warum
/// nichts angefasst wird. Eine Meldung waere derselbe Satz an einer
/// Stelle, an der man ihn wegklickt.
///
/// ⚑ **Dieselbe Entscheidung wie beim Ausgabeordner am 2026-09-09:**
/// Ein fehlender Ordner ist keine Fehlermeldung, sondern eine fehlende
/// Entscheidung, und das Fenster fuehrt dorthin, wo sie getroffen wird.
async function wurzel_verlangen() {
  if (modus_jetzt() !== "agent") return;
  try {
    const e = await invoke("einstellungen");
    if (e.werte["agent.wurzel"]) return;
  } catch {
    // Wer die Einstellungen nicht lesen kann, hat ein groesseres
    // Problem; das meldet der naechste Aufruf, der sie braucht.
    return;
  }
  await zum_feld("agent.wurzel", t("wurzel.warum"));
}

/// **Der Titel ist der ganze erste Satz, nicht sein Anfang.**
///
/// # 📌 Gemeldet vom Projektinhaber am 2026-09-10
///
/// Hier wurde auf vierzig Zeichen gekuerzt und ein `...` angehaengt,
/// und **das Ergebnis war der gespeicherte Titel**. Die Zeile in der
/// Leiste kuerzte danach ein zweites Mal, diesmal mit einer Ellipse aus
/// dem Stilblatt, und der Zeigetext zeigte beim Ueberfahren genau
/// dieselbe gekuerzte Zeichenkette. **Gekuerzt wurde also die Sache
/// statt ihrer Darstellung**, und damit war das Lange nirgends mehr zu
/// holen.
///
/// ⚑ **Kuerzen ist Anzeige und gehoert ins Stilblatt.** Was hier
/// entsteht, ist der volle Titel; `.chat .titel` kuerzt ihn auf die
/// Breite der Leiste, und `title` gibt ihn ganz her.
///
/// ⚠️ **Eine Schranke gibt es trotzdem**, aber weit oben: Wer einen
/// Absatz einwirft, soll keinen Absatz als Titel in der Ablage haben.
/// Zweihundert Zeichen sind mehr, als jede Leiste je zeigt, und
/// weniger, als ein Einwurf lang werden kann.
function titel_aus(text) {
  const eine = text.replace(/\s+/g, " ").trim();
  if (!eine) return wort().frisch;
  return eine.length > 200 ? `${eine.slice(0, 200)}…` : eine;
}

/// Dieselbe Zeile, aber kurz genug fuer eine Meldung.
///
/// ⚑ **Hier ist Kuerzen richtig.** Eine Meldung oben rechts hat keine
/// Leiste, die sie fuer sich kuerzen koennte, und keinen Zeigetext, der
/// das Lange nachreichte: Sie steht ein paar Sekunden und geht wieder.
const kurz_titel = (text) => {
  const roh = titel_aus(text);
  return roh.length > 40 ? `${roh.slice(0, 40)}…` : roh;
};

// --- Anzeige ------------------------------------------------------------

// --- Die Zeile unter der Eingabe ---------------------------------------
//
// ⚑ **Sie sagt genau eine Sache: welches Modell im Speicher liegt.**
//
// 📌 **Vorher sagte sie alles Moegliche.** „der Agent faehrt", „Modell
// gewechselt", „Fehler: …", und dazwischen der Ladesatz. Eine Zeile,
// die je nach Augenblick etwas anderes bedeutet, liest man irgendwann
// gar nicht mehr: Wer dort „das Modell antwortet" gewohnt ist, sieht
// „nicht geladen" nicht mehr.
//
// ⚑ **Was der Lauf gerade tut, gehoert dorthin, wo er es tut**, also
// an den Beitrag, und Fehler gehoeren in eine Meldung, die man
// wegklicken kann.

/// Was gerade im Speicher liegt: `null`, oder `{name, pfad}`.
let modellstand = null;

/// Schreibt die Zeile unter der Eingabe neu.
///
/// ⚑ **Das Netzmodell hat keinen Ladezustand**, denn es wird hier nicht
/// geladen; die Zeile bleibt dann leer, statt eine Unwahrheit zu sagen.
async function modellzeile_schreiben(zusatz) {
    const z = $("hinweiszeile");
    let artefakt = "";
    try {
        artefakt = (await invoke("einstellungen")).werte["modell.artefakt"] || "";
    } catch {
        // Ohne Einstellungen gibt es nichts zu sagen.
    }
    if (artefakt.startsWith(NETZMODELL)) {
        z.textContent = "";
        return;
    }
    if (modellstand) {
        z.textContent = t("modell.geladen", modellstand.name, modellstand.pfad, zusatz || "");
    } else if (zusatz) {
        z.textContent = zusatz;
    } else {
        z.textContent = artefakt
            ? t("modell.nichtGeladen", anzeigename_aus(artefakt), artefakt)
            : "";
    }
}

/// Das Kennzeichen, unter dem ein Modell im Netz gerechnet wird.
///
/// ⚑ An einer Stelle: Der Ruecken vergibt es, das Fenster erkennt es
/// daran, und zwei Schreibweisen liefen auseinander.
///
/// 📌 **Seit dem 2026-09-11 ein Praefix und kein ganzer Wert.** Vorher
/// stand genau ein Sammeleintrag `netz` in der Wahl; jetzt steht jedes
/// Modell auch als `netz:<Artefaktname>` da, weil im Netz mehr als ein
/// Modell gerechnet wird. Ein Vergleich auf Gleichheit haette danach
/// keinen einzigen Netzeintrag mehr erkannt.
const NETZMODELL = "netz:";

// --- Das Modell geht nach einer Weile wieder ---------------------------
//
// ⚑ **Ein 4B-Artefakt sind viereinhalb Gigabyte, und sie liegen im
// Speicher, solange das Fenster offen ist.** Wer morgens eine Frage
// stellt und das Fenster stehenlaesst, gibt den Rest des Tages
// Arbeitsspeicher her, ohne etwas davon zu haben.
//
// ⚑ **Das steht in derselben Reihe wie die Kapazitaetsfreigabe:** Was
// Myelith nimmt, soll es auch wieder hergeben.
//
// 📌 **Und es kostet nichts, wenn es falsch liegt.** Wer nach einer
// Stunde doch weiterfragt, wartet einmal die Ladezeit ab; `senden` laedt
// von selbst nach. Ein Entladen, das eine Frage scheitern liesse, waere
// eine andere Sache.

/// Nach so langer Ruhe geht das Modell wieder.
const RUHEFRIST_MS = 15 * 60 * 1000;
let ruheuhr = null;

/// Stellt die Frist neu, weil gerade etwas geschehen ist.
function ruhe_neu_stellen() {
  clearTimeout(ruheuhr);
  if (!modellstand) return;
  ruheuhr = setTimeout(async () => {
    // 📌 **Nicht mitten in einem Lauf.** Der Halter ist dann belegt, und
    // ein Entladen waere entweder wirkungslos oder schlimmer. Die Frist
    // wird stattdessen neu gestellt.
    if (laufender) {
      ruhe_neu_stellen();
      return;
    }
    try {
      const war_da = await invoke("modell_entladen");
      if (!war_da) return;
      const name = modellstand ? modellstand.name : "";
      const pfad = modellstand ? modellstand.pfad : "";
      modellstand = null;
      geladen = false;
      $("hinweiszeile").textContent = `${name} (${pfad}) wieder entladen.`;
      await kopf_zeichnen();
    } catch {
      // Ein misslungenes Entladen ist kein Grund, jemanden zu stoeren.
    }
  }, RUHEFRIST_MS);
}

/// Der Anzeigename zu einem Pfad, aus der zuletzt geholten Modellwahl.
///
/// 📌 **Ohne Treffer der Verzeichnisname und kein erfundener.** Ein
/// Name, den niemand vergeben hat, waere schlechter als ein
/// technischer.
let modellnamen = new Map();
let modellhardware = new Map();
const anzeigename_aus = (pfad) =>
  modellnamen.get(pfad) || pfad.split("/").filter(Boolean).pop() || pfad;

function modi_zeichnen() {
  const w = $("modi");
  w.replaceChildren();
  for (const m of MODI) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "modus blank";
    b.setAttribute("role", "radio");
    // 📌 **Hier stand `offen?.modus`**, und damit war kein Knopf
    // eingerastet, solange nichts offen war (gemeldet vom
    // Projektinhaber, 2026-09-15): Beim Start und nach jedem
    // Moduswechsel, der nichts anlegt, war `offen` leer, der Vergleich
    // also immer falsch. **Der eingerastete Modus ist `modus_jetzt()`**,
    // seit er am 2026-09-10 vom offenen Gespraech abgeloest wurde; die
    // Anzeige hing noch an der alten Quelle. Dieselbe Angabe an zwei
    // Orten, und die zweite meldet sich nicht.
    b.setAttribute("aria-checked", String(modus_jetzt() === m.id));
    if (m.offen === false) {
      b.dataset.offen = "nein";
      b.title = t(`modus.${m.id}.warum`);
    }
    const n = document.createElement("span");
    n.textContent = t(`modus.${m.id}.name`);
    const s = document.createElement("span");
    s.className = "was";
    s.textContent = t(`modus.${m.id}.was`);
    b.append(n, s);
    b.addEventListener("click", () => {
      if (m.offen === false) {
        melden(t(`modus.${m.id}.warum`));
        return;
      }
      // ⚑ **Der Wechsel legt nichts an und wirft nichts weg.** Er
      // stellt die Ansicht um; was in ihr steht, waehlt der Nutzer
      // danach selbst aus der Liste, oder er faengt an zu tippen.
      modus = m.id;
      // ⚑ **Wer den Agenten waehlt, sieht zuerst, was auf dem Spiel
      // steht** (Auftrag des Projektinhabers, 2026-09-15).
      if (m.id === "agent") agentenwarnung_zeigen();
      // ⚑ Ein offener Eintrag bleibt offen, wenn er zu diesem Modus
      // gehoert. Sonst steht der Leerzustand da, und der sagt, was
      // dieser Modus ist.
      if (offen && offen.modus !== modus) offen = null;
      alles_zeichnen();
      kontext_holen();
      wurzel_verlangen();
    });
    w.append(b);
  }
}

/// Was der Agent anfassen darf: Verzeichnis und Werkzeuge.
///
/// ⚑ Nur im Agentenmodus. Im Chat gibt es keine Werkzeuge, und eine
/// Zeile, die dort „keine Werkzeuge" sagt, waere eine Warnung ohne
/// Gegenstand.
async function reichweite_zeichnen() {
  const w = $("reichweite");
  w.replaceChildren();
  if (!offen || offen.modus !== "agent") return;
  let r;
  try {
    r = await invoke("werkzeuge", { wurzel: prozesspfad() });
  } catch (f) {
    r = { wurzel: null, kiste: "", namen: [] };
  }

  // ⚑ **Der Wert ist der Knopf** (Auftrag des Projektinhabers,
  // 2026-09-15). Vorher stand neben Pfad und Kiste je ein beschrifteter
  // Knopf („Verzeichnis wechseln", „andere Werkzeugkiste"), und die
  // Leiste ist dafuer zu schmal: Beide wurden abgeschnitten. Statt sie
  // zu kuerzen, bis die Beschriftung nichts mehr sagt, traegt jetzt die
  // Angabe selbst die Handlung. Was sie tut, steht im `title`.
  //
  // ⚑ **Und es bleibt ein `button`**, kein anklickbarer Absatz: Damit ist
  // die Handlung mit der Tastatur erreichbar und wird vorgelesen.
  const kuerzen = (s, n) => (s && s.length > n ? `…${s.slice(-(n - 1))}` : s);
  const abschnitt = (marke, wert, voll, beim_klick) => {
    const l = document.createElement("p");
    l.className = "reichweitezeile reichweitemarke";
    l.textContent = t(marke);
    w.append(l);
    const k = document.createElement("button");
    k.type = "button";
    k.className = "reichweitewert blank";
    k.textContent = wert;
    k.title = voll;
    k.addEventListener("click", beim_klick);
    w.append(k);
  };

  // 1. Der Einhaengepfad, ein Klick waehlt einen anderen.
  abschnitt(
    "reichweite.pfadmarke",
    r.wurzel ? kuerzen(r.wurzel, 34) : t("reichweite.kein"),
    `${r.wurzel || t("reichweite.hinweis")}\n${t("reichweite.wechseln")}`,
    verzeichnis_wechseln,
  );

  // 2. Die Werkzeugkiste, ein Klick schaltet weiter.
  abschnitt(
    "reichweite.kistemarke",
    r.kiste || "?",
    `${t("reichweite.kistehinweis")}\n${t("reichweite.andereKiste")}`,
    kiste_wechseln,
  );

  // ⚑ **Die Werkzeugliste steht nicht mehr hier**, sondern hinter einem
  // Knopf in den Einstellungen: In der Leiste war sie eine Wand aus
  // Namen, die bei jedem Zeichnen mitlief und die zwei Angaben
  // darueber erschlug, die man wirklich braucht.
}

/// **Verzeichnis wechseln** ueber den Ordnerdialog des Systems, dann als
/// `agent.wurzel` setzen (auf macOS ist die Auswahl zugleich die Freigabe).
async function verzeichnis_wechseln() {
  try {
    // ⚑ **Der Dialog beginnt dort, wo der Prozess gerade arbeitet**, und
    // nicht bei einer Einstellung, die ein anderer Prozess gesetzt hat.
    const r = await invoke("werkzeuge", { wurzel: prozesspfad() });
    const start = prozesspfad() || r.wurzel || "";
    const gewaehlt = await invoke("ordner_waehlen", { titel: t("reichweite.wechseln"), start });
    if (!gewaehlt) return;
    // ⚑ **Gespeichert wird am Prozess und nicht in den Einstellungen**
    // (Auftrag des Projektinhabers, 2026-09-15).
    //
    // 📌 Vorher schrieb dieser Klick `agent.wurzel` in die Ablage, also
    // **fuer alle Prozesse**: Wer fuer einen Auftrag einen anderen Ordner
    // waehlte, sah ihn danach in jedem anderen Prozess auch, und der
    // Wechsel zurueck war eine zweite Ordnerwahl. Der Ordner gehoert dem
    // Auftrag, nicht dem Programm.
    if (!offen) return;
    offen.wurzel = gewaehlt;
    sichern();
    reichweite_zeichnen();
    kontext_holen();
  } catch (f) {
    melden(t("reichweite.fehler", f));
  }
}

/// **Andere Werkzeugkiste**: schaltet `agent.werkzeuge` der Reihe nach
/// weiter. Der angezeigte Name ist der der aufgeloesten Kiste (Base,
/// Advanced), also der des gewaehlten Ordners.
/// ⚑ **Die Kiste wird als Ordner gewaehlt** (Auftrag des
/// Projektinhabers, 2026-09-15). Vorher schaltete ein Klick die
/// Aufzaehlung `automatisch`, `base`, `advanced` weiter, und damit war
/// nur erreichbar, was mitgeliefert ist. Jetzt waehlt der Klick einen
/// Ordner; sein Name ist der Name der Kiste, und was als Manifest darin
/// liegt, bekommt das Modell.
///
/// ⚠️ **Die eingebauten Werkzeuge haengen weiter an `agent.werkzeuge`**,
/// nicht am Ordner. Ein Ordnername ist keine Erlaubnis: Sonst bekaeme
/// jeder `run_command`, der seinen Ordner `advanced` nennt.
async function kiste_wechseln() {
  try {
    // ⚑ **Die Wahl geht dort auf, wo die Kisten liegen** (Meldung des
    // Projektinhabers, 2026-09-15). 📌 Hier stand der eingestellte
    // Kistenordner, und der ist per Vorgabe leer: Dann bekam der Dialog
    // keinen Startort und ging beim zuletzt gewaehlten auf, also beim
    // Einhaengepfad. Den Ort nennt jetzt die Kiste selbst.
    const r = await invoke("werkzeuge", { wurzel: prozesspfad() });
    const start = r.kistenheimat || "";
    const gewaehlt = await invoke("ordner_waehlen", {
      titel: t("reichweite.andereKiste"),
      start,
    });
    if (!gewaehlt) return;
    await invoke("setzen", { feld: "agent.kistenordner", wert: gewaehlt });
    reichweite_zeichnen();
  } catch (f) {
    melden(t("reichweite.fehler", f));
  }
}

// --- Der Kontext am Eingabefeld (2026-09-14) -----------------------------
//
// ⚑ **Der Balken steht dort, wo der Kontext voll wird**: am Eingabefeld.
// Ein Klick oeffnet, was sich damit tun laesst, und nichts geschieht ohne
// diesen Klick. Gezaehlt und verdichtet wird im Ruecken, an derselben
// Stelle wie `/context` und `/compress` in der Konsole.

/// **Das Modellgespraech je Gespraech, nur im Speicher des Fensters.**
///
/// 📌 **Nicht im Browserspeicher.** Die erste Fassung legte es als Feld am
/// Gespraech ab, samt jeder Werkzeugantwort in voller Laenge; ein Werkzeug,
/// das eine grosse Datei liest, haette den Speicher von wenigen Megabyte
/// mit einem Auftrag gefuellt. Abgelegt wird nur die Zusammenfassung,
/// falls verdichtet wurde, und ab welchem Beitrag sie gilt.
const kontexte = new Map();

/// **Die Nachrichten, die das Modell vom Gespraech sieht.**
///
/// ⚑ **Nach einem Neustart aus den Beitraegen hergeleitet**: die
/// Zusammenfassung, falls es eine gibt, dann Auftraege und Antworten ab dem
/// Beitrag, bei dem sie entstand. Die Werkzeugschritte davon sind dann
/// nicht mehr dabei, das Gespraech schon. Fehlermeldungen und Hinweise
/// gehen nicht hinein, denn die hat das Modell nie gesagt.
function kontext_von(g) {
  if (!g) return [];
  if (kontexte.has(g.id)) return kontexte.get(g.id);
  const vorn = g.zusammenfassung ? [{ role: "user", content: g.zusammenfassung }] : [];
  return vorn.concat(
    g.beitraege
      .slice(g.verdichtet_bei || 0)
      // ⚑ **Was das Modell sieht, ist `modelltext`, wenn es einen gibt.**
      // Eine angehaengte Datei steht dort als Zeile; im Fenster steht
      // statt dessen eine Karte.
      .filter((b) => !b.fehler && !b.laufend && (b.text || b.modelltext) && (b.von === "nutzer" || b.von === "modell"))
      .map((b) => ({ role: b.von === "modell" ? "assistant" : "user", content: b.modelltext || b.text })),
  );
}

/// Merkt sich das Modellgespraech; eine neue Zusammenfassung wird mit dem
/// Beitrag abgelegt, ab dem sie gilt.
function kontext_merken(g, nachrichten, zusammenfassung, ab) {
  kontexte.set(g.id, nachrichten);
  if (zusammenfassung && zusammenfassung !== g.zusammenfassung) {
    g.zusammenfassung = zusammenfassung;
    g.verdichtet_bei = ab;
  }
}

function kontext_zeichnen(k) {
  const knopf = $("kontextbalken");
  if (!k) {
    knopf.hidden = true;
    return;
  }
  knopf.hidden = false;
  // ⚑ Ein normales Zeichnen loescht die gruene Verdichtungsanzeige: Sie
  // gilt nur unmittelbar nach dem Verdichten (siehe `kontext_verdichtet`).
  knopf.classList.remove("verdichtet");
  $("kontextfuellung").style.width = `${Math.min(100, k.prozent)}%`;
  $("kontextzahl").textContent = t("kontext.zahl", k.belegt, k.grenze, k.prozent);
  knopf.title = t("kontext.titel", k.ansage, k.belegt - k.ansage, k.nachrichten);
  // ⚑ Rot ab 90 % (Auftrag des Projektinhabers): erst dann wird es eng.
  knopf.classList.toggle("eng", k.prozent >= 90);
}

/// ⚑ **Gruen und der Stand der Verdichtung**, unmittelbar nach dem
/// Verdichten (Auftrag des Projektinhabers). Der naechste `kontext_zeichnen`
/// nimmt die gruene Anzeige wieder weg.
function kontext_verdichtet(vorher, nachher) {
  const knopf = $("kontextbalken");
  knopf.hidden = false;
  knopf.classList.remove("eng");
  knopf.classList.add("verdichtet");
  const anteil = vorher > 0 ? Math.round((nachher / vorher) * 100) : 0;
  $("kontextfuellung").style.width = `${Math.min(100, anteil)}%`;
  $("kontextzahl").textContent = t("kontext.verdichtet_kurz", vorher, nachher);
}

/// ⚠️ **Nicht waehrend eines Laufs**: Der haelt das Modell, und die Frage
/// wartete bis zu seinem Ende.
async function kontext_holen() {
  if (!geladen || !offen || $("senden").disabled) {
    if (!offen) kontext_zeichnen(null);
    return;
  }
  try {
    kontext_zeichnen(await invoke("kontext", { verlauf: kontext_von(offen), modus: offen.modus, wurzel: prozesspfad() }));
  } catch {
    kontext_zeichnen(null);
  }
}

function kontextwahl_schliessen() {
  $("kontextwahl").hidden = true;
}

$("kontextbalken").addEventListener("click", (e) => {
  const m = $("kontextwahl");
  if (!m.hidden) return kontextwahl_schliessen();
  m.hidden = false;
  // Erst zeigen, dann messen, wie beim Menue am Gespraech.
  const r = e.currentTarget.getBoundingClientRect();
  const h = m.getBoundingClientRect();
  m.style.left = `${Math.max(8, Math.min(r.right - h.width, window.innerWidth - h.width - 8))}px`;
  m.style.top = `${Math.max(8, r.top - h.height - 6)}px`;
  m.querySelector(".menueeintrag")?.focus();
});

for (const eintrag of document.querySelectorAll("#kontextwahl [data-kontext]")) {
  eintrag.addEventListener("click", async () => {
    kontextwahl_schliessen();
    if (eintrag.dataset.kontext === "neu") {
      // ⚑ **Ueber den Knopf in der Leiste und nicht an ihm vorbei**:
      // Angelegt wird an genau zwei Stellen (`ein_eintrag_entsteht_nur_auf_zwei_wege`).
      $("neues-gespraech").click();
      kontext_holen();
      return;
    }
    await kontext_verdichten();
  });
}

async function kontext_verdichten() {
  if (!offen || $("senden").disabled) return;
  const verlauf = kontext_von(offen);
  if (verlauf.length === 0) {
    melden(t("kontext.leer"));
    return;
  }
  const knopf = $("senden");
  knopf.disabled = true;
  $("kontextzahl").textContent = t("kontext.laeuft");
  try {
    // ⚑ **Die Gespraechskennung geht mit** (2026-09-16): Sie gliedert
    // die Mitschnitte im Arbeitsordner. ⚠️ **Sie loescht nichts**: Ein
    // Gespraech aus der Liste zu nehmen ist Aufraeumen, einen Verlauf zu
    // loeschen eine eigene Handlung.
    const r = await invoke("verdichten", {
      verlauf,
      sitzung: (offen && offen.id) || "ohne-gespraech",
    });
    offen.beitraege.push({ von: "hinweis", text: t("kontext.verdichtet", r.vorher, r.nachher) });
    kontext_merken(offen, r.nachrichten, r.zusammenfassung, offen.beitraege.length);
    sichern();
    alles_zeichnen();
    // ⚑ Gruen und der Stand der Verdichtung; bleibt stehen bis zum
    // naechsten Auftrag, der den Balken wieder normal zeichnet.
    knopf.disabled = false;
    kontext_verdichtet(r.vorher, r.nachher);
  } catch (f) {
    melden(t("kontext.fehler", f));
    knopf.disabled = false;
    kontext_holen();
  }
}

// --- Das Menue am Gespraech ---------------------------------------------
//
// ⚑ **Ein Menue fuer alle Zeilen.** Bei dreissig Gespraechen waeren
// dreissig Menues im Baum, von denen neunundzwanzig nie zu sehen sind,
// und jedes Neuzeichnen legte sie erneut an.
//
// ⚑ **Der Loeschknopf am Rand bleibt.** Er ist der schnelle Weg und
// mit der Tastatur erreichbar; ein Rechtsklick ist keines von beidem.
// Das Menue kommt dazu, es ersetzt nichts.
let menue_fuer = null;

function menue_schliessen() {
  menue_fuer = null;
  $("kontextmenue").hidden = true;
}

function menue_oeffnen(g, x, y) {
  const m = $("kontextmenue");
  menue_fuer = g;
  m.hidden = false;
  // 📌 **Erst zeigen, dann messen.** Ein verstecktes Element hat keine
  // Masse; wer vorher misst, rechnet mit null und schiebt das Menue an
  // den Rand.
  const r = m.getBoundingClientRect();
  const rand = 8;
  const lx = Math.min(x, window.innerWidth - r.width - rand);
  const ly = Math.min(y, window.innerHeight - r.height - rand);
  m.style.left = `${Math.max(rand, lx)}px`;
  m.style.top = `${Math.max(rand, ly)}px`;
  m.querySelector(".menueeintrag")?.focus();
}

// ⚑ Alles, was das Menue ueberholt, schliesst es: ein Klick daneben,
//   die Fluchttaste, ein Bildlauf, ein anderes Fenster.
document.addEventListener("pointerdown", (e) => {
  if (menue_fuer && !$("kontextmenue").contains(e.target)) menue_schliessen();
  if (!$("kontextwahl").contains(e.target) && !$("kontextbalken").contains(e.target)) {
    kontextwahl_schliessen();
  }
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && menue_fuer) menue_schliessen();
  if (e.key === "Escape") kontextwahl_schliessen();
});
window.addEventListener("blur", () => {
  menue_schliessen();
  kontextwahl_schliessen();
});
document.addEventListener(
  "scroll",
  () => {
    menue_schliessen();
    kontextwahl_schliessen();
  },
  true,
);

/// Das Gespraech als Markdown, so wie es im Fenster steht.
function als_markdown(g) {
  const kopf = [
    `# ${g.titel}`,
    "",
    `- Modus: ${t(`modus.${g.modus}.name`) || g.modus}`,
    `- Begonnen: ${g.wann}`,
    t("md.beitraege", g.beitraege.length),
    "",
  ];
  const teile = g.beitraege.map((b) => {
    if (b.von === "hinweis") return `> ${b.text}`;
    const wer = b.von === "nutzer" ? "Du" : "Myelith";
    // ⚑ Die Werkzeugschritte eines Agentenlaufs gehoeren mit hinein;
    //   ohne sie ist ein Auftrag nicht nachvollziehbar.
    const schritte = (b.schritte || [])
      .map((z) => `  - ${z.art}: ${z.text}`)
      .join("\n");
    return [`## ${wer}`, "", b.text, schritte, b.fuss ? `\n_${b.fuss}_` : ""]
      .filter(Boolean)
      .join("\n");
  });
  return `${kopf.join("\n")}${teile.join("\n\n")}\n`;
}

async function menue_tat(tat) {
  const g = menue_fuer;
  menue_schliessen();
  if (!g) return;

  if (tat === "loeschen") {
    gespraeche = gespraeche.filter((x) => x.id !== g.id);
    if (offen && offen.id === g.id) offen = gespraeche[0] || null;
    sichern();
    alles_zeichnen();
    melden(t("geloescht", wort(g.modus).eines, g.titel));
    return;
  }

  if (tat === "umbenennen") {
    umbenennen(g);
    return;
  }

  if (tat === "ausgeben") {
    try {
      const wo = await invoke("gespraech_ausgeben", {
        titel: g.titel,
        inhalt: als_markdown(g),
      });
      melden(t("abgelegt", wort(g.modus).eines, wo), "einstellungen");
    } catch (f) {
      // 📌 **Ein fehlender Ordner ist keine Fehlermeldung, sondern eine
      // fehlende Entscheidung.** Wer „kein Ausgabeordner" liest, weiss
      // noch nicht, wo er ihn setzt. Also fuehrt das Fenster dorthin.
      if (String(f).includes("kein-ausgabeordner")) {
        await zum_feld("ausgabe.ordner", t("ausgabe.warum", wort(g.modus).eines));
        return;
      }
      melden(t("ausgabe.fehler", f));
    }
  }
}

/// Den Titel an Ort und Stelle bearbeiten.
///
/// ⚑ **Kein Dialog.** Ein Fenster, das nach einem Namen fragt, nimmt
/// den Blick von der Zeile, um die es geht. Das Feld sitzt genau auf
/// dem Titel; Eingabe uebernimmt, Flucht verwirft, und ein Klick
/// daneben uebernimmt ebenfalls, denn das ist, was ein Mensch erwartet.
function umbenennen(g) {
  const zeile = $("chatliste").querySelector(`[data-id="${g.id}"]`);
  const knopf = zeile?.querySelector(".titel");
  if (!knopf) return;

  const feld = document.createElement("input");
  feld.type = "text";
  feld.className = "titelfeld";
  feld.value = g.titel;
  feld.setAttribute("aria-label", t("umbenennen.label", wort(g.modus).eines));
  knopf.replaceWith(feld);
  feld.focus();
  feld.select();

  let fertig = false;
  const schliessen = (uebernehmen) => {
    if (fertig) return;
    fertig = true;
    if (uebernehmen) {
      const neu = feld.value.trim();
      // 📌 Ein leerer Titel waere eine Zeile ohne Aufschrift. Dann
      //   bleibt der alte.
      if (neu) {
        g.titel = neu;
        sichern();
      }
    }
    alles_zeichnen();
  };
  feld.addEventListener("keydown", (e) => {
    if (e.key === "Enter") schliessen(true);
    if (e.key === "Escape") schliessen(false);
  });
  feld.addEventListener("blur", () => schliessen(true));
}

for (const e of document.querySelectorAll(".menueeintrag")) {
  e.addEventListener("click", () => menue_tat(e.dataset.tat));
}

/// Oeffnet die Einstellungen und hebt ein Feld hervor.
///
/// ⚑ **Mit einer Begruendung daneben und nicht nur mit einem Rahmen.**
/// Ein hervorgehobenes Feld ohne Satz laesst raten, warum man hier
/// steht; der Satz steht in einer Box ueber der Tabelle und
/// verschwindet, sobald das Feld einen Wert hat.
async function zum_feld(name, warum) {
  $("einstellungsseite").hidden = false;
  await einstellungen_zeichnen();
  const kasten = $("feldhinweis");
  kasten.textContent = warum;
  kasten.hidden = false;
  const zeile = $(tabelle_fuer(name)).querySelector(`tr[data-feld="${name}"]`);
  const feld = zeile?.querySelector(`[data-feld="${name}"]`);
  if (zeile) {
    zeile.classList.add("gesucht");
    zeile.scrollIntoView({ block: "center" });
  }
  feld?.focus();
}

/// **Die Liste zeigt nur, was zum gewaehlten Modus gehoert.**
///
/// # 📌 Gemeldet vom Projektinhaber am 2026-09-10
///
/// Vorher standen alle Zeilen in einer Liste, und der Modus war nur an
/// der Klammer im Zeigetext zu erkennen. Wer vom Chat zum Agenten
/// wechselte, sah weiter seine Gespraeche und dazwischen die Prozesse.
/// **Ein Klick darauf wechselte dann stillschweigend den Modus
/// zurueck**, denn der Modus haengt am geoeffneten Eintrag.
///
/// ⚑ **Der Modus ist damit eine Ansicht und keine Eigenschaft der
/// Zeile mehr**, jedenfalls fuer den, der die Leiste liest: Was
/// dasteht, gehoert zu dem, was oben eingerastet ist.
function chats_zeichnen() {
  const w = $("chatliste");
  w.replaceChildren();

  // ⚑ Ueberschrift und Knopf sagen dasselbe Wort wie die Zeilen
  // darunter. Sie stehen im HTML in der Mehrzahl des Chats und werden
  // hier gesetzt, denn welcher Modus gilt, weiss erst das Skript.
  const kopf = document.querySelector(".leistenteil.chats h2");
  if (kopf) kopf.textContent = wort().viele;
  const neu = $("neues-gespraech");
  if (neu) neu.textContent = wort().neu;

  for (const g of gespraeche.filter((g) => g.modus === modus_jetzt())) {
    const zeile = document.createElement("div");
    zeile.className = "chat blank";
    zeile.setAttribute("role", "listitem");
    // ⚑ Die Kennung, damit `umbenennen` seine Zeile wiederfindet.
    zeile.dataset.id = g.id;
    zeile.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      menue_oeffnen(g, e.clientX, e.clientY);
    });
    if (offen && g.id === offen.id) zeile.setAttribute("aria-current", "true");

    const auf = document.createElement("button");
    auf.type = "button";
    auf.className = "titel blank";
    auf.textContent = g.titel;
    // ⚑ **Der volle Titel beim Ueberfahren, und nur er** (Festlegung
    // des Projektinhabers, 2026-09-10). Hier stand `Titel (Agent)`,
    // und die Klammer sagte dasselbe wie der eingerastete Modus zwei
    // Zeilen darueber. **Ein Zeigetext hat eine Aufgabe: das zu
    // zeigen, was die Zeile nicht fassen konnte.**
    auf.title = g.titel;
    auf.addEventListener("click", () => {
      offen = g;
      // ⚑ Der Modus folgt dem, was geoeffnet wird; sonst zeigte die
      // Liste gleich darauf etwas anderes als das, was offen ist.
      modus = g.modus;
      alles_zeichnen();
      kontext_holen();
      // ⚑ Und das Modell folgt mit, eingestellt und nicht geladen.
      modell_dem_gespraech_folgen(g);
    });

    // 📌 **Hier lag ein Loeschknopf am Zeilenrand.** Er ist am
    // 2026-09-09 entfallen, auf Festlegung des Projektinhabers:
    // Geloescht wird ueber das Menue am Rechtsklick, zusammen mit
    // Umbenennen und Ausgeben. Ein zweiter Weg an derselben Zeile ist
    // ein zweiter Ort, an dem dieselbe Entscheidung faellt.

    // 📌 **Hier standen sechs Stilangaben am Element** und nahmen dem
    // Knopf von Hand weg, was `button` ihm gibt. Sie vergassen die
    // Rundung und den Hintergrundfilter, und das war als rundes Feld
    // hinter jedem Titel zu sehen. **Aussehen gehoert ins Stilblatt**;
    // der Rueckbau steht jetzt vollstaendig unter `.blank`.
    zeile.append(auf);
    w.append(zeile);
  }
}

/// Das Ladezeichen am Beitrag: der Spalt in einem Kasten, der sich
/// als Statusmeldung vorlesen laesst.
function laufzeichen_bauen() {
  const l = document.createElement("div");
  l.className = "laeuft";
  l.append(synapse());
  l.setAttribute("aria-label", t("lauf.arbeitet"));
  l.setAttribute("role", "status");
  return l;
}

/// **Das Ladezeichen: ein synaptischer Spalt** (Auftrag des
/// Projektinhabers, 2026-09-24, statt drei Punkten; die Gestalt nach
/// seinen Vorlagen).
///
/// Zwei Endknoepfe als Glocken, die aus einem schmalen Strang aufgehen
/// und sich ueber einen engen Spalt gegenueberstehen. In der linken
/// schwellen Blaeschen an, Botenstoffe schweben hinueber, die Flaechen
/// leuchten auf, wenn etwas ankommt, und zweimal je Zyklus entlaedt sich
/// ein kurzer Blitz quer ueber den Spalt. Die Bewegung steht ganz im
/// Stilblatt (`.spalt`), hier steht nur die Gestalt.
///
/// ⚑ **Gebaut mit `createElementNS` und ohne ein einziges `style`**: Die
/// Sicherheitsregel dieser Oberflaeche erlaubt keine Stile im Dokument.
///
/// ⚑ **Die Klassen stehen ausgeschrieben und nicht zusammengesetzt**: Wer
/// eine Regel im Stilblatt sucht, soll ihr Element hier finden
/// (`jede_regel_hat_ein_element`).
///
/// ⚑ **Es sagt nichts ueber den Fortschritt, und das ist ehrlich**, wie
/// schon die Punkte: Wie lange ein Modell braucht, weiss vorher niemand.
function synapse() {
  const NS = "http://www.w3.org/2000/svg";
  const teil = (name, klasse, werte) => {
    const e = document.createElementNS(NS, name);
    if (klasse) e.setAttribute("class", klasse);
    for (const [k, v] of Object.entries(werte)) e.setAttribute(k, String(v));
    return e;
  };
  // ⚑ Verlaeufe brauchen Kennungen, und die muessen je Zeichen eindeutig
  // sein: Zwei Zeichen mit derselben Kennung teilten sich einen Verlauf,
  // und das zweite verloere ihn, sobald das erste geht.
  synapse.zaehler = (synapse.zaehler || 0) + 1;
  const kennung = (name) => `spalt${synapse.zaehler}-${name}`;
  const verlauf = (art, name, werte, stufen) => {
    const v = teil(art, "", { id: kennung(name), ...werte });
    for (const [versatz, klasse] of stufen) v.append(teil("stop", klasse, { offset: versatz }));
    return v;
  };

  const svg = teil("svg", "spalt", { viewBox: "0 0 96 28", "aria-hidden": "true", focusable: "false" });
  const defs = teil("defs", "", {});
  // Der Koerper: oben Licht, unten Schatten, also gewoelbt.
  defs.append(verlauf("linearGradient", "koerper", { x1: 0, y1: 0, x2: 0, y2: 1 },
    [[0, "licht-oben"], [0.45, "licht-mitte"], [1, "licht-unten"]]));
  // Eine Kugel: Lichtpunkt oben links, zum Rand hin dunkler.
  defs.append(verlauf("radialGradient", "kugel", { cx: 0.5, cy: 0.5, r: 0.5, fx: 0.34, fy: 0.3 },
    [[0, "kugel-hell"], [0.55, "kugel-mitte"], [1, "kugel-rand"]]));
  // Der Schein an der Flaeche zum Spalt.
  defs.append(verlauf("radialGradient", "schein", { cx: 0.5, cy: 0.5, r: 0.5 },
    [[0, "schein-kern"], [1, "schein-rand"]]));
  // ⚑ Die Axone laufen nach aussen weich aus, als gingen sie weiter:
  // Eine harte Kante am Bildrand saehe aus wie ein abgeschnittener Schlauch.
  defs.append(verlauf("linearGradient", "auslauf", { x1: 0, y1: 0, x2: 1, y2: 0 },
    [[0, "auslauf-rand"], [0.18, "auslauf-voll"], [0.82, "auslauf-voll"], [1, "auslauf-rand"]]));
  const maske = teil("mask", "", { id: kennung("maske"), maskUnits: "userSpaceOnUse", x: 0, y: 0, width: 96, height: 28 });
  maske.append(teil("rect", "", { x: 0, y: 0, width: 96, height: 28, fill: `url(#${kennung("auslauf")})` }));
  defs.append(maske);
  svg.append(defs);
  const nerv = teil("g", "", { mask: `url(#${kennung("maske")})` });
  svg.append(nerv);

  // Zwei Nervenenden, jedes ein geschwungenes Axon, das sich zur Glocke
  // weitet. Das rechte ist das Spiegelbild des linken.
  const links = "M0 17.2C7.5 17.2 11.5 9.6 21.5 11.2C27 12.1 31 4.6 44 4.4Q46.4 14 44 23.6C31 23.4 27.5 17.8 21.5 16.9C12 15.5 7.5 22.4 0 22.4Z";
  const glanz_links = "M1.5 18.2C8.2 18 12.2 10.6 21.6 12.2C27.3 13.2 31.4 6 42.6 5.6";
  // Jedes Zahlenpaar ist ein Punkt; gespiegelt wird an der Mitte.
  const spiegeln = (d) => d.replace(/(-?[0-9.]+) (-?[0-9.]+)/g, (_, x, y) => `${(96 - parseFloat(x)).toFixed(1)} ${y}`);
  for (const [d, glanz] of [[links, glanz_links], [spiegeln(links), spiegeln(glanz_links)]]) {
    nerv.append(teil("path", "neuron", { d, fill: `url(#${kennung("koerper")})` }));
    nerv.append(teil("path", "glanzlinie", { d: glanz }));
  }
  svg.append(teil("ellipse", "schein sender", { cx: 44.4, cy: 14, rx: 3.2, ry: 8, fill: `url(#${kennung("schein")})` }));
  svg.append(teil("ellipse", "schein empfaenger", { cx: 51.6, cy: 14, rx: 3.2, ry: 8, fill: `url(#${kennung("schein")})` }));
  svg.append(teil("path", "flaeche sender", { d: "M44 4.4Q46.4 14 44 23.6" }));
  svg.append(teil("path", "flaeche empfaenger", { d: "M52 4.4Q49.6 14 52 23.6" }));
  for (const [x, y, r, klasse] of [[39.8, 9.4, 1.9, "blaeschen v1"], [41.2, 17.6, 1.7, "blaeschen v2"], [36, 13.4, 1.5, "blaeschen v3"]]) {
    svg.append(teil("circle", klasse, { cx: x, cy: y, r, fill: `url(#${kennung("kugel")})` }));
  }
  // Zwei Blitze, jeder ein Zickzack von Flaeche zu Flaeche.
  svg.append(teil("path", "blitz z1", { d: "M45.4 9.2L47.2 11.2L46.4 12.6L48.6 13.8L47.9 15.3L50.8 17.4" }));
  svg.append(teil("path", "blitz z2", { d: "M45.5 18.6L47 16.3L47.9 17.2L49.2 13.4L50 14.1L50.7 10.2" }));
  for (const [y, klasse] of [[8.6, "botenstoff b1"], [14, "botenstoff b2"], [19.4, "botenstoff b3"], [11.2, "botenstoff b4"], [16.8, "botenstoff b5"]]) {
    svg.append(teil("circle", klasse, { cx: 46.2, cy: y, r: 0.8, fill: `url(#${kennung("kugel")})` }));
  }
  return svg;
}

function beitrag_zeichnen(b) {
  const wurzel = document.createElement("div");
  wurzel.className = `beitrag von-${b.von}`;

  // ⚑ **Ein Hinweis ist weder Frage noch Antwort**: Er sagt, dass das
  // Modell ab hier eine Zusammenfassung sieht, und geht nicht ins Modell.
  if (b.von === "hinweis") {
    const p = document.createElement("p");
    p.className = "kontexthinweis";
    p.textContent = b.text;
    wurzel.append(p);
    return wurzel;
  }

  if (b.von === "nutzer") {
    if (b.text) {
      const blase = document.createElement("div");
      blase.className = "blase";
      blase.textContent = b.text;
      wurzel.append(blase);
    }
    // ⚑ **Die Datei steht unter dem Auftrag, als Karte.** Was das Modell
    // dazu sieht, steht in `modelltext` und nicht hier: Ein Pfad und ein
    // Auszug sind eine Auskunft fuer das Modell, keine fuer den Leser.
    for (const a of b.anhaenge || []) {
      const karte = document.createElement("div");
      karte.className = "anhangkarte";
      const name = document.createElement("span");
      name.className = "kartename";
      name.textContent = a.name;
      const mass = document.createElement("span");
      mass.className = "kartemass";
      mass.textContent = `${a.art}, ${menschlich(a.bytes)}`;
      karte.append(name, mass);
      wurzel.append(karte);
    }
    return wurzel;
  }

  // ⚑ **Zwei Klappen je Antwort: das Nachdenken und die Befehle**
  // (Auftrag des Projektinhabers, 2026-09-14). Siehe `bloecke`.
  for (const block of bloecke(b)) wurzel.append(block);
  // ⚑ **Das Ladezeichen steht dort, wo gleich die Antwort steht**, und
  // nicht in einer Zeile am Fensterrand.
  //
  // 📌 **Vorher stand „der Agent faehrt" unter der Eingabe.** Das ist
  // die falsche Stelle: Wer auf eine Antwort wartet, sieht auf den
  // Fleck, an dem sie erscheinen wird, und nicht ans andere Ende des
  // Fensters. Dazu belegte es die Zeile, die jetzt sagt, welches
  // Modell im Speicher liegt.
  //
  // 📌 **Und hier stand `&& !b.text && !schritte.length`**, also „nur
  // solange noch gar nichts da ist". Das war falsch, und der
  // Projektinhaber hat es am selben Tag gemeldet: **Nach einem
  // Werkzeugaufruf rechnet das Modell weiter**, oft eine halbe Minute,
  // und in dieser Zeit stand nichts mehr da. Ein Ladezeichen, das nur
  // den ersten Wartezeitraum abdeckt, deckt genau den ab, in dem
  // ohnehin gleich etwas kommt.
  //
  // ⚑ **Es haengt am Lauf und nicht am Inhalt**, aber es schweigt,
  // **solange das Modell schreibt** (Festlegung des Projektinhabers,
  // 2026-09-24). Wenn Text ankommt, ist der wachsende Text selbst die
  // Auskunft, dass es weitergeht, und ein Zeichen darunter waere doppelt.
  // Es steht, waehrend geladen, nachgedacht oder ein Werkzeug gerufen
  // wird, und kommt nach jedem Werkzeugschritt wieder, denn dann rechnet
  // das Modell ohne sichtbaren Zuwachs weiter (das war Fund 292). Wer
  // `b.schreibt` setzt und loescht, steht in `live_meldung`.
  let laufzeichen = null;
  if (b.laufend && !b.schreibt) {
    // ⚑ Angehaengt wird es **am Ende** dieser Funktion, damit es unter
    // allem steht, was schon da ist: Ein Zeichen ueber dem wachsenden
    // Text saehe aus, als gehoerte es zu etwas Vergangenem.
    laufzeichen = laufzeichen_bauen();
  }

  // 📌 **`t` heisst hier nicht `t`.** Die Uebersetzung heisst so, und
  // eine lokale Bindung desselben Namens verdeckt sie **im ganzen
  // Block**, auch oberhalb ihrer eigenen Zeile: `t("lauf.arbeitet")`
  // weiter oben lief damit in „Cannot access 't' before
  // initialization", und zwar genau dann, wenn ein Lauf laeuft.
  // Gemeldet vom Projektinhaber am 2026-09-11 (Fund 324).
  const koerper = document.createElement("div");
  koerper.className = "antworttext";
  // ⚑ **Waehrend des Laufs schlichter Text, danach gesetzt.**
  //
  // 📌 Eine halb angekommene Marke ist keine Marke: `**` mitten im
  // Strom wuerde als Fettdruck aufblitzen und beim naechsten Token
  // wieder verschwinden. Und ein Text, der sich bei jedem Token neu
  // gliedert, ist unruhig zu lesen. **Der laufende Text ist die
  // Vorschau, der gesetzte das Ergebnis**, genau wie bei der Antwort
  // selbst.
  if (b.laufend || !b.bloecke) {
    koerper.textContent = b.text;
  } else {
    koerper.append(bloecke_zeichnen(b.bloecke));
  }
  wurzel.append(koerper);
  // ⛔️ **Jede Antwort traegt die Marke** (Art. 50 Abs. 1): auch waehrend
  //   sie entsteht, denn auch dann ist sie KI-erzeugt.
  const km = document.createElement("p");
  km.className = "kimarke-antwort";
  km.textContent = KI_MARKE;
  wurzel.append(km);

  if (b.fuss) {
    const f = document.createElement("div");
    f.className = "zeitzeile";
    f.textContent = b.fuss;
    wurzel.append(f);
  }
  // ⚑ **Geaenderte Anhaenge stehen unter der Antwort**, je einer mit
  //    einem Knopf. 📌 Die Liste ist ein Befund ueber die Dateien und
  //    nicht das, was das Modell ueber sich sagt; ein Modell, das
  //    behauptet geaendert zu haben, taucht hier nicht auf.
  if (b.geaendert && b.geaendert.length) {
    const kasten = document.createElement("div");
    kasten.className = "geaenderte";
    const titel = document.createElement("span");
    titel.textContent = t("anhang.geaendert");
    kasten.append(titel);
    b.geaendert.forEach((name) => {
      const knopf = document.createElement("button");
      knopf.className = "anhangknopf";
      knopf.textContent = name;
      knopf.title = t("anhang.speichern");
      knopf.addEventListener("click", async () => {
        try {
          const ziel = await invoke("anhang_herausgeben", { name, wurzel: prozesspfad() });
          // ⚑ Abbrechen ist keine Fehlermeldung wert: Wer den Dialog
          //    schliesst, hat sich entschieden.
          if (ziel) melden(`${t("anhang.gespeichert")} ${ziel}`, "gespraech");
        } catch (f) {
          melden_als_fehler(f);
        }
      });
      kasten.append(knopf);
    });
    wurzel.append(kasten);
  }
  if (laufzeichen) wurzel.append(laufzeichen);
  return wurzel;
}

// ⚑ **Aus der Schrittfolge werden zwei Klappen** (Auftrag des
// Projektinhabers, 2026-09-14): **eine fuer das Nachdenken**, als ein Faden
// ueber den ganzen Lauf fortgesetzt und waehrenddessen Token fuer Token
// wachsend, und **eine fuer die Befehle**, jeder darin einzeln aufklappbar
// mit dem genauen Befehl und der Antwort des Werkzeugs.
//
// 📌 **Bis dahin bekam jede Ueberlegung ihre eigene Klappe** und jede Folge
// von Werkzeugschritten eine weitere (Festlegung vom 2026-09-10, damals
// gegen eine Ueberlegung, die in der Befehlsliste landete). Bei zehn
// Schritten standen zwanzig Klappen untereinander, und die Antwort eines
// Werkzeugs war eine Zeile von 200 Zeichen. **Die Trennung der Ueberlegungen
// bleibt sichtbar**, als Trenner im Faden, und die Zahl in der Ueberschrift
// zaehlt sie.
//
// ⚑ **Die Schlussantwort steht nicht dabei**: Sie ist der Text darunter,
// und zweimal dasselbe zu zeigen ist keine Vollstaendigkeit.

/// Zwischen zwei Ueberlegungen im fortgesetzten Faden.
const TRENNER = "\n\n· · ·\n\n";

/// **Welche Klappen eines Beitrags offen stehen.**
///
/// ⚑ **Neben dem Beitrag und nicht in ihm**: Ein laufender Beitrag wird bei
/// jedem neuen Schritt neu gezeichnet, und eine Klappe, die dabei zufiele,
/// liesse sich waehrend des Laufs nicht lesen. Abgelegt wird der Zustand
/// nicht; nach einem Neustart steht alles zu.
const klappzustaende = new WeakMap();

const klappzustand = (b) => {
  let z = klappzustaende.get(b);
  if (!z) {
    z = { denken: false, befehle: false, befehl: new Set() };
    klappzustaende.set(b, z);
  }
  return z;
};

const klappe = (klasse, ueberschrift, offen, merken) => {
  const d = document.createElement("details");
  d.className = klasse;
  d.open = offen;
  const s = document.createElement("summary");
  s.textContent = ueberschrift;
  d.append(s);
  d.addEventListener("toggle", () => merken(d.open));
  return d;
};

/// **Die Schritte eines Beitrags, gebuendelt**: alle Ueberlegungen in ihrer
/// Reihenfolge und alle Befehle mit ihrer Antwort.
///
/// ⚑ **Befehl und Antwort werden der Reihe nach gepaart.** Live meldet der
/// Ruecken Aufruf und Ergebnis abwechselnd; die Rueckgabe am Ende nennt
/// erst alle Aufrufe einer Modellantwort und dann alle Ergebnisse. Die
/// erste noch offene Antwort gehoert in beiden Faellen zum ersten noch
/// offenen Befehl. Ein Vorhaben (was das Modell vor dem Aufruf schrieb)
/// geht an den naechsten Befehl.
const schritte_buendeln = (schritte) => {
  const denken = [];
  const befehle = [];
  const offen = [];
  let vorhaben = "";
  const neu = (e) => {
    befehle.push({ vorhaben, ...e });
    vorhaben = "";
    return befehle.length - 1;
  };
  for (const z of schritte) {
    if (z.art === "denken") {
      denken.push(z.text);
    } else if (z.art === "plan") {
      vorhaben = vorhaben ? `${vorhaben}\n${z.text}` : z.text;
    } else if (z.art === "aufruf") {
      offen.push(neu({ art: "aufruf", kurz: z.text, voll: z.voll || z.text, antwort: null }));
    } else if (z.art === "ergebnis") {
      const i = offen.shift();
      if (i === undefined) {
        neu({ art: "ergebnis", kurz: t("befehl.ohne_aufruf"), voll: "", antwort: z.text });
      } else {
        befehle[i].antwort = z.text;
      }
    } else if (z.art === "abgelehnt") {
      const [name, ...grund] = z.text.split(": ");
      neu({ art: "abgelehnt", kurz: name, voll: name, antwort: grund.join(": ") });
    } else if (z.art === "unlesbar") {
      neu({ art: "unlesbar", kurz: t("befehl.unlesbar"), voll: z.text, antwort: t("befehl.unlesbar") });
    }
  }
  return { denken, befehle };
};

/// Der fortgesetzte Denkfaden eines Beitrags.
const denkfaden = (b) =>
  (b.schritte || []).filter((z) => z.art === "denken").map((z) => z.text).join(TRENNER);

const denkueberschrift = (b) => {
  const n = (b.schritte || []).filter((z) => z.art === "denken").length;
  return t("denken.mal", n) + (b.laufend && b.denkt ? t("denken.jetzt") : "");
};

const bloecke = (b) => {
  const { denken, befehle } = schritte_buendeln(b.schritte || []);
  const zustand = klappzustand(b);
  const aus = [];
  if (denken.length > 0) {
    const d = klappe("denken", denkueberschrift(b), zustand.denken, (o) => {
      zustand.denken = o;
    });
    const faden = document.createElement("div");
    faden.className = "denktext";
    faden.textContent = denken.join(TRENNER);
    d.append(faden);
    aus.push(d);
  }
  if (befehle.length > 0) {
    const d = klappe("befehle", befehlszeile(befehle, b.laufend), zustand.befehle, (o) => {
      zustand.befehle = o;
    });
    const liste = document.createElement("div");
    liste.className = "befehlsliste";
    befehle.forEach((c, i) => liste.append(befehl_zeichnen(c, i, b.laufend, zustand)));
    d.append(liste);
    aus.push(d);
  }
  return aus;
};

// ⚑ **Die Ueberschrift der Befehle, an einer Stelle.**
//
// 📌 Solange etwas laeuft, steht dort die Verlaufsform: Ein „1 Befehl
// ausgefuehrt" waehrend der Ausfuehrung waere eine Aussage ueber etwas,
// das noch nicht geschehen ist. Das ist derselbe Unterschied wie
// zwischen zugesagt und nachgewiesen.
const befehlszeile = (befehle, laeuft) => {
  const getan = befehle.filter((c) => c.antwort !== null).length;
  if (laeuft && getan < befehle.length) return t("befehl.laufend", getan);
  if (getan === 0) return t("befehl.keine");
  return getan === 1 ? t("befehl.eins") : t("befehl.viele", getan);
};

// ⚑ **Alle Schrittarten an einer Stelle**, und es sind genau die, die
// der Ruecken erzeugt: `zeile_aus` fuer die Rueckgabe, die Meldungen
// fuer den Live-Weg. Eine Art ohne Eintrag bekaeme den Punkt, und das
// saehe aus wie eine Absicht.
const MARKE = {
  plan: "·",
  aufruf: "→",
  ergebnis: "←",
  unlesbar: "!",
  antwort: "=",
  abgelehnt: "⚑",
};

/// **Ein Befehl in der Liste**: zu die Zeile, offen Vorhaben, Befehl und
/// Antwort, und solange die Antwort fehlt, sagt die Stelle das.
const befehl_zeichnen = (c, i, laeuft, zustand) => {
  const d = klappe(`befehl ${c.art}`, "", zustand.befehl.has(i), (o) => {
    if (o) zustand.befehl.add(i);
    else zustand.befehl.delete(i);
  });
  const kopf = d.querySelector("summary");
  const m = document.createElement("span");
  m.className = "marke2";
  m.textContent = MARKE[c.art] || "·";
  kopf.append(m, document.createTextNode(c.kurz));

  const teil = (klasse, ueberschrift, text) => {
    const w = document.createElement("div");
    w.className = `befehlsteil ${klasse}`;
    const k = document.createElement("div");
    k.className = "teilkopf";
    k.textContent = ueberschrift;
    const x = document.createElement("div");
    x.className = "teiltext";
    x.textContent = text;
    w.append(k, x);
    return w;
  };
  if (c.vorhaben) d.append(teil("plan", t("befehl.vorhaben"), c.vorhaben));
  if (c.voll) d.append(teil("befehlstext", t("befehl.befehl"), c.voll));
  if (c.antwort === null) {
    d.append(teil("antwort wartet", t("befehl.antwort"), laeuft ? t("befehl.aussteht") : t("befehl.ohne")));
  } else {
    d.append(teil("antwort", t("befehl.antwort"), c.antwort));
  }
  return d;
};

// --- Markdown zeichnen ---------------------------------------------------
//
// ⚑ **Hier wird nichts zerlegt.** Was ankommt, ist ein Baum aus Text,
// den `myl-client` gebaut hat; dieses Skript setzt daraus Elemente
// zusammen. **Kein `innerHTML`, nirgends**, und das ist die eigentliche
// Zusage: Eine Modellantwort ist Daten, und diese Seite traegt die
// Bruecke zu allen Befehlen des Rueckens. Ein eingeschleuster Satz
// bekommt hier keinen Weg zu `invoke`.
//
// ⚑ **Gehalten von `das_fenster_setzt_niemals_markup`**, denn ein
// Kommentar ueber eine Regel belegt nicht, dass sie gilt.

const bloecke_zeichnen = (bloecke) => {
  const raum = document.createDocumentFragment();
  for (const b of bloecke) raum.append(block_zeichnen(b));
  return raum;
};

const block_zeichnen = (b) => {
  if (b.art === "Ueberschrift") {
    // ⚑ Die Stufe kommt aus der Kiste und wird begrenzt: Ein `h9` gibt
    // es nicht, und ein Modell zaehlt manchmal weiter, als es soll.
    const h = document.createElement(`h${Math.min(Math.max(b.stufe, 1), 6)}`);
    h.className = "mdkopf";
    h.append(teile_zeichnen(b.inhalt));
    return h;
  }
  if (b.art === "Liste") {
    const l = document.createElement(b.geordnet ? "ol" : "ul");
    l.className = "mdliste";
    for (const p of b.punkte) {
      const li = document.createElement("li");
      li.append(teile_zeichnen(p));
      l.append(li);
    }
    return l;
  }
  if (b.art === "Code") {
    const pre = document.createElement("pre");
    pre.className = "mdcode";
    const c = document.createElement("code");
    // ⚑ `textContent` und nicht `innerHTML`: In einem Codeblock steht
    // oft genau das, was als Markup gefaehrlich waere.
    c.textContent = b.text;
    if (b.sprache) c.dataset.sprache = b.sprache;
    pre.append(c);
    return pre;
  }
  if (b.art === "Zitat") {
    const q = document.createElement("blockquote");
    q.className = "mdzitat";
    q.append(teile_zeichnen(b.inhalt));
    return q;
  }
  if (b.art === "Linie") {
    const hr = document.createElement("hr");
    hr.className = "mdlinie";
    return hr;
  }
  if (b.art === "Tabelle") {
    const tab = document.createElement("table");
    tab.className = "mdtabelle";
    const kopf = document.createElement("tr");
    for (const z of b.kopf) {
      const th = document.createElement("th");
      th.append(teile_zeichnen(z));
      kopf.append(th);
    }
    tab.append(kopf);
    for (const zeile of b.zeilen) {
      const tr = document.createElement("tr");
      for (const z of zeile) {
        const td = document.createElement("td");
        td.append(teile_zeichnen(z));
        tr.append(td);
      }
      tab.append(tr);
    }
    return tab;
  }
  const p = document.createElement("p");
  p.className = "mdabsatz";
  p.append(teile_zeichnen(b.inhalt || []));
  return p;
};

const teile_zeichnen = (teile) => {
  const raum = document.createDocumentFragment();
  for (const stueck of teile) {
    if (stueck.art === "Fett") {
      const e = document.createElement("strong");
      e.textContent = stueck.text;
      raum.append(e);
    } else if (stueck.art === "Kursiv") {
      const e = document.createElement("em");
      e.textContent = stueck.text;
      raum.append(e);
    } else if (stueck.art === "Code") {
      const e = document.createElement("code");
      e.className = "mdcodewort";
      e.textContent = stueck.text;
      raum.append(e);
    } else if (stueck.art === "Verweis") {
      // 📌 **Als Text und nicht als Knopf.** Ein angeklickter Verweis
      // fuehrte die Webansicht **aus der Anwendung heraus**; sie im
      // System zu oeffnen braeuchte eine Erlaubnis, die die
      // Erlaubnisliste bewusst nicht hat. Das Ziel steht deshalb
      // sichtbar daneben: Wer hin will, sieht wohin.
      const e = document.createElement("span");
      e.className = "mdverweis";
      e.textContent = stueck.text;
      const ziel = document.createElement("span");
      ziel.className = "mdziel";
      ziel.textContent = ` (${stueck.ziel})`;
      raum.append(e, ziel);
    } else {
      raum.append(document.createTextNode(stueck.text));
    }
  }
  return raum;
};

function gespraech_zeichnen() {
  const w = $("gespraech");
  w.replaceChildren();
  if (!offen || offen.beitraege.length === 0) {
    const m = MODI.find((x) => x.id === modus_jetzt());
    // 📌 **Hier stand der Modusname noch einmal davor** („Chat. Ein
    // Gespraech mit …"). Er steht schon in der Leiste links und im
    // Kopf; ein drittes Mal sagt er nichts und macht aus einem Satz
    // eine Ueberschrift mit Satz. Entfernt auf Wunsch des
    // Projektinhabers, 2026-09-11.
    const p = document.createElement("p");
    p.className = "leerzustand";
    p.textContent = t(`modus.${m.id}.leer`);
    w.append(p);
    return;
  }
  for (const b of offen.beitraege) w.append(beitrag_zeichnen(b));
  w.scrollTop = w.scrollHeight;
}

function alles_zeichnen() {
  modi_zeichnen();
  // ⚑ Ohne `await`: Die Reichweite braucht einen Ruecken-Aufruf, und
  // das Gespraech soll darauf nicht warten. Sie erscheint, wenn sie da
  // ist.
  reichweite_zeichnen();
  chats_zeichnen();
  gespraech_zeichnen();
  terminal_umschalten();
}

// --- Das Terminal -------------------------------------------------------
//
// ⛔️ **Was hier laeuft, laeuft mit den Rechten des Nutzers**, ohne
// Einhaengegrenze und ohne Werkzeugkasten. Der Grund steht bei `MODI`
// und ausfuehrlich im Ruecken vor `terminal_ausfuehren`.

/// Was getippt wurde, fuer die Pfeiltasten.
const terminalverlauf = [];
let terminalzeiger = 0;

/// **Stellt zwischen Gespraech und Terminal um.**
///
/// ⚑ **Eine Ansicht, kein Gespraechstyp.** Das Terminal traegt keinen
/// Verlauf in der Liste links, denn es ist kein Gespraech: Es gibt
/// nichts wieder aufzunehmen.
function terminal_umschalten() {
  const an = modus_jetzt() === "terminal";
  document.body.dataset.terminal = an ? "an" : "aus";
  $("terminal").hidden = !an;
  $("gespraech").hidden = an;
  $("eingabe").hidden = an;
  const leiste = $("anhangleiste");
  if (an && leiste) leiste.hidden = true;
  if (an) {
    if (!$("terminalzeilen").childElementCount) terminal_leertext();
    terminal_prompt_holen();
    $("terminaleingabe").focus();
  }
}

function terminal_leertext() {
  const p = document.createElement("p");
  p.className = "terminalhinweis";
  p.textContent = t("modus.terminal.leer");
  $("terminalzeilen").replaceChildren(p);
}

async function terminal_prompt_holen() {
  try {
    const o = await invoke("terminal_ordner");
    $("terminalordner").textContent = o;
  } catch (f) {
    $("terminalordner").textContent = String(f);
  }
}

/// Haengt einen Block an die Ausgabe.
///
/// ⚑ **`textContent` und nie `innerHTML`.** Eine Befehlsausgabe ist
/// fremder Text; wer sie als Markup einsetzt, laesst jede Datei im
/// Dateisystem in das Fenster hineinschreiben.
function terminal_anhaengen(klasse, text) {
  const z = $("terminalzeilen");
  if (z.firstElementChild?.className === "terminalhinweis") z.replaceChildren();
  const pre = document.createElement("pre");
  pre.className = klasse;
  pre.textContent = text;
  z.append(pre);
  terminal_nach_unten();
  return pre;
}

/// ⚑ **Gerollt wird das ganze Terminal**, nicht die Ausgabe allein: Die
/// Eingabezeile steht unter der letzten Zeile und rollt mit, wie in
/// jedem Terminal.
function terminal_nach_unten() {
  const flaeche = $("terminal");
  flaeche.scrollTop = flaeche.scrollHeight;
}

async function terminal_senden(befehl) {
  const roh = befehl.trim();
  if (!roh) return;
  if (roh === "clear" || roh === "cls") {
    terminal_leertext();
    return;
  }
  terminalverlauf.push(roh);
  terminalzeiger = terminalverlauf.length;
  terminal_anhaengen("terminalbefehl", `${$("terminalordner").textContent} $ ${roh}`);
  const laeuft = terminal_anhaengen("terminallaeuft", t("terminal.laeuft"));
  // ⚑ **Waehrend ein Befehl laeuft, gibt es keine Eingabezeile**, wie
  //   in jedem Terminal: Sie kommt wieder, wenn er fertig ist, und sagt
  //   damit zugleich, dass er fertig ist.
  const form = $("terminalform");
  form.hidden = true;
  try {
    const a = await invoke("terminal_ausfuehren", { befehl: roh });
    laeuft.remove();
    if (a.ausgabe) terminal_anhaengen("terminalausgabe", a.ausgabe.replace(/\n+$/, ""));
    if (a.gekuerzt) terminal_anhaengen("terminalvermerk", t("terminal.gekuerzt"));
    if (a.abgebrochen) terminal_anhaengen("terminalvermerk", t("terminal.abgebrochen"));
    // ⚑ **Der Rueckgabewert nur, wenn er nicht null ist.** Eine Zeile
    //   „Rueckgabewert 0" nach jedem `ls` ist Rauschen; eine nach einem
    //   fehlgeschlagenen Befehl ist die Auskunft, die fehlt, wenn das
    //   Programm nichts geschrieben hat.
    if (a.kode !== 0 && a.kode !== null && a.kode !== undefined) {
      terminal_anhaengen("terminalvermerk", `[Rückgabewert ${a.kode}]`);
    }
    $("terminalordner").textContent = a.ordner;
  } catch (f) {
    laeuft.remove();
    terminal_anhaengen("terminalfehler", String(f));
  }
  form.hidden = false;
  terminal_nach_unten();
}

// --- Meldungen ----------------------------------------------------------

// ⚑ **Jedes Ereignis meldet sich, ausser man steht ohnehin davor.**
// Wer auf der Einstellungsseite zusieht, wie ein Artefakt gebaut wird,
// braucht keine Meldung darueber; wer im Gespraech sitzt, schon. Und
// umgekehrt: Eine fertige Antwort meldet sich nur, wenn man gerade
// woanders ist.
//
// 📌 **Die Maske entscheidet und nicht der Ereignistyp.** Eine Liste
// „diese Ereignisse melden sich immer" liefe auseinander, sobald ein
// Ereignis dazukommt; die Frage „sieht der Nutzer es gerade selbst"
// ist dagegen fuer jedes dieselbe.
function maske() {
  return $("einstellungsseite").hidden ? "gespraech" : "einstellungen";
}

function melden(text, wo) {
  if (wo && wo === maske()) return;
  const k = document.createElement("div");
  k.className = "meldung glas";
  // 📌 Auch hier hiess die lokale Bindung `t` und verdeckte die
  // Uebersetzung; `t("meldung.schliessen")` rief damit ein
  // Absatzelement auf.
  const absatz = document.createElement("p");
  absatz.textContent = text;
  const zu = document.createElement("button");
  zu.className = "rundknopf blank";
  zu.setAttribute("aria-label", t("meldung.schliessen"));
  zu.textContent = "×";
  zu.addEventListener("click", () => k.remove());
  k.append(absatz, zu);
  $("meldungen").append(k);
  // ⚑ Sie geht von selbst, aber langsam: Wer gerade tippt, soll sie
  // noch lesen koennen, wenn er aufsieht.
  setTimeout(() => k.remove(), 20000);
}

// --- Einstellungen ------------------------------------------------------

// ⚑ Eine Ueberschrift IN der Tabelle und nicht daneben. Sie gehoert zu
// den Zeilen darunter, und wer die Tabelle stattdessen in vier
// Tabellen zerlegt, verliert die gemeinsame Spaltenbreite: Jeder
// Bereich haette dann seine eigene, und die Eingabefelder saessen auf
// vier verschiedenen Hoehen.
// ⚑ **Die Seite hat drei Feldtabellen**, und zwischen ihnen stehen die
// Abschnitte, die keine Einstellungen sind: Aktualisierung, Modelle,
// was dieser Rechner hergibt. Die Reihenfolge steht im HTML, hier steht
// nur, welches Feld wohin gehoert.
const TABELLEN = ["felder-oberflaeche", "felder-grenzen", "felder-modell", "felder-agent"];

// ⚑ **Entschieden wird am Namen und nicht an der Ueberschrift.** Ein
// Feldname (`kap.speicher`) ist in jeder Sprache derselbe; eine
// Ueberschrift („Grenzen dieses Rechners") ist es nicht, und eine
// Zuordnung ueber uebersetzten Text bricht beim Sprachwechsel.
const tabelle_fuer = (name) =>
  name.startsWith("oberflaeche.")
    ? "felder-oberflaeche"
    : name.startsWith("kap.")
      ? "felder-grenzen"
      : // ⚑ **Agent und Modell in getrennten Tabellen**, damit die Sinne
        // dazwischen stehen koennen (2026-09-18).
        name.startsWith("agent.")
        ? "felder-agent"
        : "felder-modell";

const bereichszeile = (name) => {
  const tr = document.createElement("tr");
  tr.className = "bereichszeile";
  const th = document.createElement("th");
  th.colSpan = 2;
  th.scope = "colgroup";
  th.textContent = name;
  tr.append(th);
  return tr;
};

// ⚑ Je Feld ein Bedienelement, und Art wie Beschriftung kommen aus der
// Kiste: Sie kennt die Felder, das Fenster zeichnet sie nur.
//
// 📌 **Hier stand der technische Name als Beschriftung**, also
// `kap.beschleuniger` und `agent.bezeugtes` in einer Spalte, die ein
// Mensch liest. Das war kein Deutsch, sondern eine Kennung. Seit dem
// 2026-09-09 traegt jedes Feld einen Titel und einen Satz dazu, und
// beides steht in `myl-client` und nicht hier, damit es die
// Beschriftung nur einmal gibt.
const feldzeile = (f, wert, beim_setzen) => {
  const tr = document.createElement("tr");
  // ⚑ Damit die Seite an ein bestimmtes Feld springen kann. Hier steht
  // weiter der technische Name, denn danach sucht `zum_feld`.
  tr.dataset.feld = f.name;

  const a = document.createElement("td");
  const titel = document.createElement("span");
  titel.className = "feldtitel";
  titel.textContent = f.titel;
  const satz = document.createElement("span");
  satz.className = "feldsatz";
  satz.textContent = f.hinweis;
  a.append(titel, satz);
  // ⚑ **Beim Modellfeld steht die Mindestausstattung darunter**
  // (Festlegung des Projektinhabers, 2026-09-11), kleingedruckt und nur
  // fuer das eingestellte Modell. Die Angabe kommt aus derselben Karte
  // wie in der Wahl; eine zweite Quelle liefe auseinander.
  if (f.name === "modell.artefakt") {
    const hw = modellhardware.get(wert) || "";
    if (hw) {
      const klein = document.createElement("span");
      klein.className = "feldsatz feldhardware";
      klein.textContent = hw;
      a.append(klein);
    }
  }
  // ⚑ Der technische Name geht nicht verloren, er steht nur nicht mehr
  // in der Spalte: Wer `myl setzen` benutzt, braucht ihn, und ein
  // Zeigen auf die Beschriftung gibt ihn her.
  a.title = f.name;

  const b = document.createElement("td");
  let element;
  if (f.art === "Auswahl") {
    // ⚑ **Die Werte kommen aus der Kiste, wie alles andere auch.** Das
    // Fenster zaehlt keine Sprachen auf; es zeichnet, was `f.wahl`
    // hergibt. Wer eine dritte Sprache hinzufuegt, fuegt sie an einer
    // Stelle hinzu.
    element = document.createElement("select");
    for (const w of f.wahl || []) {
      const o = document.createElement("option");
      o.value = w.wert;
      o.textContent = w.titel;
      element.append(o);
    }
    element.value = wert === null || wert === undefined ? "" : String(wert);
    element.addEventListener("change", () => beim_setzen(element.value));
  } else if (f.art === "Budget") {
    element = budgetregler(f, wert, beim_setzen);
  } else if (f.art === "Schalter") {
    element = document.createElement("input");
    element.type = "checkbox";
    element.checked = wert === true || wert === "an" || wert === "true";
    element.addEventListener("change", () => beim_setzen(element.checked ? "an" : "aus"));
  } else {
    element = document.createElement("input");
    element.type = "text";
    element.value = wert === null || wert === undefined ? "" : String(wert);
    element.addEventListener("change", () => beim_setzen(element.value.trim() || "aus"));
  }
  element.dataset.feld = f.name;
  // 📌 **Hier hing ein zweiter Hinweis am Eingabefeld**, „leer oder
  // `aus` loescht die Grenze", nur bei der Feldart Grenze. Er steht
  // jetzt im Satz unter der Beschriftung, zusammen mit allem anderen,
  // was ueber das Feld zu sagen ist. Zwei Orte fuer Auskunft ueber
  // dasselbe Feld waren einer zu viel.

  // ⚑ **Verzeichnisfelder bekommen einen Auswaehler daneben.** Welche
  // das sind, sagt die Kiste ueber `f.ordner`; das Fenster zaehlt die
  // Feldarten nicht selbst auf. Getippt werden darf der Pfad weiter,
  // der Knopf nimmt nur den Zwang weg.
  if (f.ordner) {
    const huelle = document.createElement("div");
    huelle.className = "pfadzeile";
    huelle.append(element, ordnerknopf(f, element, beim_setzen));
    b.append(huelle);
  } else {
    b.append(element);
  }
  tr.append(a, b);
  return tr;
};

// ⚑ **Ein Budget als Schieber**: links null, rechts ohne Grenze, dazwischen
// stufenlos bis `f.bis` (die Zahl kommt aus der Kiste).
//
// ⚑ **Quadratisch und nicht gleichmaessig.** Die Unterschiede, die man
// hoert, liegen bei kleinen Budgets: 32 oder 128 Token Ueberlegung sind
// beim Vorlesen einige Sekunden auseinander, 1800 oder 1900 nicht. Auf
// einer gleichmaessigen Skala laege alles Wichtige im ersten Zehntel;
// so liegt die Mitte bei einem Viertel des Endes.
const BUDGET_STELLEN = 1000;
const budget_aus_stelle = (p, bis) => Math.round(bis * (p / BUDGET_STELLEN) ** 2);
const stelle_aus_budget = (n, bis) =>
  Math.min(BUDGET_STELLEN - 1, Math.round(BUDGET_STELLEN * Math.sqrt(n / bis)));

const budgetregler = (f, wert, beim_setzen) => {
  const huelle = document.createElement("div");
  huelle.className = "budgetzeile";
  const schieber = document.createElement("input");
  schieber.type = "range";
  schieber.min = 0;
  schieber.max = BUDGET_STELLEN;
  // ⚑ **Ohne Wert steht er am rechten Anschlag**: nicht gesetzt heisst
  //   ohne Grenze, wie bei den Reglern der Grenzen.
  const gesetzt = typeof wert === "number";
  schieber.value = gesetzt ? stelle_aus_budget(wert, f.bis) : BUDGET_STELLEN;
  const anzeige = document.createElement("span");
  anzeige.className = "reglerwert";
  const zeigen = () => {
    const p = Number(schieber.value);
    const n = budget_aus_stelle(p, f.bis);
    anzeige.textContent =
      p >= BUDGET_STELLEN ? t("budget.frei") : n === 0 ? t("budget.nicht") : t("budget.token", n);
  };
  zeigen();
  schieber.addEventListener("input", zeigen);
  schieber.addEventListener("change", () => {
    const p = Number(schieber.value);
    beim_setzen(p >= BUDGET_STELLEN ? "aus" : String(budget_aus_stelle(p, f.bis)));
  });
  huelle.append(schieber, anzeige);
  return huelle;
};

// ⚑ Der Knopf, der den Fensterdialog des Betriebssystems oeffnet.
//
// ⚑ **Auf macOS ist die Auswahl zugleich die Freigabe:** Was der
// Nutzer im Dialog waehlt, darf die Anwendung danach lesen und
// schreiben, auch unterhalb von Schreibtisch oder Dokumenten. Ein von
// Hand eingetippter Pfad bekommt dort `ENOENT`, und das sieht aus wie
// ein Fehler des Programms.
const ordnerknopf = (f, feld, beim_setzen) => {
  const k = document.createElement("button");
  k.type = "button";
  k.className = "ordnerknopf";
  k.title = t("ordner.waehlen");
  k.setAttribute("aria-label", t("ordner.waehlenFuer", f.titel));
  k.append(sinnbild(
    "M2.5 6.2V5a1 1 0 0 1 1-1h3.3a1 1 0 0 1 .8.4l.9 1.2H16a1 1 0 0 1 1 1v8.4" +
    "a1 1 0 0 1-1 1H3.5a1 1 0 0 1-1-1z"));
  k.addEventListener("click", async () => {
    k.disabled = true;
    try {
      const gewaehlt = await invoke("ordner_waehlen", {
        titel: f.titel,
        start: feld.value.trim() || null,
      });
      // ⚑ `null` heisst abgebrochen, und ein Abbruch aendert nichts.
      // Ein leerer Wert waere hier das Loeschen der Einstellung, also
      // genau das Gegenteil dessen, was ein Abbruch bedeutet.
      if (!gewaehlt) return;
      feld.value = gewaehlt;
      await beim_setzen(gewaehlt);
    } catch (fehler) {
      $("setzmeldung").textContent = t("fehler", fehler);
    } finally {
      k.disabled = false;
    }
  });
  return k;
};

// 📌 **Ein SVG braucht seinen Namensraum.** `createElement("svg")`
// erzeugt ein HTML-Element mit dem Namen „svg", das nichts zeichnet
// und auch nichts meldet; man sieht nur eine leere Flaeche.
const sinnbild = (d) => {
  const NS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  const pfad = document.createElementNS(NS, "path");
  pfad.setAttribute("d", d);
  svg.append(pfad);
  return svg;
};

// --- Modelle holen und bauen --------------------------------------------

// ⚑ Die Phasennamen heissen im Fenster anders als im Ruecken: dort
// `holen`, hier „Download". Der Ruecken spricht die Sprache des
// Protokolls, das Fenster die des Nutzers.
const PHASEN = { holen: 1, kalibrieren: 2, fertig: 3 };
const PHASENNAME = { holen: "Download", kalibrieren: "Kalibrierung", fertig: "fertig" };
let baut = null;

/// ⚠️ **Der Balken zeigt Phasen und keine erfundene Zahl.** Die
/// Skripte melden keinen Prozentsatz; wer daraus einen macht, hat
/// einen Balken, der bei siebzig Prozent stehenbleibt. Drei Phasen,
/// und daneben steht, was gerade laeuft und wie lange schon.
function baustand_setzen(phase, text, seit) {
  $("baustand").hidden = false;
  const anteil = (PHASEN[phase] || 0) / 3;
  $("balkenteil").style.width = `${Math.round(anteil * 100)}%`;
  const min = Math.floor((Date.now() - seit) / 60000);
  const wie = min < 1 ? "gerade begonnen" : `seit ${min} Minuten`;
  const name = PHASENNAME[phase] || phase;
  $("bauphase").textContent = phase === "fertig" ? "fertig" : `${name}, ${wie}`;
  if (text) {
    const z = $("bauzeilen");
    z.textContent = `${z.textContent}${text}\n`.split("\n").slice(-14).join("\n");
    z.scrollTop = z.scrollHeight;
  }
}

/// Der Knopf, der ein Artefakt loescht.
///
/// ⚑ **Er fragt einmal nach, und zwar an sich selbst.** Ein Artefakt
/// sind Gigabyte und ein Bau von Minuten bis Stunden; ein Klick daneben
/// darf das nicht kosten. Ein eigenes Fenster dafuer waere der
/// schwerere Weg und nimmt den Blick von der Zeile, um die es geht,
/// genau wie beim Umbenennen eines Gespraechs.
///
/// 📌 **Und die Rueckfrage laeuft ab.** Ein Knopf, der dauerhaft auf
/// „Wirklich?" stehen bliebe, waere beim naechsten Blick eine Falle.
const loeschknopf = (b, m) => {
  const ruhe = () => {
    b.textContent = t("loeschen");
    b.className = "";
    b.dataset.gefragt = "";
  };
  ruhe();
  let uhr = null;
  b.addEventListener("click", async () => {
    if (b.dataset.gefragt !== "ja") {
      b.dataset.gefragt = "ja";
      b.textContent = t("loeschen.frage");
      b.className = "warnt";
      clearTimeout(uhr);
      uhr = setTimeout(ruhe, 5000);
      return;
    }
    clearTimeout(uhr);
    b.disabled = true;
    b.textContent = t("loeschen.laeuft");
    try {
      const satz = await invoke("artefakt_loeschen", { schluessel: m.schluessel });
      melden(satz, "einstellungen");
      await katalog_zeichnen();
      await modellwahl_zeichnen();
      await freigabe_zeichnen(await invoke("einstellungen"));
    } catch (f) {
      $("bauhinweis").textContent = t("loeschen.fehler", f);
      b.disabled = false;
      ruhe();
    }
  });
};

async function katalog_zeichnen() {
  const v = await invoke("voraussetzungen");
  const h = $("bauhinweis");
  if (v.fehlt.length) {
    h.textContent = `Bauen geht hier nicht: ${v.fehlt.join("  ")}`;
  } else {
    h.textContent =
      t("bau.hinweis");
  }
  const w = $("katalog");
  w.replaceChildren();
  let liste;
  try {
    liste = await invoke("katalog");
  } catch (f) {
    h.textContent = t("katalog.fehler", f);
    return;
  }
  for (const m of liste) {
    const z = document.createElement("div");
    // ⚑ **Was noch nicht da ist, steht gedaempft da.** Die Liste
    // beantwortet auf einen Blick „was habe ich", und dafuer muessen
    // sich vorhandene und mögliche Modelle unterscheiden, ohne dass man
    // die Knopfbeschriftung lesen muss.
    z.className = m.artefakt_da ? "katalogzeile" : "katalogzeile fehlt";
    const links = document.createElement("div");
    const n = document.createElement("strong");
    n.textContent = m.anzeigename || m.schluessel;
    const d = document.createElement("span");
    d.className = "katalogdaten";
    // ⚑ Groesse und Lizenz stehen dabei, denn beides entscheidet die
    // Frage, ob jemand den Knopf drueckt.
    //
    // 📌 **Das Grundmodell stand hier und steht es nicht mehr**
    // (2026-09-10, Festlegung des Projektinhabers). Es ist eine Angabe
    // zum Modell und gehoert dorthin, wo Angaben zum Modell stehen: in
    // die Modellkarte. In einer Liste, die man ueberfliegt, ist es eine
    // Klammer, die jedes Mal mitzulesen ist.
    //
    // ⚠️ **Verschwunden ist es damit nicht.** Die Grundmodelle stehen
    // unter Apache-2.0, und ein Name ohne Herkunft waere eine
    // Verschleierung; sie steht in `KATALOG.json` bei jedem Eintrag und
    // in `artifacts/MODEL_CARD.md`.
    //
    // 📌 **Hier stand eine einzelne Lizenz, und sie stand am falschen
    // Ding** (Fund 295, gemeldet vom Projektinhaber am 2026-09-10).
    // Die Zeile las sich „Myelith 4B · … · Apache-2.0", also so, als
    // stuende das Artefakt unter Apache-2.0. Unter Apache-2.0 stehen
    // die **Grundgewichte**; das daraus gebaute Artefakt steht unter
    // der Lizenz dieses Repositoriums. **Jede Lizenz steht jetzt in
    // der Klammer hinter der Sache, fuer die sie gilt.**
    d.textContent = [
      m.parameter,
      `Gewichte ${m.gewichte} (${m.lizenz_gewichte})`,
      `Artefakt ${m.artefakt} (${m.lizenz_artefakt})`,
      m.status,
    ]
      .filter(Boolean)
      .join("  ·  ");
    links.append(n, d);

    const b = document.createElement("button");
    if (m.artefakt_da) {
      // 📌 **Hier stand ein gesperrter Knopf „liegt vor".** Er sagte
      // dasselbe wie die Zeile daneben und liess sich nicht druecken:
      // ein Bedienelement, das nichts bedient. Jetzt steht dort das
      // Einzige, was man mit einem vorhandenen Artefakt tun kann und
      // sonst nirgends im Fenster kann.
      loeschknopf(b, m);
    } else {
      b.textContent = t(m.modell_da ? "bau.knopf.nurbauen" : "bau.knopf");
      b.disabled = v.fehlt.length > 0 || baut !== null;
      b.addEventListener("click", () => bauen(m.schluessel, m.anzeigename || m.schluessel));
    }
    z.append(links, b);
    // 📌 **Der Balken wird VERSCHOBEN und nicht neu gebaut.** Er haengt
    // unter dem Modell, das gerade laedt, und sonst nirgends. Ein
    // zweiter Balken je Neuzeichnen haette die schon gelesenen Zeilen
    // des Laufs weggeworfen; `append` verschiebt den vorhandenen
    // Knoten samt Inhalt.
    if (baut === m.schluessel) {
      const stand = $("baustand");
      stand.hidden = false;
      z.append(stand);
    }
    w.append(z);
  }
}

async function bauen(schluessel, name) {
  baut = schluessel;
  const seit = Date.now();
  $("bauzeilen").textContent = "";
  // Erst die Liste neu zeichnen, damit der Balken in der richtigen
  // Zeile haengt, dann fuellen.
  await katalog_zeichnen();
  baustand_setzen("holen", "", seit);
  const ab = await horchen("bau-zeile", (e) => baustand_setzen(e.payload.phase, e.payload.text, seit));
  try {
    const pfad = await invoke("artefakt_bauen", { schluessel });
    baustand_setzen("fertig", `Artefakt: ${pfad}`, seit);
    // ⚑ Die Meldung geht nur raus, wenn der Nutzer NICHT zusieht.
    melden(t("bau.fertig", name), "einstellungen");
  } catch (f) {
    baustand_setzen("fehler", String(f), seit);
    melden(t("bau.fehler", name), "einstellungen");
  } finally {
    ab();
    baut = null;
    // ⚑ Der Balken bleibt stehen, bis der Nutzer die Seite verlaesst:
    // Wer gerade zusieht, will das Ergebnis noch lesen. Erst das
    // naechste Oeffnen der Seite raeumt ihn weg.
    await katalog_zeichnen();
    $("baustand").hidden = false;
    await modellwahl_zeichnen();
  }
}

/// Horcht auf ein Ereignis des Rueckens und gibt das Abmelden zurueck.
// ⚑ **Ein Kanal fuer alles Lebende**, und die Anmeldung steht an einer
// Stelle. Sechs Kanaele hiessen sechs Anmeldungen, und wer eine
// vergisst, verliert eine Art Meldung, ohne dass etwas fehlschlaegt.
horchen("lauf-lebt", (e) => live_meldung(e.payload));

// --- Ausschlag, Stimme und Sprachmodus ---------------------------------

/// Wie viele Balken der Ausschlag hat.
const PEGELBALKEN = 28;

/// ⚑ **Ein Ringspeicher statt einer wachsenden Liste.** Der Ausschlag
/// kommt viele Male je Sekunde; eine Liste, die mitwaechst, waere nach
/// einer Minute Diktat eine Liste mit tausenden Eintraegen.
let pegelwerte = new Array(PEGELBALKEN).fill(0);

/// **Zeigt oder versteckt den Ausschlag ueber dem Eingabefeld.**
function pegel_zeigen(an) {
  const kasten = $("pegel");
  if (!kasten) return;
  kasten.hidden = !an;
  if (!an) return;
  pegelwerte = new Array(PEGELBALKEN).fill(0);
  kasten.textContent = "";
  for (let i = 0; i < PEGELBALKEN; i += 1) kasten.append(document.createElement("span"));
}

/// **Schiebt einen Wert nach und zeichnet.**
function pegel_nachziehen(wert) {
  const kasten = $("pegel");
  if (!kasten || kasten.hidden) return;
  pegelwerte.push(Math.max(0, Math.min(1, Number(wert) || 0)));
  pegelwerte.shift();
  const balken = kasten.children;
  for (let i = 0; i < balken.length; i += 1) {
    // Mindesthoehe, damit die Leiste auch bei Stille eine Leiste bleibt.
    balken[i].style.height = `${8 + pegelwerte[i] * 92}%`;
  }
}

horchen("sinne-pegel", (e) => pegel_nachziehen(e.payload));

/// ⚑ **Ein einziger Tonzusammenhang fuer das ganze Fenster.** Jeder
/// Satz einen neuen zu oeffnen, liefe nach ein paar Dutzend Saetzen in
/// die Grenze des Browsers.
let tonwerk = null;
let tonmesser = null;
/// Die Warteschlange der gesprochenen Stuecke, damit sie in der
/// Reihenfolge klingen, in der sie ankommen. Sie wartet nur aufs
/// Entpacken, nicht aufs Ausklingen: Geplant wird nach der Uhr (siehe
/// `stimme_einplanen`).
let stimmkette = Promise.resolve();
/// Wann, auf der Uhr des Tonzusammenhangs, das zuletzt geplante Stueck
/// ausklingt.
let stimmende = 0;
/// Ob die Schleife fuer das Zeichen schon laeuft.
let schwingt = false;

function tonwerk_holen() {
  if (!tonwerk) {
    tonwerk = new (window.AudioContext || window.webkitAudioContext)();
    tonmesser = tonwerk.createAnalyser();
    tonmesser.fftSize = 256;
    tonmesser.connect(tonwerk.destination);
  }
  return tonwerk;
}

/// **Plant ein Stueck genau hinter das vorige.**
///
/// ⚑ **Nach der Uhr und nicht nach dem Ende des vorigen** (2026-09-25).
/// Seit die Stimme gestroemt kommt, ist ein Satz zwei bis vier Stuecke
/// von rund einer Sekunde. Wer jedes erst startet, wenn das vorige sein
/// Ende meldet, laesst dazwischen die Zeit fuers Melden und Entpacken
/// als Knacken stehen. Hier wird jedes Stueck auf den Zeitpunkt gelegt,
/// an dem das vorige endet, und klingt damit ohne Fuge an.
///
/// Zurueck kommt ein Versprechen, das erfuellt ist, sobald das Stueck
/// geplant ist; die Kette haelt nur die Reihenfolge des Entpackens.
/// Die geplanten Stuecke, damit der Notaus sie anhalten kann.
let geplant = [];

/// ⛔️ **Haelt die Stimme sofort an**: alle geplanten Stuecke, auch die,
/// die erst noch klingen wuerden.
function stimme_anhalten() {
  for (const q of geplant) {
    try {
      q.stop();
    } catch {
      // schon ausgeklungen
    }
  }
  geplant = [];
  if (tonwerk) stimmende = tonwerk.currentTime;
}

function stimme_einplanen(base64) {
  return new Promise((fertig) => {
    let roh;
    try {
      const binaer = atob(base64);
      roh = new Uint8Array(binaer.length);
      for (let i = 0; i < binaer.length; i += 1) roh[i] = binaer.charCodeAt(i);
    } catch (f) {
      fertig();
      return;
    }
    const werk = tonwerk_holen();
    werk.decodeAudioData(
      roh.buffer,
      (puffer) => {
        const quelle = werk.createBufferSource();
        quelle.buffer = puffer;
        quelle.connect(tonmesser);
        // ⚠️ Ein kleiner Vorlauf, falls die Kette hinterherhinkt: Ein
        //   Start in der Vergangenheit schnitte den Anfang ab.
        const anfang = Math.max(werk.currentTime + 0.03, stimmende);
        quelle.start(anfang);
        geplant.push(quelle);
        quelle.onended = () => (geplant = geplant.filter((q) => q !== quelle));
        stimmende = anfang + puffer.duration;
        zeichen_schwingen();
        fertig();
      },
      () => fertig(),
    );
  });
}

/// **Laesst das Zeichen mitschwingen, solange etwas geplant ist.**
///
/// ⚑ **Der Ausschlag kommt aus dem Ton selbst**, nicht aus einer Uhr:
/// Ein Zeichen, das sich nach einem Zeitgeber bewegt, sieht aus wie
/// eines, das mitschwingt, und ist eine Verzierung mit dem Anschein
/// einer Auskunft. Eine einzige Schleife fuer alle Stuecke; sie endet,
/// wenn das zuletzt geplante ausgeklungen ist.
function zeichen_schwingen() {
  if (schwingt) return;
  schwingt = true;
  const daten = new Uint8Array(tonmesser.frequencyBinCount);
  const schritt = () => {
    const zeichen = document.querySelector(".stimmzeichen");
    if (tonwerk.currentTime >= stimmende) {
      schwingt = false;
      if (zeichen) zeichen.style.setProperty("--schwung", "0");
      return;
    }
    tonmesser.getByteTimeDomainData(daten);
    let spitze = 0;
    for (const v of daten) spitze = Math.max(spitze, Math.abs(v - 128) / 128);
    if (zeichen) zeichen.style.setProperty("--schwung", spitze.toFixed(3));
    requestAnimationFrame(schritt);
  };
  requestAnimationFrame(schritt);
}

horchen("sinne-stimme", (e) => {
  stimmkette = stimmkette.then(() => stimme_einplanen(e.payload));
});

/// ⚑ **Im Sprachmodus tritt ein Zeichen an die Stelle des Verlaufs**
/// (Auftrag des Projektinhabers, 2026-09-18). Wer spricht und zuhoert,
/// liest nicht mit. ⚠️ **Er schaltet das Vorlesen mit ein**, denn ein
/// Sprachmodus ohne Stimme waere ein leerer Bildschirm.
function sprachmodus_schalten(an) {
  const buehne = $("sprachbuehne");
  const knopf = $("sprachmodus");
  if (!buehne) return;
  const neu = an === undefined ? buehne.hidden : an;
  buehne.hidden = !neu;
  // ⚑ **Der Verlauf geht weg, nicht nur unter die Buehne.** Ein
  // Vorleser im Ruecken eines mitlaufenden Gespraechs ist Unruhe, und
  // ein verdeckter Verlauf bliebe fuer Vorleseprogramme sichtbar.
  const verlauf = $("gespraech");
  if (verlauf) verlauf.hidden = neu;
  if (knopf) knopf.setAttribute("aria-pressed", neu ? "true" : "false");
  const lautsprecher = $("vorlesen");
  if (neu && !vorlesen_an) {
    vorlesen_an = true;
    if (lautsprecher) lautsprecher.setAttribute("aria-pressed", "true");
  }
  if (neu) {
    sprachtext_setzen("");
    hinweis_setzen("");
    // ⚑ Dasselbe hier: Wer den Sprachmodus einschaltet, will sprechen
    // und nicht warten.
    invoke("stimme_vorwaermen").catch(() => {});
  }
}

/// **Die Zeile unter dem Zeichen**, die sagt, was gerade zu tun ist.
///
/// ⚑ **Ohne Text ist es wieder die Anleitung.** Eine Buehne, auf der
/// nichts steht, sieht aus wie eine, die haengt.
function hinweis_setzen(text) {
  const p = $("sprachhinweis");
  if (p) p.textContent = text || t("sprachmodus.halten");
}

/// Was auf der Buehne steht, waehrend gesprochen wird.
function sprachtext_setzen(text) {
  const p = $("sprachtext");
  if (!p) return;
  // ⚑ Nur das Ende: Eine Buehne ist kein Verlauf.
  const kurz = (text || "").slice(-320);
  p.textContent = kurz;
}

async function horchen(name, fn) {
  const { listen } = window.__TAURI__.event;
  return await listen(name, fn);
}

/// **Die Werkzeuge, die ein Agentenlauf bekaeme, hinter einem Knopf.**
///
/// ⚑ **Sie standen bis zum 2026-09-15 in der Seitenleiste** und liefen
/// dort bei jedem Zeichnen mit: eine Wand aus Namen unter den zwei
/// Angaben, die man wirklich braucht. Hier kosten sie nichts, solange
/// niemand fragt, und wer fragt, bekommt sie vollstaendig statt
/// abgeschnitten.
///
/// ⚑ **Gefragt wird beim Klick, nicht beim Zeichnen.** Der Befehl baut
/// die Einhaengung wirklich; das bei jedem Oeffnen der Einstellungen zu
/// tun waere Arbeit fuer eine Angabe, die selten jemand sehen will.
function werkzeugblock() {
  // ⚑ **Ein Block in der Zelle des Pfadfeldes, keine eigene Zeile**
  // (zweimal gemeldet vom Projektinhaber, 2026-09-15).
  //
  // 📌 **Erster Anlauf: eine eigene Zeile mit `colSpan = 2`.** Die
  // spannte ueber beide Spalten und setzte den Knopf ganz links unter
  // den Erklaertext, also unter die falsche Haelfte.
  //
  // 📌 **Zweiter Anlauf: eine eigene Zeile mit leerer erster Zelle.**
  // Richtige Spalte, aber weit unten, und der Grund steckt in der
  // Tabelle: Die linke Zelle der Kistenzeile traegt 350 Zeichen
  // Erklaerung. Sie macht die **ganze Zeile** so hoch wie ihr Text, das
  // Eingabefeld sitzt oben, und was in der naechsten Zeile folgt,
  // beginnt erst unter dem letzten Satz links. **Eine Nachbarzeile
  // steht nicht unter dem Feld, sondern unter der hoeheren der beiden
  // Spalten.**
  //
  // ⚑ Deshalb haengt der Knopf jetzt **in derselben Zelle** wie das
  // Feld. Damit steht er unter dem Feld, und zwar unabhaengig davon,
  // wie lang der Erklaertext daneben einmal wird.
  const td = document.createElement("div");
  td.className = "werkzeugblock";

  const k = document.createElement("button");
  k.type = "button";
  k.className = "werkzeugknopf";
  k.textContent = t("werkzeuge.zeigen");

  const liste = document.createElement("p");
  liste.className = "werkzeugliste";
  liste.hidden = true;

  k.addEventListener("click", async () => {
    if (!liste.hidden) {
      liste.hidden = true;
      k.textContent = t("werkzeuge.zeigen");
      return;
    }
    try {
      const r = await invoke("werkzeuge", { wurzel: prozesspfad() });
      // ⚑ Ohne eingehaengtes Verzeichnis gibt es keine Dateiwerkzeuge,
      // und das ist eine Auskunft und kein leerer Kasten.
      liste.textContent = r.namen.length
        ? t("werkzeuge.aus", r.kiste || "?", r.namen.length) +
          "\n" +
          r.namen.join("  ")
        : t("reichweite.hinweis");
      liste.hidden = false;
      k.textContent = t("werkzeuge.verbergen");
    } catch (f) {
      liste.textContent = t("fehler", f);
      liste.hidden = false;
      k.textContent = t("werkzeuge.verbergen");
    }
  });

  td.append(k, liste);
  return td;
}

/// **Die Rueckfallzeile**, falls es das Kistenfeld einmal nicht gibt.
///
/// ⚠️ Ein Knopf, der dann gar nicht mehr erschiene, waere schlechter als
/// einer an der zweiten Stelle.
function werkzeugzeile() {
  const tr = document.createElement("tr");
  tr.className = "werkzeugzeile";
  const leer = document.createElement("td");
  const td = document.createElement("td");
  td.append(werkzeugblock());
  tr.append(leer, td);
  return tr;
}

async function einstellungen_zeichnen() {
  const e = await invoke("einstellungen");
  const felder = await invoke("felder");
  for (const id of TABELLEN) $(id).querySelector("tbody").replaceChildren();

  // 📌 **Hier stand bis zum 2026-09-10 eine zweite Zuordnung von
  // Feldnamen auf Werte**, von Hand gepflegt, und sie kannte drei der
  // zwoelf Felder nicht (Fund 280). Ein fehlender Schluessel ist in
  // JavaScript kein Fehler, sondern `undefined`: Der Schalter stand
  // immer aus, die Textfelder immer leer.
  //
  // ⚑ **Die Werte kommen jetzt aus der Kiste**, unter demselben Namen,
  // unter dem auch gesetzt wird. Es gibt die Zuordnung nur noch einmal,
  // und zwar dort, wo der Setzer liegt.
  const wert = e.werte;

  // ⚑ **Die Freigaben stehen nicht in dieser Tabelle.** Ein Regler
  // braucht ein Ende, und das Ende ist, was die Maschine hergibt; das
  // weiss erst der Scan. Sie bekommen deshalb ihren eigenen Abschnitt.
  const gesehen = new Set();
  // ⚑ **Die Werkzeugliste haengt an der Agentenrubrik**, und erkannt
  // wird sie am Feldnamen, nicht an der Ueberschrift: `agent.` ist in
  // jeder Sprache dasselbe, „Agent" nicht zwingend. Dieselbe Begruendung
  // wie bei `tabelle_fuer`.
  // ⚑ **Der Werkzeugknopf haengt an der Kistenzeile und nicht am Ende
  // der Rubrik** (Meldung des Projektinhabers, 2026-09-15). Er
  // beantwortet die Frage, die der Pfad darueber aufwirft: „welche
  // Werkzeuge kommen aus dieser Kiste?" Zwei Zeilen weiter unten steht
  // sie neben etwas anderem und liest sich wie eine eigene Sache.
  //
  // ⚠️ **Die letzte Agentenzeile bleibt als Rueckfall**, falls das Feld
  // einmal anders heisst oder wegfaellt: Ein Knopf, der dann gar nicht
  // mehr erscheint, waere schlechter als einer an der zweiten Stelle.
  let kistenzeile = null;
  let letzte_agentenzeile = null;
  for (const f of felder) {
    if (f.freigabe) continue;
    const koerper = $(tabelle_fuer(f.name)).querySelector("tbody");
    if (!gesehen.has(f.bereich)) {
      gesehen.add(f.bereich);
      koerper.append(bereichszeile(f.bereich));
    }
    const zeile =
      feldzeile(f, wert[f.name], async (neu) => {
        try {
          await invoke("setzen", { feld: f.name, wert: neu });
          $("setzmeldung").textContent = t("gesetzt", f.titel);
          // ⚑ **Die Sprache wirkt sofort und nicht beim naechsten
          // Start.** Wer sie umstellt, will sehen, ob er sie versteht;
          // ein Neustart dazwischen macht aus einer Probe eine
          // Entscheidung.
          //
          // 📌 Und die Seite wird danach neu gezeichnet, denn die
          // Beschriftungen der Felder kommen aus der Kiste und stehen
          // in der alten Sprache da.
          if (f.name === "oberflaeche.sprache") {
            await sprache_setzen(neu);
            await einstellungen_zeichnen();
            return;
          }
          // ⚑ **Dieselbe Regel fuer das Bild:** Wer das
          // Erscheinungsbild umstellt, will es sehen, nicht beim
          // naechsten Start davon lesen.
          if (f.name === "oberflaeche.thema" || f.name === "oberflaeche.schrift") {
            const e = await invoke("einstellungen");
            bild_setzen(e.werte["oberflaeche.thema"], e.werte["oberflaeche.schrift"]);
          }
          await kopf_zeichnen();
        } catch (fehler) {
          $("setzmeldung").textContent = t("fehler", fehler);
        }
      });
    koerper.append(zeile);
    if (f.name === "agent.kistenordner") kistenzeile = zeile;
    if (f.name.startsWith("agent.")) letzte_agentenzeile = zeile;
  }
  if (kistenzeile) {
    // In die Zelle des Pfadfeldes, direkt unter das Feld.
    kistenzeile.querySelector("td:last-child").append(werkzeugblock());
  } else if (letzte_agentenzeile) {
    letzte_agentenzeile.after(werkzeugzeile());
  }
  // ⚑ **Das leere Kistenfeld sagt, was stattdessen gilt** (Meldung des
  // Projektinhabers, 2026-09-15: der Pfad wurde nicht angezeigt). Der
  // Wert bleibt leer, denn er ist nicht gesetzt; der Platzhalter nennt
  // die Kiste, aus der die Werkzeuge wirklich kommen. 📌 Den Wert selbst
  // zu füllen wäre falsch: Dann liesse sich „nicht gesetzt" nicht mehr
  // von „auf die Vorgabe gesetzt" unterscheiden, und die Prüfung
  // `wert_und_setzer_kennen_dieselben_felder` hat genau das gefangen.
  //
  // ⚑ **Dasselbe beim Arbeitsordner** (Meldung des Projektinhabers,
  // 2026-09-15: „der Einhaengepfad ist noch nicht standardmaessig das
  // WORK_DIR"). Er **ist** die Vorgabe, seit es eine gibt; sie war nur
  // nirgends zu sehen, wenn das Feld leer war. Jetzt nennt der
  // Platzhalter den Ordner, in dem der Agent wirklich arbeitet.
  //
  // ⚠️ **Ein gesetzter Pfad gewinnt weiter**, und das ist Absicht: Wer
  // einen Ordner eingetragen hat, meint ihn. Wer zur Vorgabe zurueck
  // will, leert das Feld.
  //
  // ⚑ **Hier ohne den Pfad des Prozesses**, anders als in der Leiste und
  // am Werkzeugknopf: Ein Platzhalter sagt, was **bei leerem Feld** gilt,
  // also die Einstellung oder die Vorgabe. Den Ordner eines einzelnen
  // Prozesses hier zu zeigen hiesse, eine Einstellung mit dem Zustand
  // eines Auftrags zu beschriften, und beim naechsten Prozess stuende
  // etwas anderes da.
  try {
    const r = await invoke("werkzeuge", { wurzel: null });
    const platz = (name, pfad) => {
      const feld = $("felder-agent").querySelector(`tr[data-feld="${name}"] input`);
      if (feld && pfad) feld.placeholder = pfad;
    };
    platz("agent.kistenordner", r.kistenordner);
    platz("agent.wurzel", r.wurzel);
  } catch {
    /* ohne Platzhalter bleibt das Feld eben leer */
  }
  $("pfad").textContent = e.pfad;
  await freigabe_zeichnen(e);
  return e;
}

// --- Was dieser Rechner hergibt -----------------------------------------

const GIB = 1024 * 1024 * 1024;
const gib = (b) => `${(b / GIB).toFixed(1)} GiB`;

// ⚑ Was ein Reglerwert bedeutet, je Einheit. Steht hier einmal, damit
// Beschriftung und Anzeige nicht auseinanderlaufen koennen.
const EINHEIT = {
  Kerne: (n) => (n === 1 ? "1 Kern" : `${n} Kerne`),
  Gib: (n) => `${n} GiB`,
  Prozent: (n) => `${n} %`,
};

async function freigabe_zeichnen(e) {
  const m = await invoke("hardware");
  const ziel = $("regler");
  ziel.replaceChildren();

  const teile = [`${m.hardware.kerne} Kerne`];
  if (m.hardware.speicher_bytes) teile.push(`${gib(m.hardware.speicher_bytes)} Arbeitsspeicher`);
  if (m.hardware.platte) teile.push(t("platte.frei", gib(m.hardware.platte.frei_bytes)));
  for (const r of m.hardware.rechenwerke) teile.push(r.name);
  // ⚑ **Was Myelith heute haelt, steht daneben.** Eine Freigabe ohne
  // die heutige Belegung ist eine Zahl ohne Bezug: Wer 50 GiB freigibt
  // und schon 122 belegt, hat nichts freigegeben, sondern etwas
  // zurueckgenommen.
  let satz = teile.join(", ") + ".";
  if (e.platte_belegt > 0) satz += t("platte.haelt", gib(e.platte_belegt));
  if (e.platte_gehalten > 0) satz += t("platte.reserviert", gib(e.platte_gehalten));
  if (e.platte_belegt > 0) satz += ".";
  $("freigabehinweis").textContent = satz;

  for (const r of m.regler) ziel.append(reglerzeile(r));
}

// ⚑ Ein Regler je Betriebsmittel.
//
// ⚑ **Ganz rechts heisst „ohne Grenze", und dort steht jeder Regler,
// solange niemand ihn bewegt hat** (Auftrag des Projektinhabers,
// 2026-09-16). Der Setzer bekommt dafuer `aus`, und er kennt den
// Unterschied: `aus` **loescht** die Grenze.
//
// 📌 **Bis dahin lag diese Stellung ganz links, bei null.** Sie
// bedeutete dasselbe und las sich wie das Gegenteil: Wer einen Regler
// am linken Anschlag sieht, liest „nichts", nicht „alles".
//
// ⚑ **Die beiden Enden heissen, wie die Kiste sie nennt** (`links`,
// `rechts`), in der eingestellten Sprache. Das Fenster erfindet hier
// kein Wort: Die Konsole zeigt dieselben Regler, und zwei Stellen mit
// je eigenen Worten laufen auseinander.
const reglerzeile = (r) => {
  const zeile = document.createElement("div");
  zeile.className = r.sperrgrund ? "reglerzeile gesperrt" : "reglerzeile";
  zeile.dataset.feld = r.name;

  const kopf = document.createElement("div");
  kopf.className = "reglerkopf";
  const titel = document.createElement("span");
  titel.className = "feldtitel";
  titel.textContent = r.titel;
  const anzeige = document.createElement("span");
  anzeige.className = "reglerwert";
  kopf.append(titel, anzeige);

  const schieber = document.createElement("input");
  schieber.type = "range";
  schieber.min = r.mindestens;
  schieber.max = r.hoechstens ?? r.mindestens;
  // ⚑ **Ohne Wert steht er am rechten Anschlag.** Nicht gesetzt heisst
  // ohne Grenze, und ohne Grenze ist rechts.
  schieber.value = r.wert ?? schieber.max;
  schieber.dataset.feld = r.name;
  schieber.disabled = Boolean(r.sperrgrund) || !r.hoechstens;

  const einheit = EINHEIT[r.einheit] ?? ((n) => String(n));
  const ende = () => Number(schieber.max);
  const zeigen = () => {
    const n = Number(schieber.value);
    if (n === ende()) anzeige.textContent = r.rechts;
    else if (n === Number(schieber.min) && r.links) anzeige.textContent = r.links;
    else anzeige.textContent = einheit(n);
  };
  zeigen();
  schieber.addEventListener("input", zeigen);
  schieber.addEventListener("change", async () => {
    const n = Number(schieber.value);
    try {
      await invoke("setzen", { feld: r.name, wert: n === ende() ? "aus" : String(n) });
      $("setzmeldung").textContent = `${r.titel} gesetzt.`;
      // ⚑ Die Zeile darueber sagt, was gehalten wird, und das aendert
      // sich mit diesem Regler. Ohne dieses Nachzeichnen stuende dort
      // die Zahl von vorhin.
      await freigabe_zeichnen(await invoke("einstellungen"));
    } catch (fehler) {
      $("setzmeldung").textContent = t("fehler", fehler);
    }
  });

  const satz = document.createElement("p");
  satz.className = "feldsatz";
  // ⚑ **Ein gesperrter Regler sagt, was fehlt**, und zwar an sich
  // selbst: im Text darunter und im Titel, den ein Zeigen hervorholt.
  // „Noch nicht verfuegbar" liesse den Leser genauso klug zurueck wie
  // zuvor.
  satz.textContent = r.sperrgrund ? r.sperrgrund : r.hinweis;
  if (r.sperrgrund) {
    zeile.title = r.sperrgrund;
    schieber.title = r.sperrgrund;
  }

  zeile.append(kopf, schieber, satz);
  return zeile;
};

async function kopf_zeichnen() {
  const e = await invoke("einstellungen");
  // ⚑ Die Kurzform steht in der Marke, der ganze Satz im Titel. Wer
  // wissen will, warum sein Agent nichts anfasst, findet den Grund am
  // selben Ding und nicht in einer Anleitung.
  // 📌 **Hier stand die Marke „liest und schreibt" oben rechts.** Sie
  // ist am 2026-09-09 auf Festlegung des Projektinhabers entfallen.
  // ⚑ Die Auskunft geht nicht verloren: Was der Agent anfassen darf,
  // steht ausfuehrlich in der Seitenleiste unter `#reichweite`, und
  // dort steht auch der Pfad dazu. Die Marke war die Kurzform davon an
  // einer zweiten Stelle.
  // ⚑ Auch hier aus `werte` und nicht aus einem eigenen Feld: Es gibt
  // die Zuordnung nur einmal, und zwar in der Kiste.
  // ⚑ **Eine einzige Modellzeile** (Auftrag des Projektinhabers,
  // 2026-09-14): den vollen Ladezustand schreibt `modellzeile_schreiben`;
  // hier wird er nur angestossen, damit ein Wechsel des Artefakts ihn
  // auffrischt. Bis dahin standen der Name hier und der Ladezustand unter
  // der Eingabe, also dasselbe zweimal.
  modellzeile_schreiben();
  return e;
}

// --- Modell -------------------------------------------------------------

let geladen = false;

/// Die Modellwahl fuellen.
///
/// ⚑ Der Netzeintrag steht mit in der Liste und ist gesperrt. Die Wahl
/// zwischen dieser Maschine und dem Netz gehoert hierher: Ein Modell
/// ist ein Modell, ob es hier liegt oder dort gerechnet wird.
async function modellwahl_zeichnen() {
  const w = $("modellwahl");
  let liste;
  try {
    liste = await invoke("modelle");
  } catch (f) {
    melden(t("modelle.fehler", f));
    return;
  }
  const e = await invoke("einstellungen");
  w.replaceChildren();
  // ⚑ Die Namen kommen aus derselben Liste, aus der die Wahl entsteht;
  // eine zweite Zuordnung im Fenster liefe auseinander.
  modellnamen = new Map(liste.map((m) => [m.pfad, m.name]));
  // ⚑ Dasselbe fuer die Mindestausstattung: Sie steht klein unter der
  // Wahl und wechselt mit ihr.
  modellhardware = new Map(liste.map((m) => [m.pfad, m.hardware || ""]));
  for (const m of liste) {
    const o = document.createElement("option");
    o.value = m.pfad;
    o.textContent = m.name;
    // ⚑ **Auch am Eintrag selbst**, nicht nur unter der Liste: Beim
    // Durchgehen der Wahl sieht man sonst nur den Namen, und die Frage
    // „traegt meine Maschine das" stellt sich genau dort.
    if (m.hardware) o.title = m.hardware;
    if (!m.offen) {
      o.disabled = true;
      o.title = m.warum;
    }
    if (m.pfad === e.werte["modell.artefakt"]) o.selected = true;
    w.append(o);
  }
  hardwarezeile_schreiben();
}

/// **Die kleingedruckte Zeile unter der Modellwahl.**
///
/// ⚠️ Leer beim Netzeintrag, und das ist keine Luecke: Dort rechnet
/// eine fremde Maschine, eine Anforderung an die eigene stuende falsch.
function hardwarezeile_schreiben() {
  const z = $("modellhardware");
  if (!z) return;
  z.textContent = modellhardware.get($("modellwahl").value) || "";
}

$("modellwahl").addEventListener("change", async () => {
  const neu = $("modellwahl").value;
  hardwarezeile_schreiben();
  try {
    await invoke("setzen", { feld: "modell.artefakt", wert: neu });
    // 📌 Ein Modellwechsel wirft das geladene weg. Ohne diese Zeile
    // faehrt der naechste Auftrag mit dem alten Modell, waehrend die
    // Anzeige das neue nennt.
    geladen = false;
    // ⚑ Ein Wechsel entlaedt das alte: Es antwortet ohnehin nicht
    // mehr, und sein Speicher waere von da an geschenkt.
    try { await invoke("modell_entladen"); } catch { /* dann eben nicht */ }
    modellstand = null;
    melden(t("modell.gewechselt"));
    await kopf_zeichnen();
    await modellzeile_schreiben();
    reichweite_zeichnen();
  } catch (f) {
    melden(t("modell.wechselfehler", f));
  }
});

async function modell_laden() {
  const knopf = $("laden");
  knopf.disabled = true;
  const vorher = knopf.textContent;
  knopf.textContent = t("modell.laedt");
  try {
    const l = await invoke("modell_laden");
    geladen = true;
    modellstand = { name: l.name, pfad: l.pfad };
    kontext_holen();
    await modellzeile_schreiben(t("modell.ladefrist", l.sekunden));
    ruhe_neu_stellen();
  } catch (f) {
    modellstand = null;
    melden(t("modell.ladefehler", f));
    await modellzeile_schreiben();
  } finally {
    knopf.disabled = false;
    knopf.textContent = vorher;
    await kopf_zeichnen();
  }
}

// --- Senden -------------------------------------------------------------

// --- Die Live-Anzeige ---------------------------------------------------
//
// ⚑ **Warum der Beitrag waechst, statt am Ende dazustehen.**
// Ein 4B-Modell braucht fuer eine Antwort bis zu einer Minute. Wer erst
// am Ende zeigt, zeigt in dieser Zeit ein Fenster, das stillsteht, und
// ein stehendes Fenster sieht aus wie ein abgestuerztes. Dieselbe
// Ueberlegung wie beim Ladeknopf, der sich sperrt und es sagt.
//
// 📌 **Und der laufende Beitrag ist derselbe, der danach dasteht.**
// Er wird nicht ersetzt, sondern fertiggeschrieben: Was live ankam,
// bleibt gespeichert, samt Ueberlegung und Befehlen. Wer ihn ersetzte,
// verloere die Ueberlegung, denn die Rueckgabe traegt sie nicht.

/// Der Beitrag, der gerade waechst, oder `null`.
let laufender = null;

/// Das Element dazu, damit die Token nicht das ganze Gespraech neu
/// zeichnen.
///
/// 📌 **Ein `alles_zeichnen()` je Token waere bei sechshundert Token
/// sechshundert vollstaendige Neuaufbauten**, und die Bildlaufstelle
/// spraenge bei jedem.
let laufendes_element = null;

/// Ob der laufende Auftrag verdichten musste; steht danach in der Fusszeile.
let laufender_verdichtet = false;

async function live_anfangen(modus) {
  laufender_verdichtet = false;
  try {
    const e = await invoke("einstellungen");
    schrittgrenze = e.werte["agent.schritte"] ?? 0;
  } catch {
    schrittgrenze = 0;
  }
  laufender = {
    von: "modell",
    text: "",
    // 📌 **Hier stand ein eigenes Feld `denken`**, und damit gab es
    // genau eine Ueberlegung je Beitrag. Nach einem Werkzeugaufruf
    // faengt das Modell aber neu an zu ueberlegen; die zweite landete
    // dann in der Befehlsliste (gemeldet am 2026-09-10). Ueberlegungen
    // sind jetzt **Schritte** wie die Werkzeuge und stehen damit in der
    // Reihenfolge, in der sie entstanden sind.
    schritte: [],
    laufend: true,
    fuss: modus === "agent" ? t("lauf.agent") : t("lauf.antwort"),
  };
  offen.beitraege.push(laufender);
  alles_zeichnen();
  laufendes_element = $("gespraech").lastElementChild;
}

/// Nimmt eine Meldung des Rueckens auf.
function live_meldung(m) {
  if (!laufender || !laufendes_element) return;
  const w = laufendes_element;

  // ⚑ **Ob gerade nachgedacht wird**, fuer die Ueberschrift des Denkfadens.
  // Jede andere Meldung beendet es; geaendert wird nur die Ueberschrift.
  const dachte = laufender.denkt;
  laufender.denkt = m.art === "Denken";

  // ⚑ **Ob gerade geschrieben wird**, fuer das Ladezeichen: Text laesst
  // es gehen, jede Meldung ueber Denken, Werkzeug oder Schritt holt es
  // zurueck. Eine Verdichtung aendert nichts daran, was das Modell tut.
  const schrieb = laufender.schreibt;
  if (m.art !== "Verdichtet") laufender.schreibt = m.art === "Text";
  if (schrieb && !laufender.schreibt) {
    w.append(laufzeichen_bauen());
  } else if (!schrieb && laufender.schreibt) {
    w.querySelector(":scope > .laeuft")?.remove();
  }
  if (dachte && !laufender.denkt) {
    const s = w.querySelector(".denken > summary");
    if (s) s.textContent = denkueberschrift(laufender);
  }

  if (m.art === "Denken") {
    // ⚑ **Zuwachs geht in den letzten Denkschritt** und von dort in den
    // einen Faden; ohne Neuzeichnen, denn das geschieht je Token. Ist der
    // letzte Schritt keiner, beginnt eine neue Ueberlegung, die Zahl in
    // der Ueberschrift waechst, und dafuer wird neu gezeichnet.
    const letzter = laufender.schritte[laufender.schritte.length - 1];
    if (letzter && letzter.art === "denken") {
      letzter.text += m.text;
      const faden = w.querySelector(".denken > .denktext");
      if (faden) {
        // Mitlaufen, solange der Leser am Ende des Fadens steht.
        const unten = faden.scrollHeight - faden.scrollTop - faden.clientHeight < 24;
        faden.textContent = denkfaden(laufender);
        if (unten) faden.scrollTop = faden.scrollHeight;
      }
      if (!dachte) {
        const s = w.querySelector(".denken > summary");
        if (s) s.textContent = denkueberschrift(laufender);
      }
    } else {
      laufender.schritte.push({ art: "denken", text: m.text });
      live_neu_zeichnen();
    }
  } else if (m.art === "Text") {
    laufender.text += m.text;
    w.querySelector(".antworttext").textContent = laufender.text;
    // ⚑ **Auf der Buehne steht das Ende der Antwort**, nicht ihr Anfang:
    // Wer zuhoert, will sehen, wo das Gesprochene gerade ist.
    sprachtext_setzen(laufender.text);
  } else if (m.art === "Verdichtet") {
    laufender_verdichtet = true;
    melden(t("kontext.verdichtet", m.vorher, m.nachher));
  } else if (m.art === "Schritt") {
    // ⚑ **Der Schritt bekommt keine Zeile, sondern die Fusszeile.**
    // Er zaehlt die Fragen an das Modell und nicht die Taten; als
    // Zeile stuende er zwischen den Werkzeugen und saehe aus wie eines.
    // In der Fusszeile beantwortet er die Frage, die ein Wartender
    // wirklich hat: Wie weit ist er?
    laufender.fuss = `Schritt ${m.nummer} von ${schrittgrenze}`;
    const f = w.querySelector(".zeitzeile");
    if (f) f.textContent = laufender.fuss;
  } else {
    // ⚑ Aufruf, Ergebnis und Abgelehnt landen in derselben Liste: Sie
    // erzaehlen zusammen, was der Agent getan hat, und getrennte
    // Listen zwaengen den Leser, sie im Kopf zu verschraenken.
    const z = zeile_aus_meldung(m);
    if (!z) return;
    laufender.schritte.push(z);
    // ⚑ **Hier wird neu gezeichnet und nicht angehaengt.** Ein
    // Werkzeugschritt kann eine neue Klappe eroeffnen (wenn davor eine
    // Ueberlegung stand), und das laesst sich nicht anhaengen. Es
    // geschieht eine Handvoll Mal je Lauf und nicht je Token.
    live_neu_zeichnen();
  }
  // ⚑ Mitlaufen, aber nur, wenn der Leser ohnehin unten steht: Wer
  // hochgescrollt hat, um etwas nachzulesen, will nicht zurueckgerissen
  // werden.
  const g = $("gespraech");
  if (g.scrollHeight - g.scrollTop - g.clientHeight < 80) g.scrollTop = g.scrollHeight;
}

const zeile_aus_meldung = (m) => {
  if (m.art === "Aufruf") {
    return { art: "aufruf", text: m.argumente ? `${m.name}  ${m.argumente}` : m.name, voll: m.voll };
  }
  if (m.art === "Ergebnis") return { art: "ergebnis", text: m.text };
  if (m.art === "Abgelehnt") return { art: "abgelehnt", text: `${m.name}: ${m.grund}` };
  return null;
};

/// Wie viele Schritte der Agent hoechstens hat.
///
/// ⚑ **Aus den Einstellungen und nicht geraten.** Ein „Schritt 3" ohne
/// das Ganze sagt nichts darueber, ob es gleich zu Ende ist.
let schrittgrenze = 0;

/// Zeichnet den laufenden Beitrag neu, ohne das ganze Gespraech.
///
/// 📌 **Ein `alles_zeichnen()` je Token waeren bei sechshundert Token
/// sechshundert vollstaendige Neuaufbauten**, und die Bildlaufstelle
/// spraenge bei jedem. Hier wird genau ein Element ersetzt, und nur
/// dann, wenn sich die **Gliederung** aendert.
function live_neu_zeichnen() {
  if (!laufender || !laufendes_element) return;
  const neu = beitrag_zeichnen(laufender);
  laufendes_element.replaceWith(neu);
  laufendes_element = neu;
}

async function live_beenden() {
  if (laufender) {
    laufender.laufend = false;
    // ⚑ **Jetzt erst gliedern.** Waehrend des Laufs war der Text die
    // Vorschau; hier steht er fest, und die Kiste macht daraus die
    // Bloecke, die das Fenster zeichnet.
    laufender.bloecke = await bloecke_holen(laufender.text);
  }
  laufender = null;
  laufendes_element = null;
}

/// Holt die Gliederung eines Textes aus der Kiste.
///
/// 📌 **Ein Fehlschlag ist kein Grund, den Text zu verlieren.** Ohne
/// Bloecke zeichnet `beitrag_zeichnen` ihn schlicht, und das ist
/// schlechter aussehend und nicht falsch.
async function bloecke_holen(text) {
  if (!text) return null;
  try {
    return await invoke("markdown", { text });
  } catch {
    return null;
  }
}

/// ⚑ **Traegt die Gliederung fuer Gespraeche nach, die es schon gab.**
///
/// Was vor dieser Fassung gespeichert wurde, hat keine Bloecke. Statt
/// beim Zeichnen nachzufragen (das machte jede Zeichnung
/// unterbrechbar), geschieht es einmal beim Start.
async function bloecke_nachtragen() {
  let etwas = false;
  for (const g of gespraeche) {
    for (const b of g.beitraege) {
      if (b.von !== "modell" || b.bloecke || b.fehler || !b.text) continue;
      b.bloecke = await bloecke_holen(b.text);
      etwas = true;
    }
  }
  if (etwas) {
    sichern();
    alles_zeichnen();
  }
}

/// **Eine Datei anhaengen**, ueber den Knopf oder durch Ablegen.
///
/// ⚑ **Der Anhang ist ein Beitrag des Nutzers und kein Hinweis.** Nur
/// was als `nutzer` oder `modell` im Verlauf steht, geht an das Modell
/// (siehe `kontext_von`); ein Hinweis waere sichtbar und unwirksam,
/// also genau die Sorte Halbheit, die spaeter niemand erklaeren kann.
///
/// ⚑ **Und die Datei selbst bleibt draussen.** Ins Gespraech geht eine
/// Zeile mit Ort und Art; gelesen wird mit den Werkzeugen. **Ein Anhang
/// soll den Kontext erreichbar machen und nicht fuellen.**
/// ⚑ **Was an der Eingabe haengt und noch nicht abgeschickt ist.**
///
/// ⛔️ **Vorher wurde eine Datei sofort ein eigener Beitrag** und war
/// damit weg, bevor jemand etwas dazu schreiben konnte (gemeldet vom
/// Projektinhaber am 2026-09-18). **Eine Datei ist ein Teil der Frage**,
/// die man gerade formuliert; ins Gespraech gehoert sie, wenn die Frage
/// abgeschickt ist.
let anhaenge_offen = [];

/// Bytes, wie ein Mensch sie liest. Dieselbe Regel wie in `myl-senses`.
function menschlich(bytes) {
  return bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} kB`;
}

/// **Die Plaettchen ueber dem Eingabefeld.**
function anhangleiste_zeichnen() {
  const leiste = $("anhangleiste");
  if (!leiste) return;
  leiste.textContent = "";
  leiste.hidden = anhaenge_offen.length === 0;
  anhaenge_offen.forEach((a, i) => {
    const chip = document.createElement("span");
    chip.className = a.sieht ? "anhangchip sieht" : "anhangchip";
    const name = document.createElement("span");
    name.className = "chipname";
    name.textContent = a.name;
    const mass = document.createElement("span");
    mass.className = "chipmass";
    mass.textContent = a.sieht ? t("anhang.wirdangesehen") : `${a.art}, ${menschlich(a.bytes)}`;
    const weg = document.createElement("button");
    weg.type = "button";
    weg.className = "weg";
    weg.textContent = "×";
    weg.title = t("anhang.weg");
    weg.setAttribute("aria-label", t("anhang.weg"));
    // ⚑ **Abwaehlen geht, solange nicht abgeschickt ist.** Die Kopie im
    // Anhangordner bleibt liegen; `myl anhaenge` raeumt sie weg.
    weg.addEventListener("click", () => {
      anhaenge_offen.splice(i, 1);
      anhangleiste_zeichnen();
    });
    chip.append(name, mass, weg);
    leiste.append(chip);
  });
}

async function anhang_hinzufuegen(pfad) {
  if (!pfad) return;
  try {
    const a = await invoke("anhang_aufnehmen", { pfad, wurzel: prozesspfad(), modus: modus_jetzt() });
    const eintrag = {
      name: a.name,
      pfad: a.pfad,
      art: a.art,
      bytes: a.bytes,
      nachricht: a.nachricht,
      sieht: !!a.ansehen,
    };
    anhaenge_offen.push(eintrag);
    anhangleiste_zeichnen();
    // ⚑ **Erst das Plaettchen, dann das Hinsehen.** Ein Sehmodell
    // braucht Sekunden bis Minuten; die Datei haengt aber sofort.
    if (eintrag.sieht) {
      try {
        const gesehen = await invoke("anhang_ansehen", { pfad: a.pfad, wurzel: prozesspfad() });
        eintrag.nachricht = `${a.nachricht} ${t("anhang.angesehen")}\n\n${gesehen}`;
      } catch (f) {
        // ⚠️ **Der Grund geht an den Menschen und nicht ins Gespraech.**
        melden_als_fehler(f);
      }
      eintrag.sieht = false;
      anhangleiste_zeichnen();
    }
  } catch (f) {
    melden_als_fehler(f);
  }
}

/// **Ein Fehler der Sinne geht ins Gespraech, sichtbar.**
///
/// ⚑ **Und nicht in eine Konsole**, die niemand offen hat. ⚠️ Ein
/// Gespraech wird dafuer nicht eroeffnet: Wer nichts angefangen hat,
/// bekommt keine Fehlermeldung als ersten Beitrag.
function melden_als_fehler(f) {
  if (!offen) return;
  offen.beitraege.push({ von: "modell", text: String(f), fuss: "", fehler: true });
  alles_zeichnen();
}

/// **Was die Sprechtaste gerade tut**, an ihr selbst.
function stand_sagen(text) {
  const taste = $("sprechtaste");
  if (taste) taste.title = text || t("knopf.sprechtaste");
}

/// ⚑ **Ob vorgelesen wird, gilt fuer dieses Fenster und nicht darueber
/// hinaus.** Ein Client, der beim naechsten Start ungefragt spricht,
/// spricht im falschen Raum.
let vorlesen_an = false;
/// Ob gerade aufgenommen wird, damit zwei Ereignisse nicht zweimal enden.
let nimmt_auf = false;

/// **Die Sinne, die Sprechtaste und die Stimme.**
async function sinne_verdrahten() {
  const taste = $("sprechtaste");
  const lautsprecher = $("vorlesen");
  const buehnenknopf = $("sprachmodus");

  const stand_holen = async () => {
    let stand;
    try {
      stand = await invoke("sinne_stand");
    } catch (f) {
      return null;
    }
    // ⚑ **Ein Knopf, der nur erklaeren kann, warum er nicht geht, ist
    // kein Knopf.** Was nicht eingerichtet ist, steht auf der
    // Einstellungsseite, nicht neben dem Eingabefeld.
    if (taste) taste.hidden = !stand.zuhoeren;
    if (lautsprecher) lautsprecher.hidden = !stand.sprechen;
    if (buehnenknopf) buehnenknopf.hidden = !stand.sprechen;
    if (!stand.sprechen && vorlesen_an) {
      vorlesen_an = false;
      if (lautsprecher) lautsprecher.setAttribute("aria-pressed", "false");
    }
    sinne_zeigen(stand);
    return stand;
  };

  // ⚑ **Zwei Knoepfe, ein Verhalten.** Unten in der Reihe und gross auf
  // der Sprachbuehne; wer im Sprachmodus zum Reden woanders hinzeigen
  // muesste, haette keinen Sprachmodus.
  for (const knopf of [taste, $("buehnentaste")]) {
    if (!knopf) continue;
    // ⚑ **Gedrueckt halten, reden, loslassen.** `pointerleave` beendet
    // ebenfalls: Wer mit gedruecktem Knopf wegfaehrt, bekaeme sonst eine
    // Aufnahme, die nie endet.
    knopf.addEventListener("pointerdown", async () => {
      if (nimmt_auf) return;
      // ⚑ **Nicht aufnehmen, waehrend eine Antwort laeuft.** Sonst
      // redet man in eine Sperre hinein und merkt es erst beim
      // Loslassen.
      if (auftrag_laeuft) {
        melden(t("lauf.laeuftschon"));
        return;
      }
      try {
        await invoke("sprechtaste_start");
        nimmt_auf = true;
        for (const k of [taste, $("buehnentaste")]) if (k) k.classList.add("nimmt_auf");
        pegel_zeigen(true);
        stand_sagen(t("sinne.hoert"));
        hinweis_setzen(t("sinne.hoert"));
      } catch (f) {
        melden_als_fehler(f);
      }
    });
    const loslassen = async () => {
      if (!nimmt_auf) return;
      nimmt_auf = false;
      for (const k of [taste, $("buehnentaste")]) if (k) k.classList.remove("nimmt_auf");
      pegel_zeigen(false);
      stand_sagen(t("sinne.schreibtmit"));
      hinweis_setzen(t("sinne.schreibtmit"));
      try {
        const text = await invoke("sprechtaste_ende");
        stand_sagen("");
        hinweis_setzen("");
        // ⚑ **Der Text erscheint in der Eingabezeile** und wird nicht
        // sofort abgeschickt: Wer sich verhoert hat, soll ausbessern
        // koennen. ⚠️ **Im Sprachmodus ist das anders**, dort sieht
        // niemand auf die Eingabezeile, und ein Text, der dort liegen
        // bleibt, waere ein Gespraech, das nicht weitergeht.
        if (text && text.trim()) {
          const buehne = $("sprachbuehne");
          if (buehne && !buehne.hidden) {
            await senden(text.trim());
          } else {
            const feld = $("auftrag");
            feld.value = feld.value ? `${feld.value} ${text.trim()}` : text.trim();
            feld_messen();
            feld.focus();
          }
        }
      } catch (f) {
        stand_sagen("");
        hinweis_setzen("");
        melden_als_fehler(f);
      }
    };
    knopf.addEventListener("pointerup", loslassen);
    knopf.addEventListener("pointerleave", loslassen);
    knopf.addEventListener("pointercancel", loslassen);
  }

  if (lautsprecher) {
    lautsprecher.addEventListener("click", () => {
      vorlesen_an = !vorlesen_an;
      lautsprecher.setAttribute("aria-pressed", vorlesen_an ? "true" : "false");
      // ⚑ **Jetzt laden, nicht bei der ersten Antwort.** Das
      // Sprechmodell braucht rund achtzehn Sekunden; wer sie vor die
      // erste Antwort legt, wartet doppelt.
      if (vorlesen_an) invoke("stimme_vorwaermen").catch(() => {});
    });
  }
  if (buehnenknopf) {
    buehnenknopf.addEventListener("click", () => sprachmodus_schalten());
  }

  const waehlen = $("stimme-waehlen");
  const einwilligung = $("stimme-einwilligung");
  if (waehlen && einwilligung) {
    // ⛔️ Der Knopf geht erst mit der Einwilligung; der Befehl prueft sie
    //   noch einmal.
    einwilligung.addEventListener("change", () => (waehlen.disabled = !einwilligung.checked));
    waehlen.addEventListener("click", async () => {
      if (!einwilligung.checked) return;
      const gewaehlt = await invoke("datei_waehlen", { titel: t("sinne.stimmewaehlen") });
      if (!gewaehlt) return;
      try {
        sinne_zeigen(await invoke("stimme_setzen", { pfad: gewaehlt, einwilligung: true }));
        einwilligung.checked = false;
        waehlen.disabled = true;
      } catch (f) {
        melden_als_fehler(f);
      }
    });
  }
  // ⛔️ Das Aktionsprotokoll: erst auf den Knopf, neueste zuerst.
  const protokollknopf = $("protokoll-zeigen");
  if (protokollknopf) {
    protokollknopf.addEventListener("click", async () => {
      const [ort, eintraege] = await invoke("protokoll_lesen");
      $("protokollort").textContent = t("protokoll.ort", ort);
      const tafel = $("protokolltafel");
      const rumpf = tafel.querySelector("tbody");
      rumpf.replaceChildren(
        ...eintraege.map((e) => {
          const tr = document.createElement("tr");
          for (const wert of [e.zeit, e.art, e.werkzeug, e.entscheidung, e.ergebnis, `${e.dauer_ms} ms`]) {
            const td = document.createElement("td");
            td.textContent = wert;
            tr.append(td);
          }
          return tr;
        }),
      );
      tafel.hidden = eintraege.length === 0;
      if (eintraege.length === 0) $("protokollort").textContent += ` · ${t("protokoll.leer")}`;
    });
  }

  const weg = $("stimme-weg");
  if (weg) {
    weg.addEventListener("click", async () => {
      sinne_zeigen(await invoke("stimme_entfernen"));
    });
  }
  const einrichten = $("sinne-einrichten");
  if (einrichten) {
    einrichten.addEventListener("click", async () => {
      try {
        const satz = await invoke("sinne_einrichten");
        const mangel = $("sinnesmangel");
        if (mangel) {
          mangel.textContent = satz;
          mangel.hidden = false;
        }
        await stand_holen();
      } catch (f) {
        melden_als_fehler(f);
      }
    });
  }

  await stand_holen();
}

/// **Was auf der Einstellungsseite ueber die Sinne steht.**
function sinne_zeigen(stand) {
  if (!stand) return;
  const zeile = $("sinnestand");
  if (zeile) {
    const teile = [];
    teile.push(`${stand.sehen ? "✓" : "✗"} Sehen`);
    teile.push(`${stand.hoeren ? "✓" : "✗"} Hören`);
    teile.push(`${stand.sprechen ? "✓" : "✗"} Sprechen${stand.sprechweg ? ` (${stand.sprechweg})` : ""}`);
    zeile.textContent = teile.join("   ");
  }
  const name = $("stimmname");
  if (name) name.textContent = stand.probe ? t("sinne.stimmeliegt") : t("sinne.keinestimme");
  const weg = $("stimme-weg");
  if (weg) weg.hidden = !stand.probe;
  // ⚠️ **Der Hinweis steht da, wenn etwas zu sagen ist**, und sonst nicht.
  const hinweis = $("stimmhinweis");
  if (hinweis) {
    hinweis.textContent = stand.hinweis || "";
    hinweis.hidden = !stand.hinweis;
  }
  const mangel = $("sinnesmangel");
  if (mangel && stand.mangel && stand.mangel.length) {
    mangel.textContent = stand.mangel.join("\n");
    mangel.hidden = false;
  }
}

/// Der Knopf und das Ablegen, beide auf demselben Weg.
function anhang_verdrahten() {
  const knopf = $("anhang");
  if (knopf) {
    knopf.addEventListener("click", async () => {
      const gewaehlt = await invoke("datei_waehlen", { titel: "Datei anhängen" });
      await anhang_hinzufuegen(gewaehlt);
    });
  }
  // ⚑ **Ablegen geht ueber das Fensterereignis und nicht ueber HTML5.**
  // Im Webview traegt ein abgelegtes `File` keinen Pfad; Tauri meldet
  // dagegen die echten Pfade. **Ein Anhang ohne Pfad waere ein Anhang,
  // den kein Werkzeug findet.**
  horchen("tauri://drag-drop", async (e) => {
    document.body.classList.remove("ablegen");
    const pfade = (e && e.payload && e.payload.paths) || [];
    for (const p of pfade) await anhang_hinzufuegen(p);
  });
  horchen("tauri://drag-enter", () => document.body.classList.add("ablegen"));
  horchen("tauri://drag-leave", () => document.body.classList.remove("ablegen"));
}

/// ⛔️ **Ob gerade ein Auftrag laeuft.**
///
/// 📌 **Bis zum 2026-09-18 war der abgeschaltete Senden-Knopf die
/// einzige Sperre**, und die Sprechtaste geht an ihm vorbei: Wer im
/// Sprachmodus loslaesst, waehrend die vorige Antwort noch laeuft,
/// startete einen zweiten Lauf. Der erste raeumte beim Ende
/// `laufender` weg, und der zweite lief in
/// `TypeError: null is not an object (evaluating 'laufender.text =
/// a.text')`. Gemeldet vom Projektinhaber.
///
/// ⚑ **Die Sperre gehoert an die Sache und nicht an einen Knopf.**
let auftrag_laeuft = false;

/// **Loest die Anhaenge von der Eingabezeile ab**, fuer genau einen
/// Auftrag.
///
/// ⚑ **Zwei Fassungen desselben Auftrags:** `modelltext` sieht das
/// Modell und traegt die Zeile mit Pfad und Auszug; `text` sieht der
/// Mensch, und darunter steht je Anhang eine Karte.
function anhaenge_abloesen(text) {
  const anhaenge = anhaenge_offen;
  anhaenge_offen = [];
  anhangleiste_zeichnen();
  const modelltext = anhaenge.length
    ? [text, ...anhaenge.map((a) => a.nachricht)].filter(Boolean).join("\n\n")
    : null;
  return { anhaenge, modelltext };
}

async function senden(text) {
  // ⛔️ **Ein Auftrag zur Zeit.** Siehe `auftrag_laeuft`.
  if (auftrag_laeuft) {
    melden(t("lauf.laeuftschon"));
    return;
  }
  auftrag_laeuft = true;
  const { anhaenge, modelltext } = anhaenge_abloesen(text);
  if (!offen) neues_gespraech();
  // ⚑ **Vor dem neuen Beitrag gelesen**: Der Auftrag geht als Auftrag
  // hinein und nicht noch einmal als Teil des Verlaufs.
  const vorher = kontext_von(offen);
  if (offen.beitraege.length === 0) {
    offen.titel = titel_aus(text);
  }
  offen.beitraege.push({ von: "nutzer", text, anhaenge, modelltext });
  // Ab hier gilt eine Zusammenfassung, die dieser Auftrag erzeugt.
  const auftrag_bei = offen.beitraege.length - 1;
  // ⚑ **Womit man arbeitet, steht oben** (Auftrag des Projektinhabers,
  // 2026-09-12). Hier und nicht beim Oeffnen: siehe `nach_oben`.
  nach_oben(offen);
  sichern();
  alles_zeichnen();

  const knopf = $("senden");
  knopf.disabled = true;

  // ⚑ **Auch hier, nicht nur beim Umschalten.** Wer ein Agentengespraech
  // aus der Liste oeffnet, hat den Modus nie gewaehlt und saehe die
  // Warnung sonst nie.
  if (offen.modus === "agent") await agentenwarnung_zeigen();

  try {
    if (!geladen) await modell_laden();
    if (!geladen) throw new Error(t("lauf.nichtgeladen"));

    // ⚑ **Womit dieses Gespraech gefuehrt wurde, bleibt an ihm haengen**
    // (Auftrag des Projektinhabers, 2026-09-15). Gemerkt wird erst, wenn
    // wirklich geladen ist: Ein Gespraech soll auf ein Modell zeigen,
    // mit dem es auch gelaufen ist, und nicht auf eines, das beim
    // Versuch scheiterte.
    try {
      const e = await invoke("einstellungen");
      const artefakt = e.werte["modell.artefakt"] || "";
      if (artefakt && offen.modell !== artefakt) {
        offen.modell = artefakt;
        sichern();
      }
    } catch {
      /* ohne Merkzettel laeuft der Auftrag trotzdem */
    }

    await live_anfangen(offen.modus);

    if (offen.modus === "agent") {
      const a = await invoke("agent_fahren", { auftrag: text, verlauf: vorher, wurzel: prozesspfad() });
      kontext_merken(offen, a.nachrichten, a.zusammenfassung, auftrag_bei);
      kontext_zeichnen(a.kontext);
      // ⚑ **Die Rueckgabe schreibt den laufenden Beitrag fertig und
      // ersetzt ihn nicht.** Sie ist die vollstaendige Fassung des
      // Textes; die Ueberlegung und die Befehle traegt sie nicht, die
      // kamen live und stehen schon da.
      laufender.text = a.antwort || t("antwort.keine");
      // ⚑ **Und die Schrittliste ebenso.** Was live ankam, war die
      // Vorschau; die Rueckgabe ist aus dem Nachrichtenverlauf
      // hergeleitet und traegt auch das, was der Melder nicht meldet,
      // etwa einen Plan oder einen unlesbaren Vorschlag. Zwei Quellen
      // fuer dieselbe Liste liefen sonst auseinander, und die
      // gespeicherte waere die schlechtere.
      laufender.schritte = a.verlauf;
      laufender.fuss =
        `${a.sekunden} s` +
        (a.fertig ? "" : ", abgebrochen") +
        (a.gesperrt ? t("reichweite.gesperrt") : "") +
        (laufender_verdichtet ? `, ${t("kontext.im_lauf")}` : "");
    } else {
      // ⚑ Der ganze bisherige Verlauf geht mit. Ein Fenster, das nur
      // die letzte Frage schickte, waere ein Chatfenster ohne
      // Gespraech.
      //
      // 📌 **Ohne die Fehlermeldungen**, und das ist kein Schoenheits-
      // sondern ein Richtigkeitsgrund: Eine Meldung wie „Fehler: das
      // Modell ist nicht geladen" stammt vom Klienten und nicht vom
      // Modell. Ginge sie mit, saehe das Modell im naechsten Zug einen
      // Satz, den es nie gesagt hat, als seinen eigenen, und richtete
      // sich danach.
      //
      // ⚑ **Aus dem Kontext des Gespraechs und nicht aus den Beitraegen**
      // (2026-09-14): Nach einer Verdichtung sieht das Modell die
      // Zusammenfassung, waehrend im Fenster alles stehen bleibt.
      const verlauf = [...vorher, { role: "user", content: modelltext || text }];
      const a = await invoke("frage", {
        verlauf: verlauf.map((n) => [n.role === "assistant" ? "modell" : "nutzer", n.content]),
        // ⚑ **Die Anhaenge dieses Beitrags.** Liegt einer an, bekommt
        //    der Chat Werkzeuge, und zwar nur fuer die Anhaenge.
        anhaenge: anhaenge.map((a) => a.pfad),
        wurzel: prozesspfad(),
        // ⚑ **Satzweise gesprochen, waehrend das Modell noch schreibt.**
        // Nur hier im Chat: In der Agentenschleife stehen im Strom auch
        // Werkzeugaufrufe.
        sprechen: vorlesen_an,
      });
      laufender.text = a.text;
      laufender.fuss = `${a.sekunden} s`;
      // ⚑ **Was sich wirklich geaendert hat, steht unter der Antwort**
      //    (Auftrag des Projektinhabers, 2026-09-23). Die Liste kommt
      //    aus einem Vergleich der Dateien, nicht aus dem, was das
      //    Modell ueber sich sagt.
      //
      // ⚠️ **Der Weg hinaus ist ein Knopf und kein Werkzeug.** Die
      //    Werkzeuge des Chats kommen nicht aus dem Anhangordner
      //    heraus; wohin eine geaenderte Datei geht, entscheidet der
      //    Mensch ueber den Dialog des Systems.
      laufender.geaendert = a.geaendert || [];
      kontext_merken(offen, [...verlauf, { role: "assistant", content: a.text }], null, 0);
      kontext_zeichnen(a.kontext);
    }
    await live_beenden();
    // ⚑ Meldet sich nur, wenn der Nutzer gerade woanders ist, etwa
    // auf der Einstellungsseite: Wer die Antwort vor sich hat, braucht
    // keine Nachricht darueber, dass sie da ist.
    const wie_lange = offen.beitraege[offen.beitraege.length - 1]?.fuss || "";
    melden(
      offen.modus === "agent"
        ? `Auftrag fertig: ${kurz_titel(text)}  ${wie_lange}`
        : `Antwort da: ${kurz_titel(text)}  ${wie_lange}`,
      "gespraech",
    );
  } catch (f) {
    // 📌 **Der halb geschriebene Beitrag bleibt stehen**, und der Fehler
    // kommt darunter. Wer ihn wegnaehme, loeschte vor den Augen des
    // Nutzers, was er gerade gelesen hat, und die Ueberlegung, an der
    // vielleicht steht, woran es lag.
    await live_beenden();
    offen.beitraege.push({ von: "modell", text: t("fehler", f), fuss: "", fehler: true });
    melden(`Fehlgeschlagen: ${kurz_titel(text)}`, "gespraech");
  } finally {
    auftrag_laeuft = false;
    knopf.disabled = false;
    sichern();
    alles_zeichnen();
  }
}

// --- Der Reflex, der dem Zeiger folgt -----------------------------------

// 📌 **Das ist der Teil, an dem das Auge Glas erkennt**, und er ist der
// Grund, warum der Reflex ueberhaupt aus dem Skript kommt und nicht aus
// einer Animation im Stil. Ein fest einprogrammierter Lichtstreifen
// sieht bei jedem Knopf gleich aus und weiss nichts davon, wo der Zeiger
// steht. Ein heller Punkt, der auf der KANTE dorthin wandert, wo der
// Zeiger ist, verhaelt sich wie eine gewoelbte Oberflaeche unter einer
// Lichtquelle.
//
// ⚑ **Gerechnet wird die Stelle des Zeigers, in Prozent der Flaeche.**
// `--mx` und `--my` setzen die Mitte des `radial-gradient`, der den
// Glanz zeichnet. Prozent trifft immer, gleich wie gross oder wie rund
// die Flaeche ist.
const LINSEN = ".glas, .eingabefeld, button:not(.blank)";
let reflex_angefragt = false;
let letzte_stelle = null;

document.addEventListener("mousemove", (e) => {
  letzte_stelle = e;
  if (reflex_angefragt) return;
  reflex_angefragt = true;
  requestAnimationFrame(() => {
    reflex_angefragt = false;
    const z = letzte_stelle;
    if (!z) return;
    // ⚑ Nur die Flaeche unter dem Zeiger und die Linsen darueber: Der
    // Reflex ist ohnehin nur beim Ueberfahren sichtbar, und alle
    // anderen zu rechnen waere Arbeit fuer nichts.
    const unten = document.elementFromPoint(z.clientX, z.clientY);
    if (!unten) return;
    for (const el of document.querySelectorAll(LINSEN)) {
      if (el !== unten && !el.contains(unten)) continue;
      const r = el.getBoundingClientRect();
      // ⚑ **Die Stelle des Zeigers in Prozent der Flaeche.** Der Glanz
      // ist ein `radial-gradient` im Hintergrund des Pseudoelements;
      // ein Punkt in Prozent trifft dort immer, gleich wie gross oder
      // wie rund die Flaeche ist.
      //
      // 📌 Hier stand ein WINKEL fuer einen Kegelverlauf. Der zeichnete
      // eine wandernde Kante, und Kanten waren genau das Problem: Drei
      // Anlaeufe lagen auf demselben Pseudoelement und stritten sich.
      const mx = ((z.clientX - r.left) / r.width) * 100;
      const my = ((z.clientY - r.top) / r.height) * 100;
      el.style.setProperty("--mx", `${mx.toFixed(1)}%`);
      el.style.setProperty("--my", `${my.toFixed(1)}%`);
    }
  });
});

// --- Verdrahtung --------------------------------------------------------

/// **Die Marke im Kopf, geklont aus der Seitenleiste.**
///
/// ⚑ **Geklont und nicht abgeschrieben.** Die Spirale ist gerechnet
/// (Fibonacci-Quadrate und Viertelkreise, viermal
/// gedreht); sie ein zweites Mal ins HTML zu schreiben hiesse, dass die
/// naechste Aenderung an zwei Stellen ankommen muss. Sie wird deshalb
/// einmal beim Start aus dem Leistenkopf uebernommen.
function kopfmarke_bauen() {
  const ziel = $("kopfmarke");
  if (!ziel || ziel.childElementCount > 0) return;
  const kopf = document.querySelector("#seitenleiste .leistenkopf");
  if (!kopf) return;
  for (const teil of kopf.children) {
    const k = teil.cloneNode(true);
    k.removeAttribute("id");
    // Die Vorlesehilfe hat die Marke schon in der Leiste; ein zweites
    // Mal dasselbe Wort ist keine zweite Auskunft.
    k.setAttribute("aria-hidden", "true");
    k.removeAttribute("aria-label");
    ziel.append(k);
  }
}

/// **Zeigt oder nimmt die Marke im Kopf, mit dem passenden Stoerbild.**
///
/// 📌 **Das Gehen braucht ein Ende, und `hidden` allein ist keins.** Wer
/// beim Ausklappen nur `hidden` setzt, schneidet die Bewegung mitten
/// durch; wer nur die Klasse setzt, laesst ein unsichtbares Element im
/// Raster stehen, das die Mittelspalte weiter belegt. Deshalb beides,
/// und das `hidden` erst, wenn das Stoerbild durch ist.
let marke_uhr = null;
function kopfmarke_zeigen(sichtbar) {
  const m = $("kopfmarke");
  if (!m) return;
  kopfmarke_bauen();
  clearTimeout(marke_uhr);
  m.classList.remove("kommt", "geht");
  // Neuzeichnen erzwingen, sonst bemerkt der Browser den Klassenwechsel
  // nicht und die Bewegung faellt aus.
  void m.offsetWidth;
  if (sichtbar) {
    m.hidden = false;
    m.classList.add("kommt");
  } else {
    m.classList.add("geht");
    // ⚑ Etwas laenger als die Bewegung selbst (0,34 s), damit das
    // letzte Bild noch steht. Bei abbestellter Bewegung faellt die
    // Bewegung aus, die Frist bleibt: 0,4 s bis das Element geht.
    marke_uhr = setTimeout(() => {
      m.hidden = true;
      m.classList.remove("geht");
    }, 400);
  }
}

$("leiste-schalten").addEventListener("click", () => {
  const zu = $("seitenleiste").classList.toggle("zu");
  $("leiste-schalten").setAttribute("aria-expanded", String(!zu));
  kopfmarke_zeigen(zu);
});

// --- Aktualisierung -----------------------------------------------------
//
// ⚑ **Nachsehen und Einspielen sind zwei Knoepfe**, weil sie zwei
// Entscheidungen sind: Das eine kostet eine Netzanfrage, das andere
// Minuten und einen neuen Programmstand.
//
// ⚠️ **Der zweite Knopf ist gesperrt, solange es nichts zu tun gibt
// oder nichts zu tun MOEGLICH ist**, und er sagt beim Zeigen, warum.
// Ein Knopf, der nichts tut und nicht sagt weshalb, ist eine
// Sackgasse.

let aktstand = null;

function aktualisierung_zeichnen() {
  const zeile = $("aktstand");
  const knopf = $("akt-einspielen");
  if (!aktstand) {
    zeile.textContent = "";
    knopf.hidden = true;
    knopf.disabled = true;
    return;
  }
  const s = aktstand;
  let text;
  if (s.grund) {
    text = t("akt.fehler", s.grund);
  } else if (s.hinterher === null || s.hinterher === undefined) {
    text = t("akt.kein", s.eigene);
  } else if (s.hinterher > 0) {
    text = t("akt.hinterher", s.eigene, s.hinterher);
  } else {
    text = t("akt.aktuell", s.eigene);
  }
  if (s.neueste) text += t("akt.neueste", s.neueste, s.stand || "");
  zeile.textContent = text;

  // ⚑ **Der Knopf ist da, wenn es etwas zu tun gibt, und sonst nicht**
  // (Festlegung des Projektinhabers, 2026-09-10).
  //
  // 📌 **Vorher stand er gesperrt da.** Ein gesperrter Knopf beantwortet
  // die Frage „gibt es Updates" mit einem Bedienelement, und der Grund
  // steckte in seinem Zeigetext, wo ihn nur findet, wer mit der Maus
  // darauf wartet. **Die Zeile darueber beantwortet dieselbe Frage mit
  // einem Satz**, und den liest man, ohne zu zielen.
  const lohnt = Boolean(s.quelle) && (s.hinterher || 0) > 0;
  knopf.hidden = !lohnt;
  knopf.disabled = !lohnt;
}

$("akt-pruefen").addEventListener("click", async () => {
  const k = $("akt-pruefen");
  const vorher = k.textContent;
  k.disabled = true;
  k.textContent = t("akt.sieht");
  try {
    aktstand = await invoke("aktualisierung");
  } catch (f) {
    aktstand = { eigene: "", grund: String(f), quelle: null, hinterher: null, neueste: null };
  }
  k.disabled = false;
  k.textContent = vorher;
  aktualisierung_zeichnen();
});

$("akt-einspielen").addEventListener("click", async () => {
  const k = $("akt-einspielen");
  const vorher = k.textContent;
  const ausgabe = $("aktzeilen");
  k.disabled = true;
  k.textContent = t("akt.laeuft");
  ausgabe.hidden = false;
  ausgabe.textContent = "";
  try {
    await invoke("aktualisieren");
    melden(t("akt.fertig"), "einstellungen");
    aktstand = await invoke("aktualisierung");
  } catch (f) {
    melden(t("fehler", f));
  }
  k.textContent = vorher;
  aktualisierung_zeichnen();
});

// ⚑ **Die Zeilen laufen ein, waehrend gebaut wird.** Ein Fortschritt,
// den niemand sieht, ist von einem Stillstand nicht zu unterscheiden.
horchen("aktualisierung-zeile", (zeile) => {
  const a = $("aktzeilen");
  a.hidden = false;
  a.textContent += `${zeile}\n`;
  a.scrollTop = a.scrollHeight;
});

$("zu-einstellungen").addEventListener("click", async () => {
  $("feldhinweis").hidden = true;
  for (const id of TABELLEN) {
    for (const z of $(id).querySelectorAll(".gesucht")) z.classList.remove("gesucht");
  }
  $("einstellungsseite").hidden = false;
  await einstellungen_zeichnen();
  // Beim Oeffnen ist nichts im Bau, also auch kein Balken.
  if (!baut) {
    $("baustand").hidden = true;
    $("bauzeilen").textContent = "";
  }
  await katalog_zeichnen();
  // ⚑ **Es sieht nicht von selbst nach.** Eine Netzanfrage bei jedem
  // Oeffnen der Einstellungsseite waere eine Verbindung, die niemand
  // angefordert hat; gezeichnet wird, was beim letzten Nachsehen
  // herauskam.
  aktualisierung_zeichnen();
  $("zurueck").focus();
});
$("zurueck").addEventListener("click", () => {
  $("einstellungsseite").hidden = true;
  $("setzmeldung").textContent = "";
  $("auftrag").focus();
});

$("neues-gespraech").addEventListener("click", () => {
  neues_gespraech();
  alles_zeichnen();
  $("auftrag").focus();
  wurzel_verlangen();
});

$("laden").addEventListener("click", modell_laden);


// ⚑ Das Feld waechst mit dem Text und hat eine Obergrenze. Ein Feld,
// das nicht waechst, versteckt die eigene Eingabe; eines ohne Grenze
// frisst das Gespraech.
const feld = $("auftrag");
const feld_messen = () => {
  feld.style.height = "auto";
  feld.style.height = `${feld.scrollHeight}px`;
};
feld.addEventListener("input", feld_messen);

// ⚑ Eingabe sendet, Umschalt und Eingabe macht eine Zeile. So machen es
// die Werkzeuge, an denen sich diese Oberflaeche orientiert.
feld.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    $("eingabe").requestSubmit();
  }
});

$("eingabe").addEventListener("submit", async (e) => {
  e.preventDefault();
  const text = feld.value.trim();
  // ⚑ **Ein Anhang allein ist auch eine Frage.** Wer ein Bild schickt
  // und nichts dazu schreibt, will wissen, was darauf ist.
  if (!text && anhaenge_offen.length === 0) return;
  feld.value = "";
  feld_messen();
  await senden(text);
});

$("terminalform").addEventListener("submit", async (e) => {
  e.preventDefault();
  const feld = $("terminaleingabe");
  const befehl = feld.value;
  feld.value = "";
  await terminal_senden(befehl);
  feld.focus();
});

// ⚑ **Geleert wird wie in einem Terminal**: mit `clear` oder mit
// Befehlstaste K (unter Linux und Windows Steuerung L). Ein eigener Knopf
// dafuer war ein Formularelement mehr auf einer Flaeche, die keines
// haben soll.
$("terminaleingabe").addEventListener("keydown", (e) => {
  const leeren = (e.metaKey && e.key === "k") || (e.ctrlKey && e.key === "l");
  if (!leeren) return;
  e.preventDefault();
  terminal_leertext();
});

// ⚑ **Ein Klick irgendwo ins Terminal setzt den Einfuegepunkt in die
// Eingabezeile**, wie in jedem Terminal. Ausser wenn gerade Text markiert
// wurde: Wer eine Ausgabe kopieren will, soll die Markierung behalten.
$("terminal").addEventListener("mouseup", () => {
  if (String(window.getSelection() || "")) return;
  if (!$("terminalform").hidden) $("terminaleingabe").focus();
});

// ⚑ **Pfeil hoch und runter blaettern durch das Getippte.** Ohne das
// ist ein Terminal zum Arbeiten unbrauchbar: Jeder zweite Befehl ist
// eine Abwandlung des vorigen.
$("terminaleingabe").addEventListener("keydown", (e) => {
  if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
  if (!terminalverlauf.length) return;
  e.preventDefault();
  if (e.key === "ArrowUp") {
    terminalzeiger = Math.max(0, terminalzeiger - 1);
  } else {
    terminalzeiger = Math.min(terminalverlauf.length, terminalzeiger + 1);
  }
  const wert = terminalverlauf[terminalzeiger] ?? "";
  e.target.value = wert;
  // Der Einfuegepunkt ans Ende, sonst steht er mitten im Befehl.
  requestAnimationFrame(() => e.target.setSelectionRange(wert.length, wert.length));
});


// --- Start --------------------------------------------------------------

(async () => {
  // ⚑ **Die Sprache zuerst, vor allem anderen.** Wer sie spaeter holt,
  // zeichnet einmal auf Deutsch und danach noch einmal richtig; das
  // sieht man.
  try {
    const e = await invoke("einstellungen");
    if (e.sprache === "de" || e.sprache === "en") sprache = e.sprache;
    // ⚑ **Aus demselben Aufruf.** Ein zweiter waere eine zweite
    // Gelegenheit, verschiedene Staende zu sehen, und das Bild soll
    // stehen, bevor der Vorhang aufgeht.
    bild_setzen(e.werte["oberflaeche.thema"], e.werte["oberflaeche.schrift"]);
  } catch {
    // Ohne Einstellungen bleibt es bei den Vorgaben: Deutsch und dunkel.
  }
  beschriften();
  laden_aus_speicher();
  // ⚑ **Der Start legt nichts an.** Wer das Fenster oeffnet, hat noch
  // nichts gesagt; ein leeres Gespraech in der Liste waere ein Eintrag,
  // den niemand gemacht hat. Das Zuletzte wird geoeffnet, wenn es eines
  // gibt, und sonst steht der Leerzustand da.
  offen = gespraeche[0] || null;
  if (offen) modus = offen.modus;
  // ⚑ Knopf und Ablegen einmal beim Start verdrahten.
  anhang_verdrahten();
  sinne_verdrahten();
  alles_zeichnen();
  try {
    await modellwahl_zeichnen();
    await kopf_zeichnen();
    // ⚑ **Einmal beim Start und nicht bei jeder Zeichnung.** Was vor
    // dieser Fassung gespeichert wurde, hat keine Gliederung; sie beim
    // Zeichnen zu holen machte jede Zeichnung unterbrechbar.
    await bloecke_nachtragen();
    // ⚑ **Auch beim Start folgt das Modell dem geoeffneten Gespraech**,
    // eingestellt und nicht geladen. Vor `modellzeile_schreiben`, damit
    // die Zeile gleich das richtige nennt.
    await modell_dem_gespraech_folgen(offen);
    await modellzeile_schreiben();
  } catch (f) {
    melden(t("fehler.start", f));
  }
  await vorhangWeg();
  await starthinweis_zeigen();
  notaus_verdrahten();
  feld.focus();
})();

// ⛔️ **Der Notaus**: Knopf und Tastenkuerzel (⌘. auf dem Mac, Strg+.
// sonst). Er haelt den Auftrag in der Kiste an und die Stimme hier, und
// das Gespraech bleibt stehen, wie es ist.
function notaus_verdrahten() {
  const ausloesen = async () => {
    stimme_anhalten();
    try {
      await invoke("notaus");
      melden(t("notaus.gemeldet"));
    } catch (f) {
      melden(t("fehler", f));
    }
  };
  $("notaus").addEventListener("click", ausloesen);
  document.addEventListener("keydown", (ereignis) => {
    if ((ereignis.metaKey || ereignis.ctrlKey) && ereignis.key === ".") {
      ereignis.preventDefault();
      ausloesen();
    }
  });
}
