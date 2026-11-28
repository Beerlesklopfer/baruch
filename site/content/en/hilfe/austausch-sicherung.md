---
title: "Exchange, backup and security"
description: "Carrying a work from one Baruch to another, backing up the data and knowing who can access Baruch."
weight: 110
---

This page answers three questions: How does a work get from the study to the laptop? How do I back up my work?
And who can access my Baruch?

## Writing out a work {#ausgeben}

{{< hilfebild name="austausch-ausgeben" alt="The page Exchange: output of the sample work: an explanation of what is in the file, the scope in numbers – outline items, revisions, posts, ideas, pictures –, the name of this instance and two buttons for downloading." titel="The exchange file contains the whole work with all revisions." >}}

The **exchange file** carries a whole work from one Baruch to another – unlike the Typst export with **all**
revisions, posts, ideas and pictures.

How it works:

1. Open the work and choose **Book → Output → Exchange: Write out**.
2. The page shows what is in the file and the scope in numbers.
3. Press **Download exchange file (.baruch)**. For small works or for looking inside there is **Download only
   XML** next to it – one single readable file.

**The file contains:** the details of the work, the book details including the cover, the work template and the
outline with all revisions (proposals and discarded ones as well) and the mark "current", the posts, ideas (deleted
ones as well), the mind map with its links, threads, pictures with all revisions, findings, glossary and index.

**The file does not contain:** credentials, the connections to services, the assistants and their models, open
requests, accounts, and Bible and literature texts.

Writing out changes nothing in the work. From the command line:

```sh
baruch-server export mein-buch --baruch mein-buch.baruch
```

## Reading in and merging a work {#einlesen}

{{< hilfebild name="austausch-einlesen" alt="The page Exchange: read in: file choice for an exchange file, a field for an own title for a new work and the button Show preview." titel="Step 1: choose the exchange file." >}}

How it works:

1. Choose **Baruch ▾ → Exchange: Read in…**.
2. Choose the file (at most 128 MiB). For a new work, enter an own title if you like.
3. Press **Show preview** and read the preview.
4. Press **Adopt**.

{{< hilfebild name="austausch-vorschau" alt="The preview before adopting: which instance the file comes from, whether the work is created anew or merged, what would be added, the deviations with a link to the place and the button Adopt." titel="Step 2: the preview says exactly what would happen." >}}

If this Baruch does not know the work yet, it is created **anew**. If it already knows the work, it is
**merged**. The preview names:

- which Baruch the file comes from;
- **what would be added** (outline items, revisions, ideas, posts, pictures …);
- the **deviations**, each with a link to the place: a text changed over there comes as a *further revision*, a
  changed outline as a *request*, anything else as a *notice*;
