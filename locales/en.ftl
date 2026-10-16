# English (reference) translation of the built-in toolbar, link bar and
# status bar. Every other locale must define the same messages.

## Toolbar

toolbar-normal = Normal
# Single letters, as on the bold / italic / strikethrough buttons of word processors.
toolbar-bold = B
toolbar-italic = I
toolbar-strike = S
toolbar-link = Link
toolbar-image = Image
toolbar-table = Table
toolbar-add-row = + Row
toolbar-add-column = + Col
toolbar-delete-row = − Row
toolbar-delete-column = − Col
toolbar-source = Source
# Entry in the code block language list that means "no language".
code-plain-text = plain text

## Link bar

link-address = Link address
link-apply = Apply
link-remove = Remove link
link-cancel = Cancel

## Status bar

status-page = Page { $current } of { $total }
status-words =
    { $count ->
        [one] { $count } word
       *[other] { $count } words
    }
status-tasks = { $checked }/{ $total } tasks done
status-saving-images =
    { $count ->
        [one] Saving { $count } image…
       *[other] Saving { $count } images…
    }
status-mode-wysiwyg = WYSIWYG
status-mode-source = Markdown source
status-tab-width =
    { $count ->
        [one] Tab: { $count } space
       *[other] Tab: { $count } spaces
    }
status-modified = Modified

## Images

# Shown in the document while the application stores a pasted image.
image-saving = Saving image…
image-rejected-disabled = Images are disabled
image-rejected-type = Unsupported image type: { $mime }
image-rejected-too-large = Image too large ({ $size } KiB, maximum { $max } KiB)
image-rejected-unreadable = Cannot read the image: { $error }
