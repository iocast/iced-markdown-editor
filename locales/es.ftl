# Spanish translation. See en.ftl for the reference.

## Toolbar

toolbar-normal = Normal
toolbar-bold = N
toolbar-italic = K
toolbar-strike = T
toolbar-link = Enlace
toolbar-image = Imagen
toolbar-table = Tabla
toolbar-add-row = + Fila
toolbar-add-column = + Col.
toolbar-delete-row = − Fila
toolbar-delete-column = − Col.
toolbar-source = Código
code-plain-text = texto sin formato

## Link bar

link-address = Dirección del enlace
link-apply = Aplicar
link-remove = Quitar enlace
link-cancel = Cancelar

## Status bar

status-page = Página { $current } de { $total }
status-words =
    { $count ->
        [one] { $count } palabra
       *[other] { $count } palabras
    }
status-tasks = { $checked }/{ $total } tareas completadas
status-saving-images =
    { $count ->
        [one] Guardando { $count } imagen…
       *[other] Guardando { $count } imágenes…
    }
status-mode-wysiwyg = WYSIWYG
status-mode-source = Código Markdown
status-tab-width =
    { $count ->
        [one] Tab: { $count } espacio
       *[other] Tab: { $count } espacios
    }
status-modified = Modificado

## Images

image-saving = Guardando imagen…
image-rejected-disabled = Las imágenes están desactivadas
image-rejected-type = Tipo de imagen no compatible: { $mime }
image-rejected-too-large = Imagen demasiado grande ({ $size } KiB, máximo { $max } KiB)
image-rejected-unreadable = No se puede leer la imagen: { $error }
