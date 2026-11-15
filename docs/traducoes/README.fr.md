<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../marca/arauto-ia-escuro.svg">
    <img src="../marca/arauto-ia-claro.svg" alt="Arauto[IA]" width="440">
  </picture>
</p>

<p align="center">
  <a href="../../README.md">English</a> ·
  <a href="README.pt-BR.md">Português</a> ·
  <a href="README.es.md">Español</a> ·
  <b>Français</b> ·
  <a href="README.de.md">Deutsch</a> ·
  <a href="README.ru.md">Русский</a> ·
  <a href="README.hi.md">हिन्दी</a> ·
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.ja.md">日本語</a>
</p>

<p align="center">
  <b>Plusieurs agents Hermes qui travaillent comme une seule équipe,<br>
  chacun n'atteignant que ce qui lui revient.</b><br>
  Un hub pour connecter les agents, dire qui peut parler à qui<br>
  et donner à chacun son accès aux numéros WhatsApp.
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../marca/selos-fr-escuro.svg">
    <img src="../marca/selos-fr-claro.svg" alt="Gratuit. Licence Apache-2.0. Écrit en Rust. Premier MVP en construction." width="501">
  </picture>
</p>

<p align="center">
  <a href="../01-visao.md">Vision</a> ·
  <a href="../02-como-funciona.md">Fonctionnement</a> ·
  <a href="../04-seguranca-conexao.md">Sécurité de la connexion</a> ·
  <a href="../15-mvp.md">Le premier MVP</a> ·
  <a href="../glossario.md">Glossaire</a>
</p>

<p align="center">
  <img src="../marca/capa.webp" alt="La mascotte d'Arauto, de dos, en haut d'une colline, regardant le coucher du soleil" width="100%">
</p>

> **État : premier MVP en construction.** La conception se trouve dans ces documents ; le code en
> est à ses débuts et il n'existe pas encore de version utilisable ([docs/15-mvp.md](../15-mvp.md)).
> Le projet est gratuit et le code est ouvert, sous licence Apache-2.0
> ([décision 0015](../decisoes/0015-licenca.md)).

Hermes Agent est un agent d'IA de Nous Research. Arauto est un projet indépendant : il n'a aucun
lien avec les auteurs de Hermes Agent ni avec WhatsApp.

Les documents liés depuis cette page sont rédigés en anglais.

## Pourquoi Arauto

Un agent unique pour toute une entreprise sait tout, et peut finir par dire n'importe quoi à
n'importe qui. Demander à un modèle de langage de garder un secret est un risque : tôt ou tard,
il porte le contexte d'une équipe dans la réponse à une autre.

Arauto part de l'autre bout : **ce qu'un agent ne peut pas atteindre, il ne peut pas le divulguer.**

- **Un Hermes par équipe, chacun avec son propre accès.** L'agent qui s'occupe de la facturation
  ne détient pas ce que détient l'agent qui répond aux contacts. Chacun est configuré pour son
  propre travail.
- **Ils se parlent, là où vous l'avez permis.** Un Hermes envoie un message à un autre par le hub,
  uniquement par un lien que vous avez créé et uniquement dans le sens que vous avez indiqué. Celui
  qui le reçoit juge de ce qu'il faut faire, et ne partage que ce qui est nécessaire.
- **Un canal, des droits différents.** Un Hermes répond sur un numéro WhatsApp ; un autre se
  contente de lire les mêmes conversations, pour les auditer, et ne peut pas répondre.
- **Les personnes restent là où cela compte.** Le but est une entreprise dont le travail numérique
  est fait par des agents, avec des règles claires et une personne qui décide de ce qui est critique.

WhatsApp est le premier canal, pas le but. Le but, ce sont des agents qui travaillent ensemble sans
qu'aucun d'eux ne détienne tout.

### Où va le projet

Pas encore construit, et pas encore conçu dans le détail. Ce qui existe aujourd'hui se trouve dans
[le premier MVP](../15-mvp.md).

- **Une place pour chaque Hermes dans l'entreprise.** Une demande venue d'en bas est jugée par celui
  qui est au-dessus, pour le risque et pour l'autorisation ; une instruction venue d'en haut est
  reconnue comme telle. Aujourd'hui, un message n'a aucune autorité : c'est une information.
- **Des demandes avec réponse,** et une personne qui approuve en cours de route quand l'agent ne peut pas
  décider seul.
- **Un travail transmis par un seul canal.** Celui qui dirige parle à un Hermes, et celui-ci transmet
  le travail aux autres.
- **Davantage de canaux** à côté de WhatsApp.

## En un écran

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../marca/em-uma-tela-fr-escuro.svg">
    <img src="../marca/em-uma-tela-fr-claro.svg" width="772" alt="En trois étapes. 1 : dans le panneau, vous choisissez Connecter Hermes et vous copiez un court message pour l'envoyer dans le canal de commande de Hermes. 2 : il installe le connecteur et renvoie un code de vérification, que vous saisissez dans le panneau. 3 : vous cochez les accès et il se met à lire et à agir sur les numéros que vous avez autorisés.">
  </picture>
</p>

- **Connecter, c'est copier et envoyer.** Un court message à l'agent, et un code de vérification en
  retour. Une invitation à usage unique, valable 15 minutes. (Essayé sur deux vrais Hermes le
  14/11/2026, l'un sous Linux et l'autre sous Windows ; un bloc de commande reste en solution de repli.)
- **Aucun secret durable ne passe par le chat.** Les identifiants sont créés par les deux machines
  et changent à chaque connexion.
- **Les conversations sont archivées dans votre hub.** Hermes les consulte quand on le lui
  demande ; il n'a pas à répondre à tout.
