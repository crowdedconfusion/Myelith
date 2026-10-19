# Fähigkeiten des Agenten, 2026-10-19-nachher

Saaten: 8213088584698225, 4468002801560041, 6135214929457457. Antwortlänge eingestellt 1600 (ein Agentenlauf nimmt mindestens 4000, mit großem Kontext mehr), Denken aus, Werkzeugsatz voll, Einstellungen ab Werk, je Phase ergänzt. Jede Phase ist ein eigener Lauf im selben Ordner; bestanden heißt: Die Abnahme von außen endet mit 0. „mit Muster“: Die Phase davor fiel durch und wurde durch ihre Musterlösung ersetzt.

## myelith-35b-a3b-int4

**12/21 Phasen** in 84 Minuten

| Szenario | Art | Phase | Saaten | Sekunden | gerufen (erste Saat) | Grund, falls nicht |
|---|---|---|---|---|---|---|
| `x_nadel_im_heuhaufen` | Erinnerung | 1 | ✓ ✓ ✓ | 314 / 452 / 306 | write_file |  |
| `x_nadel_im_heuhaufen` | Erinnerung | 2 | ✓ ✓ ✓ | 117 / 295 / 112 | list_history, read_history, write_file |  |
| `x_web_recherche` | Web | 1 | ✓ ✓ ✓ | 302 / 325 / 448 | web_read, web_search, write_file |  |
| `x_pdf_zu_skill` | Skill aus PDF | 1 | ✗ ✓ ✓ | 230 / 206 / 211 | learn_skill, list_directory, read_file, run_command, search_skill, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 6, in <module> AssertionError: kein Kopf |
| `x_pdf_zu_skill` | Skill aus PDF | 2 | ✗ᴹ ✗ ✗ | 105 / 402 / 148 | learn_skill, read_file, write_file | Abnahme mit 1:  KG;297.50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-2027-01-01-002;; Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 5, in <module> AssertionError: (9, [(4, 'NL-202; Abnahme mit 1: randt KG;297,50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-26-53-002; |
| `x_werkzeug_bauen` | Werkzeug | 1 | ✗ ✗ ✗ | 211 / 472 / 148 | list_directory, read_file, read_history, run_command, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 11, in <module> AssertionError: ('DE89370400440 |
| `x_werkzeug_bauen` | Werkzeug | 2 | ✗ᴹ ✗ᴹ ✓ᴹ | 74 / 89 / 75 | iban_pruefen, read_file, write_file | Abnahme mit 1: ecent call last):   File "<stdin>", line 3, in <module> AssertionError: ['kunde;ergebnis', 'Haas;gueltig' |

ᴹ: mit Muster (die Phase davor fiel durch).
