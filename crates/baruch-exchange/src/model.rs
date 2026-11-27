// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model of the exchange format (contract R26, S1 (4); frozen after S1, changes only through the integrator –
//! Round 25 extended it according to contract §4.6 by the idea map: [`ExchangeMap`] with links and positions,
//! tombstone kinds `verbindung` and `lage`, sub-version 2; Round 27 (task umschlag-daten, exception from the
//! integrator reservation) by the cover layout [`ExchangeCoverBlock`], the addresses in the imprint, the typeset
//! view settings [`ExchangeSheet`] and the tombstone kind `umschlag-block`, sub-version 3; Round 28 (task
//! befunde, exception from the integrator reservation) by the findings [`ExchangeFinding`] with status history,
//! the old versions of edited posts [`ExchangePostVersion`] and the tombstone kinds `befund`, `befund-status`,
//! `beitrag-fassung`, sub-version 4; Round 30 (task chat-loesungen, exception from the integrator reservation) by
//! the solutions of the findings [`ExchangeSolution`], the variants of the revisions [`ExchangeVariant`] and the
//! metadata of the posts [`ExchangePostMeta`] (review basis, reply to, repetition, relay), sub-version 5; Round 29
//! (task satz-daten, exception from the integrator reservation) by the typesetting settings [`ExchangeTypeset`]
//! and the tombstone kind `satz`, sub-version 6; Round 31 (task glossar-daten, exception from the integrator
//! reservation) by the own indexes [`ExchangeRegister`], the glossary [`ExchangeGlossaryTerm`], the
//! index/glossary fields of the typesetting settings and the tombstone kinds `glossar-begriff` and `register`,
//! sub-version 7).
//!
//! All references run through UUIDs (canonical lower case, 8-4-4-4-12); numeric ids of the database do not occur
//! in the format. Times are as they are stored (`YYYY-MM-DDTHH:MM:SS.sssZ`). The XML words of the enumerations
//! are delivered by `as_xml`/`from_xml`; the vocabulary is in `docs/schema/baruch-werk-1-u7.xsd` (sub-version 8;
//! `baruch-werk-1-u7.xsd`, `baruch-werk-1-u6.xsd`, `baruch-werk-1-u5.xsd`, `baruch-werk-1-u4.xsd`,
//! `baruch-werk-1-u3.xsd`, `baruch-werk-1-u2.xsd` and `baruch-werk-1.xsd` stay frozen for the sub-versions 7, 6,
//! 5, 4, 3, 2 and 1). The words of the typesetting settings are in [`crate::typeset`].

use sha2::{Digest, Sha256};

/// Namespace = main version; only an incompatible change gets a new one.
pub const NAMESPACE: &str = "urn:baruch:werk:1";
/// Sub-version (attribute `version`); compatible extensions count up. 2 since Round 25: `<karte>` with links and
/// positions, tombstone kinds `verbindung` and `lage`. 3 since Round 27: in `<buch>` the addresses
/// `<verlag-anschrift>`, `<druckerei-anschrift>` and the cover layout `<umschlag>`, in `<angaben>` `<satzbild>`,
/// tombstone kind `umschlag-block`. 4 since Round 28: `<befunde>` with status history, on posts and thread posts
/// `geaendert` and `<alte-fassung>`, tombstone kinds `befund`, `befund-status`, `beitrag-fassung`. 5 since Round
/// 30: on `<befund>` the children `<loesung>`, on the revision `variante-gruppe`, `variante`, `variante-zeile`
/// and children `<setzt-um befund>`, on post and thread post `grundlage-fassung`, `grundlage`, `antwort-auf`,
/// `wiederholt`, `relay-von` (no new tombstone kind). 6 since Round 29: in `<angaben>` after `<satzbild>` the
/// typesetting settings `<satz>`, tombstone kind `satz`. 7 since Round 31: at the end of `<werk>` the own
/// indexes `<eigene-register>` and the glossary `<glossar>`, on `<satz>` the attributes `glossarseite`,
/// `registerseiten`, `registerseiten-spalten`, `glossar-erste-erwaehnung`, tombstone kinds `glossar-begriff` and
/// `register`. 8 since Round 22: on `<fassung>` `stand` and `beruht-stand` and per amendment `<fortschreibung>`
/// (tombstone kind `fortschreibung`), in `<satz>` the child `<entwurf/>`, on `<befund>` and `<status-eintrag>`
/// `stand`. 9 since Round 34 (mind map): on `<einfall>` the optional attributes `titel`, `eltern` (UUID of the
/// parent idea), `platz` (order among siblings) and `hauptthema` (free main topic at the work; missing = cloud
/// or at the reference).
pub const FORMAT_VERSION: u32 = 10;
pub const ZIP_MIMETYPE: &str = "application/vnd.baruch.werk+zip";
pub const XML_MEDIA_TYPE: &str = "application/vnd.baruch.werk+xml";
pub const ZIP_SUFFIX: &str = ".baruch";
pub const XML_SUFFIX: &str = ".baruch.xml";
pub const ENTRY_MIMETYPE: &str = "mimetype";
pub const ENTRY_WORK: &str = "werk.xml";
pub const ENTRY_MANIFEST: &str = "MANIFEST.xml";
pub const IMAGE_DIR: &str = "bilder/";

