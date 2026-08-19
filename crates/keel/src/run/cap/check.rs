use super::*;
use crate::adapt::Error;
use crate::face::Who;
use crate::life::{Row, Work};
use crate::plan::Plan;
use crate::wire::Wire;

pub struct Deed<'a>(&'a Row);

impl Deed<'_> {
    fn who(&self) -> &str {
        cell(self.0, "who")
    }

    fn verb(&self) -> &str {
        cell(self.0, "verb")
    }

    fn place(&self) -> &str {
        cell(self.0, "unit")
    }

    fn anchor(&self, plan: &Plan) -> String {
        let place = self.place();
        if place == "*" {
            return place.to_string();
        }
        plan.find(place)
            .map(|unit| unit.key())
            .unwrap_or_else(|_| place.to_string())
    }

    fn span(&self) -> &str {
        cell(self.0, "scope")
    }

    fn does(&self, verb: &str) -> bool {
        let deed = self.verb();
        deed == "*" || deed == verb
    }

    fn names(&self, who: Who) -> bool {
        match self.who() {
            "anon" => true,
            "all" => matches!(who, Who::Op(_)),
            id => match who {
                Who::Op(op) => id.parse::<i64>().is_ok_and(|n| n == op),
                _ => false,
            },
        }
    }

    async fn bears<W: Wire>(
        &self,
        plan: &Plan,
        work: &mut Work<'_, W>,
        who: Who,
    ) -> Result<bool, Error> {
        if self.names(who) {
            return Ok(true);
        }
        let Who::Op(op) = who else {
            return Ok(false);
        };
        let Some((place, id)) = self.who().split_once(' ') else {
            return Ok(false);
        };
        let Ok(id) = id.parse::<i64>() else {
            return Ok(false);
        };
        let Ok(node) = plan.find(place) else {
            return Ok(false);
        };
        let Some(edge) = node.crew() else {
            return Ok(false);
        };
        if !work.alive(node, id).await? {
            return Ok(false);
        }
        let ties = work.ties(node, edge, id).await?;
        Ok(ties.iter().any(|tie| tie.right() == op))
    }

    async fn held<W: Wire>(
        &self,
        plan: &Plan,
        work: &mut Work<'_, W>,
        case: &Case<'_>,
    ) -> Result<bool, Error> {
        let plea = case.plea;
        let chain = case.chain;
        if !self.bears(plan, work, plea.who).await? || !self.does(plea.verb) {
            return Ok(false);
        }
        let span = self.span();
        if span == "all" {
            let anchor = self.anchor(plan);
            return Ok(anchor == "*" || anchor == plea.unit);
        }
        let anchor = self.anchor(plan);
        if let Some(id) = span.strip_prefix("row ") {
            let Ok(id) = id.parse::<i64>() else {
                return Ok(false);
            };
            return Ok(chain.iter().any(|hop| hop.unit == anchor && hop.key == id));
        }
        let Some(pred) = span.strip_prefix("pred ") else {
            return Ok(false);
        };
        if anchor == plea.unit {
            return Ok(case.held.suits(plea.unit, pred, plea.who));
        }
        if plea.verb != "see" {
            return Ok(false);
        }
        Ok(descend(&anchor, pred, plea.who, chain))
    }
}

pub struct Case<'a> {
    pub plea: &'a Plea<'a>,
    pub chain: &'a [Hop],
    pub held: &'a Mark<'a>,
}

pub struct Court<'a, W: Wire> {
    plan: &'a Plan,
    wire: &'a mut W,
    deeds: &'a [Row],
}

impl<'a, W: Wire> Court<'a, W> {
    pub fn new(plan: &'a Plan, wire: &'a mut W, deeds: &'a [Row]) -> Self {
        Self { plan, wire, deeds }
    }

    pub async fn check(&mut self, plea: &Plea<'_>) -> Result<bool, Error> {
        self.weigh(plea, plea.mark).await
    }

    pub async fn shift(&mut self, plea: &Plea<'_>, after: &Mark<'_>) -> Result<bool, Error> {
        self.weigh(plea, after).await
    }

    pub async fn spans(&mut self, plea: &Plea<'_>) -> Result<bool, Error> {
        let mut work = Work::new(self.wire, self.plan);
        let chain = plea.mark.anchors(self.plan, &mut work, plea.unit).await?;
        for verb in VERBS {
            let seen = Plea {
                who: plea.who,
                verb,
                unit: plea.unit,
                mark: plea.mark,
            };
            let case = Case {
                plea: &seen,
                chain: &chain,
                held: plea.mark,
            };
            let mut held = false;
            for deed in self.deeds {
                if Deed(deed).held(self.plan, &mut work, &case).await? {
                    held = true;
                    break;
                }
            }
            if !held {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn broad(&mut self, plea: &Plea<'_>) -> Result<bool, Error> {
        let mut work = Work::new(self.wire, self.plan);
        for deed in self.deeds {
            let deed = Deed(deed);
            if !deed.bears(self.plan, &mut work, plea.who).await? || !deed.does(plea.verb) {
                continue;
            }
            let anchor = deed.anchor(self.plan);
            let wide = anchor == "*" || anchor == plea.unit;
            if wide && deed.span() == "all" {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn weigh(&mut self, plea: &Plea<'_>, held: &Mark<'_>) -> Result<bool, Error> {
        let mut work = Work::new(self.wire, self.plan);
        let bare = Case {
            plea,
            chain: &[],
            held,
        };
        let mut rest = Vec::new();
        for deed in self.deeds {
            if Deed(deed).span() != "all" {
                rest.push(deed);
                continue;
            }
            if Deed(deed).held(self.plan, &mut work, &bare).await? {
                return Ok(true);
            }
        }
        if rest.is_empty() {
            return Ok(false);
        }
        let chain = plea.mark.anchors(self.plan, &mut work, plea.unit).await?;
        let case = Case {
            plea,
            chain: &chain,
            held,
        };
        for deed in rest {
            if Deed(deed).held(self.plan, &mut work, &case).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
