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
// ⛑ Hier stand bis zum 2026-09-09 „oben mittig der Ort (lokal oder
// Netz)". Der Schalter ist entfallen, der Satz nicht, und ein
// Kommentar, der einen Bedienteil beschreibt, den es nicht gibt,
// schickt den Naechsten suchen.
//
// ⚑ **Der Modus steht ueber den Gespraechen, weil er bestimmt, WAS ein
// Gespraech ist.** In `Frage` traegt es seinen Verlauf mit und das
// Modell sieht ihn; beim Agenten steht jeder Auftrag fuer sich, mit
// eigenem Schrittbudget und eigener Belegkette. Das ist Entscheidung
// C2 und keine Bequemlichkeit, und ein Fenster, das beide gleich
// behandelte, versteckte genau den Unterschied.

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
const MODI = [
  {
    // ⚑ Der Modus heisst „Chat" und der Befehl im Ruecken `frage`, und
    // das ist kein Versehen: Der Befehl fragt das Modell **einmal**,
    // der Modus ist ein Gespraech, weil das Fenster den Verlauf
    // mitfuehrt und bei jedem Zug wieder mitgibt. Das eine ist der
    // Zug, das andere die Partie.
    id: "chat",
    name: "Chat",
    was: "ohne Werkzeuge",
    leer: "Ein Gespraech mit dem lokalen Modell. Der Verlauf wird mitgegeben, das Modell sieht also, was vorher gesagt wurde.",
  },
  {
    id: "agent",
    name: "Agent",
    was: "mit Werkzeugen",
    leer: "Ein Auftrag, den der Agent mit Werkzeugen erledigt. ⚑ Jeder Auftrag steht fuer sich: Schrittbudget und Belegkette gelten je Lauf, der vorige Auftrag geht nicht mit ein.",
  },
  // ⚠️ **Zwei Modi, die es geben WIRD und heute nicht gibt.** Sie
  // stehen gedaempft da und sagen beim Anfassen, was fehlt. Ein Modus,
  // der still nichts taete, waere schlimmer als keiner; einer, der
  // ganz fehlt, verschweigt, wohin der Client geht.
  {
    id: "knoten",
    name: "Knoten",
    was: "Mining",
    offen: false,
    warum:
      "Der Knoten rechnet fuer das Netz und verdient daran. Dem Klienten fehlen Knotenadresse und Vollmacht (Fahrplan 2.2 bis 2.5b).",
    leer: "",
  },
  {
    id: "wallet",
    name: "Wallet",
    was: "Guthaben",
    offen: false,
    warum:
      "Guthaben, Ueberweisungen und die Belege dazu. Braucht dieselbe Netzhaelfte wie der Knoten (Fahrplan 2.2 bis 2.5b).",
    leer: "",
  },
];

// --- Gespraeche ---------------------------------------------------------

// ⛑ **Sie liegen im Browserspeicher des Fensters und nicht in einer
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
    hinweis("Das Gespraech liess sich nicht merken; der Speicher des Fensters ist zu.");
  }
}

const jetzt = () => new Date().toISOString();

function neues_gespraech(modus) {
  const g = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    titel: "Neues Gespraech",
    modus: modus || (offen ? offen.modus : MODI[0].id),
    beitraege: [],
    wann: jetzt(),
  };
  gespraeche.unshift(g);
  offen = g;
  sichern();
  return g;
}

// ⚑ Der Titel kommt aus dem ersten Beitrag und wird nicht erfragt. Ein
// Dialog, der nach einem Namen fragt, bevor man weiss, worum es geht,
// ist eine Huerde vor dem ersten Satz.
function titel_aus(text) {
  const eine = text.replace(/\s+/g, " ").trim();
  return eine.length > 40 ? `${eine.slice(0, 40)}...` : eine || "Neues Gespraech";
}

// --- Anzeige ------------------------------------------------------------