/// `true` for a UUID in canonical lower case 8-4-4-4-12 (any version and variant).
pub fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

/// Maximum length of an instance name in characters (checked locally when setting, shortened when reading).
pub const MAX_INSTANCE_NAME_CHARS: usize = 80;

/// Clean the instance name from a foreign file: control characters and whitespace become a space, `<` and `>`
/// become `‹` and `›` (Git rejects them in author names), trimmed and shortened to [`MAX_INSTANCE_NAME_CHARS`]
/// characters. The name ends up in commit author, request origin and revision notes.
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

/// SHA-256 as hex in lower case.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// File extension of an image type in the archive; the format does not accept other types.
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

/// Name of the entry of an image revision in the archive: `bilder/<uuid>.<ext>`.
pub fn image_entry(revision_uuid: &str, mime: &str) -> Option<String> {
    image_extension(mime).map(|ext| format!("{IMAGE_DIR}{revision_uuid}.{ext}"))
}

/// Enumeration with XML words; the first variant is the default.
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
            /// All variants in the order of the contract.
            pub const ALL: &'static [$name] = &[$name::$first $(, $name::$variant)*];

            /// Word in the XML.
            pub fn as_xml(self) -> &'static str {
                match self {
                    $name::$first => $first_xml,
                    $($name::$variant => $xml,)*
                }
            }

            /// Variant for the word in the XML; unknown → `None`.
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
    /// Kind of the work (`art` on `<werk>`).
    ExchangeWorkKind { Book = "buch", Sermon = "predigt" }
);
xml_enum!(
    /// Work language (`sprache` at `<angaben>`, from sub-version 10; missing = German).
    ExchangeLanguage { De = "de", En = "en" }
);
xml_enum!(
    /// Kind of a SWORD source (`art` on `<quelle>` in `<angaben>`).
    ExchangeSourceKind { Commentary = "kommentar", Lexicon = "lexikon" }
);
xml_enum!(
    /// Title area of the cover (`titelbereich` on `<buch>`).
    ExchangeTitleArea { Top = "oben", Middle = "mitte", Bottom = "unten" }
);
xml_enum!(
    /// Kind of an outline item (`art` on `<punkt>`).
    ExchangeNodeKind { Part = "teil", Chapter = "kapitel", Section = "abschnitt" }
);
xml_enum!(
    /// Status of a revision (`status` on `<fassung>` in the `<punkt>`).
    ExchangeRevisionStatus { Current = "aktuell", Proposed = "vorschlag", Rejected = "verworfen", Superseded = "abgeloest" }
);
xml_enum!(
    /// Status of an idea.
    ExchangeIdeaStatus { Open = "offen", Done = "erledigt" }
);
xml_enum!(
    /// What a done idea became (`erledigt-als`).
    ExchangeIdeaUse { Outline = "gliederung", Post = "beitrag", Persona = "persona", Note = "notiz" }
);
xml_enum!(
    /// Origin of an idea (`herkunft-art`).
    ExchangeOriginKind { Human = "mensch", Persona = "persona", Mcp = "mcp" }
);
xml_enum!(
    /// Status of an image revision.
    ExchangeAssetStatus { Current = "aktuell", Superseded = "abgeloest" }
);
xml_enum!(
    /// Kind of a tombstone (`art` on `<eintrag>` in `<geloescht>`).
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
    /// Kind of a step in the history of a revision (`art` on `<fortschreibung>`, from sub-version 8; contract R22
    /// §2.11 names the words `amend` and `undo`, equal to the database values).
    ExchangeAmendKind { Amend = "amend", Undo = "undo" }
);
xml_enum!(
    /// Status of a glossary term (`status` on `<begriff>`, from sub-version 7).
    ExchangeGlossaryStatus { Accepted = "angenommen", Proposed = "vorgeschlagen" }
);
xml_enum!(
    /// Kind of a finding (`art` on `<befund>`, from sub-version 4).
    ExchangeFindingKind { Praise = "lob", Hint = "hinweis", Critical = "kritisch" }
);
xml_enum!(
    /// Status of a finding (`status` on `<befund>` and `<status-eintrag>`, from sub-version 4).
    ExchangeFindingStatus {
        Open = "offen",
        Adopted = "uebernommen",
        Rejected = "verworfen",
        Deferred = "verschoben",
        Done = "erledigt",
    }
);
xml_enum!(
    /// Decision on a status proposal (`entscheidung` on `<status-eintrag>`, from sub-version 4).
    ExchangeDecision { Confirmed = "bestaetigt", Rejected = "abgelehnt", Stale = "veraltet" }
);
xml_enum!(
    /// Kind of a link (`art` on `<verbindung>`).
    ExchangeLinkKind { LeadsTo = "fuehrt-weiter", BelongsTo = "gehoert-zu", ExampleOf = "beispiel-fuer", Contradicts = "widerspricht" }
);

