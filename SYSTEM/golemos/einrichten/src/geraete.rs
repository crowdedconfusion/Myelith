//! Die Geraeteliste, gelesen aus `lsblk -P`.
//!
//! ⚑ **`-P` und nicht `-J`.** Die Paarform `NAME="sda" SIZE="…"` ist
//! eine Zeile je Geraet und kommt ohne JSON-Zerleger aus. Werte mit
//! Sonderzeichen schreibt `lsblk` als `\xNN`; das wird hier
//! zurueckuebersetzt, sonst stuende im Modellnamen `Samsung\x20SSD`.
//!
//! ⚑ **`-b`, also Bytes.** Ohne den Schalter kommen Groessen wie
//! `14,9G`, gerundet und je nach Sprache mit Komma. Wer damit rechnet,
//! ob eine Platte gross genug ist, rechnet mit einer Schaetzung.

use std::io;
use std::process::Command;

/// Die Spalten in der Reihenfolge, in der `lesen` sie anfordert.
pub const SPALTEN: &str =
    "NAME,KNAME,PKNAME,TYPE,SIZE,RM,TRAN,MODEL,PARTLABEL,PARTUUID,PARTTYPE,MOUNTPOINT,FSTYPE,LABEL";

/// Ein Blockgeraet: eine Platte oder eine Partition darauf.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Geraet {
    /// Kernname ohne `/dev/`, etwa `sda`, `sda1`, `nvme0n1p2`.
    pub name: String,
    /// Die Platte, auf der eine Partition liegt; leer bei einer Platte.
    pub eltern: String,
    /// `disk`, `part`, `loop`, `rom` und so weiter.
    pub art: String,
    pub groesse: u64,
    /// Meldet sich als Wechseldatentraeger (`RM=1`).
    pub wechselbar: bool,
    /// Anschluss, etwa `usb`, `nvme`, `sata`; oft leer.
    pub anschluss: String,
    pub modell: String,
    /// Der Name im GPT-Eintrag, etwa `golem-wurzelA`.
    pub partname: String,
    pub partuuid: String,
    /// Die Typkennung im GPT-Eintrag.
    pub parttyp: String,
    /// Wo das Geraet eingehaengt ist; leer, wenn nirgends.
    pub einhaengung: String,
    pub dateisystem: String,
    /// Die Marke im Dateisystem, etwa `Golem`.
    pub marke: String,
}

impl Geraet {
    pub fn pfad(&self) -> String {
        format!("/dev/{}", self.name)
    }

    pub fn ist_platte(&self) -> bool {
        self.art == "disk"
    }

    pub fn ist_partition(&self) -> bool {
        self.art == "part"
    }
}

/// Ruft `lsblk` und zerlegt die Ausgabe.
pub fn lesen() -> io::Result<Vec<Geraet>> {
    let aus = Command::new("lsblk")
        .args(["-P", "-b", "-o", SPALTEN])
        .output()?;
    if !aus.status.success() {
        return Err(io::Error::other(format!(
            "lsblk endete mit {}: {}",
            aus.status,
            String::from_utf8_lossy(&aus.stderr).trim()
        )));
    }
    Ok(zerlegen(&String::from_utf8_lossy(&aus.stdout)))
}

/// Zerlegt die Ausgabe von `lsblk -P`, eine Zeile je Geraet.
///
/// ⚠️ Unbekannte Schluessel werden uebergangen und nicht als Fehler
/// gemeldet: Eine neuere Fassung von `lsblk` darf Spalten anders
/// benennen (`MOUNTPOINTS`), und die Liste soll dann lueckenhaft
/// sein, nicht leer.
pub fn zerlegen(text: &str) -> Vec<Geraet> {
    text.lines()
        .filter(|z| !z.trim().is_empty())
        .map(|zeile| {
            let mut g = Geraet::default();
            for (schluessel, wert) in paare(zeile) {
                match schluessel.as_str() {
                    "NAME" => g.name = wert,
                    "PKNAME" => g.eltern = wert,
                    "TYPE" => g.art = wert,
                    "SIZE" => g.groesse = wert.parse().unwrap_or(0),
                    "RM" => g.wechselbar = wert == "1",
                    "TRAN" => g.anschluss = wert,
                    "MODEL" => g.modell = wert.trim().to_string(),
                    "PARTLABEL" => g.partname = wert,
                    "PARTUUID" => g.partuuid = wert.to_lowercase(),
                    "PARTTYPE" => g.parttyp = wert.to_lowercase(),
                    "MOUNTPOINT" => g.einhaengung = wert,
                    "FSTYPE" => g.dateisystem = wert,
                    "LABEL" => g.marke = wert,
                    _ => {}
                }
            }
            g
        })
        .filter(|g| !g.name.is_empty())
        .collect()
}

