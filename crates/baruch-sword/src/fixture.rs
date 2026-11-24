// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kleine SWORD-Module für Tests schreiben (zText mit ZIP in BOOK/CHAPTER/VERSE, RawText; seit Runde 15
//! auch Kommentare RawCom/zCom und Lexika RawLD/RawLD4/zLD über [`LexiconFixture`]), mit frei
//! erfundenem Text. Die Index-Stellen rechnet dieses Modul **selbst** aus der Verszählung von
//! `baruch-scripture` aus ([`slot`]); gelesen wird über `sword-modules`. Stimmen beide überein, ist die
//! Reihenfolge belegt.
//!
//! Index je Testament: Stelle 0 = Modulkopf, 1 = Testamentkopf, dann je Buch eine Stelle für die
//! Bucheinleitung (Kapitel 0), je Kapitel eine für die Kapiteleinleitung (Vers 0) und eine je Vers.
//!
//! Runde 17 (R17c): Blöcke auch mit BZIP2 und XZ ([`Fixture::compression`]) und **verschlüsselt**
//! ([`Fixture::enciphered`]): Sapphire II nach der gemeinfreien Beschreibung (Michael Paul Johnson 1995, in
//! `sword-modules` `docs/spec/cipher.md`), hier selbst umgesetzt; die conf bekommt `CipherKey=` leer, wie ein
//! gesperrtes Modul vom Herausgeber. Der Schlüssel eines Testmoduls ist frei erfunden.
//!
//! Runde 19c: [`hyphen_remnants`] baut in erfundenen Text Silbentrennreste ein („Men schen“, „die ses“), wie sie
//! ein aus einem PDF gebautes Modul hat – für Tests der toleranten Wortlautprüfung und des Hinweises auf
//! Wortfragmente. Fremde Module (auch das Prüfmodul des Nutzers) liest kein Test.

use std::io::Write as _;
use std::path::Path;

use baruch_scripture::versification::{OLD_TESTAMENT_BOOKS, Versification};

/// Runde 19c: Trennreste in erfundenen Text einbauen. Jedes `every`-te Wort mit mindestens sechs Buchstaben (ohne
/// Satzzeichen gezählt) wird in der Mitte durch ein Leerzeichen getrennt; der hintere Teil bleibt klein
/// („Menschen“ → „Mens chen“, „dieses“ → „die ses“). `every` = 1: jedes solche Wort. Satzzeichen bleiben am Wort.
pub fn hyphen_remnants(text: &str, every: usize) -> String {
    let every = every.max(1);
    let mut seen = 0usize;
    let mut out: Vec<String> = Vec::new();
    for token in text.split(' ') {
        let core: Vec<char> = token.chars().filter(|c| c.is_alphabetic()).collect();
        let only_letters_inside = token.trim_matches(|c: char| !c.is_alphabetic()).chars().all(char::is_alphabetic);
        if core.len() >= 6 && only_letters_inside {
            seen += 1;
            if seen.is_multiple_of(every) {
                let lead = token.chars().take_while(|c| !c.is_alphabetic()).count();
                let cut = lead + core.len() / 2;
                let head: String = token.chars().take(cut).collect();
                let tail: String = token.chars().skip(cut).collect();
                out.push(format!("{head} {}", tail.to_lowercase()));
                continue;
            }
        }
        out.push(token.to_owned());
    }
    out.join(" ")
}

/// Speicherart des Test-Moduls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `zText`, `CompressType=ZIP`, Blöcke je Buch, Kapitel oder Vers.
    ZText(Block),
    /// `RawText` ohne Kompression (`.vss` mit 6 Bytes je Stelle).
    RawText,
    /// Kommentar `RawCom` (gleiche Dateien wie RawText).
    RawCom,
    /// Kommentar `zCom` (gleiche Dateien wie zText).
    ZCom(Block),
}

impl Layout {
    /// `Some(Block)` für die komprimierten Arten.
    fn block(self) -> Option<Block> {
        match self {
            Layout::ZText(b) | Layout::ZCom(b) => Some(b),
            Layout::RawText | Layout::RawCom => None,
        }
    }
}