xml_enum!(
    /// Kind of a block of the cover layout (`art` on `<block>`, from sub-version 3); database names through
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
    /// Alignment of a cover block (`ausrichtung`).
    ExchangeCoverAlign { Center = "mitte", Left = "links", Right = "rechts", Justify = "blocksatz" }
);
xml_enum!(
    /// Font style of a cover block (`schnitt`).
    ExchangeFontStyle { Normal = "normal", Bold = "fett", Italic = "kursiv" }
);
xml_enum!(
    /// Area behind a cover block (`hintergrund`).
    ExchangeBackdrop { None = "kein", Shadow = "schatten", Gradient = "verlauf" }
);
xml_enum!(
    /// Direction of a spine block (`richtung`): downwards = readable from top to bottom.
    ExchangeSpineDirection { Down = "abwaerts", Up = "aufwaerts" }
);

impl ExchangeCoverBlockKind {
    /// Name in the database (`cover_block.kind`, migration 0045) and in RPC/serde.
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

    /// Variant for the database name; unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }

    /// Block on the spine (only there does `richtung` exist).
    pub fn is_spine(self) -> bool {
        self.as_db().starts_with("spine_")
    }

    /// Own text (`<text>`) only for the free texts and the motto.
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
    /// Value in `revision_amend.kind` (migration 0050) – the same words as in the XML.
    pub fn as_db(self) -> &'static str {
        self.as_xml()
    }

    /// Variant for the database value; unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::from_xml(s)
    }
}

impl ExchangeFindingKind {
    /// Value in `finding.kind` (migration 0046).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeFindingKind::Praise => "praise",
            ExchangeFindingKind::Hint => "hint",
            ExchangeFindingKind::Critical => "critical",
        }
    }

    /// Variant for the database value; unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

impl ExchangeFindingStatus {
    /// Value in `finding.status` or `finding_status.status` (migration 0046).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeFindingStatus::Open => "open",
            ExchangeFindingStatus::Adopted => "adopted",
            ExchangeFindingStatus::Rejected => "rejected",
            ExchangeFindingStatus::Deferred => "deferred",
            ExchangeFindingStatus::Done => "done",
        }
    }

    /// Variant for the database value; unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

impl ExchangeDecision {
    /// Value in `finding_status.decision` (migration 0046).
    pub fn as_db(self) -> &'static str {
        match self {
            ExchangeDecision::Confirmed => "confirmed",
            ExchangeDecision::Rejected => "rejected",
            ExchangeDecision::Stale => "stale",
        }
    }

    /// Variant for the database value; unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

impl ExchangeTombstoneKind {
    /// Value in `uuid_tombstone.kind` (migration 0041).
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

