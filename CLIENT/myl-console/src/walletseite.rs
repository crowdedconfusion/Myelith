//! **`/wallet`**: Testwallets anlegen und auswaehlen, mit den Pfeiltasten.
//!
//! # ⚑ Dieselbe Bedienung wie `/settings` und `/mining`
//!
//! Hoch und runter waehlt, Enter waehlt ein Wallet aus, tippt einen Namen
//! oder den Einstieg (Auftrag des Projektinhabers, 2026-10-26). **Entschieden
//! wird nichts hier**: Entwurf, Suche im Netz, Bestaetigung, Speichern,
//! Auswaehlen und Einstieg stehen in `myl_client::wallet` und
//! `myl_client::mining`, und das Fenster ruft dieselben.
//!
//! ⚑ **Ein neues Wallet** (Festlegung des Projektinhabers, 2026-10-26): erst
//! im Netz suchen, dann die zwoelf Woerter nach BIP-39 auf eigenem Schirm,
//! dann „notiert?" und zwei abgefragte Woerter; erst dann wird gespeichert.
//!
//! Das ausgewaehlte Wallet bekommt beim Mining die Ertraege; auf der
//! Probekette sind das Spielgeld, je Epoche 10 MYL zur Probe.

use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print};
use crate::ansi::{ResetColor, SetAttribute, SetForegroundColor};

use myl_client::{mining, wallet};

use crate::design::Toene;
use crate::miningseite::{kuerzen, umbrechen};

/// Die Zeile ganz unten.
pub const KUERZEL: &str = "  ↑↓ wählen · ⏎ auswählen oder eingeben · Esc schließen";

/// Eine Zeile der Seite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reihe {
    Wallet(usize),
    Neu,
    Wiederherstellen,
    Einstieg,
}

fn reihen(n: usize) -> Vec<Reihe> {
    let mut r: Vec<Reihe> = (0..n).map(Reihe::Wallet).collect();
    r.extend([Reihe::Neu, Reihe::Wiederherstellen, Reihe::Einstieg]);
    r
}

/// Was die Seite weiss.
struct Seite {
    wallets: Vec<wallet::Wallet>,
    gewaehlt: Option<String>,
    eingetragen: Option<String>,
    /// Das Wallet des laufenden Minings, sein Guthaben und ob die Kette es
    /// fuehrt.
    mining: Option<(String, u64, bool)>,
    einstieg: Option<String>,
    meldung: String,
}

impl Seite {
    fn laden(meldung: String) -> Self {
        let mining = mining::zustand().and_then(|mut z| {
            let s = mining::stand(&mut z).ok()?;
            Some((z.wallet?, s.auszahlung_guthaben, s.auszahlung_eingetragen))
        });
        Seite {
            wallets: wallet::liste(),
            gewaehlt: wallet::gewaehlt().map(|w| w.name),
            eingetragen: wallet::eingetragen().map(|w| w.name),
            mining,
            einstieg: mining::einstieg(),
            meldung,
        }
    }

    fn titel(&self, r: Reihe) -> String {
        match r {
            Reihe::Wallet(i) => self.wallets[i].name.clone(),
            Reihe::Neu => "Neues Wallet".into(),
            Reihe::Wiederherstellen => "Aus zwölf Wörtern".into(),
            Reihe::Einstieg => "Einstieg ins Netz".into(),
        }
    }

    fn wert(&self, r: Reihe) -> String {
        match r {
            Reihe::Wallet(i) => {
                let w = &self.wallets[i];
                let a = w.adresse_hex();
                let mut teile = vec![format!("{}…{}", &a[..8], &a[a.len() - 6..])];
                if let Some((_, g, _)) = self.mining.as_ref().filter(|(n, _, _)| *n == w.name) {
                    teile.push(format!("{} MYL", mining::myl_text(*g)));
                }
                if self.gewaehlt.as_deref() == Some(&w.name) {
                    teile.push("ausgewählt".into());
                }
                if self.eingetragen.as_deref() == Some(&w.name) {
                    teile.push("Auszahlungskonto".into());
                }
                teile.join(" · ")
            }
            Reihe::Neu => "⏎ Namen eingeben".into(),
            Reihe::Wiederherstellen => "⏎ wiederherstellen".into(),
            Reihe::Einstieg => self.einstieg.clone().unwrap_or_else(|| "keiner (eigener Probelauf)".into()),
        }
    }