/// Kompression der Blöcke von zText/zCom (R17c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    /// `CompressType=ZIP` (zlib), Vorgabe.
    #[default]
    Zip,
    /// `CompressType=BZIP2`.
    Bzip2,
    /// `CompressType=XZ`.
    Xz,
}

impl Compression {
    fn conf_value(self) -> &'static str {
        match self {
            Compression::Zip => "ZIP",
            Compression::Bzip2 => "BZIP2",
            Compression::Xz => "XZ",
        }
    }

    /// Einen Block packen wie [`write`] (öffentlich für Tests, die einen Block selbst ersetzen).
    pub fn pack(self, body: &[u8]) -> std::io::Result<Vec<u8>> {
        match self {
            Compression::Zip => {
                let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(body)?;
                enc.finish()
            }
            Compression::Bzip2 => {
                let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
                enc.write_all(body)?;
                enc.finish()
            }
            Compression::Xz => {
                let mut out = Vec::new();
                lzma_rs::xz_compress(&mut std::io::Cursor::new(body), &mut out)?;
                Ok(out)
            }
        }
    }
}

/// Sapphire II (gemeinfrei, Michael Paul Johnson 1995), nach der Beschreibung in `sword-modules`
/// `docs/spec/cipher.md` selbst umgesetzt: nur zum **Verschlüsseln** eigener Testmodule.
struct Sapphire {
    cards: [u8; 256],
    rotor: u8,
    ratchet: u8,
    avalanche: u8,
    last_plain: u8,
    last_cipher: u8,
}

impl Sapphire {
    fn new(key: &[u8]) -> Self {
        let key = &key[..key.len().min(255)];
        let mut cards = [0u8; 256];
        if key.is_empty() {
            for (i, c) in cards.iter_mut().enumerate() {
                *c = 255 - i as u8;
            }
            return Sapphire { cards, rotor: 1, ratchet: 3, avalanche: 5, last_plain: 7, last_cipher: 11 };
        }
        for (i, c) in cards.iter_mut().enumerate() {
            *c = i as u8;
        }
        let (mut rsum, mut keypos) = (0u8, 0usize);
        for i in (0..=255u8).rev() {
            // keyrand(i)
            let t = if i == 0 {
                0
            } else {
                let mut mask = 1u8;
                while mask < i {
                    mask = (mask << 1) | 1;
                }
                let mut attempts = 0;
                loop {
                    rsum = cards[usize::from(rsum)].wrapping_add(key[keypos]);
                    keypos += 1;
                    if keypos >= key.len() {
                        keypos = 0;
                        rsum = rsum.wrapping_add(key.len() as u8);
                    }
                    let mut u = rsum & mask;
                    attempts += 1;
                    if attempts > 11 {
                        u %= i;
                    }
                    if u <= i {
                        break u;
                    }
                }
            };
            cards.swap(usize::from(i), usize::from(t));
        }
        Sapphire {
            rotor: cards[1],
            ratchet: cards[3],
            avalanche: cards[5],
            last_plain: cards[7],
            last_cipher: cards[usize::from(rsum)],
            cards,
        }
    }

    fn c(&self, i: u8) -> u8 {
        self.cards[usize::from(i)]
    }

    fn encipher(&mut self, p: u8) -> u8 {
        self.ratchet = self.ratchet.wrapping_add(self.c(self.rotor));
        self.rotor = self.rotor.wrapping_add(1);
        let x = self.c(self.last_cipher);
        self.cards[usize::from(self.last_cipher)] = self.c(self.ratchet);
        self.cards[usize::from(self.ratchet)] = self.c(self.last_plain);
        self.cards[usize::from(self.last_plain)] = self.c(self.rotor);
        self.cards[usize::from(self.rotor)] = x;
        self.avalanche = self.avalanche.wrapping_add(self.c(x));
        let a = self.c(self.c(self.ratchet).wrapping_add(self.c(self.rotor)));
        let inner = self.c(self.last_plain).wrapping_add(self.c(self.last_cipher)).wrapping_add(self.c(self.avalanche));
        let out = p ^ a ^ self.c(self.c(inner));
        self.last_plain = p;
        self.last_cipher = out;
        out
    }
}

