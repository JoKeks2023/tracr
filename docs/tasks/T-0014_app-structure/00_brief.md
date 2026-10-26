# T-0014 — App-Aufbau neu (Sidebar + Inhalt)

Anlass: "wir müssen den gesamten Aufbau neu machen". Vorgehen nach Skill `apple-enterprise-design`.

## 1. Arbeit verstehen
- Job: zeigt, wie viel Zeit am Tag erfasst wurde, was offen ist, und lässt den Tag abschließen.
- Nutzer: eine Person, täglich, kurze Besuche (Tag ansehen, offene Zeit klären, Tag prüfen).
- Fehlerfolgen: falsche Zeit im Worklog → Reviewed-Tage bleiben unveränderlich (Revisionen).
- Objekte: Tag, Projekt/Repository, offenes Zeitfenster, Eintrag/Timer, Session.

## 2. Ablauf
Start: Heute. Entscheidungen: offene Zeit klären → Tag prüfen. Sofort sichtbar: erfasst-Zahl, Bars, "offen"-Karte.
Auf Anfrage: Verlauf (Sessions/Timeline), Einstellungen, Export, andere Tage.

## 3. Natives Muster
Split View (Sidebar + Inhalt), wie Mail/Kalender: Sidebar = Tage + Einstellungen, Inhalt = ein Tag.

## 4. Hierarchie
- Sidebar (dauerhaft): Tageliste (Heute, Gestern, …), unten Erfassungsstatus + Einstellungen.
- Toolbar (pro Tag): Tag + Status links; Timer, Zeit hinzufügen, Mehr (Export, Aktualisieren) rechts.
- Inhalt: Segment "Übersicht | Verlauf" (zwei gleichrangige Ansichten desselben Tages, kein Zurück-Link).
- Übersicht: eine Zahl, Bars, eine "offen"-Karte, Kostenzeile, Banner nur bei Bedarf.
- Selten/riskant: Einstellungen (Sheet), Export, Tag prüfen (Sheet mit Bestätigung).

## 5. Layout & Navigation
- Sidebar 13rem fest, Auswahl = Tag; ⌘[ / ⌘] vorheriger/nächster Tag, ⌘1/⌘2 Segment, ⌘, Einstellungen.
- Kein Inspector; Sessions öffnen als Sheet (wie bisher).
- Schmales Fenster (<720px): Sidebar klappt weg, Tageswechsel über ‹ › in der Toolbar.