    /// Der Satz unter der Liste.
    fn hinweis(&self, r: Reihe) -> String {
        match r {
            Reihe::Wallet(i) => format!(
                "Adresse {}. ⏎ wählt es aus; beim nächsten Start des Minings bekommt es die Erträge.",
                self.wallets[i].adresse_hex()
            ),
            Reihe::Neu => "Legt ein Testwallet an (Name: Buchstaben, Ziffern, - und _). Zuerst wird im Netz gesucht, dann zeigt die Seite zwölf Wörter nach BIP-39 zum Notieren, fragt, ob sie notiert sind, und zwei davon ab. Erst dann ist es angelegt; das erste wird gleich ausgewählt.".into(),
            Reihe::Wiederherstellen => "Stellt ein Wallet aus seinen zwölf Wörtern wieder her und sucht es im Netz. Dasselbe Wallet zweimal auf dieser Maschine gibt es nicht.".into(),
            Reihe::Einstieg => "Wohin der Knoten beim Mining beitritt: eine Multiadresse oder eine Datei, die eine nennt (einstieg.txt des Probenetzes). „aus“ heißt eigener Probelauf; dann baut der Knoten erst, wenn jemand zuhört.".into(),
        }
    }

    /// Die Zeilen oben: was das Mining gerade zahlt.
    fn stand(&self) -> Vec<String> {
        let erste = match (&self.mining, &self.gewaehlt) {
            (None, _) => "Mining läuft nicht. Das ausgewählte Wallet gilt ab dem nächsten Start (/mining).".to_string(),
            (Some((n, _, _)), Some(g)) if n != g => {
                format!("Mining zahlt noch an {n}; {g} gilt ab dem nächsten Start (/mining stop, dann /mining).")
            }
            (Some((n, _, false)), _) => format!("Mining zahlt an {n}; der Eintrag als Auszahlungskonto wartet auf die Kette."),
            (Some((n, g, true)), _) => format!("Mining zahlt an {n}: {} MYL.", mining::myl_text(*g)),
        };
        vec![erste]
    }
}

