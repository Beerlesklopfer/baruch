# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Images of a work, one image, open image requests.
# Base catalog in English; locales/de/assets.ftl has the same keys in the same order.

# Shared by the image pages.
assets.back_to_work = Back to the work
assets.name = Name
assets.kind = Type
assets.size = Size
assets.paths = Paths
assets.no_path = none
assets.alt = Alternative text
assets.caption = Caption
assets.note = Note
assets.cover = Cover
assets.as_cover = As cover
assets.upload = Upload

# Page /werke/{id}/bilder (assets.html).
assets.list.window_title = Images – { $title } – Baruch
assets.list.page_title = { $title }: images
assets.list.heading = Images
assets.list.conflicts = Paths with two images
assets.list.conflicts_hint_html = These paths are equal when a leading <code>/</code> and <code>./</code> are left out, but belong to different images (legacy data). Typesetting, typeset view and export then take a different image depending on the spelling. Please choose which image keeps the path; the text stays unchanged.
assets.list.path_belongs_to = Path belongs from now on to
assets.list.assign = Assign
assets.list.search = Search
assets.list.search_placeholder = Name, caption or alternative text
assets.list.search_label = Search images
assets.list.found = { $count } of { $total } images for “{ $query }”
assets.list.show_all = show all
assets.list.col_preview = Preview
assets.list.col_revision = Revision
assets.list.of_revisions = of { $count }
assets.list.no_match = No image matches “{ $query }”.
assets.list.show_all_images = Show all images
assets.list.empty_html = No images yet. Images come with the import from the command line (<code>baruch-server import buch.typ</code> reads the files that the text names with <code>image("…")</code>) or through the form below.
assets.list.add = Add image
assets.list.file = File (PNG, JPEG, GIF, WebP or SVG, at most { $mib } MiB)
assets.list.name_field = Name (empty = from the file name; an existing name uploads a new revision)
assets.list.path_field = Path in template and text (one per line)
assets.list.as_base64 = Store SVG as Base64 (otherwise as text; other types always as Base64)
assets.list.path_hint_html = The path is the spelling in <code>#image("…")</code> or <code>#figure(image("…"))</code>; export and typeset view find the image through it. In the Git mirror every current image lies under <code>bilder/{ $name }</code> (SVG) or <code>bilder/{ $name }.b64</code>.

# Page /werke/{id}/bilder/{name} (asset.html).
assets.one.window_title = { $name } – Images – Baruch
assets.one.page_title = { $title }: image { $name }
assets.one.all_images = All images
assets.one.heading = { $name } · revision { $number }
assets.one.current_mark = (current)
assets.one.stored_as = stored as
assets.one.svg_text = SVG text
assets.one.by = By
assets.one.changed = changed { $date }
assets.one.file = File
assets.one.open = open
assets.one.revisions = Revisions
assets.one.revision = Revision { $number }
assets.one.current = current
assets.one.based_on = based on revision { $number }
assets.one.make_current = Make current
assets.one.describe = Describe revision { $number }
assets.one.paths_field = Paths of the image in template and text (one per line)
assets.one.replace = Replace (new revision)
assets.one.new_file = New file ({ $kind }, at most { $mib } MiB; another format only with “Adjust extension”)
assets.one.as_base64 = Store SVG as Base64
assets.one.rename = Adjust extension when the file has another format (e.g. PNG instead of SVG: the image is then named with the new extension, all paths stay, the text stays unchanged)
assets.one.replace_hint = The new revision becomes current at once; the previous ones stay and can be switched back.
assets.one.delete_revision_heading = Delete revision { $number }
assets.one.delete_revision_confirm = Really delete revision { $number }
assets.one.delete_revision = Delete revision
assets.one.delete_image = Delete image
assets.one.delete_image_hint = Deletes { $name } with all { $count } revisions and paths. The content stays readable in the Git mirror.
assets.one.delete_image_confirm = Really delete { $name }

# Requests of the assistants about images (fragments/asset_requests.html).
assets.requests.heading = Requests of the assistants
assets.requests.open_count = ({ $count } open)
assets.requests.intro = Assistants create images at once; replacing, setting the cover, deleting and a new cover arrangement they only request. Confirming carries out the request (one commit in the Git mirror, you as the confirmer, the assistant as the origin). If the image, the cover or the arrangement has changed in the meantime, the request is not carried out but closed as outdated. Rejecting changes nothing.
assets.requests.col_number = No.
assets.requests.col_request = Request
assets.requests.col_from = From
assets.requests.col_reason = Reason
assets.requests.view_cover = View cover
assets.requests.view_image = View image
assets.requests.view_content = View the proposed content
assets.requests.content_alt = Proposed content for { $name }
assets.requests.caption = Caption: { $caption }
assets.requests.alt = Alternative text: { $alt }
assets.requests.confirm = Confirm
assets.requests.reject = Reject
assets.requests.none = No open requests about images.
assets.requests.decided = Recently decided
assets.requests.request = Request { $id }:
