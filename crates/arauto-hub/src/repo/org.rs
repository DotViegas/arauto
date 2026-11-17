//! The org chart of the Hermes, stage A (`docs/decisoes/0043-organograma-e-niveis.md`): the
//! levels, the place and the role of each Hermes, and Publish, which writes the whole wanted
//! state, links included, in one write.
//!
//! **Only the panel and the admin command reach this module** (a test reads the code for the
//! words that name its tables). What it stores is for the admin: no function of the channel,
//! of a notice or of a tool reads it.
//!
//! **A read never writes.** [`chart`] reads in one snapshot, so the version it gives is one
//! the whole chart had at some instant.
//!
//! **Publish is one transaction.** It holds the lock of the chart and the rows of every live
//! Hermes (in the order of their identifiers, like every writer of a link), reads the chart
//! under those locks, and refuses a draft that started from another version
//! ([`PublishOutcome::Stale`]): nothing is applied partially. The links go in through the same
//! code the admin command runs ([`super::peer::set_locked`] and
//! [`super::peer::remove_locked`]), so they discard the same notices, write the same security
//! events and tell the hub the same way.

use std::collections::{BTreeMap, BTreeSet};

use arauto_protocol::names::HermesName;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::peer::{LinkRemoved, LinkSet, remove_locked, set_locked};
use super::{Admin, RepoError};
use crate::audit::{self, Action, Details, Event, TargetKind};
use crate::core::org::{
    Chart, ChartVersion, Draft, LevelKey, LevelName, MapX, OrgInvalid, Place, RoleText,
    StoredLevel, StoredLink, StoredMember,
};
use crate::db::ResolvedOrg;
use crate::mark::Mark;

/// What a publish changed, in numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Published {
    /// The version of the chart as written.
    pub version: ChartVersion,
    /// Levels created.
    pub levels_added: u32,
    /// Levels deleted.
    pub levels_removed: u32,
    /// Levels with another name.
    pub levels_renamed: u32,
    /// Levels in another position.
    pub levels_moved: u32,
    /// Hermes with another level or place, or placed for the first time.
    pub members_changed: u32,
    /// Hermes with another role.
    pub roles_changed: u32,
    /// Links created.
    pub links_created: u32,
    /// Links with another wake mark or another follow mark.
    pub links_changed: u32,
    /// Links removed.
    pub links_removed: u32,
}

/// How [`publish`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishOutcome {
    /// It was written.
    Published(Published),
    /// The draft is what the chart already is: nothing was written.
    Unchanged,
    /// The chart changed after the draft started: nothing was written.
    Stale,
    /// The draft does not fit the state of the database: nothing was written.
    Invalid(OrgInvalid),
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn stored_name(text: &str) -> Result<HermesName, RepoError> {
    HermesName::parse(text).map_err(|_| RepoError::Inconsistent("the name of a Hermes"))
}

/// A Hermes with the place it has, if it has one: its row, its name, the level, the left edge and
/// the role.
type MemberRow = (Uuid, String, Option<Uuid>, Option<i32>, Option<String>);

