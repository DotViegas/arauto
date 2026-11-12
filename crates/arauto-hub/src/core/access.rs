//! The rule of the matrix of accesses (`docs/08-permissoes.md`, sections 1 to 3), as pure
//! functions: what an access and the line of a chat allow, and the code of the refusal.
//!
//! ```text
//! no access to the number                         AR-701
//! the chat has a line          -> its verbs only  AR-705 when the verb is off
//! no line, scope "all"         -> the number's    AR-701 when the verb is off
//! no line, scope "list"                           AR-705 (the chat does not exist for it)
//! a chat that does not exist yet: "all", send and "may start chats", or AR-701 / AR-705
//! ```
//!
//! A Hermes learns that it lacks a verb on a number; it never learns that a chat outside its
//! scope exists: that refusal is the answer of a chat that does not exist.

use arauto_protocol::errors::ErrorCode;

/// One of the four verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verb {
    /// See chats and messages, download files, see groups, contacts and the state.
    Read,
    /// Write, react, mark as read, type, forward, ask whether a telephone has WhatsApp.
    Send,
    /// Edit and delete what exists.
    Modify,
    /// Be woken when a message arrives.
    Wake,
}

impl Verb {
    /// Every verb, in the order of the documentation.
    pub const ALL: [Verb; 4] = [Verb::Read, Verb::Send, Verb::Modify, Verb::Wake];

    /// The word shown to the Hermes, to the admin and in the audit.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Verb::Read => "read",
            Verb::Send => "send",
            Verb::Modify => "modify",
            Verb::Wake => "wake",
        }
    }
}

/// The verbs of a number, or of the line of a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Verbs {
    /// Read.
    pub read: bool,
    /// Send.
    pub send: bool,
    /// Modify.
    pub modify: bool,
    /// Be woken.
    pub wake: bool,
}

impl Verbs {
    /// Whether `verb` is on.
    #[must_use]
    pub const fn has(&self, verb: Verb) -> bool {
        match verb {
            Verb::Read => self.read,
            Verb::Send => self.send,
            Verb::Modify => self.modify,
            Verb::Wake => self.wake,
        }
    }

    /// The words of the verbs that are on, in the order of the documentation.
    #[must_use]
    pub fn words(&self) -> Vec<&'static str> {
        Verb::ALL
            .iter()
            .filter(|verb| self.has(**verb))
            .map(|verb| verb.as_str())
            .collect()
    }

    /// Being woken needs reading in the same line (`docs/08-permissoes.md`, section 1).
    ///
    /// # Errors
    /// [`AccessRuleError::WakeWithoutRead`].
    pub const fn check(&self) -> Result<(), AccessRuleError> {
        if self.wake && !self.read {
            return Err(AccessRuleError::WakeWithoutRead);
        }
        Ok(())
    }
}

/// Which chats the verbs of the number reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Every chat of the number, those that come later included.
    All,
    /// Only the chats that have a line.
    List,
}

impl Scope {
    /// The text stored in `access.scope`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Scope::All => "all",
            Scope::List => "list",
        }
    }

    /// Reads the stored text.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "all" => Some(Scope::All),
            "list" => Some(Scope::List),
            _ => None,
        }
    }
}

/// An access of a Hermes to a number: the scope, the verbs and whether it may start chats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Grant {
    /// Which chats the verbs reach.
    pub scope: Scope,
    /// The verbs of the number.
    pub verbs: Verbs,
    /// Writing first to someone who never talked to the number.
    pub can_start_chats: bool,
}

/// An access the rules of `docs/08-permissoes.md`, section 1, do not allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AccessRuleError {
    /// Being woken without reading.
    #[error("being woken needs reading in the same line: add --read")]
    WakeWithoutRead,
    /// Starting chats without sending, or without every chat in scope.
    #[error("starting chats needs sending and the scope all")]
    StartWithoutSendOrAll,
}

impl Grant {
    /// The rules of section 1.
    ///
    /// # Errors
    /// [`AccessRuleError`] says which one is broken.
    pub const fn check(&self) -> Result<(), AccessRuleError> {
        if let Err(error) = self.verbs.check() {
            return Err(error);
        }
        if self.can_start_chats && !(self.verbs.send && matches!(self.scope, Scope::All)) {
            return Err(AccessRuleError::StartWithoutSendOrAll);
        }
        Ok(())
    }

    /// A verb of the number itself: its state, its listings, the question about a telephone.
    ///
    /// # Errors
    /// AR-701 when the verb is off.
    pub const fn on_the_number(&self, verb: Verb) -> Result<(), ErrorCode> {
        if self.verbs.has(verb) {
            Ok(())
        } else {
            Err(ErrorCode::Ar701)
        }
    }

    /// A verb in one chat that exists, with its line when it has one (rules 3 and 4).
    ///
    /// # Errors
    /// AR-705 when the line lacks the verb or the chat is outside a "list" scope; AR-701 when
    /// the scope is "all", there is no line and the number lacks the verb.
    pub const fn in_chat(&self, line: Option<Verbs>, verb: Verb) -> Result<(), ErrorCode> {
        match (line, self.scope) {
            (Some(line), _) => {
                if line.has(verb) {
                    Ok(())
                } else {
                    Err(ErrorCode::Ar705)
                }
            }
            (None, Scope::All) => self.on_the_number(verb),
            (None, Scope::List) => Err(ErrorCode::Ar705),
        }
    }

