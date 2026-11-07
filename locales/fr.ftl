# French translation. See en.ftl for the reference.

## Toolbar

toolbar-normal = Normal
toolbar-bold = G
toolbar-italic = I
toolbar-strike = B
toolbar-link = Lien
toolbar-image = Image
toolbar-table = Tableau
toolbar-add-row = + Ligne
toolbar-add-column = + Col.
toolbar-delete-row = − Ligne
toolbar-delete-column = − Col.
toolbar-source = Source
code-plain-text = texte brut

## Context menu

menu-cut = Couper
menu-copy = Copier
menu-paste = Coller
menu-select-all = Tout sélectionner
menu-bold = Gras
menu-italic = Italique
menu-strike = Barré
menu-code = Code
menu-link = Lien…
menu-row-above = Insérer une ligne au-dessus
menu-row-below = Insérer une ligne en dessous
menu-column-left = Insérer une colonne à gauche
menu-column-right = Insérer une colonne à droite
menu-delete-row = Supprimer la ligne
menu-delete-column = Supprimer la colonne
menu-delete-table = Supprimer le tableau
menu-align-left = Aligner à gauche
menu-align-center = Centrer
menu-align-right = Aligner à droite

## Link bar

link-address = Adresse du lien
link-apply = Appliquer
link-remove = Supprimer le lien
link-cancel = Annuler
# Link dialog (opened from the context menu).
link-dialog-insert = Insérer un lien
link-dialog-edit = Modifier le lien
link-text = Texte à afficher
link-ok = OK

## Status bar

status-page = Page { $current } sur { $total }
status-words =
    { $count ->
        [one] { $count } mot
       *[other] { $count } mots
    }
status-tasks = { $checked }/{ $total } tâches terminées
status-saving-images =
    { $count ->
        [one] Enregistrement de { $count } image…
       *[other] Enregistrement de { $count } images…
    }
status-mode-wysiwyg = WYSIWYG
status-mode-source = Source Markdown
status-tab-width =
    { $count ->
        [one] Tab : { $count } espace
       *[other] Tab : { $count } espaces
    }
status-modified = Modifié

## Images

image-saving = Enregistrement de l’image…
image-rejected-disabled = Les images sont désactivées
image-rejected-type = Type d’image non pris en charge : { $mime }
image-rejected-too-large = Image trop volumineuse ({ $size } Kio, maximum { $max } Kio)
image-rejected-unreadable = Impossible de lire l’image : { $error }
