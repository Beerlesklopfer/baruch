// SPDX-License-Identifier: AGPL-3.0-or-later
//! Beispieltitel für eine leere Bibliothek: nur gemeinfreie Bibeltexte, wörtlich nach eBible.org
//! (Lutherbibel 1912: <https://ebible.org/deu1912/>, Unrevidierte Elberfelder 1905:
//! <https://ebible.org/deuelo/>, beide als „Public Domain“ ausgewiesen; abgeglichen 2026-11-05).
//! Der Vers 1. Kor 13,4 enthält „langmütig“ wie das Beispielwerk, damit die Suche Werk und
//! Bibliothek zugleich trifft.

use sqlx::SqlitePool;

use crate::LibraryError;
use crate::titles::{NewTitle, insert_quote, insert_title};

struct SeedTitle {
    key: &'static str,
    title: &'static str,
    year: i64,
    url: &'static str,
    note: &'static str,
    quotes: &'static [(&'static str, &'static str)], // (Stelle, Text)
}

const SEED: &[SeedTitle] = &[
    SeedTitle {
        key: "luther1912",
        title: "Die Bibel (Lutherbibel 1912)",
        year: 1912,
        url: "https://ebible.org/deu1912/",
        note: "Gemeinfrei. Quelle: eBible.org, Lutherbibel 1912 (Public Domain).",
        quotes: &[
            (
                "1. Kor 13,4",
                "Die Liebe ist langmütig und freundlich, die Liebe eifert nicht, die Liebe treibt nicht Mutwillen, sie blähet sich nicht,",
            ),
            (
                "Röm 12,2",
                "Und stellet euch nicht dieser Welt gleich, sondern verändert euch durch die Erneuerung eures Sinnes, auf daß ihr prüfen möget, welches da sei der gute, wohlgefällige und vollkommene Gotteswille.",
            ),
            (
                "Jer 36,32",
                "Da nahm Jeremia ein anderes Buch und gab's Baruch, dem Sohn Nerias, dem Schreiber. Der schrieb darein aus dem Munde Jeremia's alle die Reden, so in dem Buch standen, das Jojakim, der König Juda's, hatte mit Feuer verbrennen lassen; und zu denselben wurden dergleichen Reden noch viele hinzugetan.",
            ),
        ],
    },
    SeedTitle {
        key: "elberfelder1905",
        title: "Die Bibel (Unrevidierte Elberfelder 1905)",
        year: 1905,
        url: "https://ebible.org/deuelo/",
        note: "Gemeinfrei. Quelle: eBible.org, Unrevidierte Elberfelder 1905 (Public Domain).",
        quotes: &[
            (
                "1. Kor 13,4",
                "Die Liebe ist langmütig, ist gütig; die Liebe neidet nicht; die Liebe tut nicht groß, sie bläht sich nicht auf,",
            ),
            (
                "1. Kor 13,13",
                "Nun aber bleibt Glaube, Hoffnung, Liebe, diese drei; die größte aber von diesen ist die Liebe.",
            ),
        ],
    },
];

/// Legt die Beispieltitel mit Zitaten an, wenn `library_title` leer ist (eine Transaktion).
/// Gibt `true` zurück, wenn etwas angelegt wurde.
pub async fn seed_if_empty(pool: &SqlitePool) -> Result<bool, LibraryError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM library_title").fetch_one(&mut *tx).await?;
    if count > 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    for seed in SEED {
        let id = insert_title(
            &mut tx,
            &NewTitle {
                key: seed.key.into(),
                kind: "bible".into(),
                title: seed.title.into(),
                authors: String::new(),
                year: Some(seed.year),
                publisher: String::new(),
                place: String::new(),
                url: seed.url.into(),
                note: seed.note.into(),
            },
        )
        .await?;
        for (page, text) in seed.quotes {
            insert_quote(&mut tx, id, text, page, "").await?;
        }
    }
    tx.commit().await?;
    Ok(true)
}
