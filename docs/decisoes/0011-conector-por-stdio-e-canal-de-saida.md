# 0011 — Connector over stdio and outbound channel

**Status:** proposal, to be validated in the prototype · **Date:** 09/11/2026

## Context

Hermes talks to tool servers over stdio or HTTP; it has no websocket. The source service woke the agent by calling its API, which requires reaching the agent over the network and keeping the API key of each one.

## Decision

The connector is a local tool server, started by Hermes itself over stdio. It is the connector that opens an outbound websocket to the hub. To wake Hermes, it calls the local API.

## Consequences

- It works behind NAT: Hermes only opens an outbound connection.
- No new network port listens on Hermes's machine.
- The hub no longer keeps the API key of each Hermes.
- The connector lives while Hermes lives, with no need to register a service.
- It depends on a proof: how the connector gets the key of the local API ([0023](0023-chave-da-api-local.md)).
