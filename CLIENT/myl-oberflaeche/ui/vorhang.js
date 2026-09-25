// Das Vorschaltbild: eine von fuenf Szenen, beim Start gewuerfelt, ruhig
// in Bewegung, darueber die Marke.
//
// # ⚑ Gezeichnet und nicht als Bild mitgeliefert
//
// Die Szenen folgen fuenf Vorlagen des Projektinhabers (2026-09-25):
// ein Spiraltunnel aus welligen Baendern, ein Tunnel aus Vielecken, eine
// Welle aus Dreiecksflaechen, ein Wirbel aus Flecken und ein Mandala aus
// Schleifen. **Nachgebaut und nicht eingebettet**, auf seine
// Entscheidung hin: Ein fremdes Bild traegt ein Urheberrecht, eine
// Bildidee nicht. Nebenbei ist eine gezeichnete Szene auf jeder
// Fenstergroesse scharf und kostet im Repositorium keine Bilddatei.
//
// # ⚑ Eine Schleife ohne Naht
//
// Vier der fuenf Szenen sind **selbstaehnlich** gebaut: Jeder Ring ist
// der vorige, um den Faktor `q` vergroessert und um `dreh` gedreht. Eine
// Vergroesserung um `q` mit einer Drehung um `dreh` bildet das Bild also
// auf sich selbst ab. Die Bewegung fuehrt genau diese Abbildung stetig
// aus, von `q^0` bis `q^1`, und steht am Ende dort, wo sie begonnen hat.
// Das sieht aus wie ein ruhiger Flug in den Wirbel und hat keinen
// Sprung.
//
// Je Bild wird dabei **nichts neu gerechnet**: Die Szene liegt einmal
// als Leinwand im Speicher und wird nur gedreht und vergroessert. Das
// Vorschaltbild steht, waehrend das Fenster startet, und soll dem nicht
// die Rechenzeit nehmen.
//
// # ⚑ Wer Bewegung abbestellt hat
//
// bekommt ein Standbild derselben Szene statt keines. Der Vorhang traegt
// eine Wartezeit, und die soll man sehen koennen, nur eben still.

// --- Zufall ------------------------------------------------------------

/**
 * Eine Zufallsquelle mit Saat, damit jede Szene bei jedem Start gleich
 * aussieht und nur die Wahl der Szene wuerfelt.
 */