    /// First sub-version that knows this kind: `verbindung` and `lage` from 2, `umschlag-block` from 3, `befund`,
    /// `befund-status` and `beitrag-fassung` from 4, `satz` from 6, `glossar-begriff` and `register` from 7,
    /// `fortschreibung` from 8, all others from 1. A tombstone of a kind that its file should not know yet is an
    /// error (contract R25 §4.6).
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

    /// Variant for the value in `uuid_tombstone.kind`; unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_db() == s)
    }
}

/// A whole exchange file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeDocument {
    /// Sub-version from `version` on `<werk>`.
    pub version: u32,
    /// Time of the output (`<ausgabe zeit>`).
    pub exported: String,
    pub instance: ExchangeInstance,
    /// Program and version (`<ausgabe programm>`).
    pub generator: String,
    pub work: ExchangeWork,
    /// Unknown elements and attributes as a path (e.g. `werk/angaben/farbe`); no error, nothing taken over.
    pub unknown: Vec<String>,
}

/// Issuing instance.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeInstance {
    pub uuid: String,
    /// Empty = without a name.
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
    /// Work language (`sprache` at `<angaben>`, sub-version 10); files up to sub-version 9 read as German.
    pub language: ExchangeLanguage,
    pub created: String,
    pub updated: Option<String>,
    /// Outline scheme (`work.numbering_scheme`).
    pub numbering_scheme: String,
    /// Own pattern (`<muster>`), only if needed.
    pub numbering: Option<String>,
    pub bible_module: Option<String>,
    pub sword_sources: Vec<ExchangeSwordSource>,
    pub timing: Option<ExchangeTiming>,
    pub book: Option<ExchangeBook>,
    pub template: ExchangeTemplate,
    /// Roots of the outline in order.
    pub outline: Vec<ExchangeNode>,
    pub ideas: Vec<ExchangeIdea>,
    pub threads: Vec<ExchangeThread>,
    pub assets: Vec<ExchangeAsset>,
    pub deleted: Vec<ExchangeTombstone>,
    /// Idea map (`<karte>`, from sub-version 2); empty = without `<karte>`.
    pub map: ExchangeMap,
    /// Typeset view settings (`<satzbild>` in `<angaben>`, from sub-version 3; table `work_sheet`).
    pub sheet: Option<ExchangeSheet>,
    /// Findings with status history (`<befunde>`, from sub-version 4; tables `finding`, `finding_status`).
    pub findings: Vec<ExchangeFinding>,
    /// Typesetting settings (`<satz>` in `<angaben>`, from sub-version 6; table `work_typeset`). `None` = none
    /// (everything from the work template).
    pub typeset: Option<ExchangeTypeset>,
    /// Own indexes in their order (`<eigene-register>`, from sub-version 7; table `index_register`). The built-in
    /// indexes (stichwort, personen, bibel) are never in the file.
    pub registers: Vec<ExchangeRegister>,
    /// Glossary: accepted and proposed terms (`<glossar>`, from sub-version 7; tables `glossary_term`,
    /// `glossary_variant`). Index proposals (`index_proposal`) are workflow of the instance and not in the format.
    pub glossary: Vec<ExchangeGlossaryTerm>,
}

/// An own index (Round 31, table `index_register`; in the format from sub-version 7): identifier, title, creation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeRegister {
    pub uuid: String,
    pub slug: String,
    pub title: String,
    pub created: String,
}

/// A glossary term (Round 31, tables `glossary_term`, `glossary_variant`; in the format from sub-version 7). The
/// Bible references are text, as stored (normalised on creation).
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
    /// Origin: name of the human or handle of the persona or of the MCP client.
    pub origin: String,
    pub created: String,
    pub updated: Option<String>,
    pub updated_by: String,
}

/// Typesetting settings of the work (Round 29, table `work_typeset`; in the format from sub-version 6):
/// identifier, last change (time as stored, name of the human) and the settings themselves. The XML words per
/// field are in [`crate::typeset`]; the values are checked only on import (`TypesetSettings::validate`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTypeset {
    pub uuid: String,
    pub updated: String,
    pub updated_by: String,
    pub settings: baruch_typst::typeset::TypesetSettings,
}

/// Old version of an edited post or thread post (`<alte-fassung>`, from sub-version 4; table
/// `post_version`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePostVersion {
    pub uuid: String,
    pub body: String,
    /// Since when this version applied (creation of the post or the previous change).
    pub valid_from: String,
    /// When it was replaced.
    pub replaced: String,
    /// Who edited (author or `@mcp:<name>`).
    pub edited_by: String,
}

