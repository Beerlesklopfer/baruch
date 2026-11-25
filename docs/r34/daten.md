# Runde 34 – mindmap_daten: Daten, RPC und MCP der Mindmap

Fragment für `docs/RPC.md`, `docs/MCP.md`, `docs/Architektur.md` und `docs/Bedienung.md` (der Integrator setzt es
ein). Besitz: mindmap_daten. Stand: Zweig `team/mindmap_daten`.

## 1 Entscheidungen

### 1.1 Die Wolke bleibt – als Ablage neben der Mindmap (Nachtrag des Nutzers vom 2026-11-25)

Der Vertrag (§0) ließ Einfälle ohne Elternknoten und ohne Bezug als „freie Hauptthemen“ direkt am Werk hängen; die
Wolke „Ohne Kapitel“ wäre entfallen. Nach dem Nachtrag gilt stattdessen **eine Regel mit vier Plätzen**:

1. `parent` gesetzt → Unterknoten dieses Einfalls (gleiches Werk, kein Kreis, Kette höchstens 32 tief). Der Bezug
   (`key`) bleibt daneben erlaubt und ist dann nur der Link zum Abschnitt.
2. sonst `key` gesetzt → Kind des Ankers (nächster Vorfahre der Art Kapitel, sonst der Punkt selbst).
3. sonst `main_topic` → **freies Hauptthema** direkt am Werk.
4. sonst → **in der Wolke**: nicht im Baum, sondern in der Ablage neben der Mindmap.

Dafür trägt `idea` eine vierte neue Spalte `main_topic` (0/1, Vorgabe 0); der Vertrag sah drei vor. Ohne sie ließe
sich ein Einfall, den man aufs Werk zieht, nicht von einem Einfall ohne Platz unterscheiden. Folgen:

- **Bestehende Einfälle** ohne Bezug (heute Wolke „Ohne Kapitel“) liegen nach der Migration in der Wolke; Einfälle
  mit Bezug hängen wie bisher an ihrem Kapitel. Nichts wird zum Hauptast, ohne dass jemand es dorthin zieht.
- **Neue Einfälle ohne Platz** (Schnellerfassung Alt+E, Formular, RPC `idea.create` und MCP `idea_create` ohne
  `parent`/`key`) landen in der Wolke.
- **Aus der Wolke in die Mindmap:** `idea.move` mit `parent` (auf einen Einfall), mit `parent: null, key` (auf ein
  Kapitel) oder mit `parent: null, key: null` bzw. `cloud: false` (aufs Werk, wird Hauptthema).
- **Aus der Mindmap in die Wolke:** `idea.move` mit `cloud: true`. Der Einfall verliert Elternknoten und Bezug;
  **seine Unterknoten bleiben an ihm** – der Teilbaum wandert als Ganzes in die Wolke. Begründung: kein Einfall
  verliert dabei stillschweigend seinen Zusammenhang, die Aktion ist mit einem `idea.move` zurück umkehrbar
  (Strg+Z des Widgets), und „nur er“ hieße, seine Kinder ungefragt an den Großelternknoten zu hängen. Wer nur den
  einen Einfall lösen will, zieht vorher die Unterknoten woandershin.
- **Zähler:** `cloud.count` („n in der Wolke“) zählt die Einfälle der obersten Ebene der Wolke ohne Filter,
  `cloud.count_with_children` samt ihren Unterknoten.
- **Nähe als Hilfe:** `cloud.hints` nennt je Einfall der Wolke höchstens einen Vorschlag „passt zu Ast …“ – den
  nächsten Einfall in der Mindmap (gespeicherte Einbettungen und gleiche Schlagwörter wie auf der Karte, nie eine
  neue Einbettung) und den Hauptast, in dem er hängt. Die Vorschläge werden auch berechnet, wenn der Filter die
  Nähe-Linien im Bild ausschaltet.
- **Wiederherstellen** eines Einfalls, dessen Elternknoten noch gelöscht ist: Er bleibt an seinem Bezug bzw. liegt
  in der Wolke (`detached: true`), nicht als Hauptthema am Werk.
- **Baum-Import** (`tree::import` ohne `parent`): Die obersten Knoten werden freie Hauptthemen, nicht Einfälle der
  Wolke – wer eine Mindmap einliest, will sie in der Mindmap sehen.

### 1.2 Abweichungen vom Vertrag (bitte beim Einsetzen beachten)

