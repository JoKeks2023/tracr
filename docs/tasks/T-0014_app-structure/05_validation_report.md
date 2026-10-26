# T-0014 — Audit und Review (2026-10-26)

Audits: HIG-Review (Code), Barrierefreiheit/Politur (Code, Kontraste aus den Tokens berechnet), eigener Blick auf das echte Fenster (dunkel, Deutsch, breites Fenster).

## Umgesetzt (Commits f21c1bf, 067532c, f284010 und Folgecommit)
- Sidebar mit Tagen, Status und Einstellungen unten; native Menüleiste mit Kurzbefehlen (DE/EN).
- Ein Vokabular: erfasst / ungeklärt (en: tracked / unresolved), "Aufzeichnung" statt Collector, "Agent-Sitzungen" statt Verlauf, "Nicht zugeordnet" statt unattributed.
- Karte: Zahl + Prüfstatus + "Tag prüfen", dann ungeklärte Zeit (mit "Warum?"), dann Projekte/Repositories. Kosten auf der Seite Agent-Sitzungen.
- "⋯"-Menü und Pfeile im Titel entfernt (Export/Aktualisieren in der Ablage, Tageswechsel über Sidebar und ⌘[ ⌘]; Pfeile nur bei schmalem Fenster).
- Sheets: Fokus beim Öffnen, Tab-Schleife, Fokus zurück, aria-modal; Escape in den Einstellungen.
- Toast-Timer lief wegen Status-Polling nie ab (behoben); sichtbarer Fokusring; Primärbutton und Warn-/Fehlerfarben im hellen Modus; gedämpfter Text im hellen Modus; Segmente als aria-pressed-Gruppe.

## Offen (bewusst nicht in diesem Schritt)
- Sidebar-Signal pro Tag (erfasste Zeit, Punkt für Ungeklärtes/Geprüft): braucht ein neues Backend-Feld in `days`.
- Einstellungen als Gruppen/Tabs, Diagnose unter "Erweitert"; einheitliche Sheet-Knöpfe (Abbrechen/OK).
- Timer-Menüpunkt wechselt nicht auf "Timer stoppen"; Menüpunkte werden nicht ausgegraut.
- Timeline: Trefferflächen unter 24 px, Space-Taste scrollt, Atom ohne aria-label; Menü-Popover ohne Pfeiltasten.
- Tooltips mit Kernerklärungen (Kosten, Statuszeile).

## Nicht verifiziert
Helles Erscheinungsbild und englische Texte nicht im Fenster gesehen; Tab-Reihenfolge und VoiceOver nicht gelaufen; Menüleiste nicht durchgeklickt; schmales Fenster nicht gesehen.
