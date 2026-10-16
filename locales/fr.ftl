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

## Link bar

link-address = Adresse du lien
link-apply = Appliquer
link-remove = Supprimer le lien
link-cancel = Annuler

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
