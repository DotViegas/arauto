//! C11: the admin command run before `serve` on a restored database
//! (`docs/04-seguranca-conexao.md`, C11; `docs/15-mvp.md`, section 2, rule 9). The database
//! restored under the running hub is measured in `tests/timely.rs`.

use arauto_hub::admin::AdminError;
use arauto_protocol::errors::ErrorCode;
use arauto_testkit::device::Faults;

use crate::fixture::{Fixture, settings};

/// The admin command run on a restored database before `serve` writes nothing and leaves the
/// mark as it was: the proof is still there when the hub starts, which turns the mode on
/// before it accepts the first connection.
#[tokio::test(flavor = "multi_thread")]
async fn the_admin_before_serve_after_a_restoration_does_not_erase_the_proof() {
    let mut fixture = Fixture::new(settings()).await;
    let (_, session) = fixture.connected().await;
    let device = session.device().clone();
    drop(session);
    fixture.stop().await;
    fixture.restore_an_older_copy().await;

    let refused = fixture.admin.hermes_add("later", fixture.now()).await;
    assert!(
        matches!(refused, Err(AdminError::Restored(_))),
        "{refused:?}"
    );
    let refused = fixture.admin.revoke("later", fixture.now()).await;
    assert!(refused.is_err());
    assert!(!fixture.restored_mode().await);

    fixture.start().await;
    assert!(fixture.restored_mode().await);
    let device = device.at(fixture.addr());
    let back = device.channel(Faults::default()).await.unwrap();
    assert_eq!(back.closing(), Some(ErrorCode::Ar506));
}
