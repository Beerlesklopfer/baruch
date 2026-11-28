# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the cover (Round 39a, task messages_1). Base catalog in English; locales/de/cover_msg.ftl has the same keys in
# the same order with the German wording.

# Measuring the cover from the typeset book (cover.rs).
cover_msg.measure.typeset_errors =
    The typeset book has { $errors } { $errors ->
        [one] error
       *[other] errors
    }; without typeset pages there is no page size for the cover (messages under /werke/{ $work }/satz).
cover_msg.measure.page_size = Page size of the first book page not readable
cover_msg.measure.block_heights = Block heights: { $error }
cover_msg.measure.timeout = The cover blocks cannot be typeset: Typst did not finish after { $seconds } seconds (time limit).
cover_msg.measure.blocks_failed = The cover blocks cannot be typeset: { $message }
cover_msg.measure.page_format = the page format of the work template could not be typeset (typesetting error in the template)

# Checks of work.meta.set and cover.* (RPC and forms).
cover_msg.param.text = { $name }: text expected
cover_msg.param.switch = { $name }: on/off (true/false) expected
cover_msg.param.mm = { $label }: number in mm expected (e.g. 0.1).
cover_msg.param.paper = Paper thickness
cover_msg.param.paper_sheet = Paper thickness per sheet
cover_msg.param.paper_both = Do not give paper_mm and paper_sheet_mm together (paper_sheet_mm = thickness per sheet = 2 × paper_mm).
cover_msg.param.paper_sheet_range = Paper thickness per sheet: more than 0 and at most 2 mm (1 sheet = 2 pages).
cover_msg.param.author_photo = author_photo: image name or null expected
cover_msg.param.title_area = title_area: expected “top”, “middle” or “bottom”, not “{ $area }”
cover_msg.param.bleed = Bleed
cover_msg.param.name = name: image name or null expected
cover_msg.param.name_missing = name missing: image name of the cover, or null to remove it
cover_msg.imprint.not_adopted = Imprint template not adopted: { $why }
cover_msg.imprint.complete = Imprint template not adopted: the work template already has everything.

# Cover page.
cover_msg.page.template_error = Template { $name }: { $error }
cover_msg.page.error = Error: { $error }
cover_msg.page.unknown_work = unknown work
cover_msg.page.not_typeset = The cover could not be typeset.
cover_msg.page.no_pdf = No cover PDF: { $error }
cover_msg.notice.cover_set = The cover is set.
cover_msg.notice.cover_removed = The cover is removed.
cover_msg.notice.book_details = The book details are saved.
cover_msg.notice.layout = The cover layout is saved.
cover_msg.notice.imprint = The imprint template is adopted into the work template (new revision of the template).

# Fields of a cover block and their checks.
cover_msg.block_field.width = Width
cover_msg.block_field.align = Alignment
cover_msg.block_field.size = Font size
cover_msg.block_field.leading = Line spacing
cover_msg.block_field.color = Colour
cover_msg.block_field.font = Font
cover_msg.block_field.style = Style
cover_msg.block_field.backdrop = Background
cover_msg.block_field.visible = Visible
cover_msg.block_field.direction = Direction
cover_msg.block_field.text = Text
cover_msg.block_field.other = Field
cover_msg.block.bounds = { $low } and { $high }
cover_msg.block.number_range = number between { $bounds } { $unit } expected, not “{ $raw }”.
cover_msg.block.align = expected “left”, “center”, “right” or “justify”, not “{ $raw }”.
cover_msg.block.style = expected “normal”, “bold” or “italic”, not “{ $raw }”.
cover_msg.block.backdrop = expected “none”, “shadow” or “gradient”, not “{ $raw }”.
cover_msg.block.color = expected “light”, “dark” or #rrggbb, not “{ $raw }”.
cover_msg.block.font = font name of letters, digits, spaces and - _ . (at most { $max } characters) expected.
cover_msg.block.switch = on/off (1/0) expected, not “{ $raw }”.
cover_msg.block.direction = expected “down” (from top to bottom) or “up”, not “{ $raw }”.
cover_msg.block.spine_only = exists only on the spine.
cover_msg.block.text_too_long = at most { $max } characters.
cover_msg.block.text_from_work = This block takes its text from the work and the book details.
cover_msg.block.unknown_field = { $block }: unknown field “{ $field }”.
cover_msg.block.unknown_kind = Unknown block kind “{ $kind }” in the field “{ $name }”.
cover_msg.block.no_base = { $block }: no base block.

