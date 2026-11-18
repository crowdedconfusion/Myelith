//! ⚑ **Werkzeuge an der Tuer** (Fund 181, Festlegung des Projektinhabers
//! vom 2026-11-16): wie die Tuer die `tools` einer Chat-Anfrage in der
//! amtlichen Form des Netzmodells ansagt, wie Aufrufe und Werkzeugantworten
//! eines Verlaufs im Prompt stehen, und wie aus der Antwort des Modells
//! OpenAI-`tool_calls` werden. Die Vorlage selbst steht in
//! `myl_types::vorlage`, gemeinsam mit dem Klienten.

use myl_types::vorlage::{WERKZEUGE_FUSS, WERKZEUGE_KOPF};

/// Ein angebotenes Werkzeug: Name, Beschreibung, Parameterschema.
#[derive(Debug, Clone, PartialEq)]
pub struct Werkzeugangebot {
    pub name: String,
    pub beschreibung: String,
    pub parameter: serde_json::Value,
}

/// ⚑ **JSON in fester Form**: Schluessel sortiert, ohne Leerraum.
///
/// 📌 Die Reihenfolge der Schluessel eines `serde_json::Value` haengt an
/// einem Merkmal (`preserve_order`), das irgendeine Kiste im Baum
/// einschalten kann; zwei Bauten setzten dann verschiedene Prompts. Hier
/// steht die Form deshalb fest, unabhaengig vom Bau.
pub fn kanonisch(wert: &serde_json::Value) -> String {
    use serde_json::Value;
    match wert {
        Value::Object(feld) => {
            let mut schluessel: Vec<&String> = feld.keys().collect();
            schluessel.sort();
            let teile: Vec<String> = schluessel
                .into_iter()
                .map(|k| format!("{}:{}", serde_json::to_string(k).unwrap_or_default(), kanonisch(&feld[k])))
                .collect();
            format!("{{{}}}", teile.join(","))
        }
        Value::Array(reihe) => format!("[{}]", reihe.iter().map(kanonisch).collect::<Vec<_>>().join(",")),
        anders => serde_json::to_string(anders).unwrap_or_default(),
    }
}

/// **Die Werkzeugansage**: Kopf, je Werkzeug eine Zeile JSON
/// (`{"type":"function","function":{"name":…,"description":…,"parameters":…}}`),
/// Fuss. ⚑ Der Umbruch steht **vor** jedem Werkzeug, wie in der Vorlage.
pub fn werkzeuge_ansagen(werkzeuge: &[Werkzeugangebot]) -> String {
    let mut text = String::from(WERKZEUGE_KOPF);
    for w in werkzeuge {
        text.push('\n');
        text.push_str(&format!(
            "{{\"type\":\"function\",\"function\":{{\"name\":{},\"description\":{},\"parameters\":{}}}}}",
            serde_json::to_string(&w.name).unwrap_or_default(),
            serde_json::to_string(&w.beschreibung).unwrap_or_default(),
            kanonisch(&w.parameter)
        ));
    }
    text.push_str(WERKZEUGE_FUSS);
    text
}

/// **Ein Werkzeugaufruf im Verlauf**, so wie das Modell ihn schreibt:
/// `<tool_call>\n{"name": "…", "arguments": …}\n</tool_call>`.
pub fn aufruf_als_text(name: &str, argumente: &serde_json::Value) -> String {
    format!(
        "<tool_call>\n{{\"name\": {}, \"arguments\": {}}}\n</tool_call>",
        serde_json::to_string(name).unwrap_or_default(),
        kanonisch(argumente)
    )
}

/// **Eine Werkzeugantwort im Verlauf**: `<tool_response>\n…\n</tool_response>`.
/// Mehrere hintereinander stehen in **einer** Nutzerrunde, durch einen
/// Umbruch getrennt, wie in der Vorlage.
pub fn antwort_als_text(inhalt: &str) -> String {
    format!("<tool_response>\n{inhalt}\n</tool_response>")
}

