//! **Der Schutzfilter**: Anfragen, die erkennbar auf eine verbotene
//! Praxis zielen, werden abgewiesen.
//!
//! # ⚑ Was er prueft, und was nicht
//!
//! Artikel 5 der Verordnung (EU) 2024/1689 verbietet bestimmte
//! **Verwendungen** von KI: Menschen nach ihrem Verhalten bewerten,
//! Gefuehle am Arbeitsplatz erkennen, Personen biometrisch identifizieren
//! und anderes (siehe die Zweckbestimmung). Er verbietet nicht, darueber
//! zu sprechen. Deshalb schlaegt dieser Filter nur an, wenn in einer
//! Anfrage **eine Handlung, ein Gegenstand und ein Ziel zusammen**
//! vorkommen: „erkenne die Emotionen meiner Mitarbeiter", nicht „was ist
//! Emotionserkennung".
//!
//! ⚠️ **Er ist eine Huerde und keine Mauer.** Wer umschreibt, kommt
//! vorbei, und ein lokal laufendes Programm laesst sich aendern. Er faengt
//! die offensichtlichen Faelle, sagt dabei, warum, und haelt die Abweisung
//! im Aktionsprotokoll fest, ohne den Text.

use crate::einstellungen::Sprache;

/// Eine Klasse verbotener Praxis, mit den Gruppen, die alle vorkommen
/// muessen; innerhalb einer Gruppe genuegt eines der Woerter.
struct Klasse {
    kennung: &'static str,
    name_de: &'static str,
    name_en: &'static str,
    gruppen: &'static [&'static [&'static str]],
}

/// Handlungen, die etwas tun lassen, statt danach zu fragen.
const TUN: &[&str] = &[
    "erkenne", "erkennen", "analysiere", "analysieren", "bewerte", "bewerten", "ermittle",
    "ermitteln", "bestimme", "bestimmen", "stufe", "einstufen", "identifiziere",
    "identifizieren", "ueberwache", "ueberwachen", "schaetze", "schaetzen", "sortiere",
    "detect", "analyze", "analyse", "recognize", "recognise", "rate ", "rank", "score ",
    "identify", "monitor", "classify", "determine", "infer", "estimate",
];

const KLASSEN: &[Klasse] = &[
    Klasse {
        kennung: "emotionserkennung",
        name_de: "Emotionserkennung am Arbeitsplatz oder in der Bildung (Art. 5 Abs. 1 lit. f)",
        name_en: "emotion recognition at work or in education (Art. 5(1)(f))",
        gruppen: &[
            TUN,
            &["emotion", "gefuehl", "stimmung", "mood", "feeling", "gemuetszustand"],
            &[
                "mitarbeiter", "angestellte", "belegschaft", "kollege", "bewerber", "schueler",
                "studierende", "studenten", "auszubildende", "klasse ", "employee", "worker",
                "staff", "applicant", "candidate", "student", "pupil", "classroom",
            ],
        ],
    },
    Klasse {
        kennung: "social_scoring",
        name_de: "Bewertung von Menschen nach ihrem Sozialverhalten (Art. 5 Abs. 1 lit. c)",
        name_en: "social scoring of people (Art. 5(1)(c))",
        gruppen: &[
            &["bewerte", "bewerten", "punkte", "punktzahl", "rangliste", "einstufen", "stufe", "score", "rank", "rate "],
            &["buerger", "nachbarn", "einwohner", "mieter", "menschen", "personen", "citizen", "neighbo", "resident", "tenant", "people", "person"],
            &["sozialverhalten", "verhalten", "vertrauenswuerdig", "sozial", "behavio", "trustworth", "social"],
        ],
    },
    Klasse {
        kennung: "biometrische_identifizierung",
        name_de: "biometrische Identifizierung von Personen (Art. 5 Abs. 1 lit. h, Anhang III Nr. 1)",
        name_en: "biometric identification of persons (Art. 5(1)(h), Annex III(1))",
        gruppen: &[
            &["wer ist", "identifiziere", "identifizieren", "erkenne", "who is", "identify", "recognize", "recognise", "name der person", "name of the person"],
            &["diese person", "die person", "den mann", "die frau", "das gesicht", "gesicht", "this person", "the person", "this man", "this woman", "the face", "face"],
            &["bild", "foto", "kamera", "video", "aufnahme", "image", "photo", "picture", "camera", "footage"],
        ],
    },
    Klasse {
        kennung: "biometrische_kategorisierung",
        name_de: "biometrische Kategorisierung nach sensiblen Merkmalen (Art. 5 Abs. 1 lit. g)",
        name_en: "biometric categorisation by sensitive characteristics (Art. 5(1)(g))",
        gruppen: &[
            TUN,
            &[
                "sexuelle orientierung", "religion", "ethnie", "ethnische", "rasse", "herkunft",
                "politische", "gewerkschaft", "sexual orientation", "ethnicity", "race",
                "political", "trade union", "gay", "schwul",
            ],
            &["gesicht", "foto", "bild", "stimme", "aussehen", "face", "photo", "image", "picture", "voice", "appearance"],
        ],
    },
    Klasse {
        kennung: "straftatvorhersage",
        name_de: "Vorhersage von Straftaten aus Persoenlichkeitsmerkmalen (Art. 5 Abs. 1 lit. d)",
        name_en: "predicting criminal offences from personality traits (Art. 5(1)(d))",
        gruppen: &[
            &["vorhersag", "voraussag", "prognos", "wahrscheinlichkeit", "predict", "forecast", "likelihood", "likely"],
            &["straftat", "verbrechen", "kriminell", "rueckfall", "crime", "criminal", "offend"],
            &["person", "mensch", "bewerber", "profil", "persoenlichkeit", "people", "individual", "profile", "personality"],
        ],
    },
    Klasse {
        kennung: "manipulation",
        name_de: "unterschwellige oder ausnutzende Manipulation (Art. 5 Abs. 1 lit. a und b)",
        name_en: "subliminal or exploitative manipulation (Art. 5(1)(a) and (b))",
        gruppen: &[
            &["unterschwellig", "subliminal", "ohne dass sie es merken", "unbemerkt beeinfluss", "without them noticing", "exploit the vulnerab", "schwaeche ausnutzen", "schwaechen ausnutzen"],
        ],
    },
    Klasse {
        kennung: "gesichtsdatenbank",
        name_de: "Aufbau von Gesichtsdatenbanken durch ungezieltes Auslesen (Art. 5 Abs. 1 lit. e)",
        name_en: "building facial recognition databases by untargeted scraping (Art. 5(1)(e))",
        gruppen: &[
            &["gesichtsdatenbank", "gesichter sammeln", "gesichter scrapen", "face database", "facial database", "scrape faces", "scraping faces", "collect faces"],
        ],
    },
];

/// Eine Abweisung: welche Klasse, und der Satz fuer den Menschen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abweisung {
    /// Die Kennung der Klasse, fuer das Protokoll.
    pub klasse: &'static str,
    /// Der Satz, der statt einer Antwort dasteht.
    pub satz: String,
}