    /// Sending to a chat that does not exist yet (rule 5): the scope "all", sending and "may
    /// start chats".
    ///
    /// # Errors
    /// AR-705 with the scope "list" (the chat does not exist for this Hermes); AR-701 when
    /// sending or starting chats is off.
    pub const fn new_chat(&self) -> Result<(), ErrorCode> {
        match self.scope {
            Scope::List => Err(ErrorCode::Ar705),
            Scope::All if self.verbs.send && self.can_start_chats => Ok(()),
            Scope::All => Err(ErrorCode::Ar701),
        }
    }

    /// Whether this access, with these lines, has `verb` anywhere: what releases the family of
    /// tools of the verb.
    #[must_use]
    pub fn anywhere(&self, lines: impl IntoIterator<Item = Verbs>, verb: Verb) -> bool {
        self.verbs.has(verb) || lines.into_iter().any(|line| line.has(verb))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn verbs(read: bool, send: bool, modify: bool, wake: bool) -> Verbs {
        Verbs {
            read,
            send,
            modify,
            wake,
        }
    }

    const fn grant(scope: Scope, verbs: Verbs, can_start_chats: bool) -> Grant {
        Grant {
            scope,
            verbs,
            can_start_chats,
        }
    }

    #[test]
    fn a_line_replaces_the_verbs_of_the_number_whole() {
        let all = grant(Scope::All, verbs(true, true, true, false), false);
        let only_read = verbs(true, false, false, false);
        assert_eq!(all.in_chat(Some(only_read), Verb::Read), Ok(()));
        assert_eq!(
            all.in_chat(Some(only_read), Verb::Send),
            Err(ErrorCode::Ar705)
        );
        // A line with every verb off blocks the chat, whatever the number has.
        let blocked = Verbs::default();
        for verb in Verb::ALL {
            assert_eq!(all.in_chat(Some(blocked), verb), Err(ErrorCode::Ar705));
        }
        // A line can give a verb the number does not have.
        let none = grant(Scope::List, Verbs::default(), false);
        assert_eq!(none.in_chat(Some(only_read), Verb::Read), Ok(()));
    }

    #[test]
    fn without_a_line_the_scope_decides() {
        let all = grant(Scope::All, verbs(true, false, false, false), false);
        assert_eq!(all.in_chat(None, Verb::Read), Ok(()));
        assert_eq!(all.in_chat(None, Verb::Send), Err(ErrorCode::Ar701));
        let list = grant(Scope::List, verbs(true, true, true, true), false);
        for verb in Verb::ALL {
            assert_eq!(list.in_chat(None, verb), Err(ErrorCode::Ar705), "{verb:?}");
        }
    }

    #[test]
    fn a_new_chat_needs_all_send_and_start() {
        let open = grant(Scope::All, verbs(false, true, false, false), true);
        assert_eq!(open.new_chat(), Ok(()));
        assert_eq!(
            grant(Scope::All, verbs(false, true, false, false), false).new_chat(),
            Err(ErrorCode::Ar701)
        );
        assert_eq!(
            grant(Scope::All, verbs(true, false, false, false), true).new_chat(),
            Err(ErrorCode::Ar701)
        );
        assert_eq!(
            grant(Scope::List, verbs(false, true, false, false), true).new_chat(),
            Err(ErrorCode::Ar705)
        );
    }

    #[test]
    fn the_rules_of_section_1() {
        assert_eq!(
            grant(Scope::All, verbs(false, false, false, true), false).check(),
            Err(AccessRuleError::WakeWithoutRead)
        );
        assert_eq!(
            grant(Scope::All, verbs(true, false, false, false), true).check(),
            Err(AccessRuleError::StartWithoutSendOrAll)
        );
        assert_eq!(
            grant(Scope::List, verbs(true, true, false, false), true).check(),
            Err(AccessRuleError::StartWithoutSendOrAll)
        );
        assert!(
            grant(Scope::All, verbs(true, true, true, true), true)
                .check()
                .is_ok()
        );
        assert_eq!(
            verbs(false, true, false, true).check(),
            Err(AccessRuleError::WakeWithoutRead)
        );
    }

    #[test]
    fn a_family_is_released_by_the_number_or_by_a_line() {
        let list = grant(Scope::List, Verbs::default(), false);
        assert!(!list.anywhere([], Verb::Read));
        assert!(list.anywhere([verbs(true, false, false, false)], Verb::Read));
        assert!(!list.anywhere([verbs(true, false, false, false)], Verb::Send));
        assert_eq!(verbs(true, false, true, false).words(), ["read", "modify"]);
        assert_eq!(Scope::parse("all"), Some(Scope::All));
        assert_eq!(Scope::parse("ALL"), None);
    }
}
