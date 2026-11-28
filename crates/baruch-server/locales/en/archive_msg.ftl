# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the ZIP reader of the imports (src/archive.rs: EPUB and Markdown ZIP; Round 39c, task reste_1).

# The four kinds of refusal (ArchiveError, Display).
archive_msg.not_zip = not a ZIP archive: { $reason }
archive_msg.unsafe = unsafe entry “{ $path }” in the archive: { $reason }
archive_msg.too_large = archive too large: { $reason }
archive_msg.corrupt = archive damaged: { $reason }

# Reasons.
archive_msg.reason.no_pk = the beginning “PK” of a ZIP archive is missing
archive_msg.reason.toc = table of contents not readable ({ $detail })
archive_msg.reason.entries = more than { $max } entries
archive_msg.reason.directory_outside = central directory outside the file
archive_msg.reason.directory_cut = central directory cut off
archive_msg.reason.directory_unreadable = central directory not readable
archive_msg.reason.directory_count =
    central directory names { $listed ->
        [one] { $listed } entry
       *[other] { $listed } entries
    }, readable { $readable ->
        [one] is { $readable }
       *[other] are { $readable }
    }
archive_msg.reason.entry_unreadable = entry { $index } not readable ({ $detail })
archive_msg.reason.empty_name = empty name
archive_msg.reason.control = control character in the name
archive_msg.reason.absolute = absolute path
archive_msg.reason.backslash = backslash in the name (paths are separated by “/”)
archive_msg.reason.drive = drive letter in the name
archive_msg.reason.parent = “..” in the path (would lead out of the archive)
archive_msg.reason.duplicate = name occurs twice
archive_msg.reason.duplicate_case = name occurs twice (upper/lower case does not count)
archive_msg.reason.not_utf8 = name is not UTF-8
archive_msg.reason.symlink = link (symlink)
archive_msg.reason.special = not a regular file (device, FIFO or socket)
archive_msg.reason.encrypted = encrypted
archive_msg.reason.method = “{ $path }”: compression { $method } not supported (only STORED or DEFLATE)
archive_msg.reason.file_unreadable = “{ $path }” not readable ({ $detail })
archive_msg.reason.unpack = “{ $path }” cannot be unpacked ({ $detail })
archive_msg.reason.entry_size = “{ $path }” is larger than { $max } bytes unpacked
archive_msg.reason.total_size = the entries are larger than { $max } bytes together unpacked
archive_msg.reason.ratio = “{ $path }” grows to more than { $ratio } times its size when unpacked (zip bomb?)
