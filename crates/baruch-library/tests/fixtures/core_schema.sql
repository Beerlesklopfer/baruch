CREATE TABLE work (
  id          INTEGER PRIMARY KEY,
  slug        TEXT NOT NULL UNIQUE,              -- Ordnername des Typst-Spiegels
  kind        TEXT NOT NULL CHECK (kind IN ('book','sermon')),
  title       TEXT NOT NULL,
  author      TEXT NOT NULL,
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE node (
  id          INTEGER PRIMARY KEY,
  work_id     INTEGER NOT NULL REFERENCES work(id),
  seq         INTEGER NOT NULL,                  -- = baruch_core::Node.id (0-basiert, lückenlos je Werk)
  parent_seq  INTEGER,                           -- = Node.parent
  position    INTEGER NOT NULL,                  -- Reihenfolge unter demselben Elternteil
  number      TEXT NOT NULL,
  title       TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('part','chapter','section')),
  findings    INTEGER NOT NULL DEFAULT 0,
  UNIQUE (work_id, seq)
);

CREATE TABLE revision (
  id          INTEGER PRIMARY KEY,
  node_id     INTEGER NOT NULL REFERENCES node(id),
  number      INTEGER NOT NULL,                  -- ab 1, = Revision.number
  body        TEXT NOT NULL,
  author_kind TEXT NOT NULL CHECK (author_kind IN ('human','assistant')),
  author_name TEXT NOT NULL,                     -- Mensch: Name; Assistent: Persona-Handle „@lektor“
  model       TEXT,                              -- nur bei assistant, sonst NULL
  status      TEXT NOT NULL CHECK (status IN ('proposed','current','rejected','superseded')),
  note        TEXT NOT NULL DEFAULT '',
  based_on    INTEGER,                           -- Revisionsnummer, auf der der Vorschlag beruht (optional)
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (node_id, number),
  CHECK ((author_kind = 'assistant') = (model IS NOT NULL))
);
-- genau eine current je Abschnitt (höchstens eine per Index; mindestens eine sichert der Code)
CREATE UNIQUE INDEX revision_one_current ON revision(node_id) WHERE status = 'current';
-- nur anhängen: kein Löschen, Inhalt unveränderlich, nur der Status darf wechseln
CREATE TRIGGER revision_no_delete BEFORE DELETE ON revision
BEGIN SELECT RAISE(ABORT, 'Revisionen werden nur angehängt'); END;
CREATE TRIGGER revision_immutable BEFORE UPDATE OF node_id, number, body, author_kind, author_name, model, note, based_on, created ON revision
BEGIN SELECT RAISE(ABORT, 'Revisionen sind unveränderlich'); END;

CREATE TABLE persona (
  id             INTEGER PRIMARY KEY,
  handle         TEXT NOT NULL UNIQUE,           -- „@lektor“
  name           TEXT NOT NULL,
  role           TEXT NOT NULL,
  model          TEXT NOT NULL,
  context_length INTEGER NOT NULL,
  active         INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
  position       INTEGER NOT NULL
);

CREATE TABLE post (
  id          INTEGER PRIMARY KEY,
  node_id     INTEGER NOT NULL REFERENCES node(id),
  position    INTEGER NOT NULL,                  -- Reihenfolge im Abschnitts-Chat
  author      TEXT NOT NULL,                     -- „Autor“ oder Persona-Handle
  body        TEXT NOT NULL,
  sources     TEXT NOT NULL DEFAULT '[]',        -- JSON-Liste von Strings (Post.sources)
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (node_id, position)
);

CREATE TABLE post_ref (                          -- baruch_core::TextRef
  post_id     INTEGER NOT NULL REFERENCES post(id),
  position    INTEGER NOT NULL,
  node_seq    INTEGER NOT NULL,                  -- TextRef.node (seq im selben Werk)
  rev         INTEGER NOT NULL,                  -- TextRef.rev
  quote       TEXT NOT NULL,
  PRIMARY KEY (post_id, position)
);
