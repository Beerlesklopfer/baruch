// SPDX-License-Identifier: AGPL-3.0-or-later
//! Modell des Austauschformats (Vertrag R26, S1 (4); nach S1 eingefroren, Änderungen nur über den Integrator –
//! Runde 25 hat es nach Vertrag §4.6 um die Gedankenkarte erweitert: [`ExchangeMap`] mit Verbindungen und Lagen,
//! Grabsteinarten `verbindung` und `lage`, Unterversion 2; Runde 27 (Aufgabe umschlag-daten, Ausnahme vom
//! Integrator-Vorbehalt) um die Umschlag-Anordnung [`ExchangeCoverBlock`], die Anschriften im Impressum, die
//! Satzbild-Einstellungen [`ExchangeSheet`] und die Grabsteinart `umschlag-block`, Unterversion 3; Runde 28 (Aufgabe
//! befunde, Ausnahme vom Integrator-Vorbehalt) um die Befunde [`ExchangeFinding`] mit Statusverlauf, die alten
//! Fassungen bearbeiteter Beiträge [`ExchangePostVersion`] und die Grabsteinarten `befund`, `befund-status`,
//! `beitrag-fassung`, Unterversion 4; Runde 30 (Aufgabe chat-loesungen, Ausnahme vom Integrator-Vorbehalt) um die
//! Lösungen der Befunde [`ExchangeSolution`], die Varianten der Fassungen [`ExchangeVariant`] und die Metadaten der
//! Beiträge [`ExchangePostMeta`] (Prüfgrundlage, Antwort auf, Wiederholung, Relay), Unterversion 5; Runde 29 (Aufgabe
//! satz-daten, Ausnahme vom Integrator-Vorbehalt) um die Satz-Einstellungen [`ExchangeTypeset`] und die Grabsteinart
//! `satz`, Unterversion 6; Runde 31 (Aufgabe glossar-daten, Ausnahme vom Integrator-Vorbehalt) um die eigenen Register
//! [`ExchangeRegister`], das Glossar [`ExchangeGlossaryTerm`], die Register/Glossar-Felder der Satz-Einstellungen und
//! die Grabsteinarten `glossar-begriff` und `register`, Unterversion 7).
//!
//! Alle Verweise laufen über UUIDs (kanonisch klein, 8-4-4-4-12); Zahlen-ids der Datenbank kommen im Format
//! nicht vor. Zeiten stehen so, wie sie gespeichert sind (`JJJJ-MM-TTTHH:MM:SS.sssZ`). Die XML-Wörter der
//! Aufzählungen liefern `as_xml`/`from_xml`; das Vokabular steht in `docs/schema/baruch-werk-1-u7.xsd`
//! (Unterversion 8; `baruch-werk-1-u7.xsd`, `baruch-werk-1-u6.xsd`, `baruch-werk-1-u5.xsd`, `baruch-werk-1-u4.xsd`, `baruch-werk-1-u3.xsd`,
//! `baruch-werk-1-u2.xsd` und `baruch-werk-1.xsd` bleiben eingefroren für die Unterversionen 7, 6, 5, 4, 3, 2 und 1). Die Wörter der Satz-Einstellungen
//! stehen in [`crate::typeset`].

use sha2::{Digest, Sha256};

/// Namensraum = Hauptversion; nur eine unverträgliche Änderung bekommt einen neuen.
pub const NAMESPACE: &str = "urn:baruch:werk:1";
/// Unterversion (Attribut `version`); verträgliche Erweiterungen zählen hoch. 2 seit Runde 25: `<karte>` mit
/// Verbindungen und Lagen, Grabsteinarten `verbindung` und `lage`. 3 seit Runde 27: in `<buch>` die Anschriften
/// `<verlag-anschrift>`, `<druckerei-anschrift>` und die Umschlag-Anordnung `<umschlag>`, in `<angaben>`
/// `<satzbild>`, Grabsteinart `umschlag-block`. 4 seit Runde 28: `<befunde>` mit Statusverlauf, an Beiträgen und
/// Fadenbeiträgen `geaendert` und `<alte-fassung>`, Grabsteinarten `befund`, `befund-status`, `beitrag-fassung`. 5 seit
/// Runde 30: an `<befund>` die Kinder `<loesung>`, an der Fassung `variante-gruppe`, `variante`, `variante-zeile` und
/// Kinder `<setzt-um befund>`, an Beitrag und Fadenbeitrag `grundlage-fassung`, `grundlage`, `antwort-auf`,
/// `wiederholt`, `relay-von` (keine neue Grabsteinart). 6 seit Runde 29: in `<angaben>` hinter `<satzbild>` die
/// Satz-Einstellungen `<satz>`, Grabsteinart `satz`. 7 seit Runde 31: am Ende von `<werk>` die eigenen Register
/// `<eigene-register>` und das Glossar `<glossar>`, an `<satz>` die Attribute `glossarseite`, `registerseiten`,
/// `registerseiten-spalten`, `glossar-erste-erwaehnung`, Grabsteinarten `glossar-begriff` und `register`. 8 seit
/// Runde 22: an `<fassung>` `stand` und `beruht-stand` und je Fortschreibung `<fortschreibung>` (Grabsteinart
/// `fortschreibung`), in `<satz>` das Kind `<entwurf/>`, an `<befund>` und `<status-eintrag>` `stand`.
pub const FORMAT_VERSION: u32 = 8;
pub const ZIP_MIMETYPE: &str = "application/vnd.baruch.werk+zip";
pub const XML_MEDIA_TYPE: &str = "application/vnd.baruch.werk+xml";
pub const ZIP_SUFFIX: &str = ".baruch";
pub const XML_SUFFIX: &str = ".baruch.xml";
pub const ENTRY_MIMETYPE: &str = "mimetype";
pub const ENTRY_WORK: &str = "werk.xml";
pub const ENTRY_MANIFEST: &str = "MANIFEST.xml";
pub const IMAGE_DIR: &str = "bilder/";

/// `true` für eine UUID in kanonischer Kleinschreibung 8-4-4-4-12 (beliebige Version und Variante).
pub fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

/// Höchstlänge eines Instanznamens in Zeichen (lokal beim Setzen geprüft, beim Lesen gekürzt).
pub const MAX_INSTANCE_NAME_CHARS: usize = 80;

