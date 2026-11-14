# Facts checked on Easypanel

**Status: checked facts**, with the origin of each statement. What the Easypanel recipe of Arauto
(`deploy/easypanel/`) and its guide assume about Easypanel.

**Checked on 13/11/2026, on Easypanel 2.38.0 (image built on 22/10/2026), on a Compose service made
from `deploy/easypanel/compose.yaml`, with two domains.** Other versions were not checked.

Marking: **[observed]** seen on the screens of a running Easypanel, or read from them without
changing anything; **[to check]** an assumption with no test yet.

## Domains and the Open button

| Fact | Origin |
|---|---|
| The Domains screen of a Compose service lists each domain with the service and port it points to, and a star button per domain, "Make primary". One domain is the primary one (filled star). | [observed] |
| The Open button in the header of the service points to the primary domain. With two domains, it pointed to the first one created, which was the primary one. | [observed] the address of the Open link |
| Making another domain primary moves the Open button to it. | [to check] the button exists; the move was not done yet |

## Logs

| Fact | Origin |
|---|---|
| The Overview of a Compose service shows the logs of all its containers merged ("All"), and one button per service of the compose file (for example `hub` and `postgres`) to see only that one. | [observed] |
| The lines go from the oldest, at the top, to the newest, at the bottom. Each line starts with the container (`postgres-1 \|`) and a UTC time. | [observed] |
| The log viewer is a terminal drawn on a canvas: it has no search box, and the find of the browser does not see its text. | [observed] the page has one terminal with two canvases and no text rows |
| The Logs button in the header opens the same viewer in a window, with only "Add Divider", "Refresh" and "Close". | [observed] |
| How it shows the colors of a line written with ANSI codes. | [to check] |

## Console and templates

| Fact | Origin |
|---|---|
| The header of a Compose service has a Console button. | [observed] the button; it was not opened |
| Which container the console opens and with which shell. | [to check] |
| The project has a Templates tab. Whether it creates a Compose service from a JSON written by the project. | [to check] |
