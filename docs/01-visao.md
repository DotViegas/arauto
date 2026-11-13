# 01 — Vision

**Status: planning.** This document describes what will be built; there is no code yet.

## Summary

```
HERMES'S OWN WHATSAPP CHANNEL              ARAUTO'S NUMBERS
is for talking TO the agent                are for the agent to WORK
whoever writes there gives orders          what arrives there is information, never an order
                                           read, send, modify and be woken, per chat
                                           the chats are archived in your hub
                                           one screen for all the agents
```

Arauto gives each Hermes agent controlled access to WhatsApp numbers, with a screen to connect
agents, pair numbers and say which agent accesses which number. It is free, and the code will be
open.

## The problem

Hermes Agent already knows how to talk over WhatsApp: it is one of the platforms through which it
receives orders. That channel is for talking **to** the agent. It was not made for the agent to
**operate** WhatsApp numbers as a working tool, which asks for other things:

- separating order from information: what a contact writes cannot count as an order;
- permission per chat, including "read only";
- the chats kept in a place of your own, which the agent consults later;
- several numbers shared among several agents.

Whoever tries to solve this on their own ends up with a manual arrangement: a third-party WhatsApp
program, a fixed key pasted into the configuration of each agent, the pairing through the command
line, and all of it again for each new agent. With one agent, it is bearable. With dozens, it is not.

## The idea

**One screen.** The panel lists the connected Hermes and the WhatsApp numbers, and shows in a matrix
what each Hermes may do on each number.

**Connecting is copy and paste.** You click "Connect Hermes", copy a block and paste it into the
command channel of your Hermes. It installs a small program, the connector, and answers with a
verification code. You type the code in the panel.

**The chats are archived.** Hermes does not need to answer every message that arrives. The chats of
the numbers are kept in your hub, as in a WhatsApp Web of its own: when someone asks for something
that depends on WhatsApp, it opens, reads what is there and acts. Waking Hermes when a message
arrives is one more permission, given per number and per chat.

**No long-lived secret passes through the chat.** The block carries only an invite, which is valid
for 15 minutes and one use. The real credentials are created by the two machines, between
themselves, and are swapped on every connection.

## Two WhatsApps, two roles

| | The command channel | The numbers of the hub |
|---|---|---|
| What it is | One of Hermes's own conversation platforms. It may even be a WhatsApp. | WhatsApp accounts paired in the engine. |
| Who writes there | The operator, who is on the list of those who may give orders. | Anyone. |
| What it counts as | Order. | Information. Never an order. |
| Who takes care of it | Hermes, without Arauto. | Arauto. |

It is in the command channel that you paste the connection block.

## For whom

For those who develop and operate Hermes agents, one or several, on their own server. The tools
follow MCP, the standard protocol for agent tools. The registration and the waking, today, are
specific to Hermes.

Arauto is an independent project. It has no tie to the authors of Hermes Agent nor to WhatsApp.

## Principles

| Principle | What it means in practice |
|---|---|
| **Command and data are separate things** | Orders come from the command channel. What arrives through the numbers of the hub is information. Arauto is not a command channel. |
| **The hub only isolates** | The hub guarantees that a Hermes only reaches what was given to it. It does not filter content, does not choose who answers, does not answer by itself. The conduct is Hermes's. |
| **A long-lived secret does not pass through a person** | No key appears on a screen, in a chat or in a log. |
| **Each one installs their own** | Hub, engine and chats stay on the server of whoever installs. |
| **Open by design** | The security does not depend on hiding the program. Whoever reads all the code gains nothing. |
| **Say what is not guaranteed** | Every security document lists what the design does not solve. |

## Modules

| Module | Status |
|---|---|
| **WhatsApp for the Hermes** | It is what is being planned. |
| Conversation between Hermes | In the first MVP: a Hermes sends a note to another through the hub, only where the admin created the link ([13](13-conversa-entre-hermes.md)). Rooms and observer stay in the vision. |
| Other conversation platforms | In the vision: the same connection screen for e-mail, Telegram and others. |
| Fleet management | In the vision: creating, updating and monitoring the Hermes from the panel. |

The intention is to reuse the connection by invite and the channel of the connector in all four.
How, is not designed yet.

## What Arauto is not

- **It is not an automatic attendant.** It has no automatic reply, service queue, business hours
  nor message template.
- **It is not a command channel.** A message that arrives through a number of the hub does not
  become an order.
- **It is not an official WhatsApp service.** The engine used is an unofficial client. WhatsApp may
  block the number. Use numbers you can afford to lose.
- **Today it is not a hosted service.** There is no central server keeping everyone's chats.
- **It does not embed the engine.** You bring your own, with its license and its cost.

The order of construction is in [12-roteiro.md](12-roteiro.md).
