# SPDX-License-Identifier: AGPL-3.0-or-later
# System pages (about, shut down, sync, consent) (Round 39a, templates_3). Base catalog in English; locales/de/system.ftl has the same keys in the same order.

# templates/consent.html
system.consent.needed_html = <strong>Consent needed:</strong>
system.consent.one = { $origin_label } wants to rearrange the outline: { $summary }.
system.consent.many = { $requests } requests of the assistants are waiting for your consent.
system.consent.nothing_without = Nothing changes without your consent.
system.consent.open = View and decide
system.consent.title = Consent to the outline
system.consent.request_html = <strong>{ $origin_label }</strong> wants to do in the work “{ $work_title }”: { $summary }.
system.consent.note = Reason: { $note }
system.consent.preview = Outline after your consent:
system.consent.no_preview = No preview: { $preview_error }
system.consent.before_after = (before { $before_label } → after { $label })
system.consent.added = new
system.consent.adopted = moves under the new part
system.consent.confirm = Consent
system.consent.reject = Reject
system.consent.later = Decide later

# templates/about.html
system.about.window_title = About Baruch – Baruch
system.about.title = About Baruch
system.about.back_group = Back
system.about.back_to_work = Back to the work
system.about.intro = Baruch is a free writing workshop for books and sermons. It runs on your own computer, the data stays there.
system.about.license = Licence
system.about.license_text = – GNU Affero General Public License, version 3 or (at your option) any later version. Baruch comes without any warranty.
system.about.source = Source code
system.about.source_text_html = – the source code of exactly this version is there under the tag <code>v{ $version }</code>.
system.about.website = Website
system.about.help = Help with pictures:
system.about.help_online = (on the website, needs the internet)
system.about.data_dir = Data folder
system.about.data_default_html = (default: <code>{ $default_data_dir }</code>)
system.about.data_help = All data lives in this folder of the user, never in the installation directory. No package creates anything there, and removing Baruch deletes nothing there. Size, mirrors of the works and backup:
system.about.settings_data = Settings → Data
system.about.updates = New versions
system.about.updates_text = Baruch never asks the network for new versions on its own. Whoever wants to check opens the
system.about.release_page = release page of this version
system.about.updates_more_html = (<code>{ $release_url }</code>) themselves – only the click opens it in the browser. Updating is done by hand with a new package from there; only those who get Baruch on Debian or Ubuntu from the APT repository receive new versions with <code>sudo apt upgrade</code>.
system.about.third_party = Third-party libraries
system.about.third_party_text_html = The program contains third-party Rust libraries. Names, versions, licences and licence texts are in the file <code>{ $third_party_file }</code> that every package brings along:
system.about.third_party_here_html = On this computer: <code>{ $third_party }</code>
system.about.appimage_html = – in the mounted AppImage <code>{ $appimage }</code>; the path is valid only while Baruch runs.
system.about.third_party_missing_html = The file is not next to this program (built from source or copied to another place). In the source tree <code>scripts/third-party-licenses.py --out { $third_party_file }</code> creates it.

# templates/shutdown.html
system.shutdown.window_title = Quit Baruch – Baruch
system.shutdown.title = Quit Baruch
system.shutdown.back_group = Back
system.shutdown.back_to_work = Back to the work
system.shutdown.question = Should Baruch be quit on this computer?
system.shutdown.help_html = Saved texts are already in the database and in the Git mirror; nothing is lost. Save unsaved input in other windows first. Running answers still finish (at most ten seconds), an open live chat is disconnected afterwards. Afterwards Baruch is no longer reachable until it is started again (start menu, programs, <code>baruch</code> in the terminal or the configured service).
system.shutdown.submit = Quit Baruch
system.shutdown.local_only = Baruch can only be quit on the own computer.
system.shutdown.remote = This request came from another computer or through the name of an upstream server (reverse proxy). From there Baruch does not quit – otherwise anyone who reaches the page could switch off the writing workshop.
system.shutdown.how_html = Quitting works on the computer where Baruch runs: open <code>http://127.0.0.1:8765/beenden</code> in the browser there (or the address Baruch listens on there), or stop the service (<code>systemctl --user stop baruch</code>).

# templates/shutdown_done.html
system.shutdown_done.window_title = Baruch has quit
system.shutdown_done.text = Saved texts are in the database and in the Git mirror. To keep working, start Baruch again.

# templates/sync.html
system.sync.window_title = Sync with MultiGPT – Baruch
system.sync.title = Sync with MultiGPT
system.sync.back_group = Back
system.sync.back_to_work = Back to the work
system.sync.intro = Baruch uploads the current versions of the works with the switch “upload to MultiGPT” (one document per chapter) and fetches the chosen collections as a local copy that can also be searched offline. Only this button starts the sync; errors appear here as a note, nothing is lost locally.
system.sync.unconfigured = MultiGPT is not set up yet:
system.sync.settings_connections = Settings → Connections
system.sync.run_title = Sync
system.sync.run = Sync
system.sync.running = The sync is running right now …
system.sync.last = last synced
system.sync.unfinished = (started { $started }, not finished)
system.sync.ok_html = – <strong>without errors</strong>
system.sync.hints_html = – <strong>with notes</strong>
system.sync.uploaded = uploaded:
system.sync.replaced = replaced:
system.sync.removed = removed:
system.sync.downloaded = fetched:
system.sync.unchanged = unchanged:
system.sync.failed = Errors:
system.sync.never = Never synced.
system.sync.works_title = Upload works to MultiGPT
system.sync.works_help = Default: off. When switched on, the next sync puts one document per chapter with the current versions into the collection “{ $collection_pattern }”; changed chapters are replaced.
system.sync.work_title_placeholder = work title
system.sync.upload_on = – upload on, collection “{ $collection }”, { $documents } chapters
system.sync.last_synced = , last { $synced }
system.sync.switch_off = Switch upload off
system.sync.upload_off = – upload off
system.sync.still_there = ({ $documents } chapters are still in “{ $collection }”)
system.sync.confirm = Yes, the current versions of this work may go to MultiGPT
system.sync.switch_on = Switch upload on
system.sync.no_works = There is no work yet.
system.sync.collections_title = Fetch collections from MultiGPT
system.sync.chosen = Chosen collections (local copy, searchable offline)
system.sync.documents =
    { $count ->
        [one] { $count } document
       *[other] { $count } documents
    }
system.sync.documents_at_multigpt =
    { $count ->
        [one] { $count } document
       *[other] { $count } documents
    } at MultiGPT
system.sync.no_collection = No collection chosen.
system.sync.more = Another collection
system.sync.collections_apply = Take over collections
system.sync.collections_load = Load collections from MultiGPT
