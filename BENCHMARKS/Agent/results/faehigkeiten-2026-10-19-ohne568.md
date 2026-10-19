# Fähigkeiten des Agenten, 2026-10-19-ohne568

Saaten: 8213088584698225, 4468002801560041, 6135214929457457. Antwortlänge eingestellt 1600 (ein Agentenlauf nimmt mindestens 4000, mit großem Kontext mehr), Denken aus, Werkzeugsatz voll, Einstellungen ab Werk, je Phase ergänzt. Jede Phase ist ein eigener Lauf im selben Ordner; bestanden heißt: Die Abnahme von außen endet mit 0. „mit Muster“: Die Phase davor fiel durch und wurde durch ihre Musterlösung ersetzt.

## myelith-35b-a3b-int4

**14/21 Phasen** in 68 Minuten

| Szenario | Art | Phase | Saaten | Sekunden | gerufen (erste Saat) | Grund, falls nicht |
|---|---|---|---|---|---|---|
| `x_nadel_im_heuhaufen` | Erinnerung | 1 | ✓ ✓ ✓ | 304 / 242 / 250 | run_command, write_file |  |
| `x_nadel_im_heuhaufen` | Erinnerung | 2 | ✓ ✓ ✓ | 118 / 119 / 108 | list_history, read_history, write_file |  |
| `x_web_recherche` | Web | 1 | ✓ ✓ ✓ | 243 / 320 / 321 | read_file, run_command, unlesbarer, web_read, web_search, write_file |  |
| `x_pdf_zu_skill` | Skill aus PDF | 1 | ✗ ✓ ✗ | 262 / 230 / 234 | list_directory, read_file, run_command, search_files, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 6, in <module> AssertionError: kein Kopf |
| `x_pdf_zu_skill` | Skill aus PDF | 2 | ✗ᴹ ✓ ✗ᴹ | 171 / 255 / 180 | learn_skill, read_file, run_command | Abnahme mit 1: dt KG;297.50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-2026-W53-002; |
| `x_werkzeug_bauen` | Werkzeug | 1 | ✗ ✗ ✗ | 150 / 173 / 97 | list_directory, read_file, run_command, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 11, in <module> AssertionError: ('de89 3704 004; Abnahme mit 1: rk/Versions/3.9/lib/python3.9/pathlib.py", line 1110, in _opener     return self._accessor.open(self, fla |
| `x_werkzeug_bauen` | Werkzeug | 2 | ✓ᴹ ✓ᴹ ✓ᴹ | 94 / 97 / 99 | iban_pruefen, learn_skill, read_file, write_file |  |

ᴹ: mit Muster (die Phase davor fiel durch).
