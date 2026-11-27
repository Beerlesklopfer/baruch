// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 38 (task mcp_rpc, contract §4.6): exchange format sub-version 10 in the store – the work language travels
//! with the work. A new import takes the language over, a dump writes it, a file of sub-version 9 (without the
//! attribute) becomes a German work, and merging with an existing work treats the language like the field `bible`:
//! a deviation is reported (line "Sprache"), the local value stays. Invented sample work from
//! `baruch_exchange::sample`; real SQLite file in a temporary directory.

use baruch_exchange::read::read_xml;
use baruch_exchange::sample::{sample_document_u9, sample_document_u10};
use baruch_exchange::write::{ImageMode, write_xml};
use baruch_exchange::{ExchangeDocument, ExchangeLanguage, FORMAT_VERSION, Limits};
use baruch_store::exchange_apply::ApplyMode;
use baruch_store::{Store, WorkLanguage};

const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000038";

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn a_new_import_takes_the_language_over_and_the_dump_writes_it_back_exactly() {
    let (_dir, store) = open().await;
    let doc = sample_document_u10();
    assert_eq!(doc.work.language, ExchangeLanguage::En, "precondition: the sample of sub-version 10 is English");
    let work = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap().work;
    assert_eq!(
        store.work_language(work).await.unwrap(),
        WorkLanguage::En,
        "a work read in as new carries the language of the file"
    );

    let dumped = store.exchange_dump(work).await.unwrap();
    assert_eq!(dumped.language, ExchangeLanguage::En, "the dump carries the language of the work");
    let xml = write_xml(&ExchangeDocument { work: dumped.clone(), ..doc.clone() }, ImageMode::Inline).unwrap();
    assert_eq!(FORMAT_VERSION, 10, "Round 38 counts the sub-version up to 10");
    assert!(xml.contains("version=\"10\""), "the output names sub-version 10");
    assert!(xml.contains("<angaben sprache=\"en\">"), "the attribute stands at <angaben>: {}", &xml[..600]);
    let back = read_xml(xml.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(back.work.language, ExchangeLanguage::En, "the round trip through XML keeps the language");
    assert_eq!(
        (&back.work.title, &back.work.bible, back.work.outline.len(), back.work.ideas.len()),
        (&doc.work.title, &doc.work.bible, doc.work.outline.len(), doc.work.ideas.len()),
        "and everything around it: title, Bible, outline and ideas as in the file"
    );
    assert!(back.unknown.is_empty(), "the reader knows everything it wrote: {:?}", back.unknown);
}

#[tokio::test]
async fn a_german_work_writes_its_language_too_and_a_file_without_it_is_german() {
    let (_dir, store) = open().await;
    let doc = sample_document_u9();
    let work = store.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap().work;
    assert_eq!(store.work_language(work).await.unwrap(), WorkLanguage::De, "the sample of sub-version 9 is German");
    let dumped = store.exchange_dump(work).await.unwrap();
    let xml = write_xml(&ExchangeDocument { work: dumped, ..doc.clone() }, ImageMode::Inline).unwrap();
    assert!(xml.contains("<angaben sprache=\"de\">"), "the attribute is always written, also for German");

    // A file as every Baruch up to sub-version 9 wrote it: lower version, no attribute.
    let old = xml.replacen("version=\"10\"", "version=\"9\"", 1).replacen(" sprache=\"de\"", "", 1);
    assert!(old.contains("<angaben>") && old.contains("version=\"9\""), "precondition: a file of sub-version 9");
    let read = read_xml(old.as_bytes(), &Limits::DEFAULT).unwrap();
    assert_eq!(read.version, 9, "the reader reports the sub-version of the file");
    assert_eq!(read.work.language, ExchangeLanguage::De, "a missing attribute means German");

    let (_dir2, other) = open().await;
    let imported = other.exchange_apply(&read, SHA, ApplyMode::New, None).await.unwrap().work;
    assert_eq!(
        other.work_language(imported).await.unwrap(),
        WorkLanguage::De,
        "a work from a file of sub-version 9 becomes a German work"
    );
}

#[test]
fn an_unknown_language_in_a_file_is_an_error_like_an_unknown_work_kind() {
    let xml = write_xml(&sample_document_u10(), ImageMode::Inline).unwrap();
    let broken = xml.replacen("<angaben sprache=\"en\">", "<angaben sprache=\"fr\">", 1);
    assert_ne!(broken, xml, "precondition: the attribute was replaced");
    let error = read_xml(broken.as_bytes(), &Limits::DEFAULT).expect_err("an unknown work language is refused");
    let text = error.to_string();
    assert!(
        text.contains("sprache") && text.contains("fr"),
        "the error names the attribute and the value, like other enumeration attributes: {text}"
    );
}

#[tokio::test]
async fn merging_reports_a_differing_language_and_keeps_the_local_one() {
    let (_dir, store) = open().await;
    let work = store.exchange_apply(&sample_document_u9(), SHA, ApplyMode::New, None).await.unwrap().work;
    assert_eq!(store.work_language(work).await.unwrap(), WorkLanguage::De, "the local work is German");

    // The other instance has set the work to English and sends it back.
    let there = sample_document_u10();
    let applied = store.exchange_apply(&there, SHA, ApplyMode::Merge, None).await.unwrap();
    assert_eq!(applied.work, work, "the file is recognised as the same work");
    let hint = applied.report.deviations.iter().find(|d| d.kind == "werk" && d.label.contains("Sprache"));
    let hint = hint.unwrap_or_else(|| {
        panic!(
            "a differing language is reported like a differing Bible: {:?}",
            applied.report.deviations.iter().map(|d| &d.label).collect::<Vec<_>>()
        )
    });
    assert_eq!((hint.here.as_str(), hint.there.as_str()), ("de", "en"), "the report shows both values");
    assert_eq!(
        store.work_language(work).await.unwrap(),
        WorkLanguage::De,
        "merging never changes the language of the local work: the human sets it in the settings"
    );

    // The same language on both sides: no line in the report.
    let same = store.exchange_apply(&sample_document_u9(), SHA, ApplyMode::Merge, None).await.unwrap();
    assert!(
        same.report.deviations.iter().all(|d| !d.label.contains("Sprache")),
        "an equal language is no deviation: {:?}",
        same.report.deviations.iter().map(|d| &d.label).collect::<Vec<_>>()
    );
}