/// Instanzname aus einer fremden Datei säubern: Steuerzeichen und Leerraum werden ein Leerzeichen, `<` und `>`
/// werden `‹` und `›` (Git lehnt sie in Autorennamen ab), getrimmt und auf [`MAX_INSTANCE_NAME_CHARS`] Zeichen
/// gekürzt. Der Name landet in Commit-Autor, Antragsherkunft und Fassungsnotizen.
pub fn clean_instance_name(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| match c {
            '<' => '‹',
            '>' => '›',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let joined = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(MAX_INSTANCE_NAME_CHARS).collect::<String>().trim_end().to_owned()
}

/// SHA-256 als Hex in Kleinbuchstaben.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Dateiendung eines Bildtyps im Archiv; andere Typen nimmt das Format nicht an.
pub fn image_extension(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

/// Name des Eintrags einer Bildfassung im Archiv: `bilder/<uuid>.<ext>`.
pub fn image_entry(revision_uuid: &str, mime: &str) -> Option<String> {
    image_extension(mime).map(|ext| format!("{IMAGE_DIR}{revision_uuid}.{ext}"))
}

/// Aufzählung mit XML-Wörtern; die erste Variante ist die Vorgabe.
macro_rules! xml_enum {
    ($(#[$meta:meta])* $name:ident { $first:ident = $first_xml:literal $(, $variant:ident = $xml:literal)* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub enum $name {
            #[default]
            $first,
            $($variant,)*
        }

        impl $name {
            /// Alle Varianten in der Reihenfolge des Vertrags.
            pub const ALL: &'static [$name] = &[$name::$first $(, $name::$variant)*];

            /// Wort im XML.
            pub fn as_xml(self) -> &'static str {
                match self {
                    $name::$first => $first_xml,
                    $($name::$variant => $xml,)*
                }
            }

            /// Variante zum Wort im XML; unbekannt → `None`.
            pub fn from_xml(s: &str) -> Option<Self> {
                match s {
                    $first_xml => Some($name::$first),
                    $($xml => Some($name::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

xml_enum!(
    /// Art des Werks (`art` an `<werk>`).
    ExchangeWorkKind { Book = "buch", Sermon = "predigt" }
);
xml_enum!(
    /// Art einer SWORD-Quelle (`art` an `<quelle>` in `<angaben>`).
    ExchangeSourceKind { Commentary = "kommentar", Lexicon = "lexikon" }
);
xml_enum!(
    /// Titelbereich des Umschlags (`titelbereich` an `<buch>`).
    ExchangeTitleArea { Top = "oben", Middle = "mitte", Bottom = "unten" }
);
xml_enum!(
    /// Art eines Gliederungspunkts (`art` an `<punkt>`).
    ExchangeNodeKind { Part = "teil", Chapter = "kapitel", Section = "abschnitt" }
);
xml_enum!(
    /// Status einer Fassung (`status` an `<fassung>` im `<punkt>`).
    ExchangeRevisionStatus { Current = "aktuell", Proposed = "vorschlag", Rejected = "verworfen", Superseded = "abgeloest" }
);
xml_enum!(
    /// Status eines Einfalls.
    ExchangeIdeaStatus { Open = "offen", Done = "erledigt" }
);
xml_enum!(
    /// Wozu ein erledigter Einfall wurde (`erledigt-als`).
    ExchangeIdeaUse { Outline = "gliederung", Post = "beitrag", Persona = "persona", Note = "notiz" }
);
xml_enum!(
    /// Herkunft eines Einfalls (`herkunft-art`).
    ExchangeOriginKind { Human = "mensch", Persona = "persona", Mcp = "mcp" }
);
xml_enum!(
    /// Status einer Bildfassung.
    ExchangeAssetStatus { Current = "aktuell", Superseded = "abgeloest" }
);
xml_enum!(
    /// Art eines Grabsteins (`art` an `<eintrag>` in `<geloescht>`).
    ExchangeTombstoneKind {
        Work = "werk",
        Node = "punkt",
        Revision = "fassung",
        TemplateRevision = "vorlage",
        Post = "beitrag",
        Idea = "einfall",
        Thread = "faden",
        ThreadPost = "faden-beitrag",
        Asset = "bild",
        AssetRevision = "bildfassung",
        Link = "verbindung",
        Position = "lage",
        CoverBlock = "umschlag-block",
        Finding = "befund",
        FindingStatus = "befund-status",
        PostVersion = "beitrag-fassung",
        Typeset = "satz",
        GlossaryTerm = "glossar-begriff",
        Register = "register",
        Amend = "fortschreibung",
    }
);
xml_enum!(
    /// Art eines Schritts im Verlauf einer Fassung (`art` an `<fortschreibung>`, ab Unterversion 8; Vertrag R22
    /// §2.11 nennt die Wörter `amend` und `undo`, gleich den Datenbankwerten).
    ExchangeAmendKind { Amend = "amend", Undo = "undo" }
);
xml_enum!(
    /// Status eines Glossarbegriffs (`status` an `<begriff>`, ab Unterversion 7).
    ExchangeGlossaryStatus { Accepted = "angenommen", Proposed = "vorgeschlagen" }
);
xml_enum!(
    /// Art eines Befunds (`art` an `<befund>`, ab Unterversion 4).
    ExchangeFindingKind { Praise = "lob", Hint = "hinweis", Critical = "kritisch" }
);
xml_enum!(
    /// Status eines Befunds (`status` an `<befund>` und `<status-eintrag>`, ab Unterversion 4).
    ExchangeFindingStatus {
        Open = "offen",
        Adopted = "uebernommen",
        Rejected = "verworfen",
        Deferred = "verschoben",
        Done = "erledigt",
    }
);
xml_enum!(
    /// Entscheidung über einen Statusvorschlag (`entscheidung` an `<status-eintrag>`, ab Unterversion 4).
    ExchangeDecision { Confirmed = "bestaetigt", Rejected = "abgelehnt", Stale = "veraltet" }
);
xml_enum!(
    /// Art einer Verbindung (`art` an `<verbindung>`).
    ExchangeLinkKind { LeadsTo = "fuehrt-weiter", BelongsTo = "gehoert-zu", ExampleOf = "beispiel-fuer", Contradicts = "widerspricht" }
);

xml_enum!(
    /// Art eines Blocks der Umschlag-Anordnung (`art` an `<block>`, ab Unterversion 3); Datenbank-Namen über
    /// [`ExchangeCoverBlockKind::as_db`].
    ExchangeCoverBlockKind {
        FrontTitle = "vorderseite-titel",
        FrontSubtitle = "vorderseite-untertitel",
        FrontAuthor = "vorderseite-autor",
        FrontPublisher = "vorderseite-verlag",
        FrontFree = "vorderseite-freitext",
        SpineTitle = "ruecken-titel",
        SpineAuthor = "ruecken-autor",
        SpinePublisher = "ruecken-verlag",
        SpineFree = "ruecken-freitext",
        BackMotto = "rueckseite-motto",
        BackBlurb = "rueckseite-klappentext",
        BackBio = "rueckseite-vita",
        BackBarcode = "rueckseite-strichcode",
        BackPublisherPrice = "rueckseite-verlag-preis",
        BackFree = "rueckseite-freitext",
    }
);
xml_enum!(
    /// Ausrichtung eines Umschlag-Blocks (`ausrichtung`).
    ExchangeCoverAlign { Center = "mitte", Left = "links", Right = "rechts", Justify = "blocksatz" }
);
xml_enum!(
    /// Schriftschnitt eines Umschlag-Blocks (`schnitt`).
    ExchangeFontStyle { Normal = "normal", Bold = "fett", Italic = "kursiv" }
);
xml_enum!(
    /// Fläche hinter einem Umschlag-Block (`hintergrund`).
    ExchangeBackdrop { None = "kein", Shadow = "schatten", Gradient = "verlauf" }
);
xml_enum!(
    /// Laufrichtung eines Rücken-Blocks (`richtung`): abwärts = von oben nach unten lesbar.
    ExchangeSpineDirection { Down = "abwaerts", Up = "aufwaerts" }
);

impl ExchangeCoverBlockKind {
    /// Name in der Datenbank (`cover_block.kind`, Migration 0045) und in RPC/serde.
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeCoverBlockKind::FrontTitle => "front_title",
            ExchangeCoverBlockKind::FrontSubtitle => "front_subtitle",
            ExchangeCoverBlockKind::FrontAuthor => "front_author",
            ExchangeCoverBlockKind::FrontPublisher => "front_publisher",
            ExchangeCoverBlockKind::FrontFree => "front_free",
            ExchangeCoverBlockKind::SpineTitle => "spine_title",
            ExchangeCoverBlockKind::SpineAuthor => "spine_author",
            ExchangeCoverBlockKind::SpinePublisher => "spine_publisher",
            ExchangeCoverBlockKind::SpineFree => "spine_free",
            ExchangeCoverBlockKind::BackMotto => "back_motto",
            ExchangeCoverBlockKind::BackBlurb => "back_blurb",
            ExchangeCoverBlockKind::BackBio => "back_bio",
            ExchangeCoverBlockKind::BackBarcode => "back_barcode",
            ExchangeCoverBlockKind::BackPublisherPrice => "back_publisher_price",
            ExchangeCoverBlockKind::BackFree => "back_free",
        }
    }

    /// Variante zum Datenbank-Namen; unbekannt → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }

    /// Block auf dem Rücken (nur dort gibt es `richtung`).
    pub fn is_spine(self) -> bool {
        self.as_db().starts_with("spine_")
    }

    /// Eigener Text (`<text>`) nur bei den Freitexten und dem Motto.
    pub fn own_text(self) -> bool {
        matches!(
            self,
            ExchangeCoverBlockKind::FrontFree
                | ExchangeCoverBlockKind::SpineFree
                | ExchangeCoverBlockKind::BackFree
                | ExchangeCoverBlockKind::BackMotto
        )
    }
}

impl ExchangeAmendKind {
    /// Wert in `revision_amend.kind` (Migration 0050) – dieselben Wörter wie im XML.
    pub fn as_db(self) -> &'static str {
        self.as_xml()
    }

    /// Variante zum Datenbankwert; unbekannt → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::from_xml(s)
    }
}

impl ExchangeFindingKind {
    /// Wert in `finding.kind` (Migration 0046).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeFindingKind::Praise => "praise",
            ExchangeFindingKind::Hint => "hint",
            ExchangeFindingKind::Critical => "critical",
        }
    }

    /// Variante zum Datenbankwert; unbekannt → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

impl ExchangeFindingStatus {
    /// Wert in `finding.status` bzw. `finding_status.status` (Migration 0046).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeFindingStatus::Open => "open",
            ExchangeFindingStatus::Adopted => "adopted",
            ExchangeFindingStatus::Rejected => "rejected",
            ExchangeFindingStatus::Deferred => "deferred",
            ExchangeFindingStatus::Done => "done",
        }
    }

    /// Variante zum Datenbankwert; unbekannt → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

impl ExchangeDecision {
    /// Wert in `finding_status.decision` (Migration 0046).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeDecision::Confirmed => "confirmed",
            ExchangeDecision::Rejected => "rejected",
            ExchangeDecision::Stale => "stale",
        }
    }

    /// Variante zum Datenbankwert; unbekannt → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

impl ExchangeTombstoneKind {
    /// Wert in `uuid_tombstone.kind` (Migration 0041).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeTombstoneKind::Work => "work",
            ExchangeTombstoneKind::Node => "node",
            ExchangeTombstoneKind::Revision => "revision",
            ExchangeTombstoneKind::TemplateRevision => "template_revision",
            ExchangeTombstoneKind::Post => "post",
            ExchangeTombstoneKind::Idea => "idea",
            ExchangeTombstoneKind::Thread => "thread",
            ExchangeTombstoneKind::ThreadPost => "thread_post",
            ExchangeTombstoneKind::Asset => "asset",
            ExchangeTombstoneKind::AssetRevision => "asset_revision",
            ExchangeTombstoneKind::Link => "idea_link",
            ExchangeTombstoneKind::Position => "idea_map_pos",
            ExchangeTombstoneKind::CoverBlock => "cover_block",
            ExchangeTombstoneKind::Finding => "finding",
            ExchangeTombstoneKind::FindingStatus => "finding_status",
            ExchangeTombstoneKind::PostVersion => "post_version",
            ExchangeTombstoneKind::Typeset => "work_typeset",
            ExchangeTombstoneKind::GlossaryTerm => "glossary_term",
            ExchangeTombstoneKind::Register => "index_register",
            ExchangeTombstoneKind::Amend => "revision_amend",
        }
    }

    /// Erste Unterversion, die diese Art kennt: `verbindung` und `lage` ab 2, `umschlag-block` ab 3, `befund`,
    /// `befund-status` und `beitrag-fassung` ab 4, `satz` ab 6, `glossar-begriff` und `register` ab 7, `fortschreibung` ab 8,
    /// alle übrigen ab 1. Ein Grabstein einer Art, die seine Datei noch
    /// nicht kennen dürfte, ist ein Fehler (Vertrag R25 §4.6).
    pub fn since_version(self) -> u32 {
        match self {
            ExchangeTombstoneKind::Link | ExchangeTombstoneKind::Position => 2,
            ExchangeTombstoneKind::CoverBlock => 3,
            ExchangeTombstoneKind::Finding
            | ExchangeTombstoneKind::FindingStatus
            | ExchangeTombstoneKind::PostVersion => 4,
            ExchangeTombstoneKind::Typeset => 6,
            ExchangeTombstoneKind::GlossaryTerm | ExchangeTombstoneKind::Register => 7,
            ExchangeTombstoneKind::Amend => 8,
            _ => 1,
        }
    }

    /// Variante zum Wert in `uuid_tombstone.kind`; unbekannt → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

