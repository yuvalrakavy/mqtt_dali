use super::*;

pub(super) fn generations(model: &Model) -> TargetGenerations {
    let inventory = model.inventory();
    let instance = format!("{}.{}", inventory.inventory_epoch, inventory.revision);
    let mut targets = model
        .projections()
        .into_iter()
        .map(|p| {
            (
                p.target,
                format!("{}.{}", inventory.inventory_epoch, p.binding_revision),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    targets.insert("__inventory".into(), instance.clone());
    TargetGenerations { instance, targets }
}
pub(super) fn admit(
    snapshot: &Snapshot,
    change: &Change,
    context: &RequestContext,
) -> Result<(), ServiceError> {
    match (&snapshot.phase, change) {
        (Phase::Ready, Change::Edit { .. }) | (Phase::Uncertain(_), Change::Recover) => {}
        (Phase::Stopping | Phase::Closed, _) => return Err(ServiceError::Closed),
        _ => {
            return Err(ServiceError::Rejected(
                "routing is not ready for this operation".into(),
            ))
        }
    }
    let table = generations(&snapshot.model);
    let token = match change {
        Change::Edit { expected, edit } => {
            if *expected != snapshot.model.inventory().revision {
                return Err(ServiceError::Rejected("inventory revision changed".into()));
            }
            let location = match edit {
                MetadataEdit::RenameInput { bus, address, .. } => {
                    Some(crate::inventory::Location::Input {
                        bus: *bus,
                        short_address: *address,
                    })
                }
                MetadataEdit::RenameInputInstance {
                    bus,
                    address,
                    instance,
                    ..
                } => Some(crate::inventory::Location::InputInstance {
                    bus: *bus,
                    short_address: *address,
                    instance: *instance,
                }),
                MetadataEdit::RenameOutput { bus, address, .. } => {
                    Some(crate::inventory::Location::Output {
                        bus: *bus,
                        short_address: *address,
                    })
                }
                MetadataEdit::RenameGroup { bus, address, .. } => {
                    Some(crate::inventory::Location::Group {
                        bus: *bus,
                        group_address: *address,
                    })
                }
                _ => None,
            };
            match location {
                Some(location) => {
                    let projection = snapshot
                        .model
                        .projections()
                        .into_iter()
                        .find(|p| p.location == location)
                        .ok_or(ServiceError::Rejected("unknown item".into()))?;
                    table.targets[&projection.target].clone()
                }
                None => table.instance,
            }
        }
        Change::Recover => table.instance,
    };
    if context.generation() != Some(token.as_str()) {
        return Err(ServiceError::Rejected(
            "missing or retired target generation".into(),
        ));
    }
    Ok(())
}
