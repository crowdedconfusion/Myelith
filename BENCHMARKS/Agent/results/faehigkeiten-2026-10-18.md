# Fähigkeiten des Agenten, 2026-10-18

Saaten: 8213088584698225, 4468002801560041, 6135214929457457. Antwortlänge eingestellt 1600 (ein Agentenlauf nimmt mindestens 4000, mit großem Kontext mehr), Denken aus, Werkzeugsatz voll, Einstellungen ab Werk, je Phase ergänzt. Jede Phase ist ein eigener Lauf im selben Ordner; bestanden heißt: Die Abnahme von außen endet mit 0. „mit Muster“: Die Phase davor fiel durch und wurde durch ihre Musterlösung ersetzt.

## myelith-35b-a3b-int4

**11/18 Phasen** in 108 Minuten

| Szenario | Art | Phase | Saaten | Sekunden | gerufen (erste Saat) | Grund, falls nicht |
|---|---|---|---|---|---|---|
| `x_nadel_im_heuhaufen` | Erinnerung | 1 | ✓ ✓ ✓ | 738 / 726 / 730 | read_file, run_command, write_file |  |
| `x_web_recherche` | Web | 1 | ✓ ✓ ✗ | 348 / 297 / 58 | search_skill, unlesbarer, web_read, web_search, write_file | Abnahme mit 1: ython3.framework/Versions/3.9/lib/python3.9/pathlib.py", line 1110, in _opener     return self._accessor. |
| `x_pdf_zu_skill` | Skill aus PDF | 1 | ✓ ✓ ✓ | 572 / 702 / 306 | learn_skill, list_directory, read_file, run_command, search_files, search_skill, write_file |  |
| `x_pdf_zu_skill` | Skill aus PDF | 2 | ✗ ✗ ✗ | 327 / 171 / 133 | learn_skill, read_file, unlesbarer, write_file | Abnahme mit 1: ndt KG;297,50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-2027-01-002;; Abnahme mit 1: ndt KG;297.50;2027-01-14', 'NL-2026-53-001;2026-12-31;Brandt KG;297,50;2027-01-14'), (8, 'NL-2026-53-008;; Abnahme mit 1: thon3.framework/Versions/3.9/lib/python3.9/pathlib.py", line 1110, in _opener     return self._accessor.o |
| `x_werkzeug_bauen` | Werkzeug | 1 | ✗ ✗ ✓ | 320 / 89 / 647 | edit_file, list_directory, read_file, run_command, unlesbarer, write_file | Abnahme mit 1: Traceback (most recent call last):   File "<stdin>", line 11, in <module> AssertionError: ('DE89370400440; Abnahme mit 1: rk/Versions/3.9/lib/python3.9/pathlib.py", line 1110, in _opener     return self._accessor.open(self, fla |
| `x_werkzeug_bauen` | Werkzeug | 2 | ✓ᴹ ✗ᴹ ✓ | 102 / 101 / 95 | iban_pruefen, learn_skill, read_file, write_file | Abnahme mit 1: ent call last):   File "<stdin>", line 3, in <module> AssertionError: ['kunde;ergebnis', 'Haas;gueltig',  |

ᴹ: mit Muster (die Phase davor fiel durch).