- **`idea.delete`:** Der Vertrag nennt „Antwort zusätzlich `deleted: [ids]`“. `deleted` ist seit Runde 7 `true`
  (RPC und MCP, geprüft u. a. in `round23_geheimnisse.rs`); die Liste heißt darum **`deleted_ids`**, `deleted`
  bleibt `true`.
- **Ablehnungen von `idea.move` und `tree::import`** tragen den Code **-32000** (`rpc::REFUSED`, „abgelehnt“). Der
  Vertrag schreibt -32001; das ist seit Runde 19c `RAG_UNAVAILABLE`.
- **`MapIdea`** (Typ der flachen Karte) bekommt die Felder `title`, `parent`, `position` nicht am Typ, sondern in
  der Antwort von `idea.map` und `idea_map` (dazu `main_topic`, `cloud`): Die eingefrorene Datei `bild/layout.rs`
  baut `MapIdea` in ihren Tests von Hand und bräche sonst.
- **`MindMap` unverändert:** Die Wolke ist kein Feld von `MindMap` (Handbau in fremden Tests bräche), sondern eine
  zweite Funktion `tree::cloud` bzw. beide zusammen `tree::tree_and_cloud` (siehe 3).
- **XSD:** Das Schema der Unterversion 9 liegt als neue Datei `docs/schema/baruch-werk-1-u9.xsd` neben den
  eingefrorenen; `baruch-werk-1-u8.xsd` ist unverändert (SHA-256 im Test).

## 2 Daten

Migration `0052_idea_tree.sql` (nur `ADD COLUMN`, nicht in `FOREIGN_KEYS_OFF`):

```sql
ALTER TABLE idea ADD COLUMN title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200);
ALTER TABLE idea ADD COLUMN parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL;
ALTER TABLE idea ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
ALTER TABLE idea ADD COLUMN main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1));
CREATE INDEX idea_parent ON idea(parent_id, position);
```

Regeln im Store (`crates/baruch-store/src/ideas.rs`):

- **Titel:** höchstens 200 Zeichen, eine Zeile, darf leer sein. Ein Einfall braucht Titel **oder** mindestens ein
  Schlagwort. Angezeigter Titel (`Idea::display_title`): der Titel, sonst die Schlagwörter „a, b“, sonst der Anfang
  des Gedankens, sonst „Einfall <id>“ – nie leer. Der Titel steht im Volltextindex (`idea_fts`, Spalte `words`).
- **Elternknoten:** nicht gelöschter Einfall desselben Werks; kein Kreis; Kette höchstens 32 Einfälle
  (`MAX_IDEA_DEPTH`). Einfälle ohne Werk haben keinen Elternknoten.
- **Reihenfolge:** `position`, dann `id`, unter Geschwistern (gleicher Elternknoten bzw. alle Einfälle des Werks
  ohne Elternknoten). Neue kommen ans Ende; „vor ein Geschwister“ (`before`) nummeriert die Geschwister lückenlos
  ab 1 neu, ohne deren `updated` anzufassen. Bestehende Einfälle tragen 0 und ordnen sich nach `id`; solange unter
  einem Elternknoten nichts geordnet wurde, behalten auch neue die 0 (im Austausch dann ohne `platz` – ein Werk ohne
  geordneten Baum gibt dieselbe Datei aus wie vor Runde 34 und besteht auch das eingefrorene u8-Schema).
- **Löschen** nimmt die nicht gelöschten Nachfahren mit **demselben Zeitstempel** mit; **Wiederherstellen** bringt
  den Einfall und genau die Nachfahren mit diesem Zeitstempel zurück (früher einzeln Gelöschtes bleibt gelöscht).
  Verbindungen (`idea_link`) und Lagen (`idea_map_pos`) bleiben dabei gespeichert.
- **Werkwechsel** (`idea.update` mit `work`): Der Einfall löst sich vom Elternknoten und liegt im neuen Werk in der
  Wolke; seine Unterknoten bleiben im alten Werk und verlieren den Elternknoten (kein Baum über zwei Werke).
- **Lagen** (`idea_map_pos`) bleiben gespeichert und im Austausch, sind in der Mindmap aber ohne Wirkung (die
  Anordnung rechnet der Server aus dem Baum).

