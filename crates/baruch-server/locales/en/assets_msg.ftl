# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of images and image requests (Round 39a, task messages_1). Base catalog in English; locales/de/assets_msg.ftl has the same keys in
# the same order with the German wording.

# RPC methods asset.* (assets.rs).
assets_msg.rpc.encoding = encoding: expected “base64” or “svg”, not “{ $encoding }”
assets_msg.rpc.confirm_delete = Please confirm the deletion (confirm).
assets_msg.rpc.no_revision = Image { $name }: revision { $rev } does not exist; present: { $numbers }
assets_msg.rpc.no_current = Image { $name }: no current revision

# Pages „Bilder“.
assets_msg.page.template_error = Error: template { $name }: { $error }
assets_msg.page.error = Error: { $error }
assets_msg.page.unknown_work = unknown work
assets_msg.page.image = Image
assets_msg.page.cover_panel = Cover
assets_msg.page.requests_panel = Requests
assets_msg.page.no_path = Error: no path
assets_msg.page.no_image_for_path = No image for the path “{ $path }”.
assets_msg.notice.added = The image is added.
assets_msg.notice.replaced = New revision uploaded; it is current now.
assets_msg.notice.switched = The chosen revision is current now.
assets_msg.notice.saved = Saved.
assets_msg.notice.unchanged = Nothing changed.
assets_msg.notice.revision_deleted = The revision is deleted; its content is in the Git mirror.
assets_msg.notice.deleted = The image is deleted; its content is in the Git mirror.
assets_msg.notice.cover_set = The image is now the cover of the work.
assets_msg.notice.cover_removed = The work has no cover any more.
assets_msg.notice.request_confirmed = The request is confirmed and carried out.
assets_msg.notice.request_rejected = The request is rejected.
assets_msg.notice.request_stale = The request is stale (the image has changed in the meantime) and was not carried out.
assets_msg.notice.extension_adapted = New revision uploaded; the image is now named with the new extension, the paths stayed.
assets_msg.notice.path_assigned = The path now belongs to the chosen image in all spellings.

# Thumbnails.
assets_msg.thumb.unreadable = Image not readable: { $error }
assets_msg.thumb.write_failed = The thumbnail could not be written: { $error }
assets_msg.thumb.bound = b: give the longest side in pixels between { $min } and { $max }.
assets_msg.thumb.unavailable = Error: thumbnails not available

# Upload and forms.
assets_msg.upload.too_large = The image is larger than { $mib } MiB.
assets_msg.upload.not_multipart = The form did not arrive as a file upload ({ $detail }).
assets_msg.upload.form_unreadable = The form could not be read ({ $detail }).
assets_msg.upload.file_unreadable = The file could not be read ({ $detail }).
assets_msg.upload.work_missing = Work missing.
assets_msg.upload.choose_file = Please choose an image file.
assets_msg.insert.at_end = at the end
assets_msg.insert.at_start = at the start (below the heading)
assets_msg.insert.before = before “{ $excerpt }”
assets_msg.insert.place_fields = Error: give work, section (key) and revision (rev).
assets_msg.insert.rev_invalid = Error: revision invalid.
assets_msg.insert.place_invalid = Error: place invalid (end, start, block:{ $lt }number{ $gt } or offset:{ $lt }position{ $gt }).
assets_msg.insert.choose_image = Error: choose an image.
assets_msg.insert.heading_changed = The heading line is changed – save first, then insert the image.
assets_msg.form.work_missing = Error: work missing.
assets_msg.form.path_and_image = Choose a path and an image.
assets_msg.form.image_missing = Error: image missing.
assets_msg.form.rev_missing = Revision missing.
assets_msg.form.confirm_delete = Please confirm the deletion with the checkbox.
assets_msg.form.unknown_action = unknown action

# Requests about images (asset_requests.rs).
assets_msg.request.decided = This request has already been decided.
assets_msg.request.no_cover = none
assets_msg.request.quoted = “{ $name }”
assets_msg.request.cover_changed = The cover is now { $now } (at the request { $then }).
assets_msg.request.image_gone = The image “{ $name }” no longer exists.
assets_msg.request.not_executable = No longer executable: { $error }
assets_msg.request.other_current = “{ $name }” now has another current revision ({ $number }); the request referred to the state before.
assets_msg.request.revision_gone = Revision { $rev } of “{ $name }” no longer exists.
assets_msg.request.revision_changed = Revision { $rev } of “{ $name }” is now a different one.
assets_msg.request.revision_current = Revision { $rev } of “{ $name }” is now the current one and cannot be deleted.
assets_msg.request.id_number = id: number expected
assets_msg.request.id_missing = id missing (number of the request)
assets_msg.request.decision = decision: confirm or reject expected
assets_msg.request.open_flag = open: true or false expected
assets_msg.request.work_missing = work missing (work id)
assets_msg.request.no_content = There is no request with content
assets_msg.request.purged = The proposed content of request { $nr } is removed: the request was decided on { $decided }, and Baruch keeps the content of decided requests for { $days } days.
