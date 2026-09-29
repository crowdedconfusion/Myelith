// Das Vorschaltbild im eigenen Faden: dieselben Szenen, auf einer
// `OffscreenCanvas`, unberuehrt von allem, was der Hauptfaden beim Start
// des Fensters rechnet (Projektinhaber, 2026-09-29: „ruckelfrei“).
//
// ⚑ `requestAnimationFrame` gibt es in einem Worker nicht ueberall; wo es
// fehlt, taktet ein Zeitgeber mit rund sechzig Bildern je Sekunde. Die
// Bewegung haengt an der Uhr, nicht an der Bildzahl, also bleibt sie gleich
// schnell.

import { zeichner } from "./vorhang.js";

let z = null;
let leinwand = null;
let stift = null;
let laeuft = false;

const naechstes =
  typeof requestAnimationFrame === "function"
    ? (f) => requestAnimationFrame(f)
    : (f) => setTimeout(() => f(performance.now()), 16);

function masse(d) {
  leinwand.width = Math.round(d.breite * d.dpr);
  leinwand.height = Math.round(d.hoehe * d.dpr);
  z.aufbauen(d.breite, d.hoehe, d.dpr);
}

function bild(jetzt) {
  if (!laeuft) return;
  z.zeichnen(stift, jetzt);
  naechstes(bild);
}

self.onmessage = (e) => {
  const d = e.data;
  if (d.art === "start") {
    leinwand = d.leinwand;
    stift = leinwand.getContext("2d");
    z = zeichner(d.nummer, d.still);
    masse(d);
    z.zeichnen(stift, performance.now());
    laeuft = true;
    // Das erste Bild steht: Jetzt darf eingeblendet werden.
    self.postMessage({ art: "bereit" });
    if (!d.still) naechstes(bild);
  } else if (d.art === "groesse" && z) {
    masse(d);
    z.zeichnen(stift, performance.now());
  } else if (d.art === "halt") {
    laeuft = false;
  }
};