- **Vous décidez, par numéro et par conversation,** de ce que chaque Hermes peut faire : lire,
  envoyer, modifier et être réveillé.
- **Chacun installe le sien.** Le hub, le moteur et les conversations restent sur votre serveur.

## Avant de l'utiliser

- Ce qui arrive par les numéros du hub est une **information, jamais un ordre**. Arauto n'est pas
  le canal par lequel vous commandez votre agent : cela reste le canal de commande de Hermes
  lui-même ([vision](../01-visao.md), « Two WhatsApps, two roles »).
- Le moteur WhatsApp est un **client non officiel**. WhatsApp peut bloquer le numéro. Utilisez des
  numéros que vous pouvez vous permettre de perdre.
- La sécurité a des limites connues, décrites dans [SECURITY.md](../../SECURITY.md) et dans
  [docs/05-modelo-de-ameacas.md](../05-modelo-de-ameacas.md).
- La conception n'a pas encore été éprouvée. Une partie de ce qui est décrit dépend des tests de
  la [Phase 1](../12-roteiro.md).

## À propos du moteur

Arauto ne parle pas à WhatsApp tout seul. Il pilote un programme tiers, le moteur. La conception a
été vérifiée avec WAHA ([ce qui a été vérifié](../../referencia/waha-fatos.md)).

Arauto **n'embarque pas** le moteur : vous installez le vôtre. Le coût et la licence du moteur
sont une affaire entre vous et son auteur. Vérifiez avant d'installer.

## Les pièces

| Pièce | Ce que c'est |
|---|---|
| **Hub** | Le serveur : le panneau, la porte des connecteurs et l'archive des conversations. Il pilote le moteur. |
| **Connecteur** | Un petit programme à côté de Hermes. Pour Hermes, c'est un serveur d'outils local. |
| **Moteur** | Le programme qui parle à WhatsApp. C'est le vôtre. |

## Documents

| Pour savoir | Lisez |
|---|---|
| Comment l'installer | [deploy/](../../deploy/README.md) |
| Ce que c'est et ce que ce n'est pas | [docs/01-visao.md](../01-visao.md) |
| Le parcours de l'utilisateur | [docs/02-como-funciona.md](../02-como-funciona.md) |
| Les pièces et les frontières | [docs/03-arquitetura.md](../03-arquitetura.md) |
| Comment un Hermes entre, revient et sort | [docs/04-seguranca-conexao.md](../04-seguranca-conexao.md) |
| Ce qui peut mal tourner | [docs/05-modelo-de-ameacas.md](../05-modelo-de-ameacas.md) |
| Les tables | [docs/06-modelo-de-dados.md](../06-modelo-de-dados.md) |
| Les écrans | [docs/07-telas.md](../07-telas.md) |
| Ce que chaque Hermes peut faire | [docs/08-permissoes.md](../08-permissoes.md) |
| Les outils que voit Hermes | [docs/09-contrato-mcp.md](../09-contrato-mcp.md) |
| États et délais | [docs/10-maquinas-de-estado.md](../10-maquinas-de-estado.md) |
| Le connecteur | [docs/11-conector.md](../11-conector.md) |
| L'ordre de construction | [docs/12-roteiro.md](../12-roteiro.md) |
| Comment un Hermes parle à un autre | [docs/13-conversa-entre-hermes.md](../13-conversa-entre-hermes.md) |
| Ce qui passe entre le connecteur et le hub | [docs/14-protocolo-do-canal.md](../14-protocolo-do-canal.md) |
| Le premier MVP : ce qu'il contient et comment le lancer | [docs/15-mvp.md](../15-mvp.md) |
| Le nom de chaque chose | [docs/glossario.md](../glossario.md) |
| La raison de chaque choix | [docs/decisoes/](../decisoes/) |
| Ce qui a été vérifié dans Hermes et dans le moteur | [referencia/](../../referencia/) |

Commencez par la vision et par le « fonctionnement ». Le glossaire explique les termes.

## Comment lire les documents

Chaque document indique son état à la première ligne : **planification** (*planejamento*) ou
**proposition** (*proposta*). À l'intérieur :

- un **fait** (*fato*) est ce qui a été vérifié, avec version et date ;
- une **proposition** (*proposta*) est une conception pas encore éprouvée ;
- une **décision en attente** (*decisão pendente*) est ce qui reste à choisir.

Les décisions ont trois états : décidée, proposée et en attente.

## Licence, signalement de faille et maintenance

- **Licence :** Apache-2.0 ([LICENSE](../../LICENSE), [décision 0015](../decisoes/0015-licenca.md)).
- **Signalement de faille :** par le signalement privé de GitHub, quand le dépôt sera ouvert
  ([SECURITY.md](../../SECURITY.md)).
- **Qui maintient le projet et comment contribuer :** à définir avant l'ouverture du dépôt au
  public.

## Soutenir le projet

<p align="center">
  <img src="../marca/cafe.webp" alt="La mascotte d'Arauto, une tasse de café à la main, qui salue à la porte d'un café, à côté d'une ardoise où l'on lit buy me coffee" width="600">
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../marca/em-breve-fr-escuro.svg">
    <img src="../marca/em-breve-fr-claro.svg" alt="bientôt" width="96">
  </picture>
</p>

<p align="center">
  Une page Buy Me a Coffee est en préparation.<br>
  Arauto est gratuit : le café sera une façon de dire merci, pas un prix.
</p>

<p align="center">
  <img src="../marca/mascote-pixel-acenando.svg" alt="La mascotte d'Arauto, en pixels, qui salue" width="108">
</p>

<p align="center"><sub>« Arauto » signifie héraut en portugais : celui qui porte et annonce les messages.</sub></p>