/// A finding (`<befund>` in `<befunde>`, from sub-version 4; table `finding`). Origin through UUIDs: item, post
/// or thread post (never both); revision, target and similarity likewise through UUIDs (revision and target
/// without a guarantee that they still exist – as in the database without foreign keys).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeFinding {
    pub uuid: String,
    /// UUID of the item (`punkt`); missing for threads and deleted sections.
    pub node: Option<String>,
    /// UUID of the post in the section chat (`beitrag`).
    pub post: Option<String>,
    /// UUID of the thread post (`faden-beitrag`).
    pub thread_post: Option<String>,
    /// Position in the answer (from 0).
    pub position: u32,
    pub persona: String,
    pub kind: ExchangeFindingKind,
    pub body: String,
    pub quote: String,
    pub quote_check: String,
    pub status: ExchangeFindingStatus,
    pub reason: String,
    /// UUID of the revision ("adopted").
    pub adopted_revision: Option<String>,
    /// UUID of the target item ("moved").
    pub deferred_node: Option<String>,
    /// UUID of the target idea ("moved").
    pub deferred_idea: Option<String>,
    /// UUID of the finding this one resembles (attached).
    pub similar_to: Option<String>,
    /// UUIDs of the findings that a collective proposal summarises.
    pub summary_of: Vec<String>,
    pub created: String,
    pub updated: String,
    /// Status history, oldest first (`<status-eintrag>`).
    pub history: Vec<ExchangeFindingEntry>,
    /// Proposed solutions (`<loesung>`, from sub-version 5; `finding.solutions`).
    pub solutions: Vec<ExchangeSolution>,
    /// State of the adopted revision (`stand`, from sub-version 8; `finding.adopted_amend`).
    pub adopted_amend: Option<u32>,
}

/// A proposed solution for a finding (`<loesung zitat ersatz schritt/>`, from sub-version 5): quotation with new
/// wording or a step.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeSolution {
    pub quote: String,
    pub replacement: String,
    pub step: String,
}

/// Marker of a variant on a revision (from sub-version 5; `revision.variant_*`): group (UUID, the same for all
/// siblings), label A/B/C, one line and the implemented findings (UUIDs, `<setzt-um befund>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeVariant {
    pub group: String,
    pub label: String,
    pub summary: String,
    pub findings: Vec<String>,
}

/// Labels of a variant.
pub const VARIANT_LABELS: [&str; 3] = ["A", "B", "C"];
/// Longest line of a variant or review basis of a post (like the CHECKs of migration 0047).
pub const MAX_VARIANT_SUMMARY_CHARS: usize = 300;
pub const MAX_BASIS_LABEL_CHARS: usize = 300;
pub const MAX_RELAY_FROM_CHARS: usize = 200;

/// Metadata of a post or thread post (from sub-version 5; columns from migration 0047): reviewed revision (UUID,
/// only for posts of a section), header line, reply to and repetition of (UUIDs of posts of the same place),
/// relay from persona. The order (`request`) stays local.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangePostMeta {
    pub basis_revision: Option<String>,
    pub basis_label: String,
    pub reply_to: Option<String>,
    pub repeat_of: Option<String>,
    pub relay_from: Option<String>,
}

impl ExchangePostMeta {
    /// No metadata.
    pub fn is_empty(&self) -> bool {
        *self == ExchangePostMeta::default()
    }
}

/// An entry in the status history (`<status-eintrag>`, table `finding_status`): status change by the human or
/// proposal of an assistant (`proposed`) including the decision.
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
    /// State of the adopted revision (`stand`, from sub-version 8; `finding_status.adopted_amend`).
    pub adopted_amend: Option<u32>,
}

/// Limits of a finding (like the CHECKs of migration 0046).
pub const MAX_FINDING_CHARS: usize = 4000;
pub const MAX_FINDING_QUOTE_CHARS: usize = 2000;
pub const MAX_FINDING_CHECK_CHARS: usize = 200;
pub const MAX_FINDING_REASON_CHARS: usize = 2000;
pub const MAX_FINDING_PERSONA_CHARS: usize = 200;

