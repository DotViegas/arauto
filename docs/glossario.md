# Glossary

**Status: proposal.** One name for each thing. The other documents use these terms.

## The parts

| Term | What it is |
|---|---|
| **Arauto** | The project. It has two programs, the hub and the connector, and uses a third one, the engine. |
| **Hermes** | An AI agent (Hermes Agent). In the panel it is a line: the logical identity of the agent, which owns the accesses. |
| **Hermes's LLM** | The language model that decides what Hermes does and runs commands in its terminal. |
| **Hub** | The Arauto server. It has the panel, the port of the connectors and the vault, and talks to the engine. |
| **Panel** | The screens of the hub, used by people. |
| **Connector** | A small program that runs beside Hermes. To Hermes, it is a local tool server. |
| **Engine** | The program that talks to WhatsApp. Today, the WAHA of whoever installs. |
| **Message archive** | The chats kept in the hub. |
| **MCP** | The standard protocol through which an agent uses outside tools. |
| **stdio** | A way for two programs to talk through text input and output, with no network. It is how Hermes talks to the connector. |
| **Hermes's local API** | The address, on Hermes's own machine, through which a run of it is started. |

## The people

| Term | What it is |
|---|---|
| **Admin** | A person with a login in the panel. |
| **Operator** | Whoever gives orders to Hermes through the command channel. It may be the admin. |
| **Organization** | The owner of Hermes, numbers and admins. An installation has one; the database already separates by organization. |

## Command and data

| Term | What it is |
|---|---|
| **Command channel** | Where the operator gives orders to Hermes. It exists outside Arauto and does not go through it. |
| **Hermes's conversation platforms** | The platforms on which Hermes talks: the command channel and the others. |
| **Number** | A WhatsApp account paired in the engine. |
| **Alias** | The name the admin gives a number in the panel. |
| **Pair** | To link a number to the engine, with the phone. It applies only to numbers. |
| **Pairing code, QR** | What the phone reads or types to pair. A short-lived secret. |
| **Linked device** | Each device or program linked to a WhatsApp account, as it appears on the phone. The engine is one of them. |
| **Chat** | A contact or a group inside a number. |

## Connecting a Hermes