/// `data` mit `key` verschlüsseln (je Block bzw. Eintrag neu ab dem Schlüssel, wie SWORD).
pub fn encipher(key: &str, data: &[u8]) -> Vec<u8> {
    let mut s = Sapphire::new(key.as_bytes());
    data.iter().map(|&b| s.encipher(b)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Book,
    Chapter,
    Verse,
}

/// Wo ein Eintrag steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Modulkopf (Stelle 0) des Testaments (0 = AT, 1 = NT).
    Module(usize),
    /// Testamentkopf (Stelle 1).
    Testament(usize),
    /// Buch (OSIS), Kapitel (0 = Bucheinleitung), Vers (0 = Kapiteleinleitung).
    Verse(&'static str, u16, u16),
}

/// Testament (0 = AT, 1 = NT) und Index-Stelle eines Schlüssels; `None`, wenn es ihn in der Zählung nicht gibt.
pub fn slot(v11n: &Versification, key: &Key) -> Option<(usize, u32)> {
    match key {
        Key::Module(t) => Some((*t, 0)),
        Key::Testament(t) => Some((*t, 1)),
        Key::Verse(book, chapter, verse) => {
            let index = v11n.books().position(|(id, _)| id == *book)?;
            let testament = usize::from(index >= OLD_TESTAMENT_BOOKS);
            let first = if testament == 0 { 0 } else { OLD_TESTAMENT_BOOKS };
            let mut slot: u32 = 2;
            for (_, chapters) in v11n.books().skip(first).take(index - first) {
                slot += 1 + chapters.iter().map(|&v| 1 + u32::from(v)).sum::<u32>();
            }
            let chapters = v11n.chapters_of(book)?;
            if *chapter == 0 {
                return (*verse == 0).then_some((testament, slot));
            }
            slot += 1;
            let before = chapters.get(..usize::from(*chapter) - 1)?;
            slot += before.iter().map(|&v| 1 + u32::from(v)).sum::<u32>();
            if *verse > *chapters.get(usize::from(*chapter) - 1)? {
                return None;
            }
            Some((testament, slot + u32::from(*verse)))
        }
    }
}

/// Zahl der Stellen eines Testaments.
pub fn slot_count(v11n: &Versification, testament: usize) -> u32 {
    let (skip, take) = if testament == 0 { (0, OLD_TESTAMENT_BOOKS) } else { (OLD_TESTAMENT_BOOKS, usize::MAX) };
    2 + v11n
        .books()
        .skip(skip)
        .take(take)
        .map(|(_, c)| 1 + c.iter().map(|&v| 1 + u32::from(v)).sum::<u32>())
        .sum::<u32>()
}

/// Ein Test-Modul: Name, Speicherart, Verszählung, zusätzliche conf-Zeilen und Einträge.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub name: String,
    pub layout: Layout,
    pub versification: &'static Versification,
    /// Weitere Zeilen der conf (`Description=…`, `DistributionLicense=…`, …); `DataPath`, `ModDrv`,
    /// `CompressType`, `BlockType`, `Versification`, `Encoding` und `SourceType` setzt [`write`].
    pub conf: Vec<(String, String)>,
    pub entries: Vec<(Key, String)>,
    /// Kompression der Blöcke (nur zText/zCom; R17c).
    pub compression: Compression,
    /// Schlüssel, mit dem [`write`] Blöcke bzw. Einträge verschlüsselt (R17c); die conf bekommt `CipherKey=`.
    pub cipher: Option<String>,
    /// Text in Latin-1 und conf ohne `Encoding` (SWORD-Vorgabe) statt UTF-8 (Nachbesserung Runde 17).
    pub latin1: bool,
}

impl Fixture {
    pub fn new(name: &str, layout: Layout, versification: &'static Versification) -> Self {
        Fixture {
            name: name.to_owned(),
            layout,
            versification,
            conf: Vec::new(),
            entries: Vec::new(),
            compression: Compression::Zip,
            cipher: None,
            latin1: false,
        }
    }

    /// Blöcke mit `c` komprimieren (zText/zCom).
    pub fn compression(mut self, c: Compression) -> Self {
        self.compression = c;
        self
    }

    /// Blöcke (zText/zCom) bzw. Einträge (RawText/RawCom) mit Sapphire II und `key` verschlüsseln; die conf
    /// trägt `CipherKey=` leer (gesperrt, bis der Nutzer den Schlüssel hinterlegt).
    pub fn enciphered(mut self, key: &str) -> Self {
        self.cipher = Some(key.to_owned());
        self
    }

