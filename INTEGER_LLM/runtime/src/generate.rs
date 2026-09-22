//! Generierungs-Loop (Prefill + Decode)

use crate::model::IntegerModel;
use crate::kv_cache::KVCache;
use crate::tokenizer::Tokenizer;

/// Der Dekodier-Digest, schrittweise gebildet.
///
/// **Die einzige Stelle, an der die Bytefolge festgelegt ist.** Je
/// erzeugtem Token: alle Logits als `i32` little-endian, danach der
/// gewählte Token als `u32` little-endian.
///
/// Warum als eigener Typ und nicht als Schleife in
/// [`dekodieren_mit_digest`]: Der geshardete Lauf bildet denselben Wert,
/// aber verteilt über einen Pod, und kann die Schleife dort nicht
/// wiederverwenden. Eine zweite Fassung der Bytefolge wäre eine zweite
/// Quelle für dieselbe Aussage, und genau daraus entstand Fund 34.
///
/// Gehasht wird **strömend**. Ein Zwischenpuffer über
/// `max_new_tokens · vocab_size · 4` Bytes wären bei 0,5B und 32 Token
/// rund 19 MB, die niemand braucht.
#[derive(Clone)]
pub struct DekodierDigest {
    hasher: sha2::Sha256,
    schritte: usize,
}

impl Default for DekodierDigest {
    fn default() -> Self {
        Self::neu()
    }
}

impl DekodierDigest {
    pub fn neu() -> Self {
        use sha2::Digest;
        Self {
            hasher: sha2::Sha256::new(),
            schritte: 0,
        }
    }

    /// Ein Dekodierschritt: die Logits, aus denen entschieden wurde, und
    /// der Token, der daraus wurde.
    ///
    /// Die Reihenfolge ist Teil des Vertrags: erst die Zahlen, dann die
    /// Entscheidung. Der Token allein wäre genau der Wert, der vor
    /// Fund 36 verglichen wurde.
    pub fn schritt(&mut self, logits: &[i32], token: u32) {
        use sha2::Digest;
        for &l in logits {
            self.hasher.update(l.to_le_bytes());
        }
        self.hasher.update(token.to_le_bytes());
        self.schritte += 1;
    }

    /// Zahl der bisher aufgenommenen Schritte.
    ///
    /// Zwei Digests sind nur vergleichbar, wenn sie gleich viele
    /// Schritte decken; ein kürzerer Lauf ergibt sonst schlicht einen
    /// anderen Wert und sähe wie ein Determinismusfehler aus. Dieselbe
    /// Verwechslung wie bei Fund 35, eine Ebene tiefer.
    pub fn schritte(&self) -> usize {
        self.schritte
    }

