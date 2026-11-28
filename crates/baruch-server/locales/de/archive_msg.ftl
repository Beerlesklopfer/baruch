# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the ZIP reader of the imports (src/archive.rs: EPUB and Markdown ZIP; Round 39c, task reste_1).

# The four kinds of refusal (ArchiveError, Display).
archive_msg.not_zip = kein ZIP-Archiv: { $reason }
archive_msg.unsafe = unsicherer Eintrag „{ $path }“ im Archiv: { $reason }
archive_msg.too_large = Archiv zu groß: { $reason }
archive_msg.corrupt = Archiv beschädigt: { $reason }

# Reasons.
archive_msg.reason.no_pk = der Anfang „PK“ eines ZIP-Archivs fehlt
archive_msg.reason.toc = Inhaltsverzeichnis nicht lesbar ({ $detail })
archive_msg.reason.entries = mehr als { $max } Einträge
archive_msg.reason.directory_outside = Zentralverzeichnis außerhalb der Datei
archive_msg.reason.directory_cut = Zentralverzeichnis abgeschnitten
archive_msg.reason.directory_unreadable = Zentralverzeichnis nicht lesbar
archive_msg.reason.directory_count = Zentralverzeichnis nennt { $listed } Einträge, lesbar sind { $readable }
archive_msg.reason.entry_unreadable = Eintrag { $index } nicht lesbar ({ $detail })
archive_msg.reason.empty_name = leerer Name
archive_msg.reason.control = Steuerzeichen im Namen
archive_msg.reason.absolute = absoluter Pfad
archive_msg.reason.backslash = Backslash im Namen (Pfade trennt „/“)
archive_msg.reason.drive = Laufwerksangabe im Namen
archive_msg.reason.parent = „..“ im Pfad (führte aus dem Archiv heraus)
archive_msg.reason.duplicate = Name kommt doppelt vor
archive_msg.reason.duplicate_case = Name kommt doppelt vor (Groß-/Kleinschreibung zählt nicht)
archive_msg.reason.not_utf8 = Name ist kein UTF-8
archive_msg.reason.symlink = Verknüpfung (Symlink)
archive_msg.reason.special = keine reguläre Datei (Gerät, FIFO oder Socket)
archive_msg.reason.encrypted = verschlüsselt
archive_msg.reason.method = „{ $path }“: Kompression { $method } nicht unterstützt (nur STORED oder DEFLATE)
archive_msg.reason.file_unreadable = „{ $path }“ nicht lesbar ({ $detail })
archive_msg.reason.unpack = „{ $path }“ lässt sich nicht entpacken ({ $detail })
archive_msg.reason.entry_size = „{ $path }“ ist entpackt größer als { $max } Bytes
archive_msg.reason.total_size = die Einträge sind entpackt zusammen größer als { $max } Bytes
archive_msg.reason.ratio = „{ $path }“ wächst beim Entpacken auf mehr als das { $ratio }-fache (Zip-Bombe?)