    /// Text in Latin-1 speichern, conf ohne `Encoding` (Vorgabe von SWORD). Zeichen jenseits von U+00FF: Panik.
    pub fn latin1(mut self) -> Self {
        self.latin1 = true;
        self
    }

    /// Bytes eines Eintrags in der Kodierung des Moduls.
    fn encode(&self, text: &str) -> Vec<u8> {
        if self.latin1 {
            text.chars()
                .map(|c| u8::try_from(u32::from(c)).unwrap_or_else(|_| panic!("„{c}“ gibt es in Latin-1 nicht")))
                .collect()
        } else {
            text.as_bytes().to_vec()
        }
    }

    pub fn conf(mut self, key: &str, value: &str) -> Self {
        self.conf.push((key.to_owned(), value.to_owned()));
        self
    }

    pub fn verse(mut self, book: &'static str, chapter: u16, verse: u16, text: &str) -> Self {
        self.entries.push((Key::Verse(book, chapter, verse), text.to_owned()));
        self
    }

    pub fn entry(mut self, key: Key, text: &str) -> Self {
        self.entries.push((key, text.to_owned()));
        self
    }

    /// Ordner der Moduldaten relativ zum Modulordner.
    pub fn data_path(&self) -> String {
        let (group, kind) = match self.layout {
            Layout::ZText(_) => ("texts", "ztext"),
            Layout::RawText => ("texts", "rawtext"),
            Layout::RawCom => ("comments", "rawcom"),
            Layout::ZCom(_) => ("comments", "zcom"),
        };
        format!("./modules/{group}/{kind}/{}/", self.name.to_lowercase())
    }
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn u16le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Schreibt `mods.d/<name>.conf` und die Moduldaten unter `root`. Panik bei unbekanntem Schlüssel
/// (Testfehler).
pub fn write(root: &Path, m: &Fixture) -> std::io::Result<()> {
    let data_path = m.data_path();
    let dir = root.join(data_path.trim_start_matches("./"));
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(root.join("mods.d"))?;
    let driver = match m.layout {
        Layout::ZText(_) => "zText",
        Layout::RawText => "RawText",
        Layout::RawCom => "RawCom",
        Layout::ZCom(_) => "zCom",
    };
    let extra = match m.layout.block() {
        Some(b) => {
            let block = match b {
                Block::Book => "BOOK",
                Block::Chapter => "CHAPTER",
                Block::Verse => "VERSE",
            };
            format!("CompressType={}\nBlockType={block}\n", m.compression.conf_value())
        }
        None => String::new(),
    };
    let extra = match &m.cipher {
        Some(_) => format!("{extra}CipherKey=\n"),
        None => extra,
    };
    let seal = |bytes: Vec<u8>| match &m.cipher {
        Some(key) => encipher(key, &bytes),
        None => bytes,
    };
    let mut conf = format!(
        "[{}]\nDataPath={data_path}\nModDrv={driver}\n{extra}SourceType=OSIS\n{}Versification={}\n",
        m.name,
        if m.latin1 { "" } else { "Encoding=UTF-8\n" },
        m.versification.name
    );
    for (k, v) in &m.conf {
        conf.push_str(&format!("{k}={v}\n"));
    }
    std::fs::write(root.join("mods.d").join(format!("{}.conf", m.name.to_lowercase())), conf)?;

    for (testament, prefix) in [(0usize, "ot"), (1, "nt")] {
        let count = slot_count(m.versification, testament);
        let mut texts: Vec<Option<(&Key, &str)>> = vec![None; count as usize];
        for (key, text) in &m.entries {
            let (t, s) = slot(m.versification, key).unwrap_or_else(|| panic!("Schlüssel {key:?} gibt es nicht"));
            if t == testament {
                texts[s as usize] = Some((key, text.as_str()));
            }
        }
        match m.layout.block() {
            None => {
                let (mut index, mut data) = (Vec::new(), Vec::new());
                for entry in &texts {
                    match entry {
                        Some((_, text)) => {
                            let stored = seal(m.encode(text));
                            u32le(&mut index, data.len() as u32);
                            u16le(&mut index, stored.len() as u16);
                            data.extend_from_slice(&stored);
                        }
                        None => {
                            u32le(&mut index, 0);
                            u16le(&mut index, 0);
                        }
                    }
                }
                std::fs::write(dir.join(format!("{prefix}.vss")), index)?;
                std::fs::write(dir.join(prefix), data)?;
            }
            Some(block) => {
                let letter = match block {
                    Block::Book => 'b',
                    Block::Chapter => 'c',
                    Block::Verse => 'v',
                };
                // Blöcke nach Buch, Kapitel bzw. Stelle; Köpfe (Stelle 0 und 1) in einen eigenen Block.
                let group = |slot: usize, key: &Key| -> (String, u16, usize) {
                    match key {
                        Key::Verse(b, c, _) => match block {
                            Block::Book => ((*b).to_owned(), 0, 0),
                            Block::Chapter => ((*b).to_owned(), *c, 0),
                            Block::Verse => ((*b).to_owned(), *c, slot),
                        },
                        _ => (String::new(), 0, slot),
                    }
                };
                let mut blocks: Vec<((String, u16, usize), Vec<u8>)> = Vec::new();
                let mut index = Vec::new();
                for (s, entry) in texts.iter().enumerate() {
                    let Some((key, text)) = entry else {
                        u32le(&mut index, 0);
                        u32le(&mut index, 0);
                        u16le(&mut index, 0);
                        continue;
                    };
                    let g = group(s, key);
                    let number = match blocks.iter().position(|(k, _)| *k == g) {
                        Some(n) => n,
                        None => {
                            blocks.push((g, Vec::new()));
                            blocks.len() - 1
                        }
                    };
                    let body = &mut blocks[number].1;
                    u32le(&mut index, number as u32);
                    u32le(&mut index, body.len() as u32);
                    let bytes = m.encode(text);
                    u16le(&mut index, bytes.len() as u16);
                    body.extend_from_slice(&bytes);
                }
                let (mut sizes, mut zz) = (Vec::new(), Vec::new());
                for (_, body) in &blocks {
                    let packed = seal(m.compression.pack(body)?);
                    u32le(&mut sizes, zz.len() as u32);
                    u32le(&mut sizes, packed.len() as u32);
                    u32le(&mut sizes, body.len() as u32);
                    zz.extend_from_slice(&packed);
                }
                std::fs::write(dir.join(format!("{prefix}.{letter}zv")), index)?;
                std::fs::write(dir.join(format!("{prefix}.{letter}zs")), sizes)?;
                std::fs::write(dir.join(format!("{prefix}.{letter}zz")), zz)?;
            }
        }
    }
    Ok(())
}

/// Speicherart eines Test-Lexikons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexLayout {
    /// `RawLD`: `.idx` mit 4 Bytes Stelle und 2 Bytes Länge.
    RawLD,
    /// `RawLD4`: `.idx` mit 4 + 4 Bytes.
    RawLD4,
    /// `zLD` mit ZIP: Einträge in Blöcken (`.zdx`/`.zdt`), `.dat` verweist auf Block und Eintrag.
    ZLD,
}