/// Eine ganze Austauschdatei.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeDocument {
    /// Unterversion aus `version` an `<werk>`.
    pub version: u32,
    /// Zeitpunkt der Ausgabe (`<ausgabe zeit>`).
    pub exported: String,
    pub instance: ExchangeInstance,
    /// Programm und Version (`<ausgabe programm>`).
    pub generator: String,
    pub work: ExchangeWork,
    /// Unbekannte Elemente und Attribute als Pfad (z. B. `werk/angaben/farbe`); kein Fehler, nichts übernommen.
    pub unknown: Vec<String>,
}

/// Ausgebende Instanz.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeInstance {
    pub uuid: String,
    /// Leer = ohne Namen.
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeWork {
    pub uuid: String,
    pub kind: ExchangeWorkKind,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub audience: String,
    pub bible: String,
    pub created: String,
    pub updated: Option<String>,
    /// Gliederungsschema (`work.numbering_scheme`).
    pub numbering_scheme: String,
    /// Eigenes Muster (`<muster>`), nur bei Bedarf.
    pub numbering: Option<String>,
    pub bible_module: Option<String>,
    pub sword_sources: Vec<ExchangeSwordSource>,
    pub timing: Option<ExchangeTiming>,
    pub book: Option<ExchangeBook>,
    pub template: ExchangeTemplate,
    /// Wurzeln der Gliederung in Reihenfolge.
    pub outline: Vec<ExchangeNode>,
    pub ideas: Vec<ExchangeIdea>,
    pub threads: Vec<ExchangeThread>,
    pub assets: Vec<ExchangeAsset>,
    pub deleted: Vec<ExchangeTombstone>,
    /// Gedankenkarte (`<karte>`, ab Unterversion 2); leer = ohne `<karte>`.
    pub map: ExchangeMap,
    /// Satzbild-Einstellungen (`<satzbild>` in `<angaben>`, ab Unterversion 3; Tabelle `work_sheet`).
    pub sheet: Option<ExchangeSheet>,
    /// Befunde mit Statusverlauf (`<befunde>`, ab Unterversion 4; Tabellen `finding`, `finding_status`).
    pub findings: Vec<ExchangeFinding>,
    /// Satz-Einstellungen (`<satz>` in `<angaben>`, ab Unterversion 6; Tabelle `work_typeset`). `None` = keine
    /// (alles aus der Werkvorlage).
    pub typeset: Option<ExchangeTypeset>,
    /// Eigene Register in ihrer Reihenfolge (`<eigene-register>`, ab Unterversion 7; Tabelle `index_register`). Die
    /// eingebauten Register (stichwort, personen, bibel) stehen nie in der Datei.
    pub registers: Vec<ExchangeRegister>,
    /// Glossar: angenommene und vorgeschlagene Begriffe (`<glossar>`, ab Unterversion 7; Tabellen `glossary_term`,
    /// `glossary_variant`). Registervorschläge (`index_proposal`) sind Arbeitsablauf der Instanz und nicht im Format.
    pub glossary: Vec<ExchangeGlossaryTerm>,
}