- the **outline requests** that would arise (a new part that would take over chapters needs the
  [consent](/en/hilfe/gliederung/#zustimmung-teil));
- what is **deleted here** and therefore does not come back;
- what is **not adopted**, with the reason.

**Nothing is deleted or overwritten, and no revision read in becomes current by itself.** Which revision counts is
decided by the human by switching. The one exception: if the other Baruch has only continued writing the own
current revision and its history fits without a gap, it is **fast-forwarded** here (the note reads
`vorgespult von Stand k auf Stand m`, from state k to state m). Reading in the same file a second time adds
nothing twice.

From the command line:

```sh
baruch-server import mein-buch.baruch                # new work
baruch-server import mein-buch.baruch --vorschau     # only show what would happen
baruch-server import mein-buch.baruch --abgleichen   # merge with the existing work
```

### The name of this Baruch {#instanz}

Every Baruch (more precisely: every data folder) has a **name** you choose yourself – such as "Study" or
"Laptop". The other side shows it in the preview; requests from a merge carry the origin "Exchange: Study". To set
it: **Settings → Data → This instance → Instance name** (at most 80 characters). Without a name it is called
"without a name" with a short identifier behind it.

## Backup {#sicherung}

{{< hilfebild name="einstellungen-daten" alt="Settings, page Data: data folder, size and version, the section This instance with identifier and name, below it the section Backup with the check box and the button Back up now." titel="The backup puts a complete copy next to the data folder." >}}

How it works:

1. Open **Settings → Data**.
2. Tick the check box at **Back up now** and press the button.
3. The page names the folder of the new backup. It lies **next to** the data folder and carries date and time in
   its name.

- The backup contains everything: all works with all revisions, their history, ideas, pictures.
- **Nothing is ever overwritten.** If ten backups already lie there, Baruch refuses until old ones are removed or
  moved – Baruch deletes none by itself. It also creates nothing if too little space would be left afterwards.
- Make a backup **before every update**.
- **Put backups somewhere else as well.** They lie on the same disk as the data; only a copy in another place
  helps against its failure.
- **Credentials are not part of the backup.** After moving to another computer you enter them again under
  Settings → Connections.
- Only the human can back up, no assistant.

How to restore a backup is described under [Operation](/en/betrieb/#sicherung). A single deleted work is brought
back by the [archive](/en/hilfe/erste-schritte/#loeschen-archiv).

## Security {#sicherheit}

{{< hilfebild name="einstellungen-mcp" alt="Settings, page MCP access: the address for programs, the ready command for an assistant program, the note that no access key (token) has been created yet, the form Create token and the list of programs connected since the start." titel="Who may access from outside is found under Settings → MCP access." >}}

The most important points in brief:

- **By default Baruch can only be reached from the own computer.** From another computer in the network you
  cannot get to the interface.
- **Without accounts there is no sign-in:** whoever reaches the interface is the author and can read and change
  everything. Whoever opens Baruch for other computers or people sets up [accounts](/en/hilfe/konten/).
- **Foreign web pages cannot trigger anything.** If another web page in the same browser calls Baruch, Baruch
  refuses – saving as well as starting a typesetting run.
- **Programs** (such as an assistant in the terminal) access Baruch through their own entrance. From the own
  computer that works without further ado, from other computers only with an access key; see
  [Settings → MCP access](/en/hilfe/einstellungen/#mcp-zugang). Programs can only write proposals, posts and
  ideas – accepting, changing, deleting and reading in are left to the human.
- **Credentials for services** lie in the keyring of the system, never in a file of the data folder, and appear
  on no page.
- **Book text leaves the computer only with explicit permission:** per assistant for an external service, per
  work for uploading to MultiGPT, and for the web search of @belege.

Whoever wants to make Baruch reachable over the Internet or for several computers needs a server in front of it
with encryption (a so-called reverse proxy); this is described in [Operation](/en/betrieb/#reverse-proxy).

## Where the data lie {#datenordner}

{{< hilfebild name="einstellungen-daten" alt="Settings, page Data: at the top the path of the data folder, its size and the version of the program; below it, for each work, its folder with the last entry in the history." titel="Settings → Data names the data folder and its size." >}}

Everything Baruch stores lies in one single **data folder**. Which one that is can be found under
**Settings → Data** and on the page **About Baruch**.

- In it lie a database and, for each work, a folder with the **history**: every change – saving, switching,
  deleting, every decision about a request – is recorded there with time and origin. Technically this is a Git
  repository; whoever knows Git can read in it. Deleted things remain readable there as well. What exactly is
  recorded and where to see it is described under [What Baruch records](/en/hilfe/austausch-sicherung/#protokoll).
- If the data folder is also accessible to other users of the computer, **Settings → Data** reports it. Baruch
  never changes the permissions by itself.
- Copy the data folder by hand only when Baruch is not running. During operation **Back up now** is the right
  way.

More on this, including the usual places per operating system: [Operation](/en/betrieb/#datenordner).

### What Baruch records {#protokoll}

{{< hilfebild name="schreiben-fassungsverlauf" alt="The history of a revision: the saving steps, newest first, each with author, time and the difference word by word – removed text red, new text green – and the button Back to here." titel="Baruch shows the history of a revision itself." >}}

Baruch has no log under that name – still, every change is recorded. Where to see what:

| What | Where to see it |
|---|---|
| every saved state of a revision | in the **history** of the revision: a click on **state k** in the head of the section; there also **Back to here** – see [History and undo](/en/hilfe/schreiben/#verlauf) |
| all revisions of a section with author, time and status | in the panel **Revisions** on the right and in the tree view – see [Saving and revisions](/en/hilfe/schreiben/#fassungen) |
| all posts of a section | on the page **Whole chat history** – see [Chat](/en/hilfe/assistenten/#chat) |
| every change to the whole work, deleted things as well | in the **history of the work** in the data folder – see [Reading the history of a work](/en/hilfe/austausch-sicherung/#spiegel) |
| the last entry per work | under **Settings → Data** |

The **history of the work** receives an entry for:

- every save and continuation, taking over a proposal, switching to another revision, changing and deleting a
  revision – when deleting, together with the deleted text;
- every change of the outline, every request and the decision about it;
- ideas, deleting, moving and converting posts, findings, images, glossary and typesetting settings;
- reading in and syncing through the [exchange](/en/hilfe/austausch-sicherung/#einlesen).

Every entry names the **time**, the **author** – "Autor" for the human, otherwise the assistant or the program
that made the change – and the **origin**. If the human confirms a request, the entry names both: who requested
and who confirmed. Merely reading and searching leaves no entry, and neither do the display settings of the
browser and the access data.

### Reading the history of a work {#spiegel}

The history lies in the **data folder** in the subfolder `works`: one folder per work with the short name of the
work, for example `works/ein-jahr-auf-moevenoog`. Which folder belongs to which work is shown by **Settings →
Data** in the table "Git mirror per work" at the bottom of the page (columns **Work** and **Folder**, plus the
identifier of the last entry under "last commit"). The **Data folder** itself is named at the top of the same
page. Ideas without a work have their own folder `einfaelle`.

The folder of a work holds its current state as Typst files: `main.typ`, one file per section in the subfolder
`abschnitte`, the ideas in `einfaelle.md` and, if the work has a work template, `vorlage.typ`.

You can read the history with the free program Git (Baruch itself does not need it):

```sh
cd DATA_FOLDER/works/ein-jahr-auf-moevenoog
git log --stat                                 # all entries, newest first, with the changed files
git log -p -- abschnitte/001-am-morgen.typ     # all changes to one section
git show HEAD~3                                # an older entry with everything it changed
```

The first line of an entry says what happened, for example `1.1 Am Morgen: Fassung 1 fortgeschrieben (Stand 1)`
(revision 1 continued, state 1) or `Einfall 1: angelegt` (idea 1 created). Below it stand markers such as
`Baruch-Node` (the section) and `Baruch-Action` (the kind of change). The entries are written in German,
independent of the language of the interface.

**Read only:**

- Change nothing in the folder, create no entries of your own and reset nothing. The database is what counts;
  Baruch continues the history from it, and whatever you change in the folder by hand does not count.
- Reading in a copy is safest, for example in a [backup](/en/hilfe/austausch-sicherung/#sicherung).
- You get a deleted text back by copying it from the entry of the deletion and saving it in the section as a new
  revision. A whole deleted work is brought back by the [archive](/en/hilfe/erste-schritte/#loeschen-archiv).
