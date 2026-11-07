# Italian translation. See en.ftl for the reference.

## Toolbar

toolbar-normal = Normale
toolbar-bold = G
toolbar-italic = C
toolbar-strike = B
toolbar-link = Link
toolbar-image = Immagine
toolbar-table = Tabella
toolbar-add-row = + Riga
toolbar-add-column = + Col.
toolbar-delete-row = − Riga
toolbar-delete-column = − Col.
toolbar-source = Sorgente
code-plain-text = testo semplice

## Context menu

menu-cut = Taglia
menu-copy = Copia
menu-paste = Incolla
menu-select-all = Seleziona tutto
menu-bold = Grassetto
menu-italic = Corsivo
menu-strike = Barrato
menu-code = Codice
menu-link = Link…
menu-row-above = Inserisci riga sopra
menu-row-below = Inserisci riga sotto
menu-column-left = Inserisci colonna a sinistra
menu-column-right = Inserisci colonna a destra
menu-delete-row = Elimina riga
menu-delete-column = Elimina colonna
menu-delete-table = Elimina tabella
menu-align-left = Allinea a sinistra
menu-align-center = Centra
menu-align-right = Allinea a destra

## Link bar

link-address = Indirizzo del link
link-apply = Applica
link-remove = Rimuovi link
link-cancel = Annulla
# Link dialog (opened from the context menu).
link-dialog-insert = Inserisci link
link-dialog-edit = Modifica link
link-text = Testo da visualizzare
link-ok = OK

## Status bar

status-page = Pagina { $current } di { $total }
status-words =
    { $count ->
        [one] { $count } parola
       *[other] { $count } parole
    }
status-tasks = { $checked }/{ $total } attività completate
status-saving-images =
    { $count ->
        [one] Salvataggio di { $count } immagine…
       *[other] Salvataggio di { $count } immagini…
    }
status-mode-wysiwyg = WYSIWYG
status-mode-source = Sorgente Markdown
status-tab-width =
    { $count ->
        [one] Tab: { $count } spazio
       *[other] Tab: { $count } spazi
    }
status-modified = Modificato

## Images

image-saving = Salvataggio immagine…
image-rejected-disabled = Le immagini sono disattivate
image-rejected-type = Tipo di immagine non supportato: { $mime }
image-rejected-too-large = Immagine troppo grande ({ $size } KiB, massimo { $max } KiB)
image-rejected-unreadable = Impossibile leggere l’immagine: { $error }