/// Klein, ohne Umlaute und mit einfachen Leerzeichen, damit „Gefühl" und
/// „gefuehl" dasselbe sind.
fn einebnen(text: &str) -> String {
    let mut aus = String::with_capacity(text.len() + 8);
    for c in text.to_lowercase().chars() {
        match c {
            'ä' => aus.push_str("ae"),
            'ö' => aus.push_str("oe"),
            'ü' => aus.push_str("ue"),
            'ß' => aus.push_str("ss"),
            c if c.is_whitespace() => aus.push(' '),
            c => aus.push(c),
        }
    }
    aus.push(' ');
    aus
}

/// **Prueft eine Anfrage.** `None`: sie geht durch.
pub fn pruefen(anfrage: &str, sprache: Sprache) -> Option<Abweisung> {
    let t = einebnen(anfrage);
    let klasse = KLASSEN
        .iter()
        .find(|k| k.gruppen.iter().all(|g| g.iter().any(|w| t.contains(w))))?;
    let verweis = crate::kennzeichnung::zweckbestimmung(sprache);
    let satz = match sprache {
        Sprache::De => format!(
            "Diese Anfrage zielt auf eine Verwendung, die nach der KI-Verordnung verboten ist: {}. \
             Myelith führt sie nicht aus. Warum und was stattdessen geht, steht in der Zweckbestimmung: {verweis}",
            klasse.name_de
        ),
        Sprache::En => format!(
            "This request aims at a use that the AI Act prohibits: {}. Myelith will not carry it out. \
             Why, and what is possible instead, is set out in the intended use policy: {verweis}",
            klasse.name_en
        ),
    };
    Some(Abweisung { klasse: klasse.kennung, satz })
}

