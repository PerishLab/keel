#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Many2many,
    Many2one,
    One2one,
}

impl Kind {
    pub fn point(&self) -> bool {
        matches!(self, Kind::Many2one | Kind::One2one)
    }
}
