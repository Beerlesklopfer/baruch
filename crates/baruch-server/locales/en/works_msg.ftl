# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the work management (Round 39a, task messages_1). Base catalog in English; locales/de/works_msg.ftl has the same keys in
# the same order with the German wording.

# Import (POST /werke/import, work.import, work.import_archive).
works_msg.import.use_word_import = Word files are read by the dedicated import with preview: Baruch ▾ → Import Word (.docx)… (/werke/import/word) or RPC docx_import.preview.
works_msg.import.request_too_large = The request is larger than 80 MiB (limit for ZIP and folder).
works_msg.import.not_multipart = The form did not arrive as a file upload ({ $detail }).
works_msg.import.form_unreadable = The form could not be read ({ $detail }).
works_msg.import.file_unreadable = The file could not be read ({ $detail }).
works_msg.import.not_both = Please choose either a file or a folder, not both.
works_msg.import.choose = Please choose a file (.typ, .md, .markdown, .zip) or a folder.
works_msg.import.folder_failed = The folder cannot be imported: { $error }.
works_msg.import.file_failed = “{ $base }” cannot be imported: { $error }.
works_msg.import.wrong_type = “{ $base }” cannot be imported; only .typ can be imported, plus Markdown (.md, .markdown) and ZIP archives (.zip).
works_msg.import.file_too_large = The file is larger than 16 MiB.
works_msg.import.not_utf8 = “{ $base }” is not UTF-8 text.
works_msg.import.choose_other_title = Please enter a different title below and choose the file again.

# Checks of the RPC methods work.*.
works_msg.rpc.kind_invalid = kind: expected “book” or “sermon”, not “{ $other }”
works_msg.rpc.preset_invalid = preset: expected “book” or “sermon”, not “{ $other }”
works_msg.rpc.language_invalid = language: expected “de” or “en”, not “{ $other }”
works_msg.rpc.work_missing = work missing: which work? (work id, a number)
works_msg.rpc.zip_via_archive = A ZIP archive goes through work.import_archive (content as Base64).
works_msg.rpc.not_zip = “{ $base }” is not a ZIP archive; work.import_archive only takes .zip.
works_msg.rpc.confirm_title_missing = confirm_title missing: title of the work as confirmation
works_msg.rpc.slug_missing = slug missing: folder name of the archived work (from work.archives)

# Sermon text of a new sermon (work.create with text_ref).
works_msg.sermon.ref_too_long = text_ref: at most { $max } characters
works_msg.sermon.module_invalid = module: expected a module name of at most { $max } characters on one line
works_msg.sermon.not_a_passage = Sermon text: “{ $text_ref }” is not a Bible reference (e.g. “John 3:16”).
works_msg.sermon.one_passage = Sermon text: please give exactly one Bible reference, not “{ $text_ref }”.
works_msg.sermon.passage_problem = Sermon text “{ $input }”: { $problem }
works_msg.sermon.no_bible = No SWORD Bible set up (Settings → Sources → Bibles); only the reference is there.
works_msg.sermon.module_missing = Bible module { $name } not found in any module folder; only the reference is there.
works_msg.sermon.module_error = Bible module { $name }: { $error }; only the reference is there.
works_msg.sermon.passage_missing =
    The passage is missing in the Bible module { $name } ({ $missing } { $missing ->
        [one] verse
       *[other] verses
    }); only the reference is there.
works_msg.sermon.verses_missing =
    { $missing } { $missing ->
        [one] verse is
       *[other] verses are
    } missing in the Bible module { $name }; the wording is incomplete.
works_msg.sermon.truncated = Only the first { $max } verses inserted.
works_msg.sermon.only_sermon_preset = A sermon text exists only with the skeleton “Sermon”.

# Deleting, archive and restoring.
works_msg.delete.title_mismatch = The title entered does not match; nothing deleted.
works_msg.trash.move_aborted = Moving aborted: { $error }
works_msg.trash.symlink = { $path } is a symbolic link; the mirror stays where it is
works_msg.trash.create_failed = { $path } cannot be created: { $error }
works_msg.trash.move_failed = { $from } cannot be moved to { $to }: { $error }
works_msg.archive.trash_symlink = { $path } is a symbolic link; the archive is not read
works_msg.archive.unreadable = { $path } cannot be read: { $error }
works_msg.archive.parent_symlink = { $path }: folder is a symbolic link; nothing read
works_msg.archive.missing_file = { $path } is missing in the archive folder
works_msg.archive.symlink = { $path } is a symbolic link; nothing read
works_msg.archive.not_a_file = { $path } is not a file
works_msg.archive.too_large = The archive folder is larger than 16 MiB (limit as for the import); nothing restored.
works_msg.archive.not_utf8 = { $path } is not UTF-8 text
works_msg.archive.invalid_folder = invalid archive folder { $folder }
works_msg.archive.missing = { $path } is missing
works_msg.archive.not_a_folder = { $path } is not a folder
works_msg.archive.template_symlink = vorlage.typ is a symbolic link
works_msg.archive.bad_include = main.typ includes { $path }; only abschnitte/{ $lt }name{ $gt }.typ is allowed
works_msg.archive.too_many_sections = more than { $max } section files; nothing restored
works_msg.archive.read_aborted = Reading the archive aborted: { $error }
works_msg.archive.no_folder = No archive folder works/{ $trash }/{ $slug }-… found.
works_msg.archive.no_main = main.typ is missing in the archive folder.
works_msg.archive.live_title = A living work has the same title; rename that one first.
works_msg.restore.not_archived = “{ $slug }” is not in the archive of deleted works; nothing restored.
works_msg.restore.no_folder = No archive folder works/{ $trash }/{ $slug }-… found; nothing restored.
works_msg.restore.exists = works/{ $slug } already exists; nothing restored.
works_msg.restore.move_failed = { $from } cannot be moved to works/{ $slug }: { $error }
works_msg.restore.slug_missing = Which work? The folder name is missing.
works_msg.restore.confirm = Please confirm the restoring with the checkbox; nothing restored.

# Pages of the work management.
works_msg.page.template_error = Error: template { $name }: { $error }
works_msg.page.error = Error: { $error }
works_msg.page.unknown_work = unknown work
works_msg.kind.book = Book
works_msg.kind.sermon = Sermon
works_msg.list.deleted = The work “{ $title }” is deleted; its Git mirror now lies under works/{ $trash }/.
works_msg.act.work_missing = Error: work missing.
works_msg.act.unknown_action = unknown action