/// Ein Test-Lexikon: Name, Speicherart, conf-Zeilen, Stichwörter mit Artikeln (frei erfunden). Ein Artikel
/// `@LINK <Stichwort>` verweist auf ein anderes Stichwort.
#[derive(Debug, Clone)]
pub struct LexiconFixture {
    pub name: String,
    pub layout: LexLayout,
    pub conf: Vec<(String, String)>,
    pub entries: Vec<(String, String)>,
    /// Einträge je zLD-Block (Vorgabe 2, damit mehrere Blöcke entstehen).
    pub per_block: usize,
}

impl LexiconFixture {
    pub fn new(name: &str, layout: LexLayout) -> Self {
        LexiconFixture { name: name.to_owned(), layout, conf: Vec::new(), entries: Vec::new(), per_block: 2 }
    }

    pub fn conf(mut self, key: &str, value: &str) -> Self {
        self.conf.push((key.to_owned(), value.to_owned()));
        self
    }

    pub fn article(mut self, key: &str, text: &str) -> Self {
        self.entries.push((key.to_owned(), text.to_owned()));
        self
    }

    /// `DataPath` (Präfix der Dateien) relativ zum Modulordner.
    pub fn data_path(&self) -> String {
        let kind = match self.layout {
            LexLayout::RawLD => "rawld",
            LexLayout::RawLD4 => "rawld4",
            LexLayout::ZLD => "zld",
        };
        let name = self.name.to_lowercase();
        format!("./modules/lexdict/{kind}/{name}/{name}")
    }
}