/// Ein eigenes Register (Runde 31, Tabelle `index_register`; im Format ab Unterversion 7): Kennung, Titel, Anlage.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeRegister {
    pub uuid: String,
    pub slug: String,
    pub title: String,
    pub created: String,
}

/// Ein Glossarbegriff (Runde 31, Tabellen `glossary_term`, `glossary_variant`; im Format ab Unterversion 7). Die
/// Bibelstellen stehen als Text, wie gespeichert (normalisiert beim Anlegen).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeGlossaryTerm {
    pub uuid: String,
    pub term: String,
    pub short: String,
    pub long: String,
    pub source: String,
    pub variants: Vec<String>,
    pub bible_refs: Vec<String>,
    pub status: ExchangeGlossaryStatus,
    /// Herkunft: Name des Menschen bzw. Handle der Persona oder des MCP-Clients.
    pub origin: String,
    pub created: String,
    pub updated: Option<String>,
    pub updated_by: String,
}

/// Satz-Einstellungen des Werks (Runde 29, Tabelle `work_typeset`; im Format ab Unterversion 6): Kennung, letzte
/// Änderung (Zeitpunkt wie gespeichert, Name des Menschen) und die Einstellungen selbst. Die XML-Wörter je Feld stehen
/// in [`crate::typeset`]; geprüft werden die Werte erst beim Einlesen (`TypesetSettings::validate`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTypeset {
    pub uuid: String,
    pub updated: String,
    pub updated_by: String,
    pub settings: baruch_typst::typeset::TypesetSettings,
}

