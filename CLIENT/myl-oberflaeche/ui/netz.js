// Das Vorschaltbild: aus grauem Rauschen wird ein Wirbel, aus dessen
// Mitte das Netz waechst.
//
// # ⚑ Es traegt eine echte Wartezeit
//
// Ein Artefakt dieser Groesse laedt rund elf Sekunden. Eine Animation
// ohne Wartezeit dahinter waere eine Verzoegerung; diese ist eine
// Fortschrittsanzeige. Deshalb hat sie kein Ende: Sie laeuft in einen
// ruhigen Zustand und bleibt dort, bis das Modell da ist.
//
// # Die Phasen
//
// | ab | was |
// |---|---|
// | 0,0 s | **Rauschen.** Koerner ueber die ganze Flaeche, jedes Bild andere Helligkeit. Ein Schneesturm. |
// | 1,0 s | **Sog.** Die Koerner geraten in einen Wirbel: Sie wandern nach innen und drehen sich dabei, innen schneller als aussen. Wer die Mitte erreicht, verschwindet. |
// | 2,2 s | **Netz.** Aus der Mitte treten Knoten heraus, wandern auf ihre Plaetze und verbinden sich. |
// | 2,4 s | Der Schriftzug tritt aus der Unschaerfe (siehe `stil.css`, `aus_dem_rauschen`). |
//
// Die Phasen ueberlappen mit Absicht. Ein Rauschen, das erst ganz
// verschwindet und dann ein Netz freigibt, waeren zwei Bilder
// nacheinander; hier wird das eine zum anderen.
//
// # ⚑ Warum der Wirbel innen schneller dreht
//
// Weil echte Wirbel das tun: Die Winkelgeschwindigkeit waechst zur
// Mitte hin. Mit gleicher Winkelgeschwindigkeit ueberall drehte sich
// das Bild wie eine Scheibe, und das sieht aus wie ein Ladekreis. Die
// Spirale entsteht ueberhaupt erst aus dem Unterschied.
//
// # ⚑ Canvas und nicht SVG
//
// Ein paar tausend Koerner, die sich je Bild aendern, waeren in SVG
// ebenso viele Knoten im Dokument; auf Canvas sind sie ein
// Zeichenbefehl je Helligkeitsstufe.

// --- Zeiten in Sekunden ------------------------------------------------
const SOG_AB = 1.0;
const NETZ_AB = 2.2;
const NETZ_DAUER = 1.8;

// --- Rauschen ----------------------------------------------------------
// ⚑ Die Zahl folgt der Flaeche und nicht einer festen Zahl: Sonst ist
// derselbe Sturm auf einem grossen Schirm ein Nieseln und auf einem
// kleinen eine Wand.
const KOERNER_JE_FLAECHE = 1 / 260;
const KOERNER_HOECHSTENS = 9000;
// ⚑ Drei feste Helligkeitsstufen statt einer je Korn. Eine eigene
// Deckung je Korn hiesse ein `fillStyle` je Korn, also mehrere tausend
// Zustandswechsel je Bild; drei Stufen sind drei. Das Flimmern kommt
// statt dessen daher, dass je Bild ein Teil der Koerner ausgelassen
// wird, und das sieht genauso aus.
const STUFEN = [0.20, 0.42, 0.75];
const FLIMMERN = 0.45;

// --- Netz --------------------------------------------------------------
const ABSTAND = 190;
const KNOTEN_JE_FLAECHE = 1 / 26000;

/**
 * Startet das Vorschaltbild auf der uebergebenen Leinwand.
 *
 * ⚑ Der Rueckgabewert haelt es an. Eine Animation, die hinter einem
 * unsichtbaren Vorhang weiterlaeuft, kostet weiter Rechenzeit, und
 * genau die soll dem Modell gehoeren.
 */