/// Die Paare `SCHLUESSEL="wert"` einer Zeile.
fn paare(zeile: &str) -> Vec<(String, String)> {
    let mut aus = Vec::new();
    let mut rest = zeile.trim();
    while let Some(gleich) = rest.find("=\"") {
        let schluessel = rest[..gleich].trim().to_string();
        let nach = &rest[gleich + 2..];
        let Some(ende) = nach.find('"') else { break };
        aus.push((schluessel, entschluesseln(&nach[..ende])));
        rest = &nach[ende + 1..];
    }
    aus
}

/// Uebersetzt `\xNN` zurueck in das Byte, fuer das es steht.
fn entschluesseln(wert: &str) -> String {
    let b = wert.as_bytes();
    let mut aus = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        // ⚠️ Ueber Bytes und nicht ueber `&wert[..]`: Ein Schnitt mitten
        //    in ein mehrbytiges Zeichen wuerde abbrechen.
        if b[i] == b'\\' && i + 3 < b.len() && b[i + 1] == b'x' {
            let ziffern = std::str::from_utf8(&b[i + 2..i + 4]).unwrap_or("");
            if let Ok(n) = u8::from_str_radix(ziffern, 16) {
                aus.push(n);
                i += 4;
                continue;
            }
        }
        aus.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&aus).into_owned()
}

/// Groesse fuer Menschen, in GB mit einer Nachkommastelle.
///
/// ⚑ **Dezimale Gigabyte**, wie sie auf jeder Packung stehen. Eine
/// „128-GB"-Platte als „119,2 GiB" anzuzeigen, sieht aus wie ein Fehler.
pub fn gb(bytes: u64) -> String {
    let zehntel = (bytes + 50_000_000) / 100_000_000;
    format!("{},{} GB", zehntel / 10, zehntel % 10)
}

#[cfg(test)]
mod proben {
    use super::*;

    const BEISPIEL: &str = concat!(
        r#"NAME="vda" KNAME="vda" PKNAME="" TYPE="disk" SIZE="3221225472" RM="1" TRAN="usb" MODEL="SanDisk\x20Ultra" PARTLABEL="" PARTUUID="" PARTTYPE="" MOUNTPOINT="" FSTYPE="" LABEL=""
"#,
        r#"NAME="vda4" KNAME="vda4" PKNAME="vda" TYPE="part" SIZE="2300000000" RM="1" TRAN="" MODEL="" PARTLABEL="golem-daten" PARTUUID="4F1A-AB" PARTTYPE="0FC63DAF-8483-4772-8E79-3D69D8477DE4" MOUNTPOINT="/daten" FSTYPE="vfat" LABEL="GOLEM-DATEN"
"#
    );

    #[test]
    fn zerlegt_platte_und_partition() {
        let g = zerlegen(BEISPIEL);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].name, "vda");
        assert!(g[0].ist_platte());
        assert!(g[0].wechselbar);
        assert_eq!(g[0].groesse, 3_221_225_472);
        assert_eq!(g[1].eltern, "vda");
        assert_eq!(g[1].einhaengung, "/daten");
        assert_eq!(g[1].partname, "golem-daten");
    }

    /// 📌 Ohne Rueckuebersetzung hiesse die Platte im Menue
    /// `SanDisk\x20Ultra`, und der Mensch muesste raten, ob das seine ist.
    #[test]
    fn uebersetzt_maskierte_zeichen() {
        assert_eq!(zerlegen(BEISPIEL)[0].modell, "SanDisk Ultra");
    }

    /// ⚑ Kennungen klein, damit ein Vergleich mit der Kernzeile nicht an
    /// der Schreibweise scheitert.
    #[test]
    fn kennungen_klein() {
        let g = zerlegen(BEISPIEL);
        assert_eq!(g[1].partuuid, "4f1a-ab");
        assert_eq!(g[1].parttyp, "0fc63daf-8483-4772-8e79-3d69d8477de4");
    }

    #[test]
    fn leere_und_fremde_zeilen_stoeren_nicht() {
        let g = zerlegen("\n\nNAME=\"sr0\" NEU=\"x\" TYPE=\"rom\"\n");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].art, "rom");
    }

    #[test]
    fn gigabyte_dezimal() {
        assert_eq!(gb(128_000_000_000), "128,0 GB");
        assert_eq!(gb(3_221_225_472), "3,2 GB");
        assert_eq!(gb(0), "0,0 GB");
    }
}