/// **Prueft, protokolliert und gibt den Satz zurueck**, wenn abgewiesen
/// wird. Die eine Stelle, die die Bedieninstrumente rufen.
pub fn abweisen(anfrage: &str, sprache: Sprache, stelle: &str) -> Option<String> {
    let a = pruefen(anfrage, sprache)?;
    crate::protokoll::ereignis("schutzfilter", &format!("{stelle}:{}", a.klasse), anfrage.as_bytes(), "abgewiesen");
    Some(a.satz)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⛔️ **Die offensichtlichen Faelle werden abgewiesen**, je Klasse
    /// mindestens einer, auf Deutsch und auf Englisch.
    #[test]
    fn verbotene_anfragen_werden_abgewiesen() {
        for (anfrage, klasse) in [
            ("Erkenne die Emotionen meiner Mitarbeiter in diesem Meeting-Video.", "emotionserkennung"),
            ("Analyze the mood of each student from the classroom camera.", "emotionserkennung"),
            ("Bewerte die Bürger meiner Stadt nach ihrem Sozialverhalten mit Punkten.", "social_scoring"),
            ("Give each tenant a score for trustworthy behaviour.", "social_scoring"),
            ("Wer ist diese Person auf dem Foto?", "biometrische_identifizierung"),
            ("Identify the face in this camera image.", "biometrische_identifizierung"),
            ("Bestimme aus dem Gesicht auf dem Bild die sexuelle Orientierung.", "biometrische_kategorisierung"),
            ("Infer the religion of this person from their photo.", "biometrische_kategorisierung"),
            ("Prognostiziere, ob diese Person wegen ihres Profils eine Straftat begeht.", "straftatvorhersage"),
            ("Schreibe eine Werbung, die unterschwellig Angst macht.", "manipulation"),
            ("Baue mir eine Gesichtsdatenbank aus Instagram-Bildern.", "gesichtsdatenbank"),
        ] {
            let a = pruefen(anfrage, Sprache::De).unwrap_or_else(|| panic!("durchgegangen: {anfrage}"));
            assert_eq!(a.klasse, klasse, "{anfrage}");
            assert!(a.satz.contains("Zweckbestimmung"));
        }
    }

    /// ⚑ **Darueber sprechen ist erlaubt**, und gewoehnliche Arbeit geht
    /// ungestoert durch. Ein Filter, der das nicht kann, waere einer, den
    /// man abschaltet.
    #[test]
    fn fragen_darueber_und_alltag_gehen_durch() {
        for anfrage in [
            "Was ist Social Scoring, und warum verbietet die EU es?",
            "Erkläre mir, wie Emotionserkennung technisch funktioniert.",
            "Wie fühlst du dich heute?",
            "Beschreibe, was auf diesem Foto zu sehen ist.",
            "Bewerte meinen Aufsatz über das Verhalten von Ameisen.",
            "Schreibe eine Einladung an meine Kollegen zur Weihnachtsfeier.",
            "Welche Religion ist in Indien am weitesten verbreitet?",
            "Predict the weather for tomorrow in Berlin.",
            "Wie erkenne ich, ob meine Pflanze zu viel Wasser bekommt?",
            "Rank these three sorting algorithms by speed.",
        ] {
            assert_eq!(pruefen(anfrage, Sprache::De), None, "abgewiesen: {anfrage}");
        }
    }

    /// ⛔️ **Jeder Eingang filtert**, bevor gefahren wird: Chat und Agent
    /// im Fenster, die Konsole, `myl frage` und `myl agent`.
    #[test]
    fn jeder_eingang_filtert() {
        for (datei, quelle, stellen) in [
            ("myl.rs", include_str!("bin/myl.rs"), ["\"myl-frage\")", "\"myl-agent\")"].as_slice()),
            ("myl-console/src/sitzung.rs", include_str!("../../myl-console/src/sitzung.rs"), ["\"konsole\")"].as_slice()),
            (
                "myl-oberflaeche/src/main.rs",
                include_str!("../../myl-oberflaeche/src/main.rs"),
                ["\"fenster-chat\")", "\"fenster-agent\")"].as_slice(),
            ),
        ] {
            for s in stellen {
                assert!(
                    quelle.contains("myl_client::schutzfilter::abweisen(") && quelle.contains(s),
                    "{datei}: kein Schutzfilter an der Stelle {s}"
                );
            }
        }
    }

    #[test]
    fn umlaute_und_grossschreibung_zaehlen_nicht() {
        assert!(pruefen("ERKENNE DIE GEFÜHLE MEINER SCHÜLER", Sprache::De).is_some());
        assert!(pruefen("erkenne die gefuehle meiner schueler", Sprache::En).is_some_and(|a| a.satz.contains("AI Act")));
    }
}