/// Alte Fassung eines bearbeiteten Beitrags bzw. Fadenbeitrags (`<alte-fassung>`, ab Unterversion 4; Tabelle
/// `post_version`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePostVersion {
    pub uuid: String,
    pub body: String,
    /// Seit wann diese Fassung galt (Anlage des Beitrags bzw. die vorige Änderung).
    pub valid_from: String,
    /// Wann sie ersetzt wurde.
    pub replaced: String,
    /// Wer bearbeitet hat (Autor bzw. `@mcp:<name>`).
    pub edited_by: String,
}

/// Ein Befund (`<befund>` in `<befunde>`, ab Unterversion 4; Tabelle `finding`). Herkunft über UUIDs: Punkt,
/// Beitrag bzw. Fadenbeitrag (nie beides); Fassung, Ziel und Ähnlichkeit ebenfalls über UUIDs (Fassung und Ziel ohne
/// Gewähr, dass es sie noch gibt – wie in der Datenbank ohne Fremdschlüssel).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeFinding {
    pub uuid: String,
    /// UUID des Punkts (`punkt`); fehlt bei Fäden und gelöschten Abschnitten.
    pub node: Option<String>,
    /// UUID des Beitrags im Abschnitts-Chat (`beitrag`).
    pub post: Option<String>,
    /// UUID des Fadenbeitrags (`faden-beitrag`).
    pub thread_post: Option<String>,
    /// Stelle in der Antwort (ab 0).
    pub position: u32,
    pub persona: String,
    pub kind: ExchangeFindingKind,
    pub body: String,
    pub quote: String,
    pub quote_check: String,
    pub status: ExchangeFindingStatus,
    pub reason: String,
    /// UUID der Fassung („übernommen“).
    pub adopted_revision: Option<String>,
    /// UUID des Ziel-Punkts („verschoben“).
    pub deferred_node: Option<String>,
    /// UUID des Ziel-Einfalls („verschoben“).
    pub deferred_idea: Option<String>,
    /// UUID des Befunds, dem dieser gleicht (angehängt).
    pub similar_to: Option<String>,
    /// UUIDs der Befunde, die ein Sammelvorschlag zusammenfasst.
    pub summary_of: Vec<String>,
    pub created: String,
    pub updated: String,
    /// Statusverlauf, älteste zuerst (`<status-eintrag>`).
    pub history: Vec<ExchangeFindingEntry>,
    /// Lösungsvorschläge (`<loesung>`, ab Unterversion 5; `finding.solutions`).
    pub solutions: Vec<ExchangeSolution>,
    /// Stand der übernommenen Fassung (`stand`, ab Unterversion 8; `finding.adopted_amend`).
    pub adopted_amend: Option<u32>,
}

/// Ein Lösungsvorschlag zu einem Befund (`<loesung zitat ersatz schritt/>`, ab Unterversion 5): Zitat mit neuer
/// Formulierung oder ein Schritt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeSolution {
    pub quote: String,
    pub replacement: String,
    pub step: String,
}

/// Kennzeichen einer Variante an einer Fassung (ab Unterversion 5; `revision.variant_*`): Gruppe (UUID, gleich für
/// alle Geschwister), Etikett A/B/C, eine Zeile und die umgesetzten Befunde (UUIDs, `<setzt-um befund>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeVariant {
    pub group: String,
    pub label: String,
    pub summary: String,
    pub findings: Vec<String>,
}

/// Etiketten einer Variante.
pub const VARIANT_LABELS: [&str; 3] = ["A", "B", "C"];
/// Längste Zeile einer Variante bzw. Prüfgrundlage eines Beitrags (wie die CHECKs der Migration 0047).
pub const MAX_VARIANT_SUMMARY_CHARS: usize = 300;
pub const MAX_BASIS_LABEL_CHARS: usize = 300;
pub const MAX_RELAY_FROM_CHARS: usize = 200;

/// Metadaten eines Beitrags bzw. Fadenbeitrags (ab Unterversion 5; Spalten aus Migration 0047): geprüfte Fassung
/// (UUID, nur bei Beiträgen eines Abschnitts), Kopfzeile, Antwort auf und Wiederholung von (UUIDs von Beiträgen
/// derselben Stelle), Relay von Persona. Der Auftrag (`request`) bleibt lokal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangePostMeta {
    pub basis_revision: Option<String>,
    pub basis_label: String,
    pub reply_to: Option<String>,
    pub repeat_of: Option<String>,
    pub relay_from: Option<String>,
}

impl ExchangePostMeta {
    /// Keine Metadaten.
    pub fn is_empty(&self) -> bool {
        *self == ExchangePostMeta::default()
    }
}

/// Ein Eintrag im Statusverlauf (`<status-eintrag>`, Tabelle `finding_status`): Statusänderung des Menschen oder
/// Vorschlag eines Assistenten (`proposed`) samt Entscheidung.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeFindingEntry {
    pub uuid: String,
    pub status: ExchangeFindingStatus,
    pub reason: String,
    pub adopted_revision: Option<String>,
    pub deferred_node: Option<String>,
    pub deferred_idea: Option<String>,
    pub actor_kind: ExchangeOriginKind,
    pub actor: String,
    pub proposed: bool,
    pub note: String,
    pub decision: Option<ExchangeDecision>,
    pub decided: Option<String>,
    pub decided_by: Option<String>,
    pub created: String,
    /// Stand der übernommenen Fassung (`stand`, ab Unterversion 8; `finding_status.adopted_amend`).
    pub adopted_amend: Option<u32>,
}

/// Grenzen eines Befunds (wie die CHECKs der Migration 0046).
pub const MAX_FINDING_CHARS: usize = 4000;
pub const MAX_FINDING_QUOTE_CHARS: usize = 2000;
pub const MAX_FINDING_CHECK_CHARS: usize = 200;
pub const MAX_FINDING_REASON_CHARS: usize = 2000;
pub const MAX_FINDING_PERSONA_CHARS: usize = 200;

/// Satzbild-Einstellungen des Werks (Runde 18c, Tabelle `work_sheet`; im Format ab Unterversion 3). `None` = aus der
/// Werkvorlage. Werte wie gespeichert: Schriftlisten, Längen mit Einheit (`11pt`, `16cm`), Farben `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeSheet {
    pub font: Option<String>,
    pub heading_font: Option<String>,
    pub size: Option<String>,
    pub fill: Option<String>,
    pub heading_fill: Option<String>,
    pub background: Option<String>,
    pub width: Option<String>,
}

