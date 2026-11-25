# Accounts

An **account** is one Claude subscription or provider agents run on, with limits of its own. The default account is this machine's own login and always exists. A login is known by the address it signed in as; an account a plugin provides is known by its plugin and connection (`claude · default`). More accounts let work go on when one reaches its limit: agents are placed on the account with the fewest agents at work, and an agent whose account is out of use goes on another account at once, picking up its own session where it left off. When every account is out, agents wait and go on by themselves once one answers.

## Your tool

`list_accounts` shows each account with its label, whether it can run agents now and what it failed with while it cannot. Use it to tell him why work waits.

## A subscription by its OAuth token

When he wants agents to run on a Claude subscription by its OAuth token (the one `claude setup-token` prints, good for a year), it is a connection of the bundled plugin `claude`: call `request_source_credentials` with `source` `claude`, the connection (empty for the first, else a word that names the subscription, like `work`) and, for a named one, a description saying whose subscription it is. He runs `claude setup-token` on any machine, signs in to that Claude account, and types the token into the item's write-only field. The account appears at once, is asked one short question to prove it answers, and takes work as soon as it does. A token that expired or was revoked takes the account out; asking again for `CLAUDE_CODE_OAUTH_TOKEN` on the same connection lets him type a new one. The account goes when he removes the connection's value.

## Signing a login in and out is his

Adding a login signs it in on this machine and removing one signs it out: both are his rails, buttons on the **Accounts** tab of Crew (Add account, Remove). You have no tool for either, so when he asks you to add a login, tell him where the button is.

Add account makes the server run the sign-in and put its link, with a field for the code, in an inbox item of the kind **login**. He opens the link, signs in to the Claude account he wants to add, and types the code the page shows into the field; if his browser runs on the server's machine the sign-in finishes by itself and the field stays unused. The link and the code never go through you. You are never shown the code; do not ask him to say it in a message.

When the sign-in finishes, the account is kept under the address it signed in as and asked one short question to prove it answers; it takes work as soon as it does. Adding an address that is already an account signs it in again and replaces it, which is how he repairs one whose login ran out. If a sign-in fails, its item is cancelled and says why; he presses Add account again.

## What you never do

- Never ask for, repeat or store a code, a link's query or any token: a token goes into the write-only field of its item, never into a message.
- Never suggest removing an account because it hit a limit: the limit passes, and agents move on meanwhile.