/// Typeset view settings of the work (Round 18c, table `work_sheet`; in the format from sub-version 3). `None` =
/// from the work template. Values as stored: font lists, lengths with unit (`11pt`, `16cm`), colours `#rrggbb`.
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
    /// All fields `None` (then the writer writes no `<satzbild>`).
    pub fn is_empty(&self) -> bool {
        *self == ExchangeSheet::default()
    }
}

/// A block of the cover layout (`<block>` in `<umschlag>`, from sub-version 3; table `cover_block`).
/// Coordinates in mm from the trim edge of its own area (contract R27 1.2).
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
    /// As stored: `light`, `dark` or `#rrggbb` (in the XML `hell`, `dunkel` or `#rrggbb`).
    pub color: String,
    /// `None` = font of the work template.
    pub font: Option<String>,
    pub style: ExchangeFontStyle,
    pub backdrop: ExchangeBackdrop,
    pub visible: bool,
    /// Only on the spine.
    pub direction: Option<ExchangeSpineDirection>,
    /// Only for kinds with their own text.
    pub text: Option<String>,
    pub updated: String,
}

/// Limits of a cover block (like `baruch_typst::cover_layout` and the CHECKs of migration 0045).
pub const COVER_POS_MM: (f64, f64) = (-50.0, 1000.0);
pub const COVER_WIDTH_MM: (f64, f64) = (1.0, 1000.0);
pub const COVER_SIZE_PT: (f64, f64) = (4.0, 200.0);
pub const COVER_LEADING_EM: (f64, f64) = (0.0, 3.0);
pub const COVER_MAX_TEXT_CHARS: usize = 1000;
pub const COVER_MAX_FONT_CHARS: usize = 200;
/// Maximum length of the addresses (characters).
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
    /// UUID of the image (not of the revision).
    pub cover: Option<String>,
    /// UUID of the image (not of the revision).
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
    /// Address of the publisher or editor (`<verlag-anschrift>`, from sub-version 3).
    pub publisher_address: String,
    /// Address of the printer (`<druckerei-anschrift>`, from sub-version 3).
    pub printer_address: String,
    /// Cover layout (`<umschlag>`, from sub-version 3); empty = without a layout of its own.
    pub cover_blocks: Vec<ExchangeCoverBlock>,
}

/// Work template: current text (`work.template`) and all revisions.
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

/// Outline item with all revisions, posts and children.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeNode {
    pub uuid: String,
    pub kind: ExchangeNodeKind,
    pub numbered: bool,
    pub title: String,
    /// Fixed key of the item in the issuing instance (`schluessel`, from sub-version 5): cross-references
    /// `#link(<kN>)` in the text name it; on import they are converted to the keys here.
    pub key: Option<i64>,
    pub revisions: Vec<ExchangeRevision>,
    pub posts: Vec<ExchangePost>,
    pub children: Vec<ExchangeNode>,
}

/// Originator of a revision (`autor-art`, `autor`, `modell`).
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
    /// Number in the issuing instance.
    pub number: u32,
    pub status: ExchangeRevisionStatus,
    pub author: ExchangeAuthor,
    /// UUID of the parent revision in the same item.
    pub based_on: Option<String>,
    pub note: String,
    pub body: String,
    pub created: String,
    pub edited: Option<String>,
    /// Variant (from sub-version 5); `None` = none.
    pub variant: Option<ExchangeVariant>,
    /// State of the revision (`stand`, from sub-version 8; `revision.amend_seq`): number of its amendments, 0 = never.
    pub amend_seq: u32,
    /// State of the base a proposal rests on (`beruht-stand`, from sub-version 8; `revision.based_on_amend`).
    pub based_on_amend: Option<u32>,
    /// History of the amendments (`<fortschreibung>`, from sub-version 8; table `revision_amend`), by `seq`.
    pub amends: Vec<ExchangeAmend>,
}

/// A step in the history of an amended revision (Round 22, table `revision_amend`; in the format from
/// sub-version 8): amendment by the human or undo through the history (`undo_of` = the step that was undone).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExchangeAmend {
    pub uuid: String,
    /// Step from 1, without gaps per revision.
    pub seq: u32,
    pub kind: ExchangeAmendKind,
    pub undo_of: Option<u32>,
    pub author: String,
    pub created: String,
    /// Text before the step.
    pub before: String,
    /// Text after the step.
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
    /// Time of the last edit (`geaendert`, from sub-version 4); `None` = never edited.
    pub edited: Option<String>,
    /// Old versions, oldest first (`<alte-fassung>`, from sub-version 4).
    pub versions: Vec<ExchangePostVersion>,
    /// Metadata (from sub-version 5).
    pub meta: ExchangePostMeta,
}

