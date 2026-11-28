---
title: "Accounts and lectors"
description: "Setting up accounts, adding lectors and sharing single works with them – and what a lector may see and do."
weight: 130
---

As installed, Baruch runs **without accounts**: there is no sign-in; whoever reaches the interface is the author.
Accounts are only needed by those who want **lectors** to work along or want to make Baruch reachable from other
computers.

## Setting up accounts {#einrichten}

{{< hilfebild name="konten-einrichten" alt="The page Set up accounts with the fields name, display name and password and the button Create account; above it the explanation that this first account is the author." titel="The first account is the author." >}}

How it works:

1. **On the computer Baruch runs on,** choose **Baruch ▾ → Set up accounts…**. From another computer the page
   only shows an explanation – the first account can only be created on the own computer.
2. Enter a name (for signing in, at most 40 characters), a display name (optional) and a password (10 to 200
   characters).
3. Press **Create account**. This first account is the **author**.

From now on Baruch asks for a sign-in – for every page, **on the own computer as well**. Programs that work
through the [MCP access](/en/hilfe/einstellungen/#mcp-zugang) keep their own access keys.

Whoever wants to go on working on the own computer without a password switches on "On the own computer without
sign-in" under **Settings → Accounts**. Take the warning there seriously; what lies behind it is explained in
[Operation](/en/betrieb/#konten).

## Sign in, My account, Sign out {#anmelden}

{{< hilfebild name="konten-anmelden" alt="The sign-in page of Baruch with the fields name and password and the button Sign in; below it the link Already signed in? Continue." titel="With accounts every session starts on the sign-in page." >}}

- Without a sign-in every page leads to the sign-in page; afterwards you go on to the page you wanted.
- After several wrong passwords the page asks for patience; the waiting time grows up to 15 minutes.
- If you arrive through a link from an e-mail and see the sign-in page although you are signed in: click
  **Already signed in? Continue**. The browser holds back the sign-in on a jump from outside; this protects
  against foreign pages.
- The sign-in page follows the chosen theme (light or dark).

{{< hilfebild name="konten-mein-konto" alt="The page My account: name, role, origin of the own posts, the shared works and the form for changing the password." titel="My account shows the role and the shared works – and changes the password." >}}

**Baruch ▾** shows "Signed in: Name (Author)" or "(Lektor)", below it **My account** (name, role, shared works,
change password) and **Sign out**. A new password signs out all other devices of this account.

## Adding lectors and sharing works {#lektoren}

{{< hilfebild name="konten-einstellungen" alt="Settings, page Accounts: the list of accounts with the author and an invented lector, for each account the buttons Lock and Reset password, below them the form Add lector and the switch On the own computer without sign-in." titel="The author adds lectors under Settings → Accounts." >}}

How it works:

1. **Settings → Accounts → Add lector:** enter a name, display name and password. You pass the password on
   yourself.
2. Open the work and choose **Book → Contributors**.
3. Choose the lector and press **Share**.

{{< hilfebild name="konten-mitwirkende" alt="The page Contributors of a work: the choice of a lector with the button Share and the list of lectors the work is shared with, each with the button Withdraw." titel="Sharing is done per work." >}}

- At first a lector sees **no** work; sharing is done per work. **Withdraw** takes the share back.
- Under Settings → Accounts, accounts can be **locked** and **unlocked** and a **password reset**. Locking and
  withdrawing take effect at once.
- Accounts are never deleted, so that old proposals stay clearly assigned to a name. The last active account of
  the author cannot be locked.
- There is exactly one author; every further account is a lector.
- What a lector has written stays in the work – also after withdrawing.

### What a lector sees and may do {#lektor-rechte}

{{< hilfebild name="konten-lektor-arbeitsplatz" alt="The work page as a lector sees it: outline, text and right column as usual, but instead of Save the button Propose; the buttons for accepting, deleting and for assistants are missing." titel="A lector proposes – he does not save." >}}

In shared works a lector **reads**: the work page with the outline, all revisions and the chat, posts, threads,
search, findings, the history of the revisions, glossary, index and the pictures in the text.

A lector **writes** exactly three things, all under his name:

| What | Where | Note |
|---|---|---|
| Proposal of a revision | work page: change the text, **Propose** (instead of "Save"), note optional | never becomes the current revision by itself; the author accepts or discards it |
| Post in the chat of the section | on the right under "Chat": **Send** | change or remove own posts under "Manage"; calls no assistant |
| Finding | on the right under "Findings": **Write finding**; on the Findings page: **Write a note on the book** | kind: Note, Critical or Praise; the author decides about the status |

Everything else is left to the **author**: saving, accepting, discarding, switching, deleting and hiding
revisions, the outline, the status of findings, the assistants, literature, ideas, uploading pictures,
typesetting, PDF, e-book, Word, exchange and every other export, import, settings (except "Display") and quitting.
For lectors these buttons are missing. Whoever types the address of a blocked page by hand gets "Refused".

- For lectors the button in the Review tab is called **Search in the work**; only the author can replace.
- Lectors cannot fetch the book as a PDF at present.
- Versions, posts and findings of a lector show his display name before the handle, for example "Erika Example
  (@lektorat:erika)".

## Forgotten password {#passwort}

{{< hilfebild name="konten-einstellungen" alt="Settings, page Accounts: at the account of the lector the disclosure Reset password with a field for the new password." titel="The author resets the password of a lector." >}}

- **A lector:** the author sets the password anew under **Settings → Accounts → Reset password …**.
- **The author:** on the computer Baruch runs on, in the terminal

  ```sh
  baruch-server passwort NAME
  ```

  The command reads the new password as one line from the input, signs out all devices of the account and lifts
  a waiting time after failed attempts. Whoever has no access to the computer does not get in – there is no back
  door.

**What travels with a work:** accounts and shares belong to this one Baruch; an
[exchange file](/en/hilfe/austausch-sicherung/#ausgeben) brings no accounts along. Proposals, posts and findings
of a lector, on the other hand, belong to the work and travel with it.