Spiegel `einfaelle.md`: Hat ein Werk mindestens einen Einfall mit Titel, Elternknoten oder Hauptthema, steht vor den
Einträgen der Abschnitt `## Baum` – die Einfälle eingerückt nach Elternknoten, Geschwister nach `position`, dann
`id`, oben mit dem Platz („(am Abschnitt key 12)“, „(Hauptthema)“, „(Wolke)“). Jeder Eintrag nennt zusätzlich
`- Titel: …`, `- Unterknoten von: Einfall <id>` bzw. `- Platz: Hauptthema am Werk`. Ohne Baum bleibt die Datei
byte-gleich wie vor Runde 34.

Austauschformat: Unterversion 9, `<einfall>` mit `titel`, `eltern`, `platz`, `hauptthema` – siehe
`docs/Austauschformat.md` („Baum der Mindmap“).

## 3 Schnittstelle im Server (`crates/baruch-server/src/idea_map/tree.rs`)

Die Signaturen aus S1.2 sind wörtlich geblieben (`MindMap`, `TreeNode`, `TreeKind`, `tree`, `ImportTree`, `import`,
Konstanten). Neu daneben:

```rust
pub struct CloudHint { pub idea: IdeaId, pub node: String, pub node_title: String,
                       pub branch: String, pub branch_title: String, pub score: f32, pub shared_words: Vec<String> }
pub struct Cloud { pub work: WorkId, pub ideas: Vec<TreeNode>, pub count: u32, pub count_with_children: u32,
                   pub hints: Vec<CloudHint>, pub links: Vec<IdeaLink> }
pub async fn cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<Cloud, RpcError>;
pub async fn tree_and_cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<(MindMap, Cloud), RpcError>;
```

- `tree` liefert Werk (`w`), Teile und Kapitel (`n:<key>`) in Gliederungsreihenfolge, danach die freien Hauptthemen
  nach `position, id`; unter jedem Knoten erst die Gliederungspunkte, dann seine Einfälle nach `position, id`.
  Einfälle der Wolke stehen **nicht** im Baum.
- Filter `status`, `word`, `origin`: Ein Einfall, der nicht passt, bleibt als Geist (`ghost: true`, `hit: false`)
  stehen, solange ein Nachfahre passt. `q` blendet nichts aus, es setzt `hit` (Volltext, auch über den Titel).
- `MindMap::links` und `MindMap::near.pairs` nennen nur Einfälle, die im Baum stehen; `Cloud::links` die
  Verbindungen mit mindestens einem Ende in der Wolke. `total_ideas` zählt die gezeigten Einfälle im Baum (ohne
  Geister, ohne Wolke); `descendants` zählt je Knoten die Einfälle, die unter ihm im Baum stehen.
- `Cloud::ideas` sind `TreeNode` der Art `idea` mit ihren Unterknoten in `children`; derselbe Filter gilt.
- `import` legt den Baum in einer Transaktion an (Store `idea_import_tree`) und schreibt **einen** Commit „Mindmap
  eingelesen: n Einfälle“ (`Baruch-Action: idea-import`) mit der übergebenen Herkunft. Abgelehnt (-32000, nichts
  angelegt, kein Commit): kein Knoten, mehr als `MAX_IMPORT_NODES` (2000) Knoten, tiefer als `MAX_TREE_DEPTH` (32,
  auch zusammen mit der Tiefe von `parent`), `parent` gelöscht oder in einem anderen Werk, Titel über 200 bzw. Notiz
  über 4000 Zeichen, Schlagwörter über den Grenzen der Einfälle (höchstens 20, je 60 Zeichen), eine Verbindung mit
  einem Pfad ohne Knoten. Ein Knoten ohne Titel und Schlagwort heißt „(ohne Titel)“; eine Verbindung eines Knotens
  mit sich selbst wird übergangen. `ImportTree::title` steht im Rumpf des Commits, `warnings` werden nicht
  gespeichert. **Für mindmap_austausch:** Titel und Notizen vor dem Aufruf kürzen (der Import kürzt nicht, er lehnt
  ab).

## 4 RPC (für `docs/RPC.md`, Abschnitt „Einfälle“ und „Gedankenkarte“ → „Mindmap“)

Herkunft über RPC ist immer der Mensch. Jede Antwort mit `idea` trägt zusätzlich `title`, `parent` (id oder
`null`), `position`, `main_topic` und `cloud` (abgeleitet: liegt in der Wolke).