function hinweis(text) {
  $("hinweiszeile").textContent = text || "";
}

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
      b.title = m.warum;
    }
    const n = document.createElement("span");
    n.textContent = m.name;
    const s = document.createElement("span");
    s.className = "was";
    s.textContent = m.was;
    b.append(n, s);
    b.addEventListener("click", () => {
      if (m.offen === false) {
        hinweis(m.warum);
        return;
      }
      if (!offen) neues_gespraech(m.id);
      // ⚑ Den Modus eines Gespraechs mit Beitraegen zu wechseln waere
      // ein zweiter Vertrag im selben Verlauf: Die eine Haelfte traegt
      // Kontext, die andere nicht. Stattdessen ein neues Gespraech.
      else if (offen.beitraege.length > 0 && offen.modus !== m.id) neues_gespraech(m.id);
      else offen.modus = m.id;
      sichern();
      alles_zeichnen();
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
    p.textContent = "kein Verzeichnis eingehaengt";
    p.title = "Ohne `agent.wurzel` hat der Agent keine Werkzeuge. In den Einstellungen setzen.";
  } else {
    // ⛑ **Von vorn gekuerzt, im Skript.** Das Aussagekraeftige an
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
  // ⛑ **Erst zeigen, dann messen.** Ein verstecktes Element hat keine
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
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && menue_fuer) menue_schliessen();
});
window.addEventListener("blur", menue_schliessen);
document.addEventListener("scroll", menue_schliessen, true);