export function netzStarten(leinwand) {
  const stift = leinwand.getContext("2d");
  let breite = 0;
  let hoehe = 0;
  let mx = 0;
  let my = 0;
  let koerner = [];
  let knoten = [];
  let laeuft = true;
  let beginn = 0;
  let vorher = 0;
  // ⛑ **Der Sturm wird abgeschaltet und nicht nur unsichtbar.** Die
  // Phasenprobe zeigte, dass nach 3,6 s zwar nichts mehr gezeichnet
  // wird, die zweitausend Koerner aber weiter gerechnet werden, und das
  // fuer die ganze restliche Ladezeit. Genau die Rechenzeit soll dem
  // Modell gehoeren.
  let sturm = true;

  const messen = () => {
    const v = window.devicePixelRatio || 1;
    breite = leinwand.clientWidth;
    hoehe = leinwand.clientHeight;
    leinwand.width = Math.round(breite * v);
    leinwand.height = Math.round(hoehe * v);
    stift.setTransform(v, 0, 0, v, 0, 0);
    mx = breite / 2;
    my = hoehe / 2;

    // ⚑ Nach dem Sturm legt eine Groessenaenderung keine Koerner mehr
    // an: Sonst begaenne der Schneesturm mitten im Netz von vorn, weil
    // jemand das Fenster gezogen hat.
    const wie_viele = sturm
      ? Math.min(KOERNER_HOECHSTENS, Math.round(breite * hoehe * KOERNER_JE_FLAECHE))
      : 0;
    // ⚑ Die Koerner liegen in Polarkoordinaten um die Mitte, denn der
    // Wirbel rechnet in Winkel und Radius. In kartesischen waere jedes
    // Bild eine Umrechnung hin und zurueck.
    const eck = Math.hypot(breite, hoehe) / 2;
    koerner = Array.from({ length: wie_viele }, () => {
      const x = Math.random() * breite;
      const y = Math.random() * hoehe;
      return {
        r: Math.hypot(x - mx, y - my),
        w: Math.atan2(y - my, x - mx),
        // Wann dieses Korn in den Sog geraet. Ohne Streuung riesse der
        // ganze Sturm auf einmal ab, und das saehe aus wie ein Schnitt.
        ab: SOG_AB + Math.random() * 0.9,
        stufe: (Math.random() * STUFEN.length) | 0,
        tot: false,
      };
    });
    // Aussen liegende Koerner ergaenzen, damit die Ecken nicht leer
    // wirken, sobald der Sog begonnen hat.
    for (const k of koerner) {
      if (k.r > eck) k.r = Math.random() * eck;
    }

    const knotenzahl = Math.max(
      14,
      Math.min(70, Math.round(breite * hoehe * KNOTEN_JE_FLAECHE)),
    );
    knoten = Array.from({ length: knotenzahl }, () => {
      const zx = Math.random() * breite;
      const zy = Math.random() * hoehe;
      const d = Math.hypot(zx - mx, zy - my);
      return {
        zx,
        zy,
        x: mx,
        y: my,
        dx: (Math.random() - 0.5) * 0.16,
        dy: (Math.random() - 0.5) * 0.16,
        gr: 1.4 + Math.random() * 2.6,
        // ⚑ Nahe Knoten treten zuerst heraus. Dadurch waechst das Netz
        // aus der Mitte, statt ueberall gleichzeitig zu erscheinen.
        ab: NETZ_AB + (d / (Math.hypot(breite, hoehe) / 2)) * 0.7,
        // Auf welcher Seite der Knoten herausschwingt.
        drall: Math.random() < 0.5 ? -1 : 1,
      };
    });
  };

  /** Weicher Uebergang von 0 auf 1 zwischen `a` und `b`. */
  const stufe = (x, a, b) => {
    const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
    return t * t * (3 - 2 * t);
  };

  const rauschen = (t, dt) => {
    let lebende = 0;
    for (const k of koerner) {
      if (k.tot) continue;
      if (t > k.ab) {
        // ⚑ Der Wirbel: Winkelgeschwindigkeit umgekehrt zum Radius,
        // Sog nach innen wachsend. Die 40 im Nenner haelt die Drehung
        // in der Mitte endlich, sonst dreht das letzte Korn unendlich
        // schnell.
        const alter = t - k.ab;
        k.w += (dt * 320) / (k.r + 40);
        k.r -= dt * (24 + alter * 90);
        if (k.r < 6) {
          k.tot = true;
          continue;
        }
      }
      lebende++;
    }
    if (lebende === 0) return 0;

    // ⚑ Ausblenden ueber alles, sobald der Sog laeuft: Der Sturm wird
    // duenner UND schwaecher, sonst bliebe ein heller Kern stehen.
    const staerke = 1 - stufe(t, SOG_AB + 0.4, NETZ_AB + 1.4);
    if (staerke <= 0) {
      sturm = false;
      koerner = [];
      return 0;
    }

    for (let s = 0; s < STUFEN.length; s++) {
      stift.fillStyle = `rgba(226,230,234,${STUFEN[s] * staerke})`;
      for (const k of koerner) {
        if (k.tot || k.stufe !== s) continue;
        if (Math.random() < FLIMMERN) continue;
        stift.fillRect(mx + Math.cos(k.w) * k.r, my + Math.sin(k.w) * k.r, 1.2, 1.2);
      }
    }
    return lebende;
  };

  const netz = (t) => {
    for (const k of knoten) {
      const a = stufe(t, k.ab, k.ab + NETZ_DAUER);
      if (a <= 0) {
        k.sichtbar = 0;
        continue;
      }
      k.sichtbar = a;
      // ⚑ Der Weg nach aussen ist eine Kurve und keine Gerade: Der
      // Knoten schwingt auf demselben Drehsinn heraus, in dem der
      // Wirbel eingesogen hat. Erst dadurch sieht das Netz aus, als
      // entstuende es AUS der Spirale und nicht daneben.
      const bogen = (1 - a) * 1.5 * k.drall;
      const dx = k.zx - mx;
      const dy = k.zy - my;
      const co = Math.cos(bogen);
      const si = Math.sin(bogen);
      k.x = mx + (dx * co - dy * si) * a;
      k.y = my + (dx * si + dy * co) * a;
      // Sobald der Knoten steht, treibt er leise weiter.
      if (a >= 1) {
        k.zx += k.dx;
        k.zy += k.dy;
        if (k.zx < 0 || k.zx > breite) k.dx *= -1;
        if (k.zy < 0 || k.zy > hoehe) k.dy *= -1;
        k.x = k.zx;
        k.y = k.zy;
      }
    }

    // Erst die Verbindungen, dann die Knoten darueber.
    stift.lineWidth = 0.7;
    for (let i = 0; i < knoten.length; i++) {
      const a = knoten[i];
      if (!a.sichtbar) continue;
      for (let j = i + 1; j < knoten.length; j++) {
        const b = knoten[j];
        if (!b.sichtbar) continue;
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        if (d > ABSTAND) continue;
        stift.strokeStyle =
          `rgba(200,206,212,${(1 - d / ABSTAND) * 0.2 * a.sichtbar * b.sichtbar})`;
        stift.beginPath();
        stift.moveTo(a.x, a.y);
        stift.lineTo(b.x, b.y);
        stift.stroke();
      }
    }
    for (const k of knoten) {
      if (!k.sichtbar) continue;
      stift.fillStyle = `rgba(236,238,240,${0.85 * k.sichtbar})`;
      stift.beginPath();
      stift.arc(k.x, k.y, k.gr * k.sichtbar, 0, Math.PI * 2);
      stift.fill();
    }
  };

  const zeichnen = (jetzt) => {
    if (!laeuft) return;
    if (!beginn) {
      beginn = jetzt;
      vorher = jetzt;
    }
    const t = (jetzt - beginn) / 1000;
    // ⛑ Der Schritt wird gedeckelt. Wird das Fenster minimiert, laeuft
    // `requestAnimationFrame` nicht, und beim Zurueckkommen waere `dt`
    // mehrere Sekunden gross: Der ganze Wirbel spraenge in einem Bild
    // durch. Ein Deckel von 50 ms macht daraus eine kurze Zeitlupe.
    const dt = Math.min(0.05, (jetzt - vorher) / 1000);
    vorher = jetzt;

    stift.clearRect(0, 0, breite, hoehe);
    if (sturm) rauschen(t, dt);
    if (t > NETZ_AB) netz(t);
    requestAnimationFrame(zeichnen);
  };

  messen();
  window.addEventListener("resize", messen);
  requestAnimationFrame(zeichnen);

  return () => {
    laeuft = false;
    window.removeEventListener("resize", messen);
  };
}
