// SPDX-License-Identifier: AGPL-3.0-or-later
// Einstieg: Widgets registrieren und in die vom Server gelieferte Seite einhängen.

import "./assist.js";
import "./band.js";
import "./bibles.js";
import "./chat.js";
import "./commentaries.js";
import "./consent.js";
import "./context_menu.js";
import "./cover_editor.js";
import "./epub_preview.js";
import "./findings.js";
import "./glossary.js";
import "./idea_map.js";
import "./ideas.js";
import "./jump.js";
import "./laptop.js";
import "./library.js";
import "./literature.js";
import "./mention.js";
import "./menu.js";
import "./outline.js";
import "./panes.js";
import "./persona_tabs.js";
import "./proposals.js";
import "./ribbon.js";
import "./scripture_check.js";
import "./search.js";
import "./search_keys.js";
import "./settings.js";
import "./sheet.js";
import "./threads.js";
import "./timing.js";
import "./toast.js";
import "./typeset_settings.js";
import "./variants.js";
import "./workspace.js";
import "./works.js";
import "./wysiwyg.js";
import "./xref.js";
import "./zoom.js";
import { mount } from "./widgets.js";

document.documentElement.classList.add("js");
mount(document);