/// Reads the chart of the organization with the connection it is given.
async fn read(
    connection: &mut PgConnection,
    organization: ResolvedOrg,
) -> Result<Chart, RepoError> {
    let levels: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM org_level WHERE organization_id = $1 ORDER BY position",
    )
    .bind(organization.id())
    .fetch_all(&mut *connection)
    .await?;
    let members: Vec<MemberRow> = sqlx::query_as(
        "SELECT h.id, h.name, m.level_id, m.map_x, m.role \
         FROM hermes h LEFT JOIN org_member m ON m.hermes_id = h.id \
         WHERE h.organization_id = $1 AND h.deleted_at IS NULL \
         ORDER BY h.id",
    )
    .bind(organization.id())
    .fetch_all(&mut *connection)
    .await?;
    let links: Vec<(Uuid, String, Uuid, String, bool, bool)> = sqlx::query_as(
        "SELECT l.from_hermes_id, f.name, l.to_hermes_id, t.name, l.wake, l.follow \
         FROM peer_link l \
         JOIN hermes f ON f.id = l.from_hermes_id \
         JOIN hermes t ON t.id = l.to_hermes_id \
         WHERE l.organization_id = $1 AND f.deleted_at IS NULL AND t.deleted_at IS NULL \
         ORDER BY l.from_hermes_id, l.to_hermes_id",
    )
    .bind(organization.id())
    .fetch_all(&mut *connection)
    .await?;
    let inconsistent = || RepoError::Inconsistent("a row of the org chart");
    Ok(Chart {
        levels: levels
            .into_iter()
            .map(|(id, name)| {
                Ok(StoredLevel {
                    id,
                    name: LevelName::parse(&name).map_err(|_| inconsistent())?,
                })
            })
            .collect::<Result<_, RepoError>>()?,
        members: members
            .into_iter()
            .map(|(id, name, level, x, role)| {
                let place = match (level, x) {
                    (Some(level), Some(x)) => Some(Place {
                        level,
                        x: MapX::new(i64::from(x)).map_err(|_| inconsistent())?,
                        role: match role {
                            Some(role) => RoleText::parse(&role).map_err(|_| inconsistent())?,
                            None => None,
                        },
                    }),
                    _ => None,
                };
                Ok(StoredMember {
                    id,
                    name: stored_name(&name)?,
                    place,
                })
            })
            .collect::<Result<_, RepoError>>()?,
        links: links
            .into_iter()
            .map(|(from_id, from, to_id, to, wake, follow)| {
                Ok(StoredLink {
                    from_id,
                    from: stored_name(&from)?,
                    to_id,
                    to: stored_name(&to)?,
                    wake,
                    follow,
                })
            })
            .collect::<Result<_, RepoError>>()?,
    })
}

/// The chart of the organization, read in one snapshot. Never writes.
///
/// # Errors
/// [`RepoError::Database`] when the database refuses.
pub async fn chart(pool: &PgPool, organization: ResolvedOrg) -> Result<Chart, RepoError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *transaction)
        .await?;
    let chart = read(&mut transaction, organization).await?;
    transaction.rollback().await?;
    Ok(chart)
}

/// What a level of the wanted state is, once its key is resolved.
struct Resolved {
    id: Uuid,
    new: bool,
}