function zufallsquelle(saat) {
  let s = saat >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// --- Leinwaende ---------------------------------------------------------

function leinwand(breite, hoehe) {
  const l = document.createElement("canvas");
  l.width = Math.max(1, Math.ceil(breite));
  l.height = Math.max(1, Math.ceil(hoehe));
  return l;
}

/**
 * Eine Kopie mit Unschaerfe, fuer die Tiefe: in zwei Stufen verkleinert
 * und wieder hochgezogen, mit weicher Interpolation.
 *
 * 📌 **Nicht ueber `filter` auf der Leinwand.** Die Webansicht unter
 * macOS kennt die Eigenschaft, zeichnete aber gemessen scharf; eine
 * Unschaerfe, die auf einem der drei Systeme fehlt, ist keine
 * Gestaltung. Verkleinern koennen alle.
 */
function unscharf(quelle, stufen) {
  // ⚑ In Halbierungen: Jede mittelt je vier Pixel sauber zu einem. Ein
  //   einziger grosser Schritt zog Stichproben statt zu mitteln, und die
  //   feinen Perlen der Baender wurden zu grobem, flackerndem Rauschen.
  const schritt = (q, faktor) => {
    const z = leinwand(q.width * faktor, q.height * faktor);
    const s = z.getContext("2d");
    s.imageSmoothingEnabled = true;
    s.imageSmoothingQuality = "high";
    s.drawImage(q, 0, 0, z.width, z.height);
    return z;
  };
  let bild = quelle;
  for (let i = 0; i < stufen; i++) bild = schritt(bild, 0.5);
  for (let i = 0; i < stufen; i++) bild = schritt(bild, 2);
  const ziel = leinwand(quelle.width, quelle.height);
  const s = ziel.getContext("2d");
  s.imageSmoothingEnabled = true;
  s.drawImage(bild, 0, 0, ziel.width, ziel.height);
  return ziel;
}

/**
 * Zeichnet eine selbstaehnliche Textur um die Bildmitte, vergroessert um
 * `q^u` und gedreht um `dreh * u`.
 *
 * ⚑ **Mit Ueberblendung in den naechsten Umlauf** (`mischung`). Eine
 * Textur ist nur so selbstaehnlich wie ihr Raster: Am Ende des Umlaufs
 * ist sie um `q` hochgezogen und damit weicher als am Anfang, und
 * haarfeine Linien werden anders geglaettet. Gemessen sprang das Bild an
 * der Wende deshalb sichtbar. Hier wird die Textur ueber den Umlauf
 * hinweg in sich selbst einen Umlauf weiter uebergeblendet; bei `u = 1`
 * steht damit genau das Bild von `u = 0` da.
 *
 * - `"lighter"` fuer helle Linien auf dunklem Grund: Beide Lagen addieren
 *   sich, und wo sie sich decken, ergibt die Summe genau eine Linie.
 * - `"source-over"` fuer deckende Texturen: Die zweite Lage legt sich
 *   mit wachsender Deckung ueber die erste.
 * - ohne Angabe keine Ueberblendung, fuer eine Textur, die exakt
 *   selbstaehnlich ist.
 */
function schleife(stift, textur, massstab, mx, my, q, dreh, u, mischung) {
  const lage = (v, deckung) => {
    const s = Math.pow(q, v) / massstab;
    stift.save();
    if (mischung) stift.globalCompositeOperation = mischung;
    stift.globalAlpha = deckung;
    stift.translate(mx, my);
    stift.rotate(dreh * v);
    stift.scale(s, s);
    stift.drawImage(textur, -textur.width / 2, -textur.height / 2);
    stift.restore();
  };
  if (!mischung) {
    lage(u, 1);
  } else if (mischung === "lighter") {
    lage(u, 1 - u);
    lage(u - 1, u);
  } else {
    lage(u, 1);
    lage(u - 1, u);
  }
}

/** Ein weicher dunkler Hof in der Mitte, damit die Marke lesbar bleibt. */
function hof(stift, mx, my, radius, staerke) {
  const g = stift.createRadialGradient(mx, my, 0, mx, my, radius);
  g.addColorStop(0, `rgba(8, 9, 10, ${staerke})`);
  g.addColorStop(0.55, `rgba(8, 9, 10, ${staerke * 0.55})`);
  g.addColorStop(1, "rgba(8, 9, 10, 0)");
  stift.fillStyle = g;
  stift.fillRect(mx - radius, my - radius, 2 * radius, 2 * radius);
}

/** Dunkle Raender, wie durch ein Objektiv. */
function vignette(stift, breite, hoehe, staerke) {
  const mx = breite / 2;
  const my = hoehe / 2;
  const r = Math.hypot(mx, my);
  const g = stift.createRadialGradient(mx, my, r * 0.35, mx, my, r);
  g.addColorStop(0, "rgba(0, 0, 0, 0)");
  g.addColorStop(1, `rgba(0, 0, 0, ${staerke})`);
  stift.fillStyle = g;
  stift.fillRect(0, 0, breite, hoehe);
}

/**
 * Die Masse einer selbstaehnlichen Textur: Sie muss die Bildflaeche auch
 * bei der kleinsten Vergroesserung und jeder Drehung ganz decken, also
 * die Diagonale.
 */
function texturmasse(breite, hoehe, dpr) {
  const aufloesung = Math.min(dpr, 1.5);
  const seite = Math.min(2600, Math.ceil(Math.hypot(breite, hoehe) * 1.04 * aufloesung));
  return { seite, massstab: aufloesung };
}

/**
 * Ein radialer Verlauf ueber die ganze Flaeche, fest am Schirm.
 *
 * ⚑ **Alles, was vom Abstand zur Mitte abhaengt, steht hier und nicht in
 * der Textur**: Helligkeit, Tiefe, Grund, Kern. Die Textur dreht und
 * waechst; was in ihr vom Radius abhinge, waere nach einem Umlauf um
 * einen Ring verschoben, und an der Wende der Schleife sprang das Bild
 * (gemessen, siehe `SZENEN`). Am Schirm dagegen bleibt es stehen.
 */
function radial(stift, breite, hoehe, stopps, modus = "source-over") {
  const mx = breite / 2;
  const my = hoehe / 2;
  const r = Math.hypot(mx, my);
  const g = stift.createRadialGradient(mx, my, 0, mx, my, r);
  for (const [lage, farbe] of stopps) g.addColorStop(lage, farbe);
  stift.save();
  stift.globalCompositeOperation = modus;
  stift.fillStyle = g;
  stift.fillRect(0, 0, breite, hoehe);
  stift.restore();
}

/** Ein Punkt eines Rings: der Grundring, vergroessert und gedreht. */
function ringpunkt(c, r, winkel, x0, y0) {
  const cw = Math.cos(winkel);
  const sw = Math.sin(winkel);
  return [c + r * (cw * x0 - sw * y0), c + r * (sw * x0 + cw * y0)];
}

// --- Szene 1: Spiraltunnel aus welligen Baendern ------------------------

/**
 * Helle, gekoernte Baender, die sich in Wellen um einen dunklen Kern
 * legen; aussen unscharf, als laege die Schaerfe in der Tiefe.
 */
function wellenbaender(breite, hoehe, dpr) {
  const q = 1.075;
  const dreh = 0.13;
  const { seite, massstab } = texturmasse(breite, hoehe, dpr);
  const textur = leinwand(seite, seite);
  const s = textur.getContext("2d");
  const c = seite / 2;
  s.lineCap = "round";
  const zipfel = 15;
  // ⚑ Ring i ist der Grundring, um q^i vergroessert und um i * dreh
  //   gedreht, samt Strichbreite und Koernung. Nur so ist die Textur
  //   unter der Schleife sich selbst gleich.
  for (let r = 1, i = 0; r < seite * 0.75; r *= q, i++) {
    const breiteLinie = r * 0.016;
    for (const [phase, deckung] of [[0, 0.85], [Math.PI / zipfel, 0.5]]) {
      s.beginPath();
      const schritte = Math.max(120, Math.ceil(r * 1.1));
      for (let k = 0; k <= schritte; k++) {
        const th = (k / schritte) * 2 * Math.PI;
        // Eine Welle mit Schlaufe: Kreis plus schnellerer Umlauf.
        const x0 = Math.cos(th + phase) + 0.07 * Math.cos((zipfel + 1) * th + phase);
        const y0 = Math.sin(th + phase) + 0.07 * Math.sin((zipfel + 1) * th + phase);
        const [x, y] = ringpunkt(c, r, i * dreh, x0, y0);
        if (k === 0) s.moveTo(x, y);
        else s.lineTo(x, y);
      }
      // ⚑ Gekoernt: ein Band aus Perlen statt einer glatten Linie.
      s.setLineDash([breiteLinie * 0.3, breiteLinie * 0.6]);
      s.lineWidth = breiteLinie;
      s.strokeStyle = `rgba(238, 238, 238, ${deckung})`;
      s.stroke();
    }
  }
  s.setLineDash([]);
  const weich = unscharf(textur, 3);

  // Die scharfe Zone als Maske: innen scharf, aussen die weiche Kopie.
  const schicht = leinwand(breite * dpr, hoehe * dpr);
  const ss = schicht.getContext("2d");
  return {
    // ⚑ Textur und Schleifenmass stehen offen, damit eine Probe die
    //   Selbstaehnlichkeit an der Textur selbst pruefen kann.
    schleifenmass: { textur, q, dreh },
    periode: 11,
    zeichnen(stift, t) {
      const u = (t / this.periode) % 1;
      const mx = breite / 2;
      const my = hoehe / 2;
      stift.fillStyle = "#040404";
      stift.fillRect(0, 0, breite, hoehe);
      schleife(stift, weich, massstab, mx, my, q, dreh, u, "lighter");
      ss.setTransform(dpr, 0, 0, dpr, 0, 0);
      ss.globalCompositeOperation = "source-over";
      ss.clearRect(0, 0, breite, hoehe);
      schleife(ss, textur, massstab, mx, my, q, dreh, u, "lighter");
      ss.globalCompositeOperation = "destination-in";
      const rand = Math.min(breite, hoehe);
      const g = ss.createRadialGradient(mx, my, rand * 0.1, mx, my, rand * 0.55);
      g.addColorStop(0, "rgba(0, 0, 0, 1)");
      g.addColorStop(1, "rgba(0, 0, 0, 0)");
      ss.fillStyle = g;
      ss.fillRect(0, 0, breite, hoehe);
      stift.drawImage(schicht, 0, 0, breite, hoehe);
      // Hell in der Tiefe, grauer nach aussen, und der dunkle Kern.
      // ⚑ Die Mitte gedaempft: Dort liegen die Ringe am dichtesten, und
      //   ihre Summe wuerde genau unter dem Schriftzug weiss.
      radial(stift, breite, hoehe, [[0, "rgba(0, 0, 0, 0.95)"], [0.04, "rgba(0, 0, 0, 0.62)"],
        [0.13, "rgba(0, 0, 0, 0.35)"], [0.3, "rgba(0, 0, 0, 0.12)"], [1, "rgba(0, 0, 0, 0.72)"]]);
    },
    hofstaerke: 0.55,
  };
}

// --- Szene 2: Tunnel aus Vielecken --------------------------------------

/**
 * Siebenecke, dicht gestaffelt und je Ring ein wenig gedreht: Ihre
 * Kanten laufen zu Buendeln zusammen, dazwischen liegen matte Flaechen.
 */
function vielecke(breite, hoehe, dpr) {
  const q = 1.055;
  const dreh = 0.045;
  // ⚑ Ein Umlauf sind vier Ringe: Jede vierte Kante ist hell, und die
  //   Schleife muss genau um diese Staffelung weiterruecken.
  const ringeJeUmlauf = 4;
  const { seite, massstab } = texturmasse(breite, hoehe, dpr);
  const textur = leinwand(seite, seite);
  const s = textur.getContext("2d");
  const c = seite / 2;
  const zufall = zufallsquelle(0x7e11);
  const vieleck = (r, w, ecken) => {
    s.beginPath();
    for (let k = 0; k <= ecken; k++) {
      const th = (k / ecken) * 2 * Math.PI;
      const [x, y] = ringpunkt(c, r, w, Math.cos(th), Math.sin(th));
      if (k === 0) s.moveTo(x, y);
      else s.lineTo(x, y);
    }
  };
  // Zwei Familien, Siebenecke und um einen halben Ring versetzte
  // Sechsecke, beide mit derselben Drehung je Ring.
  const familien = [[7, 0, 1], [6, Math.PI / 6, Math.sqrt(q)]];
  const ringe = [];
  for (let r = 0.8, i = 0; r < seite * 0.78; r *= q, i++) ringe.push([r, i]);
  for (const [r, i] of ringe) {
    if (i % ringeJeUmlauf !== 0) continue;
    vieleck(r * q * q, (i + 2) * dreh, 7);
    vieleck(r, i * dreh, 7);
    s.fillStyle = "rgba(225, 225, 225, 0.09)";
    s.fill("evenodd");
  }
  for (const [ecken, versatz, stufe] of familien) {
    for (const [r, i] of ringe) {
      vieleck(r * stufe, i * dreh + versatz, ecken);
      s.lineWidth = r * 0.0026;
      const hell = (i % ringeJeUmlauf === 0 ? 0.75 : 0.3) * (ecken === 7 ? 1 : 0.55);
      s.strokeStyle = `rgba(250, 250, 250, ${hell})`;
      s.stroke();
    }
  }
  // Ein Hauch Korn, fest am Schirm.
  const korn = leinwand(128, 128);
  const k = korn.getContext("2d");
  const pix = k.createImageData(128, 128);
  for (let p = 0; p < pix.data.length; p += 4) {
    const v = 120 + zufall() * 135;
    pix.data[p] = pix.data[p + 1] = pix.data[p + 2] = v;
    pix.data[p + 3] = 10;
  }
  k.putImageData(pix, 0, 0);
  let kornmuster = null;
  return {
    schleifenmass: { textur, q: Math.pow(q, ringeJeUmlauf), dreh: ringeJeUmlauf * dreh },
    periode: 9,
    zeichnen(stift, t) {
      const u = (t / this.periode) % 1;
      // ⚑ Dunkel in der Tiefe, heller zum Rand: ein Schacht, in den man sieht.
      radial(stift, breite, hoehe, [[0, "#050505"], [0.3, "#262626"], [1, "#565656"]]);
      schleife(stift, textur, massstab, breite / 2, hoehe / 2,
        Math.pow(q, ringeJeUmlauf), ringeJeUmlauf * dreh, u, "lighter");
      // Die inneren Ringe liegen so dicht, dass sie zu einem Grau
      // verschwimmen; die Mitte wird deshalb am Schirm abgedunkelt.
      radial(stift, breite, hoehe, [[0, "rgba(0, 0, 0, 0.92)"], [0.14, "rgba(0, 0, 0, 0)"]]);
      kornmuster ??= stift.createPattern(korn, "repeat");
      stift.fillStyle = kornmuster;
      stift.fillRect(0, 0, breite, hoehe);
      vignette(stift, breite, hoehe, 0.6);
    },
    hofstaerke: 0.6,
  };
}

// --- Szene 3: Welle aus Dreiecksflaechen --------------------------------

/**
 * Ein Feld aus flach schattierten Dreiecken auf Schwarz, durch das eine
 * helle Welle zieht; sie hebt und senkt sich langsam.
 */
function dreieckswelle(breite, hoehe) {
  const zufall = zufallsquelle(0x3a1f);
  const masche = Math.max(38, Math.min(breite, hoehe) / 14);
  const spalten = Math.ceil(breite / masche) + 2;
  const zeilen = Math.ceil(hoehe / masche) + 2;
  // Feste Unordnung je Punkt, damit die Flaechen nicht wie ein Raster aussehen.
  const punkte = [];
  for (let j = 0; j < zeilen; j++) {
    for (let i = 0; i < spalten; i++) {
      punkte.push({
        x: (i - 1) * masche + (zufall() - 0.5) * masche * 0.7,
        y: (j - 1) * masche + (zufall() - 0.5) * masche * 0.7,
        phase: zufall() * 2 * Math.PI,
      });
    }
  }
  // Die Hoehe: zwei Baender, das grosse quer von links nach oben rechts,
  // ein kleines unten rechts.
  const hoeheBei = (x, y, w) => {
    const nx = x / breite;
    const mitte1 = hoehe * (0.62 - 0.32 * nx * nx + 0.05 * Math.sin(2 * Math.PI * nx + w));
    const b1 = Math.exp(-(((y - mitte1) / (hoehe * 0.11)) ** 2));
    const dx = x - breite * 0.95;
    const dy = y - hoehe * 0.95;
    const b2 = 0.9 * Math.exp(-((dx * dx + dy * dy) / (hoehe * hoehe * 0.05)));
    return b1 * (0.75 + 0.25 * Math.sin(3 * nx + w)) + b2;
  };
  return {
    // Stetig in der Zeit, ohne Wende; die Periode nennt nur den Takt.
    periode: 14,
    zeichnen(stift, t) {
      const w = (t / this.periode) * 2 * Math.PI;
      stift.fillStyle = "#070707";
      stift.fillRect(0, 0, breite, hoehe);
      const lage = punkte.map((p) => {
        const x = p.x + Math.sin(w + p.phase) * masche * 0.05;
        const y = p.y + Math.cos(w * 0.8 + p.phase) * masche * 0.05;
        return { x, y, z: hoeheBei(x, y, Math.sin(w) * 0.6) };
      });
      const dreieck = (a, b, d) => {
        // Flache Schattierung aus der Neigung der Flaeche gegen das Licht
        // von oben links.
        const ux = b.x - a.x, uy = b.y - a.y, uz = (b.z - a.z) * masche * 2.2;
        const vx = d.x - a.x, vy = d.y - a.y, vz = (d.z - a.z) * masche * 2.2;
        let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
        const n = Math.hypot(nx, ny, nz) || 1;
        nx /= n; ny /= n; nz /= n;
        if (nz < 0) { nx = -nx; ny = -ny; nz = -nz; }
        const licht = Math.max(0, -0.45 * nx - 0.55 * ny + 0.7 * nz);
        const h = (a.z + b.z + d.z) / 3;
        const g = Math.round(10 + h * (60 + 150 * licht) + 14 * licht);
        stift.fillStyle = `rgb(${g}, ${g}, ${g})`;
        stift.beginPath();
        stift.moveTo(a.x, a.y);
        stift.lineTo(b.x, b.y);
        stift.lineTo(d.x, d.y);
        stift.closePath();
        stift.fill();
        // ⚑ Dieselbe Farbe als Kante, sonst bleiben zwischen den
        //   Dreiecken haarfeine helle Fugen der Kantenglaettung stehen.
        stift.strokeStyle = stift.fillStyle;
        stift.lineWidth = 0.6;
        stift.stroke();
      };
      for (let j = 0; j < zeilen - 1; j++) {
        for (let i = 0; i < spalten - 1; i++) {
          const a = lage[j * spalten + i];
          const b = lage[j * spalten + i + 1];
          const d = lage[(j + 1) * spalten + i];
          const e = lage[(j + 1) * spalten + i + 1];
          if ((i + j) % 2 === 0) {
            dreieck(a, b, e);
            dreieck(a, e, d);
          } else {
            dreieck(a, b, d);
            dreieck(b, e, d);
          }
        }
      }
      vignette(stift, breite, hoehe, 0.5);
    },
    hofstaerke: 0.35,
  };
}

// --- Szene 4: Wirbel aus Flecken ----------------------------------------

/**
 * Ein Rauschfeld in Polarkoordinaten, zur Spirale verdreht: helle
 * Flecken mit dunklen Rissen, die in einen dunklen Kern gezogen werden;
 * aussen reissen sie zu Strahlen aus.
 */
function wirbel(breite, hoehe, dpr) {
  const q = 1.5;
  const dreh = 0.9;
  const zufall = zufallsquelle(0x51bd);
  // ⚑ Pixelweise gerechnet, deshalb kleiner als der Schirm und hochgezogen:
  //   Die Flecken sind ohnehin weich.
  const diag = Math.hypot(breite, hoehe);
  const seite = Math.min(900, Math.ceil(diag * 0.55 * Math.min(dpr, 1.5)));
  const massstab = seite / diag;
  const textur = leinwand(seite, seite);
  const s = textur.getContext("2d");
  const pix = s.createImageData(seite, seite);

  // ⚑ **Periodisches Wertrauschen, je Lage ein eigenes Gitter**, im Winkel
  //   rundherum geschlossen und in `log r` genau einmal je Umlauf der
  //   Schleife. Ein Gitter, das nicht ganzzahlig in einen Umlauf passt,
  //   springt an der Wende; so geschah es im ersten Entwurf.
  const glatt = (f) => f * f * (3 - 2 * f);
  const feld = (pw, pr) => {
    const g = new Float32Array(pw * pr).map(() => zufall());
    return (tu, rho) => {
      const x = tu * pw;
      const y = rho * pr;
      const x0 = Math.floor(x), y0 = Math.floor(y);
      const fx = glatt(x - x0), fy = glatt(y - y0);
      const w = (i, j) => g[(((j % pr) + pr) % pr) * pw + (((i % pw) + pw) % pw)];
      const a = w(x0, y0), b = w(x0 + 1, y0), c = w(x0, y0 + 1), d = w(x0 + 1, y0 + 1);
      return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
    };
  };
  const grob = feld(48, 4);
  const mittel = feld(96, 8);
  const fein = feld(384, 24);
  const risse = feld(192, 12);
  const c = seite / 2;
  const r0 = seite * 0.01;
  const lq = Math.log(q);
  for (let y = 0; y < seite; y++) {
    for (let x = 0; x < seite; x++) {
      const dx = x - c, dy = y - c;
      const r = Math.hypot(dx, dy) + 1e-6;
      const rho = Math.log(r / r0) / lq;
      // ⚑ Die Verdrehung wandert je Umlauf um genau `dreh`, also ist die
      //   Textur unter `q` und `dreh` sich selbst gleich.
      const th = Math.atan2(dy, dx) - rho * dreh;
      const tu = ((th / (2 * Math.PI)) % 1 + 1) % 1;
      const g = 0.6 * grob(tu, rho) + 0.4 * mittel(tu, rho + 0.37);
      let n = Math.max(0, Math.min(1, (g - 0.38) * 2.6)) * (0.72 + 0.28 * fein(tu, rho + 0.21));
      // Risse: wo das Rissfeld die Mitte kreuzt, eine dunkle Fuge.
      if (Math.abs(risse(tu, rho + 0.53) - 0.5) < 0.035) n *= 0.25;
      const v = Math.round(24 + 180 * n);
      const p = (y * seite + x) * 4;
      pix.data[p] = v;
      pix.data[p + 1] = v + 2;
      pix.data[p + 2] = v + 4;
      pix.data[p + 3] = 255;
    }
  }
  s.putImageData(pix, 0, 0);

  // Die Strahlen aussen: eine eigene Schicht, die sich nur langsam dreht.
  const strahlen = leinwand(seite, seite);
  const st = strahlen.getContext("2d");
  st.lineCap = "round";
  for (let i = 0; i < 900; i++) {
    const w = zufall() * 2 * Math.PI;
    const a = seite * (0.28 + zufall() * 0.2);
    const l = seite * (0.08 + zufall() * 0.25);
    st.strokeStyle = `rgba(215, 218, 220, ${(0.04 + zufall() * 0.1).toFixed(3)})`;
    st.lineWidth = 0.6 + zufall() * 1.8;
    st.beginPath();
    st.moveTo(c + a * Math.cos(w), c + a * Math.sin(w));
    st.lineTo(c + (a + l) * Math.cos(w + 0.08), c + (a + l) * Math.sin(w + 0.08));
    st.stroke();
  }
  return {
    schleifenmass: { textur, q, dreh },
    periode: 12,
    zeichnen(stift, t) {
      const u = (t / this.periode) % 1;
      const mx = breite / 2;
      const my = hoehe / 2;
      stift.imageSmoothingQuality = "high";
      schleife(stift, textur, massstab, mx, my, q, dreh, u, "source-over");
      // Der dunkle Kern, fest am Schirm.
      radial(stift, breite, hoehe, [[0, "rgba(10, 12, 14, 0.97)"], [0.07, "rgba(10, 12, 14, 0.6)"],
        [0.16, "rgba(10, 12, 14, 0)"]]);
      // ⚑ Die Strahlen drehen stetig und ohne Wende, also ohne Naht.
      stift.save();
      stift.translate(mx, my);
      stift.rotate(t * 0.02);
      stift.scale(1 / massstab, 1 / massstab);
      stift.drawImage(strahlen, -seite / 2, -seite / 2);
      stift.restore();
      vignette(stift, breite, hoehe, 0.7);
    },
    hofstaerke: 0.5,
  };
}

// --- Szene 5: Mandala aus Schleifen -------------------------------------

/**
 * Dunkle, netzartige Baender, die in Schlaufen um die Mitte laufen, Ring
 * um Ring kleiner, auf hellem Grund.
 */
function schleifen(breite, hoehe, dpr) {
  const q = 1.2;
  const dreh = 0.32;
  const { seite, massstab } = texturmasse(breite, hoehe, dpr);
  const textur = leinwand(seite, seite);
  const s = textur.getContext("2d");
  const c = seite / 2;
  // Ein feines Netz fuer das Innere der Baender.
  const netz = leinwand(7, 7);
  const n = netz.getContext("2d");
  n.strokeStyle = "rgba(6, 6, 6, 0.42)";
  n.lineWidth = 0.9;
  n.beginPath();
  n.moveTo(0, 0); n.lineTo(7, 7);
  n.moveTo(7, 0); n.lineTo(0, 7);
  n.stroke();
  const zipfel = 9;
  s.lineJoin = "round";
  // Zwei Familien, um einen halben Ring und einen halben Zipfel versetzt;
  // beide mit derselben Drehung je Ring.
  for (const [stufe, versatz] of [[1, 0], [Math.sqrt(q), Math.PI / zipfel]]) {
    for (let r = 1.5 * stufe, i = 0; r < seite * 0.8; r *= q, i++) {
      const winkel = i * dreh + versatz;
      const pfad = new Path2D();
      const schritte = Math.max(180, Math.ceil(r * 1.4));
      for (let k = 0; k <= schritte; k++) {
        const th = (k / schritte) * 2 * Math.PI;
        // ⚑ Kreis plus schnellerer Umlauf mit mehr als dem Kehrwert der
        //   Zipfelzahl: Dann schlaegt die Bahn Schlaufen.
        const x0 = Math.cos(th) + 0.2 * Math.cos((zipfel + 1) * th);
        const y0 = Math.sin(th) + 0.2 * Math.sin((zipfel + 1) * th);
        const [x, y] = ringpunkt(c, r, winkel, x0, y0);
        if (k === 0) pfad.moveTo(x, y);
        else pfad.lineTo(x, y);
      }
      const band = Math.max(1, r * 0.11);
      // ⚑ Auch das Netz waechst und dreht mit dem Ring, sonst waeren
      //   seine Maschen nach einem Umlauf um ein Fuenftel groesser.
      const muster = s.createPattern(netz, "repeat");
      muster.setTransform(new DOMMatrix()
        .translate(c, c).rotate((winkel * 180) / Math.PI).scale(r / (seite * 0.18)).translate(-c, -c));
      s.lineWidth = band;
      s.strokeStyle = "rgba(10, 10, 10, 0.2)";
      s.stroke(pfad);
      s.lineWidth = band * 0.8;
      s.strokeStyle = muster;
      s.stroke(pfad);
      s.lineWidth = Math.max(0.5, band * 0.1);
      s.strokeStyle = "rgba(6, 6, 6, 0.8)";
      s.stroke(pfad);
    }
  }
  return {
    schleifenmass: { textur, q, dreh },
    periode: 12,
    zeichnen(stift, t) {
      const u = (t / this.periode) % 1;
      radial(stift, breite, hoehe, [[0, "#c4c4c4"], [0.45, "#9a9a9a"], [1, "#555555"]]);
      schleife(stift, textur, massstab, breite / 2, hoehe / 2, q, dreh, u);
      vignette(stift, breite, hoehe, 0.55);
    },
    // Heller Grund, also ein kraeftiger Hof unter der Marke.
    hofstaerke: 0.65,
  };
}

// --- Auswahl und Lauf ---------------------------------------------------

/** Die Szenen, in der Reihenfolge der Vorlagen. */
export const SZENEN = [wellenbaender, vielecke, dreieckswelle, wirbel, schleifen];

/**
 * Startet das Vorschaltbild auf der uebergebenen Leinwand, mit einer
 * gewuerfelten Szene oder der angegebenen.
 *
 * ⚑ Der Rueckgabewert haelt es an. Eine Animation, die hinter einem
 * unsichtbaren Vorhang weiterlaeuft, kostet weiter Rechenzeit, und die
 * soll dem Fenster gehoeren.
 */
export function vorhangStarten(ziel, nummer = Math.floor(Math.random() * SZENEN.length)) {
  const stift = ziel.getContext("2d");
  const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const bauen = SZENEN[nummer % SZENEN.length];
  let szene = null;
  let breite = 0;
  let hoehe = 0;
  let dpr = 1;
  let laeuft = true;
  const beginn = performance.now();

  const zeichnen = (jetzt) => {
    if (!laeuft || !szene) return;
    stift.setTransform(dpr, 0, 0, dpr, 0, 0);
    szene.zeichnen(stift, still ? 0 : (jetzt - beginn) / 1000);
    hof(stift, breite / 2, hoehe / 2, Math.min(breite, hoehe) * 0.34, szene.hofstaerke);
  };
  const aufbauen = () => {
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    breite = window.innerWidth;
    hoehe = window.innerHeight;
    ziel.width = Math.round(breite * dpr);
    ziel.height = Math.round(hoehe * dpr);
    szene = bauen(breite, hoehe, dpr);
    zeichnen(performance.now());
  };
  const bild = (jetzt) => {
    if (!laeuft) return;
    zeichnen(jetzt);
    requestAnimationFrame(bild);
  };

  aufbauen();
  window.addEventListener("resize", aufbauen);
  if (!still) requestAnimationFrame(bild);
  return () => {
    laeuft = false;
    window.removeEventListener("resize", aufbauen);
  };
}