/// Reference of a post to an item (and a revision) with quotation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePostRef {
    pub node: String,
    pub revision: Option<String>,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeIdea {
    pub uuid: String,
    /// UUID of the item (`bezug`).
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
    /// Soft-deleted since this time.
    pub deleted: Option<String>,
    /// From sub-version 9 (Round 34): title of the node in the mind map (`titel`; missing = empty).
    pub title: String,
    /// UUID of the parent idea (`eltern`).
    pub parent: Option<String>,
    /// Order among siblings (`platz`; missing = 0).
    pub position: i64,
    /// Free main topic at the work (`hauptthema`; missing = no).
    pub main_topic: bool,
}

/// Thread without a section.
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
    /// Time of the last edit (`geaendert`, from sub-version 4); `None` = never edited.
    pub edited: Option<String>,
    /// Old versions, oldest first (`<alte-fassung>`, from sub-version 4).
    pub versions: Vec<ExchangePostVersion>,
    /// Metadata (from sub-version 5; `basis_revision` stays empty, a thread has no revisions).
    pub meta: ExchangePostMeta,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeAsset {
    pub uuid: String,
    pub name: String,
    pub created: String,
    /// Paths under which the work names the image.
    pub paths: Vec<String>,
    pub revisions: Vec<ExchangeAssetRevision>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeAssetRevision {
    pub uuid: String,
    pub number: u32,
    /// UUID of the parent revision in the same image.
    pub based_on: Option<String>,
    pub mime: String,
    /// SHA-256 of `data` (lower-case hex).
    pub sha256: String,
    /// Raw bytes (SVG as UTF-8).
    pub data: Vec<u8>,
    pub caption: String,
    pub alt: String,
    pub author: String,
    pub note: String,
    pub status: ExchangeAssetStatus,
    pub created: String,
    pub edited: Option<String>,
}

/// Tombstone: a row with UUID deleted here (table `uuid_tombstone`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeTombstone {
    pub uuid: String,
    pub kind: ExchangeTombstoneKind,
    pub deleted: String,
}

/// Link between two ideas of this work (`<verbindung>` in `<karte>`, table `idea_link`).
/// `from`/`to` are idea UUIDs; „widerspricht“ is undirected (the order carries no meaning).
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
    /// Soft-deleted since this time.
    pub deleted: Option<String>,
}

/// Stored position of an idea on the map of this work (`<lage>`, table `idea_map_pos`), in SVG user units of the
/// map (work at 0,0).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangePosition {
    pub uuid: String,
    /// UUID of the idea.
    pub idea: String,
    pub x: f64,
    pub y: f64,
    pub updated: String,
}

/// Content of `<karte>`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExchangeMap {
    pub links: Vec<ExchangeLink>,
    pub positions: Vec<ExchangePosition>,
}

impl ExchangeMap {
    /// Without link and position (then the writer writes no `<karte>`).
    pub fn is_empty(&self) -> bool {
        self.links.is_empty() && self.positions.is_empty()
    }
}

/// Highest absolute value of a coordinate in `<lage>` (like `baruch_store::MAP_COORD_LIMIT`).
pub const MAP_COORD_LIMIT: f64 = 1_000_000.0;

/// Maximum length of the note of a link in characters (like `baruch_store::MAX_LINK_NOTE_CHARS`).
pub const MAX_LINK_NOTE_CHARS: usize = 500;