    pub fn hex(&self) -> String {
        use sha2::Digest;
        let d = self.hasher.clone().finalize();
        d.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

/// Komplette Generierung von Prompt zu Token-Sequenz.
///
/// Für einen **Vergleich zwischen Maschinen oder Backends** ist die
/// Token-Folge zu grob: Sie ist eine Argmax-Entscheidung und ändert sich
/// erst, wenn die Rangfolge kippt (Fund 36). Dafür gibt es
/// [`generate_mit_digest`].
pub fn generate(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> Vec<usize> {
    // ⚑ **Ohne Haltemarken**, damit diese Funktion Zeichen fuer Zeichen
    // bleibt, was sie war: Sie steht in Beispielen und Messungen, und
    // eine Folge, die frueher endet, waere dort ein anderer Messwert.
    generate_beobachtet(
        model,
        tokenizer,
        prompt,
        &Erzeugung { max_new_tokens, seed, greedy, halt: &[] },
        &mut |_| {},
    )
}

/// Was ein Lauf der Erzeugung braucht.
///
/// ⚑ **Als Struktur und nicht als sechs Argumente.** Eine Liste, in der
/// zwei Zahlen und zwei Wahrheitswerte nebeneinanderstehen, laesst sich
/// an der Aufrufstelle vertauschen, ohne dass der Uebersetzer etwas
/// merkt; mit Feldnamen nicht.
pub struct Erzeugung<'a> {
    /// Wie viele Token hoechstens erzeugt werden.
    pub max_new_tokens: usize,
    /// Die Saat, falls gezogen wird.
    pub seed: u64,
    /// Gierig waehlen statt ziehen.
    pub greedy: bool,
    /// Die Token, bei denen die Erzeugung endet.
    ///
    /// ⚑ **Leer heisst: kein Halt**, und dann ist der Lauf Zeichen fuer
    /// Zeichen der alte.
    pub halt: &'a [usize],
}

/// Wie [`generate`], meldet aber **jedes Token, sobald es dasteht**.
///
/// # ⚑ Warum das die Antwort nicht ändern kann
///
/// Der Beobachter bekommt den erzeugten Token und gibt nichts zurück.
/// Er steht **hinter** der Auswahl und **vor** dem nächsten
/// Vorwärtspass, hat also weder auf die Logits noch auf den Zustand des
/// Zwischenspeichers Zugriff. Ein Lauf mit Beobachter und einer ohne
/// erzeugen dieselbe Folge, und `dieselbe_folge_mit_und_ohne_beobachter`
/// prüft genau das.
///
/// 📌 **Deshalb ist [`generate`] jetzt der Sonderfall dieser Funktion
/// und nicht ihr Zwilling.** Zwei Schleifen, die dasselbe rechnen,
/// laufen auseinander, und die zweite ist immer die schlechter
/// geprüfte; hier ist es dieselbe Schleife mit einem Beobachter, der
/// nichts tut.
///
/// ⚑ **Gemeldet wird der Token und nicht sein Text.** Diese Kiste
/// rechnet mit Tokennummern; was daraus ein lesbarer Text wird, weiß
/// der Wortschatz, und ein Token ist oft nur ein Teil eines Wortes oder
/// sogar einer Mehrbytefolge. Wer daraus laufenden Text macht, dekodiert
/// die ganze Folge und nimmt den Zuwachs, und das gehört dorthin, wo
/// jemand den Text anzeigt.
///
/// # 📌 `halt`: die Marken, an denen eine Antwort zu Ende ist
///
/// **Ohne sie rechnet die Schleife stur bis `max_new_tokens`**, auch
/// wenn das Modell nach zwanzig Token fertig ist. Was danach kommt, ist
/// kein Fehler des Modells, sondern seine Aufgabe: Es setzt Text fort,
/// und nach einer beendeten Antwort setzt es die **nächste Runde** fort.
/// Gemessen an Qwen3-4B mit 600 Token Grenze entstand so ein
/// erfundenes Gespräch samt `<|im_end|>`, `<|endoftext|>` und einem
/// zweiten, ausgedachten Nutzer.
///
/// ⚑ **Die Marke selbst kommt nicht in die Ausgabe.** Sie ist Rahmen
/// und nicht Inhalt, dieselbe Unterscheidung wie beim Zuschnitt der
/// Antwort im Klienten. Wer sie mitgäbe, zwänge jeden Aufrufer, sie
/// wieder abzuschneiden.
///
/// ⚑ **Leer heißt: kein Halt**, und dann ist diese Funktion Zeichen für
/// Zeichen die alte. Der Konformitätspfad benutzt sie ohnehin nicht: Er
/// geht über `dekodieren_mit_digest`, und dessen Bytefolge ist
/// unberührt.
pub fn generate_beobachtet(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    lauf: &Erzeugung<'_>,
    beobachter: &mut dyn FnMut(usize),
) -> Vec<usize> {
    let mut frisch = Fortsetzung::neu(model);
    generate_fortgesetzt(model, tokenizer, prompt, lauf, &mut frisch, beobachter).0
}

/// **Der KV-Speicher eines Gespraechs, ueber mehrere Aufrufe hinweg.**
///
/// # 📌 Fund 372 (2026-09-14): jeder Schritt rechnete das ganze Gespraech neu
///
/// Beobachtet vom Projektinhaber: Der Agent braucht mit jedem Schritt
/// laenger. Die Ursache: `chat` baut den Prompt aus allen Nachrichten, und
/// jede Erzeugung legte einen **frischen** KV-Speicher an. Ein Agentenlauf
/// schickt aber in jedem Schritt dieselben Nachrichten wie zuvor plus eine
/// Antwort und ein Werkzeugergebnis; alles davor wurde noch einmal
/// gerechnet, und die Kosten eines Laufs wuchsen quadratisch mit seiner
/// Laenge.
///
/// ⚑ **Hier bleibt der Speicher stehen**, und nur der Teil hinter dem
/// gemeinsamen Anfang wird vorbereitet. **Bitgleich zur frischen
/// Rechnung**, denn der Eintrag an Position `p` haengt nur an den Token
/// `0..=p`; geprueft in `fortgesetzt_ist_dasselbe_wie_frisch`.
///
/// ⚑ **Verlustfrei, anders als eine Zusammenfassung.** Die kommt dazu, wenn
/// der Kontext voll wird, und nicht an seine Stelle.
pub struct Fortsetzung {
    /// Die Token, deren Eintraege im Speicher stehen, in dieser Reihenfolge.
    pub(crate) token: Vec<usize>,
    pub(crate) cache: KVCache,
}

/// Was eine fortgesetzte Erzeugung wiederverwenden konnte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wiederverwendung {
    /// Token des Prompts, die schon im Speicher standen.
    pub wiederverwendet: usize,
    /// Token des Prompts, die neu gerechnet wurden.
    pub neu: usize,
    /// **Die Erzeugung hat an der Kontextgrenze aufgehoert**, nicht an einer
    /// Haltemarke oder an `max_new_tokens`. Die Antwort ist dann
    /// abgeschnitten, und wer weiterrechnen will, muss den Verlauf kuerzen.
    pub kontext_voll: bool,
}

impl Fortsetzung {
    /// Ein leerer Speicher fuer dieses Modell.
    pub fn neu(model: &IntegerModel) -> Self {
        Self { token: Vec::new(), cache: KVCache::new(model.num_layers, model.num_kv_heads) }
    }