/// **Faehrt die Seite.**
pub fn fahren(t: Toene) {
    crate::schimmer::vergessen();
    let mut s = Seite::laden(String::new());
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        for z in s.stand() {
            println!("  {z}");
        }
        for (i, w) in s.wallets.iter().enumerate() {
            println!("  {} {}  {}", if s.gewaehlt.as_deref() == Some(&w.name) { "*" } else { " " }, w.name, s.wert(Reihe::Wallet(i)));
        }
        return;
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        return;
    };
    let mut liste = reihen(s.wallets.len());
    // ⚑ Die Vorauswahl steht auf dem ausgewaehlten Wallet, sonst auf „Neu".
    let mut hier = s
        .wallets
        .iter()
        .position(|w| s.gewaehlt.as_deref() == Some(&w.name))
        .unwrap_or(s.wallets.len());
    zeichnen(&s, &liste, hier, t);
    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let strg = k.modifiers.contains(KeyModifiers::CONTROL);
        s.meldung.clear();
        let r = liste[hier];
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => break,
            KeyCode::Char('c') if strg => break,
            KeyCode::Up => hier = hier.saturating_sub(1),
            KeyCode::Down => hier = (hier + 1).min(liste.len() - 1),
            KeyCode::Enter => {
                let mut angelegt: Option<String> = None;
                let meldung = match r {
                    Reihe::Wallet(i) => {
                        let name = s.wallets[i].name.clone();
                        match wallet::waehlen(&name) {
                            Ok(_) if s.mining.is_some() => format!("{name} ausgewählt; es gilt ab dem nächsten Start des Minings."),
                            Ok(_) => format!("{name} ausgewählt."),
                            Err(f) => f,
                        }
                    }
                    Reihe::Neu => match crate::einstellseite::tippen("Name", "neu", t) {
                        Some(name) => match neu_anlegen(&name, t) {
                            Ok(w) => {
                                angelegt = Some(w.name.clone());
                                format!("{} angelegt: {}", w.name, w.adresse_hex())
                            }
                            Err(f) => f,
                        },
                        None => String::new(),
                    },
                    Reihe::Wiederherstellen => match crate::einstellseite::tippen("Name", "neu", t) {
                        Some(name) => match wiederherstellen(&name, t) {
                            Ok((w, befund)) => {
                                angelegt = Some(w.name.clone());
                                format!("{} wiederhergestellt. {}", w.name, befund.satz())
                            }
                            Err(f) => f,
                        },
                        None => String::new(),
                    },
                    Reihe::Einstieg => {
                        let jetzt = s.einstieg.clone().unwrap_or_else(|| "keiner".into());
                        match crate::einstellseite::tippen("Einstieg (oder aus)", &jetzt, t) {
                            Some(angabe) => {
                                let angabe = if angabe == "aus" { String::new() } else { angabe };
                                match mining::einstieg_setzen(&angabe) {
                                    Ok(Some(e)) => format!("Einstieg: {e}"),
                                    Ok(None) => "Kein Einstieg: eigener Probelauf.".into(),
                                    Err(f) => f,
                                }
                            }
                            None => String::new(),
                        }
                    }
                };
                s = Seite::laden(meldung);
                liste = reihen(s.wallets.len());
                // Ein neues Wallet steht danach unter dem Zeiger.
                if let Some(p) = angelegt.and_then(|n| s.wallets.iter().position(|w| w.name == n)) {
                    hier = p;
                }
                hier = hier.min(liste.len() - 1);
            }
            _ => continue,
        }
        zeichnen(&s, &liste, hier, t);
    }
    drop(_roh);
}

/// **Ein neues Wallet**, Schritt fuer Schritt; `Err` mit dem Grund, und
/// dann ist nichts angelegt.
fn neu_anlegen(name: &str, t: Toene) -> Result<wallet::Wallet, String> {
    wallet::name_pruefen(name)?;
    let e = wallet::entwerfen()?;
    let befund = wallet::im_netz_suchen(&e.adresse);
    if matches!(befund, wallet::Netzbefund::Bekannt { .. }) {
        return Err(format!("{} Es wird nichts angelegt.", befund.satz()));
    }
    woerter_zeigen(name, &e, &befund, t);
    let notiert = crate::janein::fragen("Hast du alle zwölf Wörter notiert?", None, None);
    // Die Woerter vom Schirm, bevor gefragt wird.
    let mut aus = std::io::stdout();
    let _ = crossterm::queue!(aus, Print("\x1b[H\x1b[J"));
    let _ = aus.flush();
    if !notiert.ja() {
        return Err("Abgebrochen; es wurde nichts angelegt.".into());
    }
    let _ = crossterm::queue!(aus, SetForegroundColor(t.akzent), Print("  Zur Bestätigung zwei der zwölf Wörter.\r\n"), ResetColor);
    let (a, b) = e.abfrage();
    let wa = crate::einstellseite::tippen(&format!("Wort {a}"), "?", t).unwrap_or_default();
    let wb = crate::einstellseite::tippen(&format!("Wort {b}"), "?", t).unwrap_or_default();
    if !e.bestaetigt(&[(a, &wa), (b, &wb)]) {
        return Err("Die Wörter stimmen nicht; es wurde nichts angelegt. Noch einmal mit „Neues Wallet“.".into());
    }
    wallet::speichern(name, &e)
}

/// **Ein Wallet aus zwoelf Woertern**, mit Suche im Netz.
fn wiederherstellen(name: &str, t: Toene) -> Result<(wallet::Wallet, wallet::Netzbefund), String> {
    wallet::name_pruefen(name)?;
    let text = crate::einstellseite::tippen("Die zwölf Wörter", "mit Leerzeichen", t).unwrap_or_default();
    let e = wallet::aus_woertern(&text)?;
    let befund = wallet::im_netz_suchen(&e.adresse);
    Ok((wallet::speichern(name, &e)?, befund))
}