# Guides and labels of the cover editor.
cover_msg.guide.bleed = Bleed (outer edge)
cover_msg.guide.trim = Trim edge
cover_msg.guide.safety = Safety margin { $mm } mm ({ $face })
cover_msg.guide.spine_left = Spine edge left
cover_msg.guide.spine_right = Spine edge right
cover_msg.guide.middle_back = Middle of the back
cover_msg.guide.middle_front = Middle of the front
cover_msg.guide.middle_height = Middle of the height
cover_msg.editor.block_aria = { $label }: x { $x } mm, y { $y } mm, width { $width } mm
cover_msg.editor.block_aria_hidden = { $label }: x { $x } mm, y { $y } mm, width { $width } mm, hidden

# Forms of the cover.
cover_msg.form.work_missing = Error: work missing.
cover_msg.form.choose_image = Please choose an image for the cover.
cover_msg.form.unknown_action = unknown action
cover_msg.form.no_blocks = No blocks in the form (fields b.{ $lt }kind{ $gt }.{ $lt }field{ $gt }).
cover_msg.form.quick_area = Quick choice: expected “top”, “middle” or “bottom”, not “{ $area }”.
cover_msg.form.faces = Faces: expected “front”, “spine” or “back”, not “{ $word }”.

# Cover layout (cover_layout.rs).
cover_msg.layout.not_object = Cover block no. { $number }: an object with kind, x_mm, … expected
cover_msg.layout.unknown_kind = Cover block no. { $number }, field kind: unknown kind “{ $word }” (allowed: { $allowed })
cover_msg.layout.kind_text = Cover block no. { $number }, field kind: kind as text expected ({ $allowed })
cover_msg.layout.block_label = Cover block “{ $label }” ({ $kind })
cover_msg.layout.field = { $label }, field { $field }: { $what }
cover_msg.layout.unknown_field = unknown field
cover_msg.layout.missing = missing
cover_msg.layout.number = number expected
cover_msg.layout.allowed = “{ $value }” – allowed: { $allowed }
cover_msg.layout.text_allowed = text expected ({ $allowed })
cover_msg.layout.text_or_null = text or null expected
cover_msg.layout.color = text expected (light, dark or #rrggbb)
cover_msg.layout.flag = true or false expected
cover_msg.layout.direction = down, up or null expected
cover_msg.layout.blocks_list = blocks: list of blocks expected (each {"{"}kind, x_mm, y_mm, width_mm, …{"}"})
cover_msg.layout.stale = The cover layout has been changed in the meantime; the request referred to the state before.
cover_msg.layout.not_executable = No longer executable: { $error }
cover_msg.layout.fonts = Fonts: { $error }
cover_msg.layout.not_layout_request = This request does not concern the cover layout.
cover_msg.layout.faces = faces: list of “front”, “spine”, “back” expected
cover_msg.layout.area_not = area: expected “top”, “middle” or “bottom”, not “{ $word }”
cover_msg.layout.area = area: “top”, “middle” or “bottom” expected

# Print check of the cover (cover_print.rs).
cover_msg.print.draft = The cover PDF carries the watermark “Draft” – do not give it to the printer like this (without watermark: cover PDF with ?entwurf=0, or switch off “Draft also on the cover”).
cover_msg.print.font_missing = { $block }: font “{ $font }” is missing; typeset with the default “{ $default }”.
cover_msg.print.spine = { $who } falls out of the spine: { $w } thick from x { $x }, the spine is { $fw } wide.
cover_msg.print.bleed_spine = { $who } reaches into the bleed at the top or bottom (y { $y } to { $y2 }).
cover_msg.print.safety_spine = { $who } lies closer than { $safety } to the trim edge at the top or bottom.
cover_msg.print.bleed = { $who } reaches over the trim edge or the fold into the bleed (x { $x } to { $x2 }, y { $y } to { $y2 }; face { $fw } × { $fh }).
cover_msg.print.safety = { $who } lies closer than { $safety } to the trim edge or the fold.
cover_msg.print.barcode = { $who } covers the barcode.
cover_msg.print.contrast = { $who }: contrast { $ratio } : 1 to the background is too low (at least { $min } : 1); choose a lighter or darker font, a shadow or a gradient.
cover_msg.print.imprint_publisher = Imprint: publisher or editor or their address is missing (when self-publishing, name and address of the author or editor).
cover_msg.print.imprint_printer = Imprint: printer or its address is missing.
cover_msg.print.imprint_year = Imprint: year of publication is missing.
cover_msg.print.imprint_bible_notice = Imprint: a Bible translation is set, but the rights notice of the translation is missing.