| Methode | Parameter | Antwort |
|---|---|---|
| `idea.create` | wie bisher, dazu `title?` (≤ 200 Zeichen), `parent?` (Einfall-id, gleiches Werk; setzt `work` daraus, wenn es fehlt), `before?` (id eines Geschwisters; ohne = ans Ende), `cloud?` (`false` = ohne `parent` und `key` freies Hauptthema; Vorgabe: in die Wolke). `words` darf fehlen, wenn `title` nicht leer ist; ohne beides -32602. | `{idea, commit?}` |
| `idea.update` | wie bisher, dazu `title?` (Umbenennen; leer nur, solange ein Schlagwort bleibt) | `{idea, changed, commit?}` |
| `idea.move` | `{id, parent?: id\|null, key?: key\|null, before?: id\|null, cloud?: bool}` – fehlende Felder bleiben. `parent: null` löst vom Elternknoten; bleibt auch kein Bezug, wird der Einfall freies Hauptthema am Werk. `key` setzt bzw. löscht den Bezug. `before` ordnet vor ein Geschwister, `null` ans Ende. `cloud: true` legt in die Wolke (löst Elternknoten und Bezug, Unterknoten bleiben am Einfall); `cloud: false` macht einen Einfall ohne Elternknoten und Bezug zum Hauptthema. | `{idea, changed, commit?}`; unverändert → `changed: false` ohne Commit |
| `idea.delete` | `{id, confirm: true}` | `{id, deleted: true, deleted_ids: [ids], idea, commit?}` – der Einfall zuerst, dann die mitgelöschten Unterknoten |
| `idea.restore` | `{id}` | `{idea, restored: [ids], detached?: true, commit?}` |
| `idea.tree` | `{work, status?, word?, origin?, q?, near?, near_limit?, near_threshold?, findings?}` (Filter wie `idea.map`) | `{map: MindMap, cloud: Cloud}` |
| `idea.map` | unverändert | wie bisher; je Einfall zusätzlich `title`, `parent`, `position`, `main_topic`, `cloud`; `label` ist der Titel, sonst die Schlagwörter |

`idea.move` lehnt mit -32000 ab (nichts geändert, kein Commit): Elternknoten in einem anderen Werk, der Einfall
selbst oder ein eigener Nachfahre als Elternknoten (Kreis), Tiefe über 32, gelöschter Einfall oder Elternknoten,
`before` kein Geschwister, `cloud: true` zusammen mit `parent` oder `key`. Unbekannter Einfall bzw. falsche Typen:
-32602. Jede Änderung ist ein Commit „Einfall <id>: umgehängt“ (`Baruch-Action: idea-move`) mit Platz vorher und
nachher im Rumpf. Kapitel und Teile lassen sich über `idea.*` nicht ändern – ihre Kennung `n:<key>` ist keine
Einfall-id; dafür bleiben `outline.*` und die Antragsregel.

`idea.map_pos_set` und `idea.map_pos_reset` bleiben: gespeichert, in der Mindmap ohne Wirkung.

Umkehr für „Rückgängig“ im Widget: anlegen ↔ `idea.delete`, löschen ↔ `idea.restore` (bringt die Unterknoten mit),
umbenennen ↔ `idea.update` mit dem alten `title`, umhängen ↔ `idea.move` mit `parent`/`key`/`before` bzw. `cloud`
aus dem Einfall vor der Aktion (`cloud: true`, wenn er in der Wolke lag; `parent: null, key: null, cloud: false`,
wenn er Hauptthema war).

Formulare ohne JavaScript: `POST /einfaelle/act` nimmt zusätzlich `method=idea.move` mit `parent` („in die Mindmap
unter Einfall Nr. …“), `cloud=1` („in die Wolke“) bzw. `parent=none&key=none&cloud=0` („als Hauptthema ans Werk“);
die Seite `/einfaelle/{id}` zeigt den Platz und diese drei Formulare, `idea.create`/`idea.update` haben ein Feld
`title`.

## 5 MCP (für `docs/MCP.md`)