/// Die zwoelf Woerter auf eigenem Schirm, in zwei Spalten.
fn woerter_zeigen(name: &str, e: &wallet::Entwurf, befund: &wallet::Netzbefund, t: Toene) {
    let breite = (crate::banner::fensterbreite() as usize).clamp(40, 110);
    let mut aus = std::io::stdout();
    let _ = crossterm::queue!(aus, crossterm::cursor::Hide, Print("\x1b[H\x1b[J"));
    let zeile = |aus: &mut std::io::Stdout, text: &str, farbe| {
        let _ = crossterm::queue!(aus, SetForegroundColor(farbe), Print(kuerzen(text, breite)), ResetColor, Print("\r\n"));
    };
    zeile(&mut aus, &format!("  Wallet {name} · Wiederherstellungsschlüssel (BIP-39)"), t.akzent);
    zeile(&mut aus, "", t.beiwerk);
    zeile(&mut aus, &format!("  {}", befund.satz()), t.beiwerk);
    zeile(&mut aus, "", t.beiwerk);
    let w = e.woerter();
    for i in 0..6 {
        zeile(&mut aus, &format!("     {:>2}. {:<14}  {:>2}. {}", i + 1, w[i], i + 7, w[i + 6]), t.akzent);
    }
    zeile(&mut aus, "", t.beiwerk);
    for z in umbrechen(
        "Schreib die Wörter in dieser Reihenfolge auf Papier. Wer sie hat, hat das Wallet; ohne sie ist es nach einem Verlust dieses Rechners weg. Sie werden nie wieder gezeigt.",
        breite.saturating_sub(4),
    ) {
        zeile(&mut aus, &format!("  {z}"), t.beiwerk);
    }
    zeile(&mut aus, "", t.beiwerk);
    let _ = aus.flush();
}

fn zeichnen(s: &Seite, liste: &[Reihe], hier: usize, t: Toene) {
    let breite = (crate::banner::fensterbreite() as usize).clamp(40, 110);
    let mut aus = std::io::stdout();
    let _ = crossterm::queue!(aus, crossterm::cursor::Hide, Print("\x1b[H\x1b[J"));
    let zeile = |aus: &mut std::io::Stdout, text: &str, farbe| {
        let _ = crossterm::queue!(aus, SetForegroundColor(farbe), Print(kuerzen(text, breite)), ResetColor, Print("\r\n"));
    };
    zeile(&mut aus, "  Wallet · Testwallets auf der Probekette (Spielgeld)", t.akzent);
    zeile(&mut aus, "", t.beiwerk);
    for z in s.stand() {
        zeile(&mut aus, &format!("  {z}"), if s.mining.is_some() { t.akzent } else { t.beiwerk });
    }
    zeile(&mut aus, "", t.beiwerk);
    if s.wallets.is_empty() {
        zeile(&mut aus, "  Noch kein Wallet. Leg unten eines an.", t.beiwerk);
    }
    for (i, r) in liste.iter().enumerate() {
        let hierhin = i == hier;
        let marke = if hierhin { "▸" } else { " " };
        if *r == Reihe::Neu {
            zeile(&mut aus, "", t.beiwerk);
        }
        let titel = format!("  {marke} {:<30}", kuerzen(&s.titel(*r), 30));
        let _ = crossterm::queue!(aus, Print("\x1b[2K"));
        if hierhin {
            let _ = crossterm::queue!(aus, SetForegroundColor(t.akzent), SetAttribute(Attribute::Bold), Print(&titel), SetAttribute(Attribute::Reset));
        } else {
            let _ = crossterm::queue!(aus, ResetColor, Print(&titel));
        }
        let platz = breite.saturating_sub(titel.chars().count() + 2);
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(if hierhin { t.akzent } else { t.beiwerk }),
            Print(kuerzen(&s.wert(*r), platz)),
            ResetColor,
            Print("\r\n")
        );
    }
    zeile(&mut aus, "", t.beiwerk);
    for z in umbrechen(&s.hinweis(liste[hier]), breite.saturating_sub(4)).iter().take(HINWEISZEILEN) {
        zeile(&mut aus, &format!("  {z}"), t.beiwerk);
    }
    if s.meldung.is_empty() {
        zeile(
            &mut aus,
            &format!("  Die Schlüssel liegen unverschlüsselt unter {}.", wallet::ordner().display()),
            t.beiwerk,
        );
    } else {
        zeile(&mut aus, &format!("  ⚑ {}", s.meldung), t.akzent);
    }
    zeile(&mut aus, "", t.beiwerk);
    zeile(&mut aus, KUERZEL, t.kante);
    let _ = aus.flush();
}

