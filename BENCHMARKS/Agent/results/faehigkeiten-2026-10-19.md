# Fähigkeiten des Agenten, 2026-10-19

Saaten: 8213088584698225, 4468002801560041, 6135214929457457. Antwortlänge eingestellt 1600 (ein Agentenlauf nimmt mindestens 4000, mit großem Kontext mehr), Denken aus, Werkzeugsatz voll, Einstellungen ab Werk, je Phase ergänzt. Jede Phase ist ein eigener Lauf im selben Ordner; bestanden heißt: Die Abnahme von außen endet mit 0. „mit Muster“: Die Phase davor fiel durch und wurde durch ihre Musterlösung ersetzt.

## myelith-35b-a3b-int4

**12/21 Phasen** in 111 Minuten

| Szenario | Art | Phase | Saaten | Sekunden | gerufen (erste Saat) | Grund, falls nicht |
|---|---|---|---|---|---|---|
| `x_nadel_im_heuhaufen` | Erinnerung | 1 | ✗ ✓ ✓ | 1145 / 246 / 262 | write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 4, in <module> AssertionError: ['ORCA-5521-TIDE |
| `x_nadel_im_heuhaufen` | Erinnerung | 2 | ✗ᴹ ✓ ✓ | 1294 / 122 / 120 | list_directory, list_history, read_file, read_history, search_history, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 3, in <module> AssertionError: Die Vorgangsnumm |
| `x_web_recherche` | Web | 1 | ✓ ✗ ✓ | 385 / 318 / 443 | read_file, web_read, web_search, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 17, in <module> AssertionError: {'ripgrep_datum |
| `x_pdf_zu_skill` | Skill aus PDF | 1 | ✓ ✓ ✗ | 254 / 258 / 250 | learn_skill, list_directory, read_file, run_command, search_skill, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 6, in <module> AssertionError: kein Kopf |
| `x_pdf_zu_skill` | Skill aus PDF | 2 | ✓ ✗ ✗ᴹ | 282 / 256 / 201 | learn_skill, read_file, run_command, search_skill, write_file | Abnahme mit 1: dt KG;297.50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-2026-W53-002;; Abnahme mit 1: ndt KG;297,50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-2027-01-002; |
| `x_werkzeug_bauen` | Werkzeug | 1 | ✗ ✗ ✗ | 162 / 226 / 114 | list_directory, read_file, run_command, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 11, in <module> AssertionError: ('de89 3704 004; Abnahme mit 1: rk/Versions/3.9/lib/python3.9/pathlib.py", line 1110, in _opener     return self._accessor.open(self, fla |
| `x_werkzeug_bauen` | Werkzeug | 2 | ✓ᴹ ✓ᴹ ✓ᴹ | 104 / 109 / 112 | iban_pruefen, learn_skill, read_file, write_file |  |

ᴹ: mit Muster (die Phase davor fiel durch).