| Werkzeug | Argumente | Hinweis |
|---|---|---|
| `idea_create` / `add_idea` | wie bisher, dazu `title?`, `parent?`, `before?`, `cloud?` (wie RPC); `words` ist nicht mehr Pflicht, wenn `title` gesetzt ist | Ohne Platz landet der Einfall in der Wolke. Herkunft `persona` oder `@mcp:<client>`, ein Commit. |
| `idea_update` | wie bisher, dazu `title?` | ein Commit je Änderung |
| `idea_move` (neu) | `{id, parent?: id\|null, key?: key\|null, before?: id\|null, cloud?: bool, persona?}` | wie `idea.move`; verletzte Regel → `isError`. Herkunft `persona` oder `@mcp:<client>`, ein Commit „umgehängt“ im Spiegel. |
| `idea_tree` (neu) | `{work, status?, word?, origin?, q?}` | nur lesend: `{map, cloud}` wie `idea.tree` (Nähe-Linien aus, Vorschläge der Wolke an), jeder Einfall mit `url`. |
| `idea_delete` | unverändert | löscht Unterknoten mit (`deleted_ids`), wiederherstellbar. |
| `idea_read`, `idea_list`, `idea_search` | unverändert | Ausgabe mit `title`, `parent`, `position`, `main_topic`, `cloud`. |
| `idea_map`, `idea_links`, `idea_link`, `idea_unlink` | unverändert | Beschreibungen sagen „Mindmap“; `idea_map` ist die flache Sicht mit `title`, `parent`, `position` je Einfall. |

Reihenfolge in `tools/list`: `idea_move` direkt nach `idea_delete`, `idea_tree` direkt nach `idea_map`. Kapitel und
Teile ändert weiter nur `outline_*` (Antrag). Einlesen von .mm/.xmind gibt es über MCP nicht. Kein Stub offen. Der
Hinweistext von `initialize` nennt `idea_tree`, `idea_move` und die Wolke.

## 6 Architektur (für `docs/Architektur.md`)

- Knoten der Mindmap sind Zeilen der Tabelle `idea` – keine zweite Struktur. Der Store kennt die Regeln des Baums
  (`idea_create_at`, `idea_move`, `idea_delete_tree`, `idea_restore_tree`, `idea_import_tree`, `idea_tree_rows`),
  der Server setzt ihn mit der Gliederung zusammen (`idea_map::tree`), der Spiegel schreibt ihn in `einfaelle.md`.
- Jede schreibende Aktion läuft unter der Schreibsperre und ergibt genau einen Commit mit Herkunft (`idea-create`,
  `idea-update`, `idea-move`, `idea-delete`, `idea-restore`, `idea-import`).
- Die flache Karte (`idea_map::build`, RPC `idea.map`) bleibt als Abwärtsverträglichkeit; der Baum baut auf ihr auf
  (Gliederung, Filter, Verbindungen, Nähe, Befunde) und liest die Einfälle zusätzlich ungefiltert, um Geister und
  die Wolke zu bestimmen.

## 7 Tests

- `crates/baruch-store/tests/migration_0052.rs` – nachgebaute Datenbank im Stand 0051 (Einfälle auch gelöscht,
  Verbindungen, Lagen): alles byte-gleich, neue Spalten mit Vorgabe, nur `idea` und der Index `idea_parent` neu.
- `crates/baruch-store/tests/idea_tree.rs` – Titel statt Schlagwort, Wolke/Hauptthema, Reihenfolge, Umhängen mit
  allen Ablehnungen, Löschen/Wiederherstellen samt Unterknoten, Werkwechsel, Baum-Import, Volltext, Spiegel.
- `crates/baruch-store/tests/exchange_u9.rs`, `crates/baruch-exchange/tests/read.rs`, `xsd.rs` – Unterversion 9.
- `crates/baruch-server/tests/round34_daten.rs` – Baum und Wolke, Geister, RPC, Import mit einem Commit, MCP mit
  Herkunft, Rundreise zwischen zwei Instanzen, Spiegel und Suche, Formulare.
- Angepasst: Wächter der Werkzeugnamen (`mcp.rs`, `mcp_ideas.rs`, `round4.rs`), alte Migrationstests
  (`migration_0016/0017/0018/0040/0041.rs`, `revision_fold.rs`).

## 8 Für die Handprüfung (HANDOFF)

- Werk 3 im eigenen Browser: Die 33 Einfälle sind da; Einfälle mit Bezug hängen an ihren Kapiteln, **die übrigen
  liegen in der Wolke** (nicht als Hauptäste). Verbindungen und Lagen sind vorhanden.
- Einen Einfall aus der Wolke auf einen Knoten ziehen und wieder zurück; „n in der Wolke“ zählt mit.
- Einen Knoten mit Unterknoten in die Wolke ziehen: Die Unterknoten bleiben an ihm (Entscheidung 1.1) – passt das?
- Alt+E: Der neue Einfall erscheint in der Wolke.