/// Ein gelesener Werkzeugaufruf: Name und Argumente als JSON-Text in fester
/// Form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aufruf {
    pub name: String,
    pub argumente: String,
}

/// Was [`aufrufe_lesen`] aus einer Antwort macht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gelesen {
    /// Der Text ohne die gelesenen Aufrufe, an den Raendern beschnitten.
    pub text: String,
    /// Die Aufrufe in ihrer Reihenfolge.
    pub aufrufe: Vec<Aufruf>,
}

/// **Liest die Werkzeugaufrufe aus einer Modellantwort.**
///
/// ⛔️ **Nur auf eine Antwort des Modells anwenden, nie auf ein
/// Werkzeugergebnis** (Kap. 8.3): Enthielte ein Ergebnis `<tool_call>` und
/// ginge es hier hindurch, bestimmten abgerufene Daten den Kontrollfluss.
///
/// ⚑ **Nur angebotene Werkzeuge** (2026-11-17): Ein Aufruf eines Namens,
/// der nicht in `angeboten` steht, wird kein Aufruf und bleibt im Text, wie
/// bei OpenAI, wo nur angesagte Funktionen zurueckkommen. Ein Harness, das
/// nach Namen verteilt, bekaeme sonst einen Aufruf eines Werkzeugs, das es
/// fuer diese Anfrage gar nicht angeboten hat (etwa in einem engen Modus),
/// und eine eingeschleuste Anweisung haette dafuer nur den Namen zu raten.
///
/// Gelesen wird zwischen den Marken. ⚑ **Nachsichtig bei eindeutigen
/// Formfehlern**, wie im Agenten: ein Codezaun um das JSON, `parameters`
/// statt `arguments`, Argumente als JSON-Zeichenkette, die Huelle
/// `{"function": {…}}`. ⚑ **Was sich nicht lesen laesst, bleibt im Text**
/// und wird kein Aufruf: Ein Harness sieht dann, was das Modell schrieb,
/// statt dass es still verschwindet.
pub fn aufrufe_lesen(antwort: &str, angeboten: &[String]) -> Gelesen {
    const AUF: &str = "<tool_call>";
    const ZU: &str = "</tool_call>";
    let mut text = String::new();
    let mut aufrufe = Vec::new();
    let mut rest = antwort;
    while let Some(a) = rest.find(AUF) {
        let nach = &rest[a + AUF.len()..];
        let Some(e) = nach.find(ZU) else {
            break;
        };
        match aufruf_lesen(nach[..e].trim()).filter(|a| angeboten.contains(&a.name)) {
            Some(aufruf) => {
                text.push_str(&rest[..a]);
                aufrufe.push(aufruf);
            }
            None => text.push_str(&rest[..a + AUF.len() + e + ZU.len()]),
        }
        rest = &nach[e + ZU.len()..];
    }
    text.push_str(rest);
    Gelesen { text: text.trim().to_string(), aufrufe }
}

