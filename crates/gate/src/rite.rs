use crate::{Deny, Gate, TTL, digest, wild};
use axum::http::HeaderMap;
use keel::adapt::Error;
use keel::life::tick;
use keel::{Op, Wire, form};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed<'a> {
    pub who: &'a str,
    pub verb: &'a str,
    pub unit: &'a str,
    pub scope: &'a str,
}

impl<W: Wire + 'static> Gate<W> {
    pub async fn seed(&self) -> Result<(), Error> {
        let who = self.svc.to_string();
        self.sow(&self.rites(&who)).await
    }

    pub async fn ready(&self) -> Result<bool, Error> {
        let who = self.svc.to_string();
        self.sown(&self.rites(&who)).await
    }

    fn rites<'a>(&'a self, who: &'a str) -> [Seed<'a>; 4] {
        let unit = self.core.identity().unwrap_or("");
        let rite = |verb, unit| Seed {
            who,
            verb,
            unit,
            scope: "all",
        };
        [
            rite("see", "Token"),
            rite("see", "Session"),
            rite("see", unit),
            rite("put", "Session"),
        ]
    }

    pub async fn whom(&self, headers: &HeaderMap) -> Option<i64> {
        crate::whom(self, headers).await
    }

    pub async fn barred(&self, key: i64) -> bool {
        crate::barred(self, key).await
    }

    pub async fn session(&self, actor: i64) -> Result<(i64, String), Error> {
        let sid = wild();
        let face = self.core.of(self.svc);
        let row = face
            .put(
                "Session",
                &[("hash", &digest(&sid)), ("actor", &actor.to_string())],
            )
            .await?;
        face.lease("Session", row, tick() + TTL).await?;
        Ok((row, sid))
    }

    pub async fn token(&self, actor: i64, name: &str) -> Result<String, Error> {
        let pat = wild();
        self.core
            .of(actor)
            .put(
                "Token",
                &[
                    ("name", name),
                    ("hash", &digest(&pat)),
                    ("actor", &actor.to_string()),
                ],
            )
            .await?;
        Ok(pat)
    }

    pub async fn birth(&self, fields: &[(&str, &str)]) -> Result<i64, Error> {
        if self.core.identity().is_none() {
            return Err(Error::Adapt("no identity unit".into()));
        }
        self.core
            .sudo()
            .batch(async |tx| tx.birth(fields).await)
            .await
    }

    pub async fn sow(&self, seeds: &[Seed<'_>]) -> Result<(), Error> {
        let sudo = self.core.sudo();
        for seed in seeds {
            if !held(&sudo, seed).await? {
                sudo.put(
                    "@grant",
                    &[
                        ("who", seed.who),
                        ("verb", seed.verb),
                        ("unit", seed.unit),
                        ("scope", seed.scope),
                    ],
                )
                .await?;
            }
        }
        Ok(())
    }

    pub async fn sown(&self, seeds: &[Seed<'_>]) -> Result<bool, Error> {
        let sudo = self.core.sudo();
        for seed in seeds {
            if !held(&sudo, seed).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn logout(&self, sid: &str) -> Result<(), Deny> {
        self.fell("Session", &digest(sid)).await
    }

    pub async fn revoke(&self, token: &str) -> Result<(), Deny> {
        self.fell("Token", &digest(token)).await
    }

    async fn fell(&self, unit: &str, hash: &str) -> Result<(), Deny> {
        let face = self.core.of(self.svc);
        let ask = form(unit).when("hash", Op::Eq, hash);
        let held = face.one(&ask).await.map_err(Deny::from)?;
        let Some(row) = held else {
            return Err(Deny::gone(unit));
        };
        let Some(key) = row.int("actor") else {
            return Err(Deny::misfit());
        };
        self.core
            .of(key)
            .end(unit, row.key())
            .await
            .map_err(Deny::from)?;
        Ok(())
    }
}

async fn held<W: Wire>(face: &keel::Face<'_, W>, seed: &Seed<'_>) -> Result<bool, Error> {
    let ask = form("@grant")
        .when("who", Op::Eq, seed.who)
        .when("verb", Op::Eq, seed.verb)
        .when("unit", Op::Eq, seed.unit)
        .when("scope", Op::Eq, seed.scope)
        .count();
    Ok(face.ask(&ask).await?.count().is_some_and(|count| count > 0))
}