| Term | What it is |
|---|---|
| **Connect** | To link a Hermes to the hub. It applies only to Hermes. |
| **Block** | The text the admin copies from the panel and pastes into Hermes's command channel. |
| **Invite** | A temporary secret that goes inside the block. It is valid for 15 minutes and for one use. It only serves to ask for a connection. |
| **Enrollment** | The moment a connector presents an invite to the hub. |
| **Connection request** | What an enrollment creates. It gives access to nothing until the admin types the code. |
| **Verification code** | Six digits the connector shows and the admin types in the panel. It is not a secret. |
| **Device** | An installation of the connector, with its identity. A Hermes has at most one active device. |
| **Device identity** | A key pair created on Hermes's machine. The private part never leaves it. |
| **Sealed credential** | An envelope issued by the vault, which the connector keeps and cannot open. To use the hub it has to be swapped for another one. |
| **Seal** | The cipher that closes the credential. Only the hub opens it. |
| **Generation** | The sequence number of each credential issued to a device. The hub also compares the digest of the credential, because after a restoration a number may repeat. |
| **Generation zero** | An approved device that has not confirmed its first credential yet. |
| **Vault** | The part of the hub that draws random values and seals. |
| **Master key** | The key of the vault. It stays outside the database. |
| **Hub key** | The key pair of the hub. The public part goes in every block: it is how the connector recognizes the right hub. |
| **Release key** | The project key that signs each published version of the connector. It is in no hub. |
| **Channel** | The outbound connection from the connector to the hub, encrypted end to end. |
| **Handshake** | The initial exchange in which connector and hub recognize each other and agree on the key of the conversation. |
| **Channel session** | A period in which the channel is open and the credential of that session has already been swapped. |
| **Heartbeat** | A periodic message that shows the other side is still there. |
| **Leader** | The only copy of the connector that holds the channel for a Hermes. |
| **Relay** | A copy of the connector that is not the leader: it relays the calls to the leader. |
| **Reuse** | Presenting a credential that was already swapped, or none when there should be one. A sign of a copy. |
| **Double connection** | Two live connections of the same device at the same time. A sign of a copy. |
| **Taken for dead** | An old connection that did not answer the hub's check and was closed so that another could take its place. |
| **Quarantine** | A device stopped because the hub no longer trusts its credential: there was a sign of a copy, or the file was lost. It only comes back with a new invite. |
| **Suspended** | A device stopped because the hub came back from a backup. It only comes back with a new invite. |
| **Restored mode** | The state of the hub after it notices it came back from an old backup. It lasts until no suspended device is left. |
| **Audit mark** | The number of the last security event, kept outside the database. A database behind the mark is a restored database. |
| **Revoke** | To close a Hermes in the hub: the device, the request in progress and the issued invite. The accesses are kept. The admin revokes, in the panel; a device may also ask for its own closing. |
| **Local leave** | Erasing the files of the connector without talking to the hub. It is for a person, when the hub no longer exists or changed its key. |
| **Reconnect** | To issue a new invite for a Hermes that already exists. The accesses are kept. |
| **Quick mode** | Hermes itself installs the connector. The credential stays where the LLM reaches it. |
| **Shielded mode** | A person installs the connector apart from Hermes. The LLM does not reach the credential. Planned, not in the first version. |
| **Digest** | The result of a hash function (SHA-256) over a piece of data. It serves to check without keeping the data. |
| **Current and pending** | The two credentials the hub tracks per device: the one in force, and the one it has just issued and that has not been confirmed yet. |
| **Swap** | The step in which the connector presents the credential it has and receives another. "Saved" is the connector's confirmation that the new one is on disk. |
| **Waiting** | A channel connection that proved its identity and waits: the request has not been approved yet. |
| **Established connection** | A channel connection that is waiting or in session. It is the one the hub checks before treating another as a double connection. |
| **Origin** | Where a connection comes from, for the ceilings of the hub: the IPv4 address, or the first 64 bits of the IPv6 address. Not to be confused with the source service. |
| **Sweep** | The task of the hub that, every 2 seconds, checks the mark, applies the deadlines and rereads the state of the devices with an open connection. It is what guarantees a revocation in under 5 seconds. |
| **Orderly stop** | The way the hub stops: it stops accepting connections, lets the calls in progress answer and sends `closing` to each connection before leaving. |
| **Short connection** | A connection of `join` or of `leave`: it proves the identity, carries one request and closes. It is not a channel. |
| **Request in progress** | A device that has not confirmed its first credential yet: pending or approved, even if it went to quarantine before confirming. |
| **Device in use** | A device that has already confirmed a credential: active, suspended, or in quarantine after being active. |
| **Closed** | A device in a final state: denied, expired, replaced or revoked. |
| **Alert** | A warning from the panel to the admin. It is not the notice that wakes Hermes. |
| **Proof of restoration** | A sign that the hub's database went back in time: a database behind the mark, or a valid seal the database does not explain. |
| **Lost timeline** | What happened between the backup and the restoration, and that the restored database does not know. |

## The use

