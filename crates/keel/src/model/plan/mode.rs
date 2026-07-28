use super::Edge;

pub(super) fn check(frozen: bool, bonds: &[Edge]) -> Result<(), crate::adapt::Error> {
    let set = bonds
        .iter()
        .any(|edge| edge.kind() == crate::bond::Kind::Many2many);
    if frozen && set {
        return Err(crate::adapt::Error::Adapt(
            "frozen unit cannot have set relation".into(),
        ));
    }
    Ok(())
}
