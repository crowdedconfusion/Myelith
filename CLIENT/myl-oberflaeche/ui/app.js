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

import { netzStarten } from "./netz.js";

const { invoke } = window.__TAURI__.core;
const $ = (k) => document.getElementById(k);

// --- Vorschaltbild ------------------------------------------------------

// ⚑ **Das Vorschaltbild geht erst weg, wenn die Einstellungen da
// sind**, und mindestens nach anderthalb Sekunden. Beides zusammen:
// Wer sofort verschwindet, hat nichts gezeigt; wer auf eine feste Zeit
// wartet, obwohl er fertig ist, haelt den Nutzer auf.
const VORHANG_MINDESTENS = 1500;
const vorhang = $("vorhang");
const netzAnhalten = netzStarten($("netz"));
const start = performance.now();

async function vorhangWeg() {
  const rest = VORHANG_MINDESTENS - (performance.now() - start);
  if (rest > 0) await new Promise((r) => setTimeout(r, rest));
  vorhang.classList.add("weg");
  // ⚑ Erst nach der Blende anhalten, sonst friert das Netz sichtbar
  // ein, waehrend es noch durchscheint.
  setTimeout(() => {
    netzAnhalten();
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
    b.setAttribute("aria-checked", String(offen?.modus === m.id));
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
    r = await invoke("werkzeuge");
  } catch (f) {
    r = { wurzel: null, namen: [] };
  }
  const p = document.createElement("p");
  p.className = "reichweitezeile";
  if (!r.wurzel) {
    p.textContent = t("reichweite.kein");
    // 📌 Hier stand der technische Name. Wer den Hinweis liest, sucht
    // danach in den Einstellungen, und dort steht seit dem
    // 2026-09-09 die Beschriftung: „Arbeitsordner".
    p.title = t("reichweite.hinweis");
  } else {
    // 📌 **Von vorn gekuerzt, im Skript.** Das Aussagekraeftige an
    // einem Pfad steht hinten; `direction: rtl` taete dasselbe und
    // schoebe dabei den fuehrenden Schraegstrich ans Ende, sodass ein
    // Pfad angezeigt wuerde, den es nicht gibt.
    const GRENZE = 34;
    p.textContent =
      r.wurzel.length > GRENZE ? `…${r.wurzel.slice(-(GRENZE - 1))}` : r.wurzel;
    p.title = r.wurzel;
  }
  w.append(p);
  if (r.namen.length) {
    const l = document.createElement("p");
    l.className = "reichweitezeile werkzeugliste";
    l.textContent = r.namen.join("  ");
    l.title = `${r.namen.length} Werkzeuge`;
    w.append(l);
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
      .filter((b) => !b.fehler && !b.laufend && b.text && (b.von === "nutzer" || b.von === "modell"))
      .map((b) => ({ role: b.von === "modell" ? "assistant" : "user", content: b.text })),
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
  $("kontextfuellung").style.width = `${Math.min(100, k.prozent)}%`;
  $("kontextzahl").textContent = t("kontext.zahl", k.belegt, k.grenze, k.prozent);
  knopf.title = t("kontext.titel", k.ansage, k.belegt - k.ansage, k.nachrichten);
  knopf.classList.toggle("eng", k.prozent >= 80);
}

/// ⚠️ **Nicht waehrend eines Laufs**: Der haelt das Modell, und die Frage
/// wartete bis zu seinem Ende.
async function kontext_holen() {
  if (!geladen || !offen || $("senden").disabled) {
    if (!offen) kontext_zeichnen(null);
    return;
  }
  try {
    kontext_zeichnen(await invoke("kontext", { verlauf: kontext_von(offen), modus: offen.modus }));
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
    const r = await invoke("verdichten", { verlauf });
    offen.beitraege.push({ von: "hinweis", text: t("kontext.verdichtet", r.vorher, r.nachher) });
    kontext_merken(offen, r.nachrichten, r.zusammenfassung, offen.beitraege.length);
    sichern();
    alles_zeichnen();
  } catch (f) {
    melden(t("kontext.fehler", f));
  } finally {
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
    const blase = document.createElement("div");
    blase.className = "blase";
    blase.textContent = b.text;
    wurzel.append(blase);
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
  // ⚑ **Es haengt jetzt am Lauf und nicht am Inhalt:** Solange
  // gerechnet wird, steht es unten, und es geht, wenn der Lauf endet.
  // Waehrend Text ankommt, waechst er darueber; die Punkte sagen dann
  // „und es geht weiter", und das stimmt.
  let laufzeichen = null;
  if (b.laufend) {
    const l = document.createElement("div");
    l.className = "laeuft";
    for (let i = 0; i < 3; i += 1) l.append(document.createElement("span"));
    l.setAttribute("aria-label", t("lauf.arbeitet"));
    l.setAttribute("role", "status");
    // ⚑ Angehaengt wird es **am Ende** dieser Funktion, damit es unter
    // allem steht, was schon da ist: Ein Zeichen ueber dem wachsenden
    // Text saehe aus, als gehoerte es zu etwas Vergangenem.
    laufzeichen = l;
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

  if (b.fuss) {
    const f = document.createElement("div");
    f.className = "zeitzeile";
    f.textContent = b.fuss;
    wurzel.append(f);
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
const TABELLEN = ["felder-oberflaeche", "felder-grenzen", "felder-rest"];

// ⚑ **Entschieden wird am Namen und nicht an der Ueberschrift.** Ein
// Feldname (`kap.speicher`) ist in jeder Sprache derselbe; eine
// Ueberschrift („Grenzen dieses Rechners") ist es nicht, und eine
// Zuordnung ueber uebersetzten Text bricht beim Sprachwechsel.
const tabelle_fuer = (name) =>
  name.startsWith("oberflaeche.")
    ? "felder-oberflaeche"
    : name.startsWith("kap.")
      ? "felder-grenzen"
      : "felder-rest";

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

async function horchen(name, fn) {
  const { listen } = window.__TAURI__.event;
  return await listen(name, fn);
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
  for (const f of felder) {
    if (f.freigabe) continue;
    const koerper = $(tabelle_fuer(f.name)).querySelector("tbody");
    if (!gesehen.has(f.bereich)) {
      gesehen.add(f.bereich);
      koerper.append(bereichszeile(f.bereich));
    }
    koerper.append(
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
          await kopf_zeichnen();
        } catch (fehler) {
          $("setzmeldung").textContent = t("fehler", fehler);
        }
      }),
    );
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
// ⚑ **Ganz links heisst „ohne Grenze" und nicht „nichts".** Bei einer
// Grenze ist das dasselbe wie `aus`, und der Setzer kennt den
// Unterschied: `aus` **loescht** sie, null waere ein Stillstand.
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
  schieber.min = 0;
  schieber.max = r.hoechstens ?? 0;
  schieber.value = r.wert ?? 0;
  schieber.dataset.feld = r.name;
  schieber.disabled = Boolean(r.sperrgrund) || !r.hoechstens;

  const einheit = EINHEIT[r.einheit] ?? ((n) => String(n));
  const zeigen = () => {
    const n = Number(schieber.value);
    anzeige.textContent = n === 0 ? "ohne Grenze" : einheit(n);
  };
  zeigen();
  schieber.addEventListener("input", zeigen);
  schieber.addEventListener("change", async () => {
    const n = Number(schieber.value);
    try {
      await invoke("setzen", { feld: r.name, wert: n === 0 ? "aus" : String(n) });
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
  const artefakt = e.werte["modell.artefakt"];
  $("modellzeile").textContent = geladen ? artefakt : t("modell.artefaktKlammer", artefakt);
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

async function senden(text) {
  if (!offen) neues_gespraech();
  // ⚑ **Vor dem neuen Beitrag gelesen**: Der Auftrag geht als Auftrag
  // hinein und nicht noch einmal als Teil des Verlaufs.
  const vorher = kontext_von(offen);
  if (offen.beitraege.length === 0) {
    offen.titel = titel_aus(text);
  }
  offen.beitraege.push({ von: "nutzer", text });
  // Ab hier gilt eine Zusammenfassung, die dieser Auftrag erzeugt.
  const auftrag_bei = offen.beitraege.length - 1;
  // ⚑ **Womit man arbeitet, steht oben** (Auftrag des Projektinhabers,
  // 2026-09-12). Hier und nicht beim Oeffnen: siehe `nach_oben`.
  nach_oben(offen);
  sichern();
  alles_zeichnen();

  const knopf = $("senden");
  knopf.disabled = true;

  try {
    if (!geladen) await modell_laden();
    if (!geladen) throw new Error(t("lauf.nichtgeladen"));

    await live_anfangen(offen.modus);

    if (offen.modus === "agent") {
      const a = await invoke("agent_fahren", { auftrag: text, verlauf: vorher });
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
      const verlauf = [...vorher, { role: "user", content: text }];
      const a = await invoke("frage", {
        verlauf: verlauf.map((n) => [n.role === "assistant" ? "modell" : "nutzer", n.content]),
      });
      laufender.text = a.text;
      laufender.fuss = `${a.sekunden} s`;
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
  if (!text) return;
  feld.value = "";
  feld_messen();
  await senden(text);
});


// --- Start --------------------------------------------------------------

(async () => {
  // ⚑ **Die Sprache zuerst, vor allem anderen.** Wer sie spaeter holt,
  // zeichnet einmal auf Deutsch und danach noch einmal richtig; das
  // sieht man.
  try {
    const s = (await invoke("einstellungen")).sprache;
    if (s === "de" || s === "en") sprache = s;
  } catch {
    // Ohne Einstellungen bleibt es bei der Vorgabe, und die ist Deutsch.
  }
  beschriften();
  laden_aus_speicher();
  // ⚑ **Der Start legt nichts an.** Wer das Fenster oeffnet, hat noch
  // nichts gesagt; ein leeres Gespraech in der Liste waere ein Eintrag,
  // den niemand gemacht hat. Das Zuletzte wird geoeffnet, wenn es eines
  // gibt, und sonst steht der Leerzustand da.
  offen = gespraeche[0] || null;
  if (offen) modus = offen.modus;
  alles_zeichnen();
  try {
    await modellwahl_zeichnen();
    await kopf_zeichnen();
    // ⚑ **Einmal beim Start und nicht bei jeder Zeichnung.** Was vor
    // dieser Fassung gespeichert wurde, hat keine Gliederung; sie beim
    // Zeichnen zu holen machte jede Zeichnung unterbrechbar.
    await bloecke_nachtragen();
    await modellzeile_schreiben();
  } catch (f) {
    melden(t("fehler.start", f));
  }
  await vorhangWeg();
  feld.focus();
})();