/// Das Gespraech als Markdown, so wie es im Fenster steht.
function als_markdown(g) {
  const kopf = [
    `# ${g.titel}`,
    "",
    `- Modus: ${MODI.find((m) => m.id === g.modus)?.name || g.modus}`,
    `- Begonnen: ${g.wann}`,
    `- Beitraege: ${g.beitraege.length}`,
    "",
  ];
  const teile = g.beitraege.map((b) => {
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
    melden(`Gespraech "${g.titel}" geloescht.`);
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
      melden(`Gespraech abgelegt: ${wo}`);
    } catch (f) {
      melden(`Fehler beim Ausgeben: ${f}`);
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
  feld.setAttribute("aria-label", "Gespraech umbenennen");
  knopf.replaceWith(feld);
  feld.focus();
  feld.select();

  let fertig = false;
  const schliessen = (uebernehmen) => {
    if (fertig) return;
    fertig = true;
    if (uebernehmen) {
      const neu = feld.value.trim();
      // ⛑ Ein leerer Titel waere eine Zeile ohne Aufschrift. Dann
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

function chats_zeichnen() {
  const w = $("chatliste");
  w.replaceChildren();
  for (const g of gespraeche) {
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
    auf.title = `${g.titel} (${MODI.find((m) => m.id === g.modus)?.name || g.modus})`;
    auf.addEventListener("click", () => {
      offen = g;
      alles_zeichnen();
    });

    const weg = document.createElement("button");
    weg.type = "button";
    weg.className = "weg blank";
    weg.textContent = "×";
    weg.setAttribute("aria-label", `Gespraech "${g.titel}" loeschen`);
    weg.addEventListener("click", (e) => {
      e.stopPropagation();
      gespraeche = gespraeche.filter((x) => x.id !== g.id);
      if (offen && offen.id === g.id) offen = gespraeche[0] || null;
      sichern();
      alles_zeichnen();
    });

    // ⚑ Der Titelknopf traegt keine eigene Umrandung, die Zeile
    // uebernimmt sie; sonst haette jede Zeile zwei Rahmen.
    auf.style.background = "none";
    auf.style.border = "0";
    auf.style.boxShadow = "none";
    auf.style.color = "inherit";
    auf.style.padding = "0";
    auf.style.textAlign = "left";
    zeile.append(auf, weg);
    w.append(zeile);
  }
}

function beitrag_zeichnen(b) {
  const wurzel = document.createElement("div");
  wurzel.className = `beitrag von-${b.von}`;

  if (b.von === "nutzer") {
    const blase = document.createElement("div");
    blase.className = "blase";
    blase.textContent = b.text;
    wurzel.append(blase);
    return wurzel;
  }

  if (b.verlauf && b.verlauf.length) {
    const d = document.createElement("details");
    d.className = "verlauf";
    const s = document.createElement("summary");
    s.textContent = `${b.verlauf.length} Schritte`;
    d.append(s);
    for (const z of b.verlauf) {
      const p = document.createElement("div");
      p.className = `schritt ${z.art}`;
      const m = document.createElement("span");
      m.className = "marke2";
      m.textContent = { aufruf: "→", ergebnis: "←", unlesbar: "!", hinweis: "i" }[z.art] || "·";
      p.append(m, document.createTextNode(z.text));
      d.append(p);
    }
    wurzel.append(d);
  }

  const t = document.createElement("div");
  t.className = "antworttext";
  t.textContent = b.text;
  wurzel.append(t);

  if (b.fuss) {
    const f = document.createElement("div");
    f.className = "zeitzeile";
    f.textContent = b.fuss;
    wurzel.append(f);
  }
  return wurzel;
}

function gespraech_zeichnen() {
  const w = $("gespraech");
  w.replaceChildren();
  if (!offen || offen.beitraege.length === 0) {
    const m = MODI.find((x) => x.id === (offen ? offen.modus : MODI[0].id));
    const p = document.createElement("p");
    p.className = "leerzustand";
    const stark = document.createElement("strong");
    stark.textContent = `${m.name}. `;
    p.append(stark, document.createTextNode(m.leer));
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
// ⛑ **Die Maske entscheidet und nicht der Ereignistyp.** Eine Liste
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
  const t = document.createElement("p");
  t.textContent = text;
  const zu = document.createElement("button");
  zu.className = "rundknopf blank";
  zu.setAttribute("aria-label", "Meldung schliessen");
  zu.textContent = "×";
  zu.addEventListener("click", () => k.remove());
  k.append(t, zu);
  $("meldungen").append(k);
  // ⚑ Sie geht von selbst, aber langsam: Wer gerade tippt, soll sie
  // noch lesen koennen, wenn er aufsieht.
  setTimeout(() => k.remove(), 20000);
}

// --- Einstellungen ------------------------------------------------------

// ⚑ Je Feld ein Bedienelement, und die Art kommt aus der Kiste: Text,
// Zahl, Schalter, Grenze, Pfad. Eine zweite Liste im Fenster liefe
// irgendwann auseinander.
const feldzeile = (name, art, wert, beim_setzen) => {
  const tr = document.createElement("tr");
  const a = document.createElement("td");
  a.textContent = name;
  const b = document.createElement("td");

  let element;
  if (art === "Schalter") {
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
  b.append(element);
  if (art === "Grenze") {
    const h = document.createElement("span");
    h.className = "grenze";
    h.textContent = "leer oder `aus` loescht die Grenze";
    b.append(h);
  }
  tr.append(a, b);
  return tr;
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

async function katalog_zeichnen() {
  const v = await invoke("voraussetzungen");
  const h = $("bauhinweis");
  if (v.fehlt.length) {
    h.textContent = `Bauen geht hier nicht: ${v.fehlt.join("  ")}`;
  } else {
    h.textContent =
      "Herunterladen und Kalibrieren dauert Minuten bis Stunden und braucht Gigabyte.";
  }
  const w = $("katalog");
  w.replaceChildren();
  let liste;
  try {
    liste = await invoke("katalog");
  } catch (f) {
    h.textContent = `Katalog nicht lesbar: ${f}`;
    return;
  }
  for (const m of liste) {
    const z = document.createElement("div");
    z.className = "katalogzeile";
    const links = document.createElement("div");
    const n = document.createElement("strong");
    n.textContent = m.anzeigename || m.schluessel;
    const d = document.createElement("span");
    d.className = "katalogdaten";
    // ⚑ Groesse und Lizenz stehen dabei, denn beides entscheidet die
    // Frage, ob jemand den Knopf drueckt.
    //
    // ⚠️ **Und das Grundmodell steht dabei.** Der Name heisst „Myelith
    // 4B", weil das Artefakt selbst gebaut ist und anders rechnet als
    // das Gleitkommamodell. Die Herkunft darf dabei nicht
    // verschwinden: Die Grundmodelle stehen unter Apache-2.0, und ein
    // Name ohne Herkunft waere eine Verschleierung statt einer
    // Unterscheidung.
    d.textContent = [
      m.grundmodell ? `aus ${m.grundmodell}` : "",
      m.parameter,
      m.gewichte,
      `Artefakt ${m.artefakt}`,
      m.lizenz,
      m.status,
    ]
      .filter(Boolean)
      .join("  ·  ");
    links.append(n, d);

    const b = document.createElement("button");
    if (m.artefakt_da) {
      b.textContent = "liegt vor";
      b.disabled = true;
    } else {
      b.textContent = m.modell_da ? "kalibrieren" : "Download und kalibrieren";
      b.disabled = v.fehlt.length > 0 || baut !== null;
      b.addEventListener("click", () => bauen(m.schluessel, m.anzeigename || m.schluessel));
    }
    z.append(links, b);
    // ⛑ **Der Balken wird VERSCHOBEN und nicht neu gebaut.** Er haengt
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
    melden(`${name} ist fertig kalibriert.`, "einstellungen");
  } catch (f) {
    baustand_setzen("fehler", String(f), seit);
    melden(`${name} ist fehlgeschlagen.`, "einstellungen");
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
async function horchen(name, fn) {
  const { listen } = window.__TAURI__.event;
  return await listen(name, fn);
}

async function einstellungen_zeichnen() {
  const e = await invoke("einstellungen");
  const felder = await invoke("felder");
  const koerper = $("einstellungen").querySelector("tbody");
  koerper.replaceChildren();

  const wert = {
    "modell.artefakt": e.artefakt,
    "modell.token": e.token,
    "modell.denken": e.denken,
    "agent.schritte": e.schritte,
    "agent.bezeugtes": e.bezeugtes,
    "agent.wurzel": e.wurzel,
    "agent.schreiben": e.schreiben,
    "kap.kerne": e.kerne_eingestellt,
  };
  for (const [name, art] of felder) {
    koerper.append(
      feldzeile(name, art, wert[name], async (neu) => {
        try {
          await invoke("setzen", { feld: name, wert: neu });
          $("setzmeldung").textContent = `${name} gesetzt.`;
          await kopf_zeichnen();
        } catch (f) {
          $("setzmeldung").textContent = `Fehler: ${f}`;
        }
      }),
    );
  }
  $("pfad").textContent = e.pfad;
  return e;
}

async function kopf_zeichnen() {
  const e = await invoke("einstellungen");
  // ⚑ Die Kurzform steht in der Marke, der ganze Satz im Titel. Wer
  // wissen will, warum sein Agent nichts anfasst, findet den Grund am
  // selben Ding und nicht in einer Anleitung.
  // ⛑ **Hier stand die Marke „liest und schreibt" oben rechts.** Sie
  // ist am 2026-09-09 auf Festlegung des Projektinhabers entfallen.
  // ⚑ Die Auskunft geht nicht verloren: Was der Agent anfassen darf,
  // steht ausfuehrlich in der Seitenleiste unter `#reichweite`, und
  // dort steht auch der Pfad dazu. Die Marke war die Kurzform davon an
  // einer zweiten Stelle.
  $("modellzeile").textContent = geladen ? e.artefakt : `${e.artefakt} (nicht geladen)`;
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
    hinweis(`Fehler: ${f}`);
    return;
  }
  const e = await invoke("einstellungen");
  w.replaceChildren();
  for (const m of liste) {
    const o = document.createElement("option");
    o.value = m.pfad;
    o.textContent = m.name;
    if (!m.offen) {
      o.disabled = true;
      o.title = m.warum;
    }
    if (m.pfad === e.artefakt) o.selected = true;
    w.append(o);
  }
}

$("modellwahl").addEventListener("change", async () => {
  const neu = $("modellwahl").value;
  try {
    await invoke("setzen", { feld: "modell.artefakt", wert: neu });
    // ⛑ Ein Modellwechsel wirft das geladene weg. Ohne diese Zeile
    // faehrt der naechste Auftrag mit dem alten Modell, waehrend die
    // Anzeige das neue nennt.
    geladen = false;
    hinweis("Modell gewechselt; es wird beim naechsten Auftrag geladen.");
    await kopf_zeichnen();
    reichweite_zeichnen();
  } catch (f) {
    hinweis(`Fehler: ${f}`);
  }
});

async function modell_laden() {
  const knopf = $("laden");
  knopf.disabled = true;
  const vorher = knopf.textContent;
  knopf.textContent = "laedt";
  try {
    const wort = await invoke("modell_laden");
    geladen = true;
    hinweis(wort);
  } catch (f) {
    hinweis(`Fehler: ${f}`);
  } finally {
    knopf.disabled = false;
    knopf.textContent = vorher;
    await kopf_zeichnen();
  }
}

// --- Senden -------------------------------------------------------------

async function senden(text) {
  if (!offen) neues_gespraech();
  if (offen.beitraege.length === 0) {
    offen.titel = titel_aus(text);
  }
  offen.beitraege.push({ von: "nutzer", text });
  sichern();
  alles_zeichnen();

  const knopf = $("senden");
  knopf.disabled = true;
  hinweis(offen.modus === "agent" ? "der Agent faehrt" : "das Modell antwortet");

  try {
    if (!geladen) await modell_laden();
    if (!geladen) throw new Error("das Modell ist nicht geladen");

    if (offen.modus === "agent") {
      const a = await invoke("agent_fahren", { auftrag: text });
      offen.beitraege.push({
        von: "modell",
        text: a.antwort || "(keine Schlussantwort)",
        verlauf: a.verlauf,
        fuss:
          `${a.sekunden} s` +
          (a.fertig ? "" : ", abgebrochen") +
          (a.gesperrt ? ", Werkzeuge durch die Betriebsart gesperrt" : ""),
      });
    } else {
      // ⚑ Der ganze bisherige Verlauf geht mit. Ein Fenster, das nur
      // die letzte Frage schickte, waere ein Chatfenster ohne
      // Gespraech.
      //
      // ⛑ **Ohne die Fehlermeldungen**, und das ist kein Schoenheits-
      // sondern ein Richtigkeitsgrund: Eine Meldung wie „Fehler: das
      // Modell ist nicht geladen" stammt vom Klienten und nicht vom
      // Modell. Ginge sie mit, saehe das Modell im naechsten Zug einen
      // Satz, den es nie gesagt hat, als seinen eigenen, und richtete
      // sich danach.
      const verlauf = offen.beitraege
        .filter((b) => !b.fehler)
        .map((b) => [b.von, b.text]);
      const a = await invoke("frage", { verlauf });
      offen.beitraege.push({ von: "modell", text: a.text, fuss: `${a.sekunden} s` });
    }
    // ⚑ Meldet sich nur, wenn der Nutzer gerade woanders ist, etwa
    // auf der Einstellungsseite: Wer die Antwort vor sich hat, braucht
    // keine Nachricht darueber, dass sie da ist.
    const wie_lange = offen.beitraege[offen.beitraege.length - 1]?.fuss || "";
    melden(
      offen.modus === "agent"
        ? `Auftrag fertig: ${titel_aus(text)}  ${wie_lange}`
        : `Antwort da: ${titel_aus(text)}  ${wie_lange}`,
      "gespraech",
    );
    hinweis("");
  } catch (f) {
    offen.beitraege.push({ von: "modell", text: `Fehler: ${f}`, fuss: "", fehler: true });
    melden(`Fehlgeschlagen: ${titel_aus(text)}`, "gespraech");
    hinweis("");
  } finally {
    knopf.disabled = false;
    sichern();
    alles_zeichnen();
  }
}

// --- Der Reflex, der dem Zeiger folgt -----------------------------------

// ⛑ **Das ist der Teil, an dem das Auge Glas erkennt**, und er ist der
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
const LINSEN = ".glas, .eingabefeld, button:not(.blank), section";
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
      // ⛑ Hier stand ein WINKEL fuer einen Kegelverlauf. Der zeichnete
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

$("leiste-schalten").addEventListener("click", () => {
  const zu = $("seitenleiste").classList.toggle("zu");
  $("leiste-schalten").setAttribute("aria-expanded", String(!zu));
});

$("zu-einstellungen").addEventListener("click", async () => {
  $("einstellungsseite").hidden = false;
  await einstellungen_zeichnen();
  // Beim Oeffnen ist nichts im Bau, also auch kein Balken.
  if (!baut) {
    $("baustand").hidden = true;
    $("bauzeilen").textContent = "";
  }
  await katalog_zeichnen();
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
  laden_aus_speicher();
  if (gespraeche.length === 0) neues_gespraech();
  else offen = gespraeche[0];
  alles_zeichnen();
  try {
    await modellwahl_zeichnen();
    await kopf_zeichnen();
  } catch (f) {
    hinweis(`Fehler: ${f}`);
  }
  await vorhangWeg();
  feld.focus();
})();