impl ExchangeSheet {
    /// Alle Felder `None` (dann schreibt der Schreiber kein `<satzbild>`).
    pub fn is_empty(&self) -> bool {
        *self == ExchangeSheet::default()
    }
}

/// Ein Block der Umschlag-Anordnung (`<block>` in `<umschlag>`, ab Unterversion 3; Tabelle `cover_block`).
/// Koordinaten in mm ab der Schnittkante der eigenen Fläche (Vertrag R27 1.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeCoverBlock {
    pub uuid: String,
    pub kind: ExchangeCoverBlockKind,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub align: ExchangeCoverAlign,
    pub size_pt: f64,
    pub leading_em: f64,
    /// Wie gespeichert: `light`, `dark` oder `#rrggbb` (im XML `hell`, `dunkel` oder `#rrggbb`).
    pub color: String,
    /// `None` = Schrift der Werkvorlage.
    pub font: Option<String>,
    pub style: ExchangeFontStyle,
    pub backdrop: ExchangeBackdrop,
    pub visible: bool,
    /// Nur auf dem Rücken.
    pub direction: Option<ExchangeSpineDirection>,
    /// Nur bei Arten mit eigenem Text.
    pub text: Option<String>,
    pub updated: String,
}

/// Grenzen eines Umschlag-Blocks (wie `baruch_typst::cover_layout` und die CHECKs der Migration 0045).
pub const COVER_POS_MM: (f64, f64) = (-50.0, 1000.0);
pub const COVER_WIDTH_MM: (f64, f64) = (1.0, 1000.0);
pub const COVER_SIZE_PT: (f64, f64) = (4.0, 200.0);
pub const COVER_LEADING_EM: (f64, f64) = (0.0, 3.0);
pub const COVER_MAX_TEXT_CHARS: usize = 1000;
pub const COVER_MAX_FONT_CHARS: usize = 200;
/// Höchstlänge der Anschriften (Zeichen).
pub const MAX_ADDRESS_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeSwordSource {
    pub kind: ExchangeSourceKind,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTiming {
    pub words_per_minute: u32,
    pub target_seconds: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeBook {
    pub title_area: ExchangeTitleArea,
    pub front_matter: bool,
    pub cover_pages: bool,
    pub paper_mm: f64,
    pub bleed_mm: f64,
    /// UUID des Bilds (nicht der Fassung).
    pub cover: Option<String>,
    /// UUID des Bilds (nicht der Fassung).
    pub author_photo: Option<String>,
    pub updated: Option<String>,
    pub publisher: String,
    pub place: String,
    pub year: String,
    pub edition: String,
    pub isbn: String,
    pub price: String,
    pub series: String,
    pub printer: String,
    pub bible_notice: String,
    pub image_credit: String,
    pub dedication: String,
    pub blurb: String,
    pub bio: String,
    /// Anschrift des Verlags bzw. Herausgebers (`<verlag-anschrift>`, ab Unterversion 3).
    pub publisher_address: String,
    /// Anschrift der Druckerei (`<druckerei-anschrift>`, ab Unterversion 3).
    pub printer_address: String,
    /// Umschlag-Anordnung (`<umschlag>`, ab Unterversion 3); leer = ohne eigene Anordnung.
    pub cover_blocks: Vec<ExchangeCoverBlock>,
}

/// Werkvorlage: aktueller Text (`work.template`) und alle Fassungen.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTemplate {
    pub current: String,
    pub revisions: Vec<ExchangeTemplateRevision>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTemplateRevision {
    pub uuid: String,
    pub number: u32,
    pub body: String,
    pub author: String,
    pub note: String,
    pub created: String,
}

/// Gliederungspunkt mit allen Fassungen, Beiträgen und Kindern.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeNode {
    pub uuid: String,
    pub kind: ExchangeNodeKind,
    pub numbered: bool,
    pub title: String,
    /// Fester Schlüssel des Punkts in der ausgebenden Instanz (`schluessel`, ab Unterversion 5): Querverweise
    /// `#link(<kN>)` im Text nennen ihn; beim Einlesen werden sie auf die hiesigen Schlüssel umgesetzt.
    pub key: Option<i64>,
    pub revisions: Vec<ExchangeRevision>,
    pub posts: Vec<ExchangePost>,
    pub children: Vec<ExchangeNode>,
}

/// Urheber einer Fassung (`autor-art`, `autor`, `modell`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeAuthor {
    Human { name: String },
    Assistant { persona: String, model: String },
}

