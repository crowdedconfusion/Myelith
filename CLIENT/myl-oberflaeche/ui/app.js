// ⚑ Kein Bauwerkzeug, kein Rahmenwerk. Die Oberflaeche ruft dieselben
// Unterbefehle wie `myl` und hat keine eigene Logik; dafuer braucht es
// weder einen Buendler noch eine Node-Werkzeugkette.
//
// # Der Aufbau, seit dem 2026-09-09
//
// Ein Gespraechsfenster: links eine Seitenleiste mit Modus und
// Gespraechen, oben mittig der Ort (lokal oder Netz), oben rechts das
// Zahnrad. Davor waren es drei untereinanderliegende Abschnitte.
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

// --- Der Ort ------------------------------------------------------------

// ⛑ **`netz` ist noch nicht verdrahtet, und das steht hier statt in
// einer Ausrede.** Der Klient hat kein Feld fuer eine Knotenadresse und
// keines fuer eine Vollmacht; das sind die Punkte 2.2 bis 2.5b des
// Fahrplans. Der Schalter ist trotzdem da, weil der Ort die Frage ist,
// die ueber allem steht, und weil ein Schalter, der still nichts tut,
// schlimmer waere als einer, der sagt was fehlt.
const ORTE = {
  lokal: { offen: true },
  netz: {
    offen: false,
    warum:
      "Netz ist noch nicht verdrahtet: Es fehlen die Knotenadresse und die Vollmacht (Fahrplan 2.2 bis 2.5b). Bis dahin rechnet diese Maschine.",
  },
};
let ort = "lokal";

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
    const n = document.createElement("span");
    n.textContent = m.name;
    const s = document.createElement("span");
    s.className = "was";
    s.textContent = m.was;
    b.append(n, s);
    b.addEventListener("click", () => {
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

function chats_zeichnen() {
  const w = $("chatliste");
  w.replaceChildren();
  for (const g of gespraeche) {
    const zeile = document.createElement("div");
    zeile.className = "chat blank";
    zeile.setAttribute("role", "listitem");
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

function ort_zeichnen() {
  const w = $("ort");
  const knoepfe = [...w.querySelectorAll("button")];
  for (const b of knoepfe) {
    const gewaehlt = b.dataset.ort === ort;
    b.setAttribute("aria-checked", String(gewaehlt));
    b.dataset.offen = ORTE[b.dataset.ort].offen ? "ja" : "nein";
  }
  const aktiv = knoepfe.find((b) => b.dataset.ort === ort);
  const reiter = w.querySelector(".reiter");
  if (aktiv && reiter) {
    reiter.style.width = `${aktiv.offsetWidth}px`;
    reiter.style.transform = `translateX(${aktiv.offsetLeft - 3}px)`;
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
  chats_zeichnen();
  ort_zeichnen();
  gespraech_zeichnen();
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
  const m = $("betriebsart");
  m.textContent = e.betriebsart;
  m.title = e.betriebsart_warum;
  const offen_jetzt = e.bezeugtes && e.wurzel;
  m.classList.toggle("eng", !offen_jetzt);
  m.classList.toggle("offen", Boolean(offen_jetzt));
  $("modellzeile").textContent = geladen ? e.artefakt : `${e.artefakt} (nicht geladen)`;
  return e;
}

// --- Modell -------------------------------------------------------------

let geladen = false;

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
    hinweis("");
  } catch (f) {
    offen.beitraege.push({ von: "modell", text: `Fehler: ${f}`, fuss: "", fehler: true });
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
// ⚑ **Gerechnet wird ein Winkel und keine Stelle.** `--w` ist der
// Winkel von der Mitte der Flaeche zum Zeiger, im Bezugssystem von
// `conic-gradient`: null bei zwoelf Uhr, im Uhrzeigersinn. Der Verlauf
// im Stil traegt seine scharfe Spitze bei null Grad, also liegt sie
// immer dem Zeiger zugewandt.
//
// ⚑ **Ein Horcher am Fenster und nicht einer je Knopf.** Die
// Oberflaeche baut ihre Knoepfe staendig neu (Gespraechsliste, Modi);
// je Element einen Horcher zu haengen hiesse, sie bei jedem Neuzeichnen
// wieder zu haengen und die alten zu vergessen.
//
// ⚑ **Und gerechnet wird im Bildtakt.** `mousemove` feuert oefter als
// der Schirm zeichnet; ohne die Sperre setzte man Werte, die niemand je
// sieht, und das kostet Rechenzeit, die dem Modell gehoert.
const LINSEN = ".glas, .eingabefeld, button:not(.blank), .schalter, section";
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
      const dx = z.clientX - (r.left + r.width / 2);
      const dy = z.clientY - (r.top + r.height / 2);
      // `atan2` misst von drei Uhr gegen den Uhrzeigersinn, `conic`
      // von zwoelf Uhr im Uhrzeigersinn; die Bildschirmachse zeigt nach
      // unten, also dreht sich das Vorzeichen mit. Plus neunzig Grad
      // bringt beide zur Deckung.
      const w = (Math.atan2(dy, dx) * 180) / Math.PI + 90;
      el.style.setProperty("--w", String(w));
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

for (const b of $("ort").querySelectorAll("button")) {
  b.addEventListener("click", () => {
    const gewuenscht = b.dataset.ort;
    if (!ORTE[gewuenscht].offen) {
      hinweis(ORTE[gewuenscht].warum);
      return;
    }
    ort = gewuenscht;
    hinweis("");
    ort_zeichnen();
  });
}

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

window.addEventListener("resize", ort_zeichnen);

// --- Start --------------------------------------------------------------

(async () => {
  laden_aus_speicher();
  if (gespraeche.length === 0) neues_gespraech();
  else offen = gespraeche[0];
  alles_zeichnen();
  try {
    await kopf_zeichnen();
  } catch (f) {
    hinweis(`Fehler: ${f}`);
  }
  await vorhangWeg();
  feld.focus();
})();
