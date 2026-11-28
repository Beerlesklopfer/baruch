# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Cover editor page and its panel.
# Base catalog in English; locales/de/cover.ftl has the same keys in the same order.

# Shared by the cover page and its panel.
cover.back_to_work = Back to the work
cover.image_alt = Cover: back, spine and front
cover.area.top = top
cover.area.middle = middle
cover.area.bottom = bottom

# Page /werke/{id}/umschlag (cover.html).
cover.page.window_title = Cover – { $title } – Baruch
cover.page.page_title = { $title }: cover
cover.page.download = Download
cover.page.pdf = Cover PDF
cover.page.heading = Cover
cover.page.facts =
    Total { $width } × { $height } mm · page { $page_width } × { $page_height } mm · spine { $spine } mm = { $book_pages } { $book_pages ->
        [one] book page ({ $sheets } sheets)
        [2] book pages ({ $sheets } sheet)
       *[other] book pages ({ $sheets } sheets)
    } × { $sheet_mm } mm per sheet
cover.page.facts_cover_pages = (the { $count } cover pages in the typesetting do not count)
cover.page.facts_bleed = · bleed { $bleed } mm
cover.page.caption = On the left the back, in the middle the spine, on the right the front; outside the bleed.
cover.page.print_check = Print check
cover.page.line = , line { $line }
cover.page.no_messages = No messages when typesetting the cover.

# Cover editor (cover.html, section "anordnung").
cover.editor.heading = Arrangement of the blocks
cover.editor.preview_notice = Preview – not saved yet. “Apply arrangement” saves these values.
cover.editor.help = Coordinates in mm from the trim edge (top left) of the own face; negative values reach into the bleed.
cover.editor.help_js = Dragging a frame moves it, the handles at the corners change the width. Keyboard: choose a frame with Tab, arrow keys 0.5 mm (with Shift 5 mm), Ctrl+←/→ width, Ctrl+Z undo, Ctrl+S save. While dragging, the frame snaps to the guide lines.
cover.editor.toolbar = Cover editor
cover.editor.undo = Undo
cover.editor.tone_hint = Switch the colour of the chosen block between light and dark
cover.editor.tone = Light/Dark
cover.editor.preview_mark = (preview)
cover.editor.block_role = movable block
cover.editor.own_layout = The work has an arrangement of its own.
cover.editor.defaults_before = No arrangement of its own yet: the defaults apply (title area
cover.editor.defaults_after = ).
cover.editor.default_font = Default font: { $font }.
cover.editor.default_mark = (default)
cover.editor.hidden_mark = – hidden
cover.editor.width = Width (mm)
cover.editor.size = Font size (pt)
cover.editor.leading = Line spacing (em)
cover.editor.align = Alignment
cover.editor.align_left = left
cover.editor.align_right = right
cover.editor.align_justify = justified
cover.editor.color = Colour
cover.editor.color_placeholder = light, dark or #rrggbb
cover.editor.font = Font
cover.editor.style = Style
cover.editor.style_normal = normal
cover.editor.style_bold = bold
cover.editor.style_italic = italic
cover.editor.backdrop = Background
cover.editor.backdrop_none = none
cover.editor.backdrop_shadow = Shadow
cover.editor.backdrop_gradient = Gradient
cover.editor.visible = Visible
cover.editor.yes = yes
cover.editor.no = no
cover.editor.direction = Direction
cover.editor.direction_down = from top to bottom
cover.editor.direction_up = from bottom to top
cover.editor.text = Text
cover.editor.apply = Apply arrangement
cover.editor.preview_without_saving = Preview without saving
cover.editor.quick = Quick choice of the title area (resets the front)
cover.editor.reset_legend = Reset the arrangement (no face chosen = all)
cover.editor.reset = Reset arrangement
cover.editor.unavailable = No editor without a set cover: { $reason }

# Group "Umschlag" (fragments/cover_panel.html).
cover.panel.image_html = Cover image: <strong>{ $name }</strong>
cover.panel.no_image = No cover image yet. Choose one from the images of the work.
cover.panel.image_for_cover = Image for the cover
cover.panel.set_cover = Set as cover
cover.panel.no_images = The work has no images yet:
cover.panel.upload_images = Upload images …
cover.panel.remove = Remove cover
cover.panel.area_legend = Title and subtitle on the front
cover.panel.apply_area = Apply title area
cover.panel.view = View the cover (back, spine, front) …
cover.panel.imprint_adopt = Adopt the imprint template into the work template
cover.panel.imprint_hint = Appends the functions schmutztitel, haupttitel, impressum and widmung that the work template still lacks as a new revision of the template; afterwards front matter and imprint can be designed freely in the work template.
cover.panel.imprint_complete = The work template already has all four.
cover.panel.imprint_present = Already present: { $present }.
cover.panel.imprint_none = The work template has none of them yet.

# Form "Buchangaben" (fragments/cover_panel.html).
cover.book.heading = Book details
cover.book.intro = For front matter, imprint and cover. Title, subtitle, author and Bible translation are part of the details of the work.
cover.book.publisher = Publisher
cover.book.place = Place of publication
cover.book.year = Year
cover.book.edition = Edition
cover.book.edition_placeholder = 1st edition
cover.book.price = Price
cover.book.series = Series
cover.book.printer = Printing
cover.book.author_photo = Author photo
cover.book.none = – none –
cover.book.paper = Paper thickness per sheet in mm (1 sheet = 2 pages)
cover.book.paper_hint = Determines the spine width. Printers name the thickness per sheet. Default 0.2 mm per sheet (rather thick); usual book paper around 80–90 g/m² has about 0.1 mm per sheet.
cover.book.bleed = Bleed (mm)
cover.book.title_area = Title area
cover.book.front_matter = Set front matter (half title, page 2, main title, imprint, dedication)
cover.book.cover_pages = Show the cover front and back in the typesetting (only with a cover image)
cover.book.dedication = Dedication
cover.book.blurb = Blurb (back)
cover.book.bio = Short biography
cover.book.publisher_address = Address of the publisher or editor
cover.book.publisher_address_hint = (for self-publishing that of the author or editor; multi-line)
cover.book.printer_address = Address of the printer
cover.book.multiline = (multi-line)
cover.book.bible_notice = Rights notice of the Bible translation
cover.book.image_credit = Picture credits / cover design
cover.book.image_credit_hint = (empty: caption of the cover)
cover.book.save = Save book details