| Term | What it is |
|---|---|
| **Access** | What a Hermes may do on a number. |
| **Verb** | Each thing an access allows: read, send, modify, be woken. |
| **Scope** | Whether the access applies to every chat of the number or only to the chosen ones. |
| **Chat line** | An access exception for one chat. |
| **Matrix** | The screen that crosses Hermes and numbers and shows the accesses. |
| **Notice** | A signal from the hub that there is a new message in a chat, or a new note from another Hermes. It is a pointer: it does not carry the content. |
| **Fixed sentence** | The text, kept in the connector, that wakes Hermes. |
| **Wake** | The connector starts a run in Hermes from a notice. |
| **Audit** | The record, which only grows, of what each one did in the hub. No message content. |
| **System log** | The technical log of the hub. |
| **Purge** | Deleting what passed its retention period. |
| **Passkey** | The access key of the phone or the computer, used in place of a password. |
| **Installation code** | A single-use code that only opens the registration of a passkey in the panel. Before the first access (no passkey was ever registered) the hub writes one in the output of its start and writes it again every 10 minutes: it has no time limit, the block with the latest time is the valid one, unless a code from the command replaced it (the hub then writes once that the code was replaced, without any code). Creating the first passkey spends it, and each start replaces it. The one the start writes is called the start code; the one the command issues, the command code. A command on the hub's machine issues another, valid for 10 minutes to be typed, for who lost the passkey or cannot open the log. The hub keeps only the digest of a code ([decision 0040](decisoes/0040-painel-dentro-do-hub.md)). |

## Conversation between Hermes

| Term | What it is |
|---|---|
| **Note** | A text message from one Hermes to another, delivered by the hub. It is data, never an order. |
| **Link** | An entry of the peer list: from one Hermes to another, in one direction only. |
| **Peer list** | The set of links of an organization. Only the admin changes it. |
| **Peer** | For a Hermes, another Hermes it has a link with. |
| **Thread** | A sequence of notes between the same two Hermes, about the same subject. |
| **Ceiling** | A fixed limit on quantity, which the hub applies without looking at the content. |

## Sessions

The word "session" always comes with its owner:

| Term | What it is |
|---|---|
| **Channel session** | See above. |
| **Engine session** | The link between the engine and a WhatsApp account. Whoever copies it has the number. |
| **Panel session** | The login of an admin in the browser. |

## Origin

| Term | What it is |
|---|---|
| **Source service** | The private program the project was born from. It linked Hermes agents to WhatsApp by manual configuration. |
| **WAHA** | The third-party program used today as the engine. It is an unofficial WhatsApp client. |
| **Noise** | A well-known family of handshakes for encrypted channels. |

## Words we do not use

| Do not use | Use | Reason |
|---|---|---|
| ticket, token, Hermes key | sealed credential | "Ticket" was once confused with the invite. |
| connection link | invite, block | The invite is not an address: it goes as an argument of the connector. |
| pair a Hermes | connect a Hermes | Pairing is for numbers. |
| tenant, slot | Hermes, organization | They were names from the source service. |
| device, for WhatsApp | linked device | A device is an installation of the connector. |
| channel, for Telegram or e-mail | conversation platform | The channel is the connection from the connector to the hub. |
| customer, client, meaning whoever uses the project | whoever installs, organization | The project is free. The technical sense ("unofficial client", "MCP client") still holds. |

## From Portuguese

The documents were written in Portuguese until 13/11/2026. The terms above replaced these:

| Portuguese | English |
|---|---|
| convite · bloco · inscrição · pedido de conexão | invite · block · enrollment · connection request |
| código de conferência · código de instalação | verification code · installation code |
| aparelho · credencial lacrada · lacre · caixa preta · chave-mestra | device · sealed credential · seal · vault · master key |
| conectar · parear · reconectar · revogar · acordar | connect · pair · reconnect · revoke · wake |
| canal · aperto de mãos · batimento · troca · "gravei" | channel · handshake · heartbeat · swap · "saved" |
| líder · repassadora · reuso · conexão dupla · dada por morta | leader · relay · reuse · double connection · taken for dead |
| quarentena · suspenso · modo restaurado · marca da auditoria | quarantine · suspended · restored mode · audit mark |
| varredura · expurgo · teto · resumo | sweep · purge · ceiling · digest |
| número · apelido · conversa · linha de conversa · escopo · verbo · matriz | number · alias · chat · chat line · scope · verb · matrix |
| aviso · alerta · frase fixa · recado · ligação · par · fio | notice · alert · fixed sentence · note · link · peer · thread |
| motor · painel · arquivo de mensagens · registro do sistema | engine · panel · message archive · system log |
| serviço de origem · quem instala · meio de conversa | source service · whoever installs · conversation platform |
