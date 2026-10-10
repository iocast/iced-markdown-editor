# CommonMark

CommonMark 0.31.2 plus GFM strikethrough and task lists. Paragraphs, headings,
lists (nested, tight/loose, any start), task lists, block quotes, code blocks
with language, HTML blocks, thematic breaks, emphasis, strong, strikethrough,
code spans, links with titles, autolinks, images, raw inline HTML.

`cargo test --all-features`: 632 of the 652 spec examples produce the same
HTML after a round trip. The other 20 look the same when rendered:

- 19 with emphasis inside the same emphasis (`*(*foo*)*`), written as one level;
- example 168 (`<del>*foo*</del>`): the nesting order of different formats on
  the same text changes (`*~~foo~~*`).

Not preserved: link reference definitions (written inline), original
markers (`+`, `__bold__`, indented code are normalised), hard breaks at the
start or end of a paragraph, task items without text.