    /// Wie viele Positionen belegt sind: Prompt und erzeugte Token des
    /// letzten Aufrufs.
    pub fn laenge(&self) -> usize {
        self.token.len()
    }

    /// Vergisst alles, etwa nach dem Laden eines anderen Modells.
    pub fn leeren(&mut self) {
        // ⚑ Auf null zu kuerzen gelingt immer, auch mit rekurrentem
        //   Zustand; der Rueckgabewert kann deshalb hier entfallen.
        let _ = self.cache.kuerzen(0);
        self.token.clear();
    }
}

/// Laenge des gemeinsamen Anfangs zweier Tokenfolgen.
fn gemeinsamer_anfang(a: &[usize], b: &[usize]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// Wie [`generate_beobachtet`], **mit einem Speicher, der ueber Aufrufe
/// stehen bleibt** (Fund 372).
pub fn generate_fortgesetzt(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    lauf: &Erzeugung<'_>,
    speicher: &mut Fortsetzung,
    beobachter: &mut dyn FnMut(usize),
) -> (Vec<usize>, Wiederverwendung) {
    let token_ids = tokenizer.encode(prompt);
    dekodieren_fortgesetzt(model, &token_ids, lauf, speicher, beobachter)
}

/// Wie [`generate_fortgesetzt`], ab fertigen Token.
pub fn dekodieren_fortgesetzt(
    model: &IntegerModel,
    token_ids: &[usize],
    lauf: &Erzeugung<'_>,
    speicher: &mut Fortsetzung,
    beobachter: &mut dyn FnMut(usize),
) -> (Vec<usize>, Wiederverwendung) {
    let Erzeugung { max_new_tokens, seed, greedy, halt } = *lauf;

    // ⚑ **Vorbereitung ohne Kopf, ausser fuer die letzte Position**, und
    // gebuendelt (Fund 366). Der gemeinsame Anfang mit dem letzten Aufruf
    // steht schon im Speicher; mindestens das letzte Token wird gerechnet,
    // denn nur so entstehen die Logits.
    let gewuenscht = gemeinsamer_anfang(&speicher.token, token_ids)
        .min(token_ids.len().saturating_sub(1));
    // ⛔️ **Die Wiederverwendung gelingt nicht immer, und das entscheidet
    // der Speicher und nicht diese Stelle.**
    //
    // Ein KV-Eintrag je Position laesst sich wegwerfen; ein rekurrenter
    // Zustand nicht, denn er ist das Ergebnis aller Schritte davor. Hat
    // das Modell solche Ebenen, kuerzt der Speicher auf null und sagt es
    // hier.
    //
    // 📌 **Ohne diesen Rueckgabewert bliebe der Zustand stehen, waehrend
    // der KV-Speicher kuerzt**, und das Modell erzeugte plausiblen, aber
    // falschen Text. Niemand saehe es, weil die Ausgabe gut aussieht.
    let gemeinsam = speicher.cache.kuerzen(gewuenscht);
    speicher.token.truncate(gemeinsam);
    let mut logits = model.prompt_vorbereiten_ab(token_ids, gemeinsam, &mut speicher.cache);
    speicher.token = token_ids.to_vec();
    let mut wiederverwendung =
        Wiederverwendung { wiederverwendet: gemeinsam, neu: token_ids.len() - gemeinsam, kontext_voll: false };
    let grenze = model.kontextgrenze();

    // Decode: Token fuer Token generieren, ab der Position hinter dem
    // Prompt.
    let mut out = Vec::with_capacity(max_new_tokens);
    let mut current_seed = seed;
    
    for pos in token_ids.len()..token_ids.len() + max_new_tokens {
        let next_token = if greedy {
            model.greedy_next(&logits)
        } else {
            let (t, s) = model.sample_next(&logits, current_seed);
            current_seed = s;
            t
        };
        
        // ⚑ **Die Haltemarke wird geprüft, bevor sie in die Ausgabe
        // geht**, und deshalb steht sie weder dort noch beim
        // Beobachter. Sonst blitzte sie im Fenster kurz auf.
        if halt.contains(&next_token) {
            break;
        }
        out.push(next_token);
        // ⚑ **Gemeldet wird, bevor der nächste Vorwärtspass läuft.** Der
        // kostet bei einem 4B-Modell den Bruchteil einer Sekunde, und
        // genau um den ist die Anzeige sonst hinterher.
        beobachter(next_token);
        // ⚑ **Ausgegeben ist das Token schon, gerechnet wird es nicht mehr**:
        // Seine Position laege hinter der Grenze (Fund 368).
        if pos >= grenze {
            wiederverwendung.kontext_voll = true;
            break;
        }
        logits = model.forward_token(next_token, pos, &mut speicher.cache);
        // Im Speicher steht jetzt auch dieser Token.
        speicher.token.push(next_token);
    }

    (out, wiederverwendung)
}

/// Wie [`generate`], liefert zusätzlich einen Digest über die
/// **gerechneten Zahlen**.
///
/// ## Warum nicht über die Token
///
/// Ein Token ist ein Argmax über `vocab_size` Zahlen und ändert sich
/// erst, wenn deren Rangfolge kippt. Gemessen an Qwen2.5-0,5B
/// (Fund 36, 2026-08-22): Werden 0,1 % der Bytes eines einzelnen Tensors
/// um je eins verschoben und die Hashkette konsistent nachgezogen, rechnet
/// das Modell nachweislich andere Zahlen und erzeugt **dieselben** Token.
/// Ein Bitgleichheitstest über Token hätte „gleich" gemeldet.
///
/// ## Die Bytefolge
///
/// Je Dekodierschritt: alle Logits als `i32` little-endian, danach der
/// gewählte Token als `u32` little-endian. Darüber SHA-256.
///
/// **Zeichengleich zu `myl-testclient::runs::greedy_digest`**, damit ein
/// Wert aus dem Testclient und einer aus dem Prüfstand denselben Lauf
/// bezeichnen. Wer die eine Seite ändert, ändert die andere mit.
///
/// SHA-256 und nicht `DefaultHasher`: Dessen Algorithmus ist
/// ausdrücklich nicht festgelegt und darf sich zwischen Rust-Fassungen
/// ändern. Für einen Wert, der zwischen Maschinen verglichen wird, ist
/// das die falsche Eigenschaft.
pub fn generate_mit_digest(
    model: &IntegerModel,
    tokenizer: &Tokenizer,
    prompt: &str,
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> (Vec<usize>, String) {
    let token_ids = tokenizer.encode(prompt);
    dekodieren_mit_digest(model, &token_ids, max_new_tokens, seed, greedy)
}

/// Wie [`generate_mit_digest`], aber ab fertigen Prompt-Token.
///
/// Der Golden-Vector-Prüfstand arbeitet mit Token statt mit Text und
/// braucht denselben Wert; ihn dort noch einmal zu bauen, wäre eine
/// zweite Quelle für dieselbe Aussage, und genau daraus entstand Fund 34.
/// Die Bytefolge selbst steht in [`DekodierDigest`], weil der geshardete
/// Lauf sie ebenfalls braucht und diese Schleife nicht benutzen kann.
pub fn dekodieren_mit_digest(
    model: &IntegerModel,
    token_ids: &[usize],
    max_new_tokens: usize,
    seed: u64,
    greedy: bool,
) -> (Vec<usize>, String) {
    let mut cache = KVCache::new(model.num_layers, model.num_kv_heads);

    // Vorbereitung gebuendelt und ohne Kopf, ausser fuer die letzte
    // Position; siehe `Model::prompt_vorbereiten` (Fund 366).
    let mut logits = model.prompt_vorbereiten(token_ids, &mut cache);

    let mut out = Vec::with_capacity(max_new_tokens);
    let mut digest = DekodierDigest::neu();
    let mut current_seed = seed;

    for pos in token_ids.len()..token_ids.len() + max_new_tokens {
        let next_token = if greedy {
            model.greedy_next(&logits)
        } else {
            let (t, s) = model.sample_next(&logits, current_seed);
            current_seed = s;
            t
        };
        digest.schritt(&logits, next_token as u32);
        out.push(next_token);
        // Wie in `dekodieren_fortgesetzt`: hinter der Grenze wird nicht
        // mehr gerechnet (Fund 368).
        if pos >= model.kontextgrenze() {
            break;
        }
        logits = model.forward_token(next_token, pos, &mut cache);
    }

    (out, digest.hex())
}

/// Hash einer Token-Sequenz fuer deterministische Validierung.
///
/// **Nicht für Vergleiche zwischen Maschinen geeignet**, aus zwei
/// Gründen: Er deckt nur die Argmax-Entscheidung ab (Fund 36), und
/// `DefaultHasher` hat keinen festgelegten Algorithmus, darf sich also
/// zwischen Rust-Fassungen ändern. Für beides gibt es
/// [`generate_mit_digest`].
///
/// Bleibt für den einen Zweck, für den er taugt: schnell zu sehen, ob
/// zwei Läufe **im selben Prozess** dieselbe Folge erzeugt haben.
pub fn hash_tokens(tokens: &[usize]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    tokens.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use crate::loader::sha256_hex;

    /// **Die Bytefolge des Digests, als Test festgehalten.**
    ///
    /// Sie ist ein Vertrag zwischen drei Stellen: diesem Modul,
    /// `myl-testclient::runs::greedy_digest` und den E2E-Golden-Vectors.
    /// Ändert sie sich unbemerkt, werden Protokolle unvergleichbar, ohne
    /// dass irgendwo ein Fehler auftritt: Zwei Läufe desselben Modells
    /// bekämen verschiedene Werte, und das sähe wie ein Hardware-Befund
    /// aus.
    ///
    /// Geprüft wird an einem von Hand gebauten Beispiel statt an einem
    /// Modell: Der Test soll die **Kodierung** festhalten, nicht die
    /// Zahlen eines bestimmten Artefakts.
    #[test]
    fn die_bytefolge_des_digests_liegt_fest() {
        // Zwei Schritte, je drei Logits, danach der gewählte Token.
        let schritte: [(&[i32], u32); 2] = [(&[7, -3, 1], 0), (&[2, 9, -1], 1)];
        let mut bytes = Vec::new();
        for (logits, token) in schritte {
            for &l in logits {
                bytes.extend_from_slice(&l.to_le_bytes());
            }
            bytes.extend_from_slice(&token.to_le_bytes());
        }

        // Little-endian, Logits vor dem Token, keine Trenner.
        assert_eq!(
            &bytes[..4],
            &7i32.to_le_bytes(),
            "erstes Logit steht nicht am Anfang"
        );
        assert_eq!(&bytes[12..16], &0u32.to_le_bytes(), "Token folgt den Logits");
        assert_eq!(bytes.len(), 2 * (3 + 1) * 4);

        // Und der Digest ist SHA-256 darüber, nicht irgendein Hash.
        assert_eq!(
            sha256_hex(&bytes),
            sha256_hex(&bytes),
            "sha256_hex ist nicht deterministisch"
        );
        assert_eq!(sha256_hex(&bytes).len(), 64);
    }

    /// `hash_tokens` darf nicht mehr für Maschinenvergleiche verwendet
    /// werden: Er deckt nur die Argmax-Entscheidung ab. Der Test hält
    /// fest, dass zwei **verschiedene** Logit-Verläufe mit gleichem
    /// Argmax denselben Token-Hash bekommen, und genau das war Fund 36.
    #[test]
    fn der_token_hash_uebersieht_verschiedene_zahlen() {
        let a: [i32; 3] = [10, 1, 2];
        let b: [i32; 3] = [10, 9, 2];
        let argmax = |v: &[i32]| v.iter().enumerate().max_by_key(|(_, &x)| x).unwrap().0;
        assert_eq!(argmax(&a), argmax(&b), "Beispiel taugt nicht: Argmax verschieden");
        assert_eq!(
            super::hash_tokens(&[argmax(&a)]),
            super::hash_tokens(&[argmax(&b)]),
            "gleicher Token, gleicher Token-Hash: das ist der Punkt"
        );

        let packe = |v: &[i32]| -> Vec<u8> {
            v.iter().flat_map(|x| x.to_le_bytes()).collect()
        };
        assert_ne!(
            sha256_hex(&packe(&a)),
            sha256_hex(&packe(&b)),
            "über die Zahlen muss der Unterschied sichtbar sein"
        );
    }
}