/// Entry in `MANIFEST.xml`.
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
        // Canonical, lower case, any version: v4 and v8 are valid.
        assert!(is_uuid("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6"), "v4 klein");
        assert!(is_uuid("0f8c2a4e-1b3d-8c5e-9f60-718293a4b5c6"), "v8 klein");
        // Upper-case letters, missing hyphens, wrong length and foreign characters are not canonical.
        assert!(!is_uuid("0F8C2A4E-1B3D-4C5E-9F60-718293A4B5C6"), "groß ist nicht kanonisch");
        assert!(!is_uuid("0f8c2a4e1b3d4c5e9f60718293a4b5c6"), "ohne Striche nicht kanonisch");
        assert!(!is_uuid("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c"), "zu kurz");
        assert!(!is_uuid("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5cg"), "g ist kein Hex");
    }

    #[test]
    fn xml_words_round_trip() {
        // Every word leads back to the same variant; otherwise the reader would reject what was written.
        for kind in ExchangeTombstoneKind::ALL {
            assert_eq!(ExchangeTombstoneKind::from_xml(kind.as_xml()), Some(*kind), "XML-Wort {kind:?}");
            assert_eq!(ExchangeTombstoneKind::from_db(kind.as_db()), Some(*kind), "Datenbankwert {kind:?}");
        }
        for kind in ExchangeLinkKind::ALL {
            assert_eq!(ExchangeLinkKind::from_xml(kind.as_xml()), Some(*kind), "Verbindungsart {kind:?}");
        }
        // The database values of the new tombstone kinds are those from migration 0043.
        assert_eq!(ExchangeTombstoneKind::Link.as_db(), "idea_link", "Grabstein einer Verbindung");
        assert_eq!(ExchangeTombstoneKind::Position.as_db(), "idea_map_pos", "Grabstein einer Lage");
        // Only the kinds of the map came with sub-version 2; a reader rejects them in sub-version 1.
        let since: Vec<u32> = ExchangeTombstoneKind::ALL.iter().map(|k| k.since_version()).collect();
        assert_eq!(since, [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 4, 4, 4, 6, 7, 7, 8], "Unterversion je Grabsteinart");
        // Round 22: tombstone of an amendment (migration 0050).
        assert_eq!(ExchangeTombstoneKind::Amend.as_db(), "revision_amend", "Datenbankwert der Migration 0050");
        assert_eq!(ExchangeTombstoneKind::Amend.as_xml(), "fortschreibung", "XML-Wort laut Vertrag R22 §2.11");
        for kind in ExchangeAmendKind::ALL {
            assert_eq!(ExchangeAmendKind::from_db(kind.as_db()), Some(*kind), "Art der Fortschreibung {kind:?}");
        }
        // Round 31: tombstones of deleted glossary terms and own indexes (migration 0049).
        assert_eq!(ExchangeTombstoneKind::GlossaryTerm.as_db(), "glossary_term", "Datenbankwert der Migration 0049");
        assert_eq!(ExchangeTombstoneKind::Register.as_db(), "index_register", "Datenbankwert der Migration 0049");
        assert_eq!(ExchangeTombstoneKind::GlossaryTerm.as_xml(), "glossar-begriff", "XML-Wort laut Vertrag R31 §5");
        assert_eq!(ExchangeTombstoneKind::Register.as_xml(), "register", "XML-Wort laut Vertrag R31 §5");
        // Round 29: tombstone of reset typesetting settings (migration 0048).
        assert_eq!(ExchangeTombstoneKind::Typeset.as_db(), "work_typeset", "Datenbankwert der Migration 0048");
        assert_eq!(ExchangeTombstoneKind::Typeset.as_xml(), "satz", "XML-Wort laut Vertrag R29 §6.6");
        // Round 28: tombstones of the findings, their history and old post versions (migration 0046).
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
        // Round 27: tombstone of a cover block, database value from migration 0045.
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
        // The default is the first variant (contract S1 (4)).
        assert_eq!(ExchangeNodeKind::default(), ExchangeNodeKind::Part, "Vorgabe = erste Variante");
        assert_eq!(
            ExchangeAuthor::default(),
            ExchangeAuthor::Human { name: String::new() },
            "Vorgabe Mensch ohne Namen"
        );
    }

    #[test]
    fn image_entries_follow_the_container_layout() {
        // bilder/<fassungs-uuid>.<ext> with ext from png|jpg|gif|webp|svg (F1).
        assert_eq!(
            image_entry("0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6", "image/jpeg").as_deref(),
            Some("bilder/0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6.jpg"),
            "JPEG endet auf .jpg"
        );
        assert_eq!(image_extension("image/svg+xml"), Some("svg"), "SVG");
        assert_eq!(image_extension("image/bmp"), None, "andere Typen nimmt das Format nicht an");
        // Known check value of SHA-256 over the empty input.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "SHA-256 als Kleinbuchstaben-Hex"
        );
    }
}