fn aufruf_lesen(inhalt: &str) -> Option<Aufruf> {
    let mut t = inhalt.trim();
    if let Some(r) = t.strip_prefix("```") {
        let r = r.trim_start_matches(|c: char| c.is_ascii_alphabetic()).trim();
        t = r.strip_suffix("```").unwrap_or(r).trim();
    }
    let wert: serde_json::Value = serde_json::from_str(t).ok()?;
    let wert = match wert.get("function") {
        Some(f) if f.is_object() && wert.get("name").is_none() => f.clone(),
        _ => wert,
    };
    let name = wert.get("name")?.as_str()?.to_string();
    if name.is_empty() {
        return None;
    }
    let argumente = match wert.get("arguments").or_else(|| wert.get("parameters")) {
        None => serde_json::Value::Object(Default::default()),
        Some(serde_json::Value::String(s)) => serde_json::from_str(s).ok()?,
        Some(v) => v.clone(),
    };
    if !argumente.is_object() {
        return None;
    }
    Some(Aufruf { name, argumente: kanonisch(&argumente) })
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Die Ansage hat Kopf und Fuss der Vorlage**, je Werkzeug eine
    /// Zeile, und die Schluessel stehen sortiert, gleich wie der Aufrufer sie
    /// schickte.
    #[test]
    fn die_werkzeugansage_steht_fest() {
        let w = Werkzeugangebot {
            name: "rechnen".into(),
            beschreibung: "rechnet".into(),
            parameter: serde_json::json!({"type": "object", "properties": {"b": {"type": "string"}, "a": {"type": "number"}}}),
        };
        let text = werkzeuge_ansagen(&[w]);
        assert!(text.starts_with(WERKZEUGE_KOPF) && text.ends_with(WERKZEUGE_FUSS), "{text}");
        let zeile = &text[WERKZEUGE_KOPF.len() + 1..text.len() - WERKZEUGE_FUSS.len()];
        assert_eq!(
            zeile,
            r#"{"type":"function","function":{"name":"rechnen","description":"rechnet","parameters":{"properties":{"a":{"type":"number"},"b":{"type":"string"}},"type":"object"}}}"#
        );
    }

    /// ⚑ **Aufrufe werden gelesen, der Text bleibt**, und ein unlesbarer
    /// Block bleibt im Text stehen, statt zu verschwinden.
    #[test]
    fn aufrufe_werden_gelesen_und_unlesbares_bleibt_stehen() {
        let angeboten = ["rechnen".to_string(), "x".to_string()];
        let aufrufe_lesen = |t: &str| aufrufe_lesen(t, &angeboten);
        let g = aufrufe_lesen("Ich rechne. <tool_call>\n{\"name\": \"rechnen\", \"arguments\": {\"ausdruck\": \"1+1\"}}\n</tool_call>");
        assert_eq!(g.text, "Ich rechne.");
        assert_eq!(g.aufrufe, vec![Aufruf { name: "rechnen".into(), argumente: r#"{"ausdruck":"1+1"}"#.into() }]);
        // Nachsicht: Zaun, `parameters`, Argumente als Zeichenkette, Huelle.
        for roh in [
            "```json\n{\"name\":\"x\",\"arguments\":{\"a\":1}}\n```",
            "{\"name\":\"x\",\"parameters\":{\"a\":1}}",
            "{\"name\":\"x\",\"arguments\":\"{\\\"a\\\":1}\"}",
            "{\"function\":{\"name\":\"x\",\"arguments\":{\"a\":1}}}",
        ] {
            let g = aufrufe_lesen(&format!("<tool_call>{roh}</tool_call>"));
            assert_eq!(g.aufrufe, vec![Aufruf { name: "x".into(), argumente: r#"{"a":1}"#.into() }], "{roh}");
        }
        // Gegenprobe: Unlesbares wird kein Aufruf und bleibt stehen.
        let g = aufrufe_lesen("vorher <tool_call>kein json</tool_call> nachher");
        assert!(g.aufrufe.is_empty());
        assert_eq!(g.text, "vorher <tool_call>kein json</tool_call> nachher");
        // ⚑ Ein Name, der nicht angeboten ist, wird kein Aufruf und bleibt
        //   stehen; Gegenprobe: angeboten wird er einer.
        let fremd = "<tool_call>{\"name\":\"loeschen\",\"arguments\":{}}</tool_call>";
        let g = aufrufe_lesen(fremd);
        assert!(g.aufrufe.is_empty());
        assert_eq!(g.text, fremd);
        assert_eq!(super::aufrufe_lesen(fremd, &["loeschen".to_string()]).aufrufe.len(), 1);
    }

    /// Aufruf und Antwort im Verlauf, wie die Vorlage sie schreibt.
    #[test]
    fn aufruf_und_antwort_im_verlauf() {
        assert_eq!(
            aufruf_als_text("rechnen", &serde_json::json!({"b": 2, "a": 1})),
            "<tool_call>\n{\"name\": \"rechnen\", \"arguments\": {\"a\":1,\"b\":2}}\n</tool_call>"
        );
        assert_eq!(antwort_als_text("42"), "<tool_response>\n42\n</tool_response>");
    }
}