/// Writes the wanted state of the chart in one write, if the draft started from `base`.
///
/// # Errors
/// [`RepoError::Restored`] when the database is behind the audit mark: nothing is written.
/// [`RepoError::Audit`] with [`audit::AuditError::MarkNotAdvanced`] means the publish **is
/// committed**.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub async fn publish(
    pool: &PgPool,
    mark: &Mark,
    organization: ResolvedOrg,
    admin: &Admin,
    draft: &Draft,
    base: ChartVersion,
    now: i64,
) -> Result<PublishOutcome, RepoError> {
    let mut write = audit::begin_security(pool, mark).await?;
    // One publisher of a chart at a time. Nobody else takes this lock.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 43))")
        .bind(organization.id().to_string())
        .execute(write.connection())
        .await?;
    // Every live Hermes, in the order of the identifiers: the order every writer of a link takes
    // two of them in, so none can deadlock with this one, and no link of the organization
    // changes while these locks are held.
    let live: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM hermes WHERE organization_id = $1 AND deleted_at IS NULL \
         ORDER BY id FOR NO KEY UPDATE",
    )
    .bind(organization.id())
    .fetch_all(write.connection())
    .await?;
    let before = read(write.connection(), organization).await?;
    if before.version() != base {
        write.rollback().await?;
        return Ok(PublishOutcome::Stale);
    }
    let invalid = |reason| PublishOutcome::Invalid(reason);

    // The state checks.
    let hermes_ids: BTreeMap<&str, Uuid> =
        live.iter().map(|(id, name)| (name.as_str(), *id)).collect();
    let wanted_names: BTreeSet<&str> = draft
        .hermes
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    let live_names: BTreeSet<&str> = hermes_ids.keys().copied().collect();
    if wanted_names != live_names {
        write.rollback().await?;
        return Ok(invalid(OrgInvalid::Unknown));
    }
    let mut levels: Vec<Resolved> = Vec::with_capacity(draft.levels.len());
    for wanted in &draft.levels {
        match wanted.key {
            LevelKey::Stored(id) => {
                if before.position_of(id).is_none() {
                    write.rollback().await?;
                    return Ok(invalid(OrgInvalid::Unknown));
                }
                levels.push(Resolved { id, new: false });
            }
            LevelKey::New(_) => levels.push(Resolved {
                id: Uuid::new_v4(),
                new: true,
            }),
        }
    }
    let level_of = |key: LevelKey| {
        draft
            .levels
            .iter()
            .position(|level| level.key == key)
            .map(|index| levels[index].id)
    };

    // The diff.
    let mut summary = Published {
        version: base,
        levels_added: 0,
        levels_removed: 0,
        levels_renamed: 0,
        levels_moved: 0,
        members_changed: 0,
        roles_changed: 0,
        links_created: 0,
        links_changed: 0,
        links_removed: 0,
    };
    struct LevelChange {
        id: Uuid,
        added: bool,
        renamed: bool,
        moved: bool,
        before: Option<usize>,
        after: usize,
    }
    let mut level_changes: Vec<LevelChange> = Vec::new();
    for (index, (wanted, resolved)) in draft.levels.iter().zip(&levels).enumerate() {
        if resolved.new {
            summary.levels_added += 1;
            level_changes.push(LevelChange {
                id: resolved.id,
                added: true,
                renamed: false,
                moved: false,
                before: None,
                after: index,
            });
            continue;
        }
        let position = before.position_of(resolved.id);
        let renamed = position.is_some_and(|at| before.levels[at].name != wanted.name);
        let moved = position != Some(index);
        if renamed || moved {
            summary.levels_renamed += u32::from(renamed);
            summary.levels_moved += u32::from(moved);
            level_changes.push(LevelChange {
                id: resolved.id,
                added: false,
                renamed,
                moved,
                before: position,
                after: index,
            });
        }
    }
    let kept: BTreeSet<Uuid> = levels.iter().map(|level| level.id).collect();
    let removed_levels: Vec<(usize, Uuid)> = before
        .levels
        .iter()
        .enumerate()
        .filter(|(_, level)| !kept.contains(&level.id))
        .map(|(at, level)| (at, level.id))
        .collect();
    summary.levels_removed = count(removed_levels.len());

    struct MemberChange<'a> {
        id: Uuid,
        name: &'a HermesName,
        level: Uuid,
        x: MapX,
        role: Option<&'a RoleText>,
        placed: bool,
        level_before: Option<Uuid>,
        level_changed: bool,
        x_changed: bool,
        role_changed: bool,
    }
    let mut member_changes: Vec<MemberChange<'_>> = Vec::new();
    for wanted in &draft.hermes {
        let Some(&id) = hermes_ids.get(wanted.name.as_str()) else {
            write.rollback().await?;
            return Ok(invalid(OrgInvalid::Unknown));
        };
        let Some(level) = level_of(wanted.level) else {
            write.rollback().await?;
            return Ok(invalid(OrgInvalid::Unknown));
        };
        let stored = before
            .members
            .iter()
            .find(|member| member.id == id)
            .and_then(|member| member.place.as_ref());
        let role_changed = stored.and_then(|place| place.role.as_ref()) != wanted.role.as_ref();
        let level_changed = stored.is_some_and(|place| place.level != level);
        let x_changed = stored.is_some_and(|place| place.x != wanted.x);
        let placed = stored.is_none();
        if placed || level_changed || x_changed || role_changed {
            summary.members_changed += 1;
            summary.roles_changed += u32::from(role_changed);
            member_changes.push(MemberChange {
                id,
                name: &wanted.name,
                level,
                x: wanted.x,
                role: wanted.role.as_ref(),
                placed,
                level_before: stored.map(|place| place.level),
                level_changed,
                x_changed,
                role_changed,
            });
        }
    }

    let current: BTreeMap<(&str, &str), &StoredLink> = before
        .links
        .iter()
        .map(|link| ((link.from.as_str(), link.to.as_str()), link))
        .collect();
    let mut removals: Vec<&StoredLink> = Vec::new();
    for link in &before.links {
        let kept = draft
            .links
            .iter()
            .any(|wanted| wanted.from == link.from && wanted.to == link.to);
        if !kept {
            removals.push(link);
        }
    }
    summary.links_removed = count(removals.len());
    // A follow mark needs the wake mark: refused before any write. Where the levels let it be
    // is checked after the writes, against the positions as they will be.
    if draft.links.iter().any(|link| link.follow && !link.wake) {
        write.rollback().await?;
        return Ok(invalid(OrgInvalid::Follow));
    }
    // The sender and the recipient with their rows, and the wake and follow marks wanted.
    type LinkSets<'a> = Vec<(&'a HermesName, Uuid, &'a HermesName, Uuid, (bool, bool))>;
    let mut sets: LinkSets<'_> = Vec::new();
    for wanted in &draft.links {
        let (Some(&from_id), Some(&to_id)) = (
            hermes_ids.get(wanted.from.as_str()),
            hermes_ids.get(wanted.to.as_str()),
        ) else {
            write.rollback().await?;
            return Ok(invalid(OrgInvalid::Unknown));
        };
        match current.get(&(wanted.from.as_str(), wanted.to.as_str())) {
            None => summary.links_created += 1,
            Some(stored) if (stored.wake, stored.follow) == (wanted.wake, wanted.follow) => {
                continue;
            }
            Some(_) => summary.links_changed += 1,
        }
        sets.push((
            &wanted.from,
            from_id,
            &wanted.to,
            to_id,
            (wanted.wake, wanted.follow),
        ));
    }

    if level_changes.is_empty()
        && removed_levels.is_empty()
        && member_changes.is_empty()
        && removals.is_empty()
        && sets.is_empty()
    {
        write.rollback().await?;
        return Ok(PublishOutcome::Unchanged);
    }

    // The writes: levels, members, levels that go, links, events.
    for change in &level_changes {
        let name = draft.levels[change.after].name.as_str();
        let position = i32::try_from(change.after).unwrap_or(i32::MAX);
        if change.added {
            sqlx::query(
                "INSERT INTO org_level (id, organization_id, position, name, created_at, changed_at) \
                 VALUES ($1, $2, $3, $4, $5, $5)",
            )
            .bind(change.id)
            .bind(organization.id())
            .bind(position)
            .bind(name)
            .bind(now)
            .execute(write.connection())
            .await?;
        } else {
            sqlx::query(
                "UPDATE org_level SET name = $3, position = $4, changed_at = $5 \
                 WHERE organization_id = $1 AND id = $2",
            )
            .bind(organization.id())
            .bind(change.id)
            .bind(name)
            .bind(position)
            .bind(now)
            .execute(write.connection())
            .await?;
        }
    }
    for change in &member_changes {
        sqlx::query(
            "INSERT INTO org_member (hermes_id, organization_id, level_id, map_x, role, changed_at) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (hermes_id) DO UPDATE SET level_id = EXCLUDED.level_id, \
                 map_x = EXCLUDED.map_x, role = EXCLUDED.role, changed_at = EXCLUDED.changed_at",
        )
        .bind(change.id)
        .bind(organization.id())
        .bind(change.level)
        .bind(i32::from(change.x.get()))
        .bind(change.role.map(RoleText::as_str))
        .bind(now)
        .execute(write.connection())
        .await?;
    }
    for (_, id) in &removed_levels {
        sqlx::query("DELETE FROM org_level WHERE organization_id = $1 AND id = $2")
            .bind(organization.id())
            .bind(id)
            .execute(write.connection())
            .await?;
    }
    for link in &removals {
        let done = remove_locked(
            &mut write,
            organization,
            admin,
            (&link.from, link.from_id),
            (&link.to, link.to_id),
            now,
        )
        .await?;
        if done != LinkRemoved::Removed {
            return Err(RepoError::Inconsistent(
                "a link read under the lock is gone",
            ));
        }
    }
    for (from, from_id, to, to_id, marks) in sets {
        let done = set_locked(
            &mut write,
            organization,
            admin,
            (from, from_id),
            (to, to_id),
            marks,
            now,
        )
        .await?;
        if matches!(
            done,
            LinkSet::Unchanged | LinkSet::Missing(_) | LinkSet::SameHermes
        ) {
            return Err(RepoError::Inconsistent(
                "a link read under the lock changed",
            ));
        }
    }
    sqlx::query("SET CONSTRAINTS org_level_name_once, org_level_position_once IMMEDIATE")
        .execute(write.connection())
        .await?;
    // The follow marks, against the positions as they now are (decision 0050). It runs on every
    // publish that writes: a moved card or level can leave a link that already was followed
    // without a higher sender or without a lower receiver. A mark is off a link that goes sideways,
    // up, or touches a Hermes without a level.
    let broken: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
           SELECT 1 FROM peer_link l \
           LEFT JOIN org_member mf ON mf.hermes_id = l.from_hermes_id \
           LEFT JOIN org_level  vf ON vf.id = mf.level_id \
           LEFT JOIN org_member mt ON mt.hermes_id = l.to_hermes_id \
           LEFT JOIN org_level  vt ON vt.id = mt.level_id \
           WHERE l.organization_id = $1 AND l.follow \
             AND NOT (vf.position IS NOT NULL AND vt.position IS NOT NULL \
                      AND vf.position < vt.position))",
    )
    .bind(organization.id())
    .fetch_one(write.connection())
    .await?;
    if broken {
        write.rollback().await?;
        return Ok(invalid(OrgInvalid::Follow));
    }

    // The events: ordinary ones, with ids, flags and numbers; never a name of a level or a role.
    let number = |at: usize| i64::try_from(at).unwrap_or(i64::MAX);
    for change in &level_changes {
        let mut details = Details::new()
            .flag("added", change.added)
            .flag("removed", false)
            .flag("renamed", change.renamed)
            .flag("moved", change.moved)
            .number("position_after", number(change.after));
        if let Some(at) = change.before {
            details = details.number("position_before", number(at));
        }
        write
            .record(
                now,
                &Event::new(Action::OrgLevelChanged, admin.actor())
                    .in_organization(organization)
                    .about(TargetKind::OrgLevel, change.id)
                    .with(details),
            )
            .await?;
    }
    for (at, id) in &removed_levels {
        write
            .record(
                now,
                &Event::new(Action::OrgLevelChanged, admin.actor())
                    .in_organization(organization)
                    .about(TargetKind::OrgLevel, *id)
                    .with(
                        Details::new()
                            .flag("added", false)
                            .flag("removed", true)
                            .flag("renamed", false)
                            .flag("moved", false)
                            .number("position_before", number(*at)),
                    ),
            )
            .await?;
    }
    for change in &member_changes {
        let mut details = Details::new()
            .name("name", change.name)
            .flag("placed", change.placed)
            .flag("level_changed", change.level_changed)
            .flag("x_changed", change.x_changed)
            .flag("role_changed", change.role_changed)
            .id("level_after", change.level);
        if let Some(level) = change.level_before {
            details = details.id("level_before", level);
        }
        write
            .record(
                now,
                &Event::new(Action::OrgMemberChanged, admin.actor())
                    .in_organization(organization)
                    .about(TargetKind::Hermes, change.id)
                    .with(details),
            )
            .await?;
    }
    // What the chart is now: the draft as written, with the rows it resolved. No query: the
    // Hermes rows stay locked until the commit, so asking again would only lengthen the wait of
    // every writer of a link. A test compares this version with a fresh read.
    summary.version = Chart {
        levels: draft
            .levels
            .iter()
            .zip(&levels)
            .map(|(wanted, resolved)| StoredLevel {
                id: resolved.id,
                name: wanted.name.clone(),
            })
            .collect(),
        members: draft
            .hermes
            .iter()
            .filter_map(|wanted| {
                Some(StoredMember {
                    id: *hermes_ids.get(wanted.name.as_str())?,
                    name: wanted.name.clone(),
                    place: Some(Place {
                        level: level_of(wanted.level)?,
                        x: wanted.x,
                        role: wanted.role.clone(),
                    }),
                })
            })
            .collect(),
        links: draft
            .links
            .iter()
            .filter_map(|wanted| {
                Some(StoredLink {
                    from_id: *hermes_ids.get(wanted.from.as_str())?,
                    from: wanted.from.clone(),
                    to_id: *hermes_ids.get(wanted.to.as_str())?,
                    to: wanted.to.clone(),
                    wake: wanted.wake,
                    follow: wanted.follow,
                })
            })
            .collect(),
    }
    .version();
    write
        .record(
            now,
            &Event::new(Action::OrgChartPublished, admin.actor())
                .in_organization(organization)
                .about(TargetKind::Organization, organization.id())
                .with(
                    Details::new()
                        .number("levels_added", i64::from(summary.levels_added))
                        .number("levels_removed", i64::from(summary.levels_removed))
                        .number("levels_renamed", i64::from(summary.levels_renamed))
                        .number("levels_moved", i64::from(summary.levels_moved))
                        .number("members_changed", i64::from(summary.members_changed))
                        .number("roles_changed", i64::from(summary.roles_changed))
                        .number("links_created", i64::from(summary.links_created))
                        .number("links_changed", i64::from(summary.links_changed))
                        .number("links_removed", i64::from(summary.links_removed)),
                ),
        )
        .await?;
    write.commit().await?;
    Ok(PublishOutcome::Published(summary))
}