/// Schreibt `mods.d/<name>.conf` und die Dateien des Lexikons unter `root`. Stichwörter werden wie in
/// SWORD groß geschrieben abgelegt und nach ihren Bytes sortiert.
pub fn write_lexicon(root: &Path, m: &LexiconFixture) -> std::io::Result<()> {
    let data_path = m.data_path();
    let prefix = root.join(data_path.trim_start_matches("./"));
    std::fs::create_dir_all(prefix.parent().expect("Präfix mit Ordner"))?;
    std::fs::create_dir_all(root.join("mods.d"))?;
    let (driver, extra) = match m.layout {
        LexLayout::RawLD => ("RawLD", ""),
        LexLayout::RawLD4 => ("RawLD4", ""),
        LexLayout::ZLD => ("zLD", "CompressType=ZIP\n"),
    };
    let mut conf =
        format!("[{}]\nDataPath={data_path}\nModDrv={driver}\n{extra}SourceType=OSIS\nEncoding=UTF-8\n", m.name);
    for (k, v) in &m.conf {
        conf.push_str(&format!("{k}={v}\n"));
    }
    std::fs::write(root.join("mods.d").join(format!("{}.conf", m.name.to_lowercase())), conf)?;

    let mut entries: Vec<(String, &str)> = m.entries.iter().map(|(k, t)| (k.to_uppercase(), t.as_str())).collect();
    entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let file = |ext: &str| {
        let mut p = prefix.clone().into_os_string();
        p.push(ext);
        std::path::PathBuf::from(p)
    };
    let (mut idx, mut dat) = (Vec::new(), Vec::new());
    let mut blocks: Vec<Vec<(usize, &str)>> = Vec::new();
    for (i, (key, text)) in entries.iter().enumerate() {
        let offset = dat.len() as u32;
        dat.extend_from_slice(key.as_bytes());
        dat.extend_from_slice(b"\r\n");
        match m.layout {
            LexLayout::ZLD if !text.starts_with("@LINK ") => {
                let per = m.per_block.max(1);
                if blocks.last().is_none_or(|b| b.len() >= per) {
                    blocks.push(Vec::new());
                }
                let block = blocks.len() - 1;
                let entry = blocks[block].len();
                blocks[block].push((i, text));
                u32le(&mut dat, block as u32);
                u32le(&mut dat, entry as u32);
            }
            _ => dat.extend_from_slice(text.as_bytes()),
        }
        let size = dat.len() as u32 - offset;
        u32le(&mut idx, offset);
        match m.layout {
            LexLayout::RawLD => u16le(&mut idx, size as u16),
            LexLayout::RawLD4 | LexLayout::ZLD => u32le(&mut idx, size),
        }
    }
    std::fs::write(file(".idx"), idx)?;
    std::fs::write(file(".dat"), dat)?;
    if m.layout == LexLayout::ZLD {
        let (mut zdx, mut zdt) = (Vec::new(), Vec::new());
        for block in &blocks {
            // Block: Zahl der Einträge, je Eintrag (Stelle, Länge) ab Blockanfang, dann die Texte mit 0 am Ende.
            let head = 4 + block.len() * 8;
            let mut table = Vec::new();
            let mut body = Vec::new();
            u32le(&mut table, block.len() as u32);
            for (_, text) in block {
                u32le(&mut table, (head + body.len()) as u32);
                u32le(&mut table, text.len() as u32 + 1);
                body.extend_from_slice(text.as_bytes());
                body.push(0);
            }
            table.extend_from_slice(&body);
            let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(&table)?;
            let packed = enc.finish()?;
            u32le(&mut zdx, zdt.len() as u32);
            u32le(&mut zdx, packed.len() as u32);
            zdt.extend_from_slice(&packed);
        }
        std::fs::write(file(".zdx"), zdx)?;
        std::fs::write(file(".zdt"), zdt)?;
    }
    Ok(())
}
