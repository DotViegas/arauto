//! The allowance of the tests of [`super`]: loopback IPs, a root of the tests and shorter
//! limits, so that one download can reach a server of the test on this machine. The module is
//! declared under `#[cfg(test)]`: no build that is shipped has it, and the test
//! `the_allowance_exists_only_in_a_test_build` reads the file of the client to prove it.

use std::net::IpAddr;
use std::sync::Arc;

use super::{Fetcher, Limits, Resolve, client_config, public_roots};

/// What a test lets through: loopback IPs only, when it says so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Allowance {
    pub(super) loopback: bool,
}

impl Allowance {
    pub(super) fn lets(self, ip: IpAddr) -> bool {
        self.loopback && ip.is_loopback()
    }
}

impl Fetcher {
    /// A client that also trusts `root`, reaches loopback IPs when `loopback`, and has
    /// `limits`.
    pub(super) fn for_tests(
        resolver: Arc<dyn Resolve>,
        root: &[u8],
        loopback: bool,
        limits: Limits,
    ) -> Self {
        let mut roots = public_roots();
        roots
            .add(rustls::pki_types::CertificateDer::from(root.to_vec()))
            .expect("the root of the tests is a certificate");
        Self {
            resolver,
            tls: client_config(roots),
            limits,
            allowance: Allowance { loopback },
        }
    }
}
