# German translation. See en.ftl for the reference.

## Toolbar

toolbar-normal = Standard
toolbar-bold = F
toolbar-italic = K
toolbar-strike = S
toolbar-link = Link
toolbar-image = Bild
toolbar-table = Tabelle
toolbar-add-row = + Zeile
toolbar-add-column = + Spalte
toolbar-delete-row = − Zeile
toolbar-delete-column = − Spalte
toolbar-source = Quelltext
code-plain-text = reiner Text

## Context menu

menu-cut = Ausschneiden
menu-copy = Kopieren
menu-paste = Einfügen
menu-select-all = Alles auswählen
menu-bold = Fett
menu-italic = Kursiv
menu-strike = Durchgestrichen
menu-code = Code
menu-link = Link…
menu-row-above = Zeile oberhalb einfügen
menu-row-below = Zeile unterhalb einfügen
menu-column-left = Spalte links einfügen
menu-column-right = Spalte rechts einfügen
menu-delete-row = Zeile löschen
menu-delete-column = Spalte löschen
menu-delete-table = Tabelle löschen
menu-align-left = Linksbündig
menu-align-center = Zentriert
menu-align-right = Rechtsbündig

## Link bar

link-address = Linkadresse
link-apply = Übernehmen
link-remove = Link entfernen
link-cancel = Abbrechen
# Link dialog (opened from the context menu).
link-dialog-insert = Link einfügen
link-dialog-edit = Link bearbeiten
link-text = Anzuzeigender Text
link-ok = OK

## Status bar

status-page = Seite { $current } von { $total }
status-words =
    { $count ->
        [one] { $count } Wort
       *[other] { $count } Wörter
    }
status-tasks = { $checked }/{ $total } Aufgaben erledigt
status-saving-images =
    { $count ->
        [one] { $count } Bild wird gespeichert…
       *[other] { $count } Bilder werden gespeichert…
    }
status-mode-wysiwyg = WYSIWYG
status-mode-source = Markdown-Quelltext
status-tab-width = Tab: { $count } Leerzeichen
status-modified = Geändert

## Images

image-saving = Bild wird gespeichert…
image-rejected-disabled = Bilder sind deaktiviert
image-rejected-type = Nicht unterstütztes Bildformat: { $mime }
image-rejected-too-large = Bild zu groß ({ $size } KiB, maximal { $max } KiB)
image-rejected-unreadable = Bild kann nicht gelesen werden: { $error }