impl Default for ExchangeAuthor {
    fn default() -> Self {
        ExchangeAuthor::Human { name: String::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeRevision {
    pub uuid: String,
    /// Nummer in der ausgebenden Instanz.
    pub number: u32,
    pub status: ExchangeRevisionStatus,
    pub author: ExchangeAuthor,
    /// UUID der Elternfassung im selben Punkt.
    pub based_on: Option<String>,
    pub note: String,
    pub body: String,
    pub created: String,
    pub edited: Option<String>,
    /// Variante (ab Unterversion 5); `None` = keine.
    pub variant: Option<ExchangeVariant>,
    /// Stand der Fassung (`stand`, ab Unterversion 8; `revision.amend_seq`): Zahl ihrer Fortschreibungen, 0 = nie.
    pub amend_seq: u32,
    /// Stand der Basis, auf dem ein Vorschlag beruht (`beruht-stand`, ab Unterversion 8; `revision.based_on_amend`).
    pub based_on_amend: Option<u32>,
    /// Verlauf der Fortschreibungen (`<fortschreibung>`, ab Unterversion 8; Tabelle `revision_amend`), nach `seq`.
    pub amends: Vec<ExchangeAmend>,
}

/// Ein Schritt im Verlauf einer fortgeschriebenen Fassung (Runde 22, Tabelle `revision_amend`; im Format ab
/// Unterversion 8): Fortschreibung des Menschen oder Rücknahme über den Verlauf (`undo_of` = zurückgenommener Schritt).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeAmend {
    pub uuid: String,
    /// Schritt ab 1, lückenlos je Fassung.
    pub seq: u32,
    pub kind: ExchangeAmendKind,
    pub undo_of: Option<u32>,
    pub author: String,
    pub created: String,
    /// Text vor dem Schritt.
    pub before: String,
    /// Text nach dem Schritt.
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePost {
    pub uuid: String,
    pub position: u32,
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    pub refs: Vec<ExchangePostRef>,
    pub created: String,
    /// Zeit der letzten Bearbeitung (`geaendert`, ab Unterversion 4); `None` = nie bearbeitet.
    pub edited: Option<String>,
    /// Alte Fassungen, älteste zuerst (`<alte-fassung>`, ab Unterversion 4).
    pub versions: Vec<ExchangePostVersion>,
    /// Metadaten (ab Unterversion 5).
    pub meta: ExchangePostMeta,
}

/// Bezug eines Beitrags auf einen Punkt (und eine Fassung) mit Zitat.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePostRef {
    pub node: String,
    pub revision: Option<String>,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeIdea {
    pub uuid: String,
    /// UUID des Punkts (`bezug`).
    pub node: Option<String>,
    pub words: Vec<String>,
    pub note: String,
    pub quote: String,
    pub source: String,
    pub bible: String,
    pub origin_kind: ExchangeOriginKind,
    pub origin_name: String,
    pub status: ExchangeIdeaStatus,
    pub done_as: Option<ExchangeIdeaUse>,
    pub done_ref: Option<String>,
    pub created: String,
    pub updated: String,
    /// Weich gelöscht seit diesem Zeitpunkt.
    pub deleted: Option<String>,
}

/// Faden ohne Abschnitt.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeThread {
    pub uuid: String,
    pub title: String,
    pub created_by: String,
    pub created: String,
    pub updated: String,
    pub posts: Vec<ExchangeThreadPost>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeThreadPost {
    pub uuid: String,
    pub position: u32,
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    pub created: String,
    /// Zeit der letzten Bearbeitung (`geaendert`, ab Unterversion 4); `None` = nie bearbeitet.
    pub edited: Option<String>,
    /// Alte Fassungen, älteste zuerst (`<alte-fassung>`, ab Unterversion 4).
    pub versions: Vec<ExchangePostVersion>,
    /// Metadaten (ab Unterversion 5; `basis_revision` bleibt leer, ein Faden hat keine Fassungen).
    pub meta: ExchangePostMeta,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeAsset {
    pub uuid: String,
    pub name: String,
    pub created: String,
    /// Pfade, unter denen das Werk das Bild nennt.
    pub paths: Vec<String>,
    pub revisions: Vec<ExchangeAssetRevision>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeAssetRevision {
    pub uuid: String,
    pub number: u32,
    /// UUID der Elternfassung im selben Bild.
    pub based_on: Option<String>,
    pub mime: String,
    /// SHA-256 von `data` (Kleinbuchstaben-Hex).
    pub sha256: String,
    /// Rohbytes (SVG als UTF-8).
    pub data: Vec<u8>,
    pub caption: String,
    pub alt: String,
    pub author: String,
    pub note: String,
    pub status: ExchangeAssetStatus,
    pub created: String,
    pub edited: Option<String>,
}

/// Grabstein: hier gelöschte Zeile mit UUID (Tabelle `uuid_tombstone`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTombstone {
    pub uuid: String,
    pub kind: ExchangeTombstoneKind,
    pub deleted: String,
}

/// Verbindung zwischen zwei Einfällen dieses Werks (`<verbindung>` in `<karte>`, Tabelle `idea_link`).
/// `from`/`to` sind Einfall-UUIDs; „widerspricht“ ist ungerichtet (die Reihenfolge trägt keine Bedeutung).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeLink {
    pub uuid: String,
    pub from: String,
    pub to: String,
    pub kind: ExchangeLinkKind,
    pub note: String,
    pub origin_kind: ExchangeOriginKind,
    pub origin_name: String,
    pub created: String,
    pub updated: String,
    /// Weich gelöscht seit diesem Zeitpunkt.
    pub deleted: Option<String>,
}

/// Gespeicherte Lage eines Einfalls auf der Karte dieses Werks (`<lage>`, Tabelle `idea_map_pos`), in
/// SVG-Benutzereinheiten der Karte (Werk bei 0,0).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePosition {
    pub uuid: String,
    /// UUID des Einfalls.
    pub idea: String,
    pub x: f64,
    pub y: f64,
    pub updated: String,
}

/// Inhalt von `<karte>`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeMap {
    pub links: Vec<ExchangeLink>,
    pub positions: Vec<ExchangePosition>,
}

impl ExchangeMap {
    /// Ohne Verbindung und Lage (dann schreibt der Schreiber kein `<karte>`).
    pub fn is_empty(&self) -> bool {
        self.links.is_empty() && self.positions.is_empty()
    }
}

/// Höchster Betrag einer Koordinate in `<lage>` (wie `baruch_store::MAP_COORD_LIMIT`).
pub const MAP_COORD_LIMIT: f64 = 1_000_000.0;

/// Höchstlänge der Notiz einer Verbindung in Zeichen (wie `baruch_store::MAX_LINK_NOTE_CHARS`).
pub const MAX_LINK_NOTE_CHARS: usize = 500;

/// Eintrag in `MANIFEST.xml`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeManifestEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_canonical_lowercase() {
        // Kanonisch, klein, beliebige Version: v4 und v8 gelten.
        assert!(is_uuid("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6"), "v4 klein");
        assert!(is_uuid("0f8c2a4e-1b3d-8c5e-9f60-718293a4b5c6"), "v8 klein");
        // Großbuchstaben, fehlende Striche, falsche Länge und fremde Zeichen sind nicht kanonisch.
        assert!(!is_uuid("0F8C2A4E-1B3D-4C5E-9F60-718293A4B5C6"), "groß ist nicht kanonisch");
        assert!(!is_uuid("0f8c2a4e1b3d4c5e9f60718293a4b5c6"), "ohne Striche nicht kanonisch");
        assert!(!is_uuid("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c"), "zu kurz");
        assert!(!is_uuid("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5cg"), "g ist kein Hex");
    }

    #[test]
    fn xml_words_round_trip() {
        // Jedes Wort führt zur selben Variante zurück; sonst würde der Leser Geschriebenes ablehnen.
        for kind in ExchangeTombstoneKind::ALL {
            assert_eq!(ExchangeTombstoneKind::from_xml(kind.as_xml()), Some(*kind), "XML-Wort {kind:?}");
            assert_eq!(ExchangeTombstoneKind::from_db(kind.as_db()), Some(*kind), "Datenbankwert {kind:?}");
        }
        for kind in ExchangeLinkKind::ALL {
            assert_eq!(ExchangeLinkKind::from_xml(kind.as_xml()), Some(*kind), "Verbindungsart {kind:?}");
        }
        // Die Datenbankwerte der neuen Grabsteinarten sind die aus Migration 0043.
        assert_eq!(ExchangeTombstoneKind::Link.as_db(), "idea_link", "Grabstein einer Verbindung");
        assert_eq!(ExchangeTombstoneKind::Position.as_db(), "idea_map_pos", "Grabstein einer Lage");
        // Nur die Arten der Karte kamen mit Unterversion 2; ein Leser lehnt sie in Unterversion 1 ab.
        let since: Vec<u32> = ExchangeTombstoneKind::ALL.iter().map(|k| k.since_version()).collect();
        assert_eq!(since, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 4, 4, 4, 6, 7, 7, 8], "Unterversion je Grabsteinart");
        // Runde 22: Grabstein einer Fortschreibung (Migration 0050).
        assert_eq!(ExchangeTombstoneKind::Amend.as_db(), "revision_amend", "Datenbankwert der Migration 0050");
        assert_eq!(ExchangeTombstoneKind::Amend.as_xml(), "fortschreibung", "XML-Wort laut Vertrag R22 §2.11");
        for kind in ExchangeAmendKind::ALL {
            assert_eq!(ExchangeAmendKind::from_db(kind.as_db()), Some(*kind), "Art der Fortschreibung {kind:?}");
        }
        // Runde 31: Grabsteine gelöschter Glossarbegriffe und eigener Register (Migration 0049).
        assert_eq!(ExchangeTombstoneKind::GlossaryTerm.as_db(), "glossary_term", "Datenbankwert der Migration 0049");
        assert_eq!(ExchangeTombstoneKind::Register.as_db(), "index_register", "Datenbankwert der Migration 0049");
        assert_eq!(ExchangeTombstoneKind::GlossaryTerm.as_xml(), "glossar-begriff", "XML-Wort laut Vertrag R31 §5");
        assert_eq!(ExchangeTombstoneKind::Register.as_xml(), "register", "XML-Wort laut Vertrag R31 §5");
        // Runde 29: Grabstein zurückgesetzter Satz-Einstellungen (Migration 0048).
        assert_eq!(ExchangeTombstoneKind::Typeset.as_db(), "work_typeset", "Datenbankwert der Migration 0048");
        assert_eq!(ExchangeTombstoneKind::Typeset.as_xml(), "satz", "XML-Wort laut Vertrag R29 §6.6");
        // Runde 28: Grabsteine der Befunde, ihres Verlaufs und alter Beitragsfassungen (Migration 0046).
        let r28: Vec<&str> =
            [ExchangeTombstoneKind::Finding, ExchangeTombstoneKind::FindingStatus, ExchangeTombstoneKind::PostVersion]
                .iter()
                .map(|k| k.as_db())
                .collect();
        assert_eq!(r28, ["finding", "finding_status", "post_version"], "Datenbankwerte der Migration 0046");
        for kind in ExchangeFindingKind::ALL {
            assert_eq!(ExchangeFindingKind::from_xml(kind.as_xml()), Some(*kind), "Befundart {kind:?}");
            assert_eq!(ExchangeFindingKind::from_db(kind.as_db()), Some(*kind), "Datenbankwert {kind:?}");
        }
        for status in ExchangeFindingStatus::ALL {
            assert_eq!(ExchangeFindingStatus::from_xml(status.as_xml()), Some(*status), "Befundstatus {status:?}");
            assert_eq!(ExchangeFindingStatus::from_db(status.as_db()), Some(*status), "Datenbankwert {status:?}");
        }
        for decision in ExchangeDecision::ALL {
            assert_eq!(ExchangeDecision::from_db(decision.as_db()), Some(*decision), "Entscheidung {decision:?}");
        }
        // Runde 27: Grabstein eines Umschlag-Blocks, Datenbankwert aus Migration 0045.
        assert_eq!(ExchangeTombstoneKind::CoverBlock.as_db(), "cover_block", "Grabstein eines Umschlag-Blocks");
        for kind in ExchangeCoverBlockKind::ALL {
            assert_eq!(ExchangeCoverBlockKind::from_xml(kind.as_xml()), Some(*kind), "Blockart {kind:?}");
            assert_eq!(ExchangeCoverBlockKind::from_db(kind.as_db()), Some(*kind), "Datenbankname {kind:?}");
        }
        assert_eq!(ExchangeCoverBlockKind::ALL.len(), 15, "alle 15 Arten des Vertrags R27 1.1");
        assert!(since.iter().all(|&v| v <= FORMAT_VERSION), "keine Art jünger als das Format");
        for status in ExchangeRevisionStatus::ALL {
            assert_eq!(ExchangeRevisionStatus::from_xml(status.as_xml()), Some(*status), "Status {status:?}");
        }
        // Vorgabe ist die erste Variante (Vertrag S1 (4)).
        assert_eq!(ExchangeNodeKind::default(), ExchangeNodeKind::Part, "Vorgabe = erste Variante");
        assert_eq!(
            ExchangeAuthor::default(),
            ExchangeAuthor::Human { name: String::new() },
            "Vorgabe Mensch ohne Namen"
        );
    }

    #[test]
    fn image_entries_follow_the_container_layout() {
        // bilder/<fassungs-uuid>.<ext> mit ext aus png|jpg|gif|webp|svg (F1).
        assert_eq!(
            image_entry("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6", "image/jpeg").as_deref(),
            Some("bilder/0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6.jpg"),
            "JPEG endet auf .jpg"
        );
        assert_eq!(image_extension("image/svg+xml"), Some("svg"), "SVG");
        assert_eq!(image_extension("image/bmp"), None, "andere Typen nimmt das Format nicht an");
        // Bekannter Prüfwert von SHA-256 über die leere Eingabe.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "SHA-256 als Kleinbuchstaben-Hex"
        );
    }
}