/// Wie viele Zeilen der Satz unter der Liste hoechstens hat.
const HINWEISZEILEN: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    fn seite(n: usize) -> Seite {
        let wallets = (0..n)
            .map(|i| wallet::Wallet { name: format!("w{i}"), adresse: wallet::Address::new([i as u8; 32]) })
            .collect();
        Seite { wallets, gewaehlt: Some("w0".into()), eingetragen: Some("w0".into()), mining: None, einstieg: None, meldung: String::new() }
    }

    /// ⚑ **Je Wallet eine Reihe, dann „Neu" und der Einstieg**, und jede
    /// Reihe hat ihren Satz.
    #[test]
    fn jedes_wallet_hat_eine_reihe_und_jede_reihe_einen_satz() {
        let s = seite(2);
        let r = reihen(s.wallets.len());
        assert_eq!(r, [Reihe::Wallet(0), Reihe::Wallet(1), Reihe::Neu, Reihe::Wiederherstellen, Reihe::Einstieg]);
        for x in &r {
            assert!(!s.hinweis(*x).is_empty());
            assert!(!s.wert(*x).is_empty());
        }
        assert!(s.wert(Reihe::Wallet(0)).contains("ausgewählt · Auszahlungskonto"));
        assert!(!s.wert(Reihe::Wallet(1)).contains("ausgewählt"));
    }

    /// ⚑ **Was das Mining zahlt, steht oben**, und wenn ein anderes Wallet
    /// ausgewaehlt ist, auch dass es erst ab dem naechsten Start gilt.
    #[test]
    fn der_stand_sagt_wohin_das_mining_zahlt() {
        let mut s = seite(2);
        assert!(s.stand()[0].contains("läuft nicht"));
        s.mining = Some(("w0".into(), 20_000_000, true));
        assert!(s.stand()[0].contains("w0: 20 MYL"));
        assert!(s.wert(Reihe::Wallet(0)).contains("20 MYL"));
        s.mining = Some(("w0".into(), 0, false));
        assert!(s.stand()[0].contains("wartet auf die Kette"));
        s.gewaehlt = Some("w1".into());
        assert!(s.stand()[0].contains("w1 gilt ab dem nächsten Start"));
    }

    /// ⛔️ **Kein Schluessel und kein Prozess an der Kiste vorbei**: Die Seite
    /// speichert nur ueber `wallet::speichern`, und ein neues Wallet erst nach
    /// der Bestaetigung (`bestaetigt` steht davor).
    #[test]
    fn angelegt_wird_nur_ueber_die_kiste() {
        let quelle = include_str!("walletseite.rs");
        let rumpf = quelle.split("#[cfg(test)]").next().unwrap();
        assert_eq!(rumpf.matches("wallet::speichern(").count(), 2);
        let neu = &rumpf[rumpf.find("fn neu_anlegen").unwrap()..rumpf.find("fn wiederherstellen").unwrap()];
        assert!(neu.find("e.bestaetigt(").unwrap() < neu.find("wallet::speichern(").unwrap(), "gespeichert vor der Bestätigung");
        assert!(neu.find("janein::fragen(").unwrap() < neu.find("e.bestaetigt(").unwrap());
        assert!(!rumpf.contains("Command::new"), "die Seite startet selbst einen Prozess");
        assert!(!rumpf.contains("getrandom"), "die Seite erzeugt selbst Schlüssel");
    }
}
