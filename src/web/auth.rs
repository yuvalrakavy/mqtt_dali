use super::digest;
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::time::Instant;

pub(super) const COOKIE: &str = "DaliSession";
#[derive(Clone, Copy)]
pub(super) struct Policy {
    pub idle: Duration,
    pub absolute: Duration,
    pub sessions: usize,
    pub attempts: usize,
    pub window: Duration,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(1800),
            absolute: Duration::from_secs(43200),
            sessions: 16,
            attempts: 10,
            window: Duration::from_secs(60),
        }
    }
}
struct Session {
    csrf: String,
    created: Instant,
    activity: Instant,
}
#[derive(Clone)]
pub(super) struct Access {
    pub csrf: String,
    pub expires: Instant,
    pub absolute: Instant,
}
pub(super) enum Refusal {
    Password,
    Rate,
    Capacity,
}
pub(super) struct Auth {
    password: [u8; 32],
    policy: Policy,
    sessions: BTreeMap<String, Session>,
    attempts: VecDeque<Instant>,
    launches: BTreeMap<[u8; 32], Instant>,
}
fn random_secret() -> String {
    let bytes: [u8; 32] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl Auth {
    pub fn new(password: [u8; 32], policy: Policy) -> Self {
        Self {
            password,
            policy,
            sessions: BTreeMap::new(),
            attempts: VecDeque::new(),
            launches: BTreeMap::new(),
        }
    }
    fn expiry(policy: Policy, session: &Session) -> Instant {
        (session.activity + policy.idle).min(session.created + policy.absolute)
    }
    pub fn login(
        &mut self,
        password: &str,
        previous: Option<&str>,
        now: Instant,
    ) -> Result<(String, Access), Refusal> {
        self.check_attempt(now)?;
        if password.len() > 1024 || !bool::from(self.password.ct_eq(&digest(password))) {
            return Err(Refusal::Password);
        }
        self.create_session(previous, now)
    }
    fn check_attempt(&mut self, now: Instant) -> Result<(), Refusal> {
        while self
            .attempts
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= self.policy.window)
        {
            self.attempts.pop_front();
        }
        if self.attempts.len() >= self.policy.attempts {
            return Err(Refusal::Rate);
        }
        self.attempts.push_back(now);
        Ok(())
    }
    pub fn issue_launch(&mut self, now: Instant) -> Result<String, Refusal> {
        self.launches.retain(|_, expiry| now < *expiry);
        if self.launches.len() >= 16 {
            return Err(Refusal::Capacity);
        }
        let token = random_secret();
        self.launches
            .insert(digest(&token), now + Duration::from_secs(120));
        Ok(token)
    }
    pub fn redeem_launch(
        &mut self,
        token: &str,
        previous: Option<&str>,
        now: Instant,
    ) -> Result<(String, Access), Refusal> {
        self.check_attempt(now)?;
        self.launches.retain(|_, expiry| now < *expiry);
        if token.len() != 64 || self.launches.remove(&digest(token)).is_none() {
            return Err(Refusal::Password);
        }
        // A credential is consumed even when all browser session slots are occupied.
        self.create_session(previous, now)
    }
    fn create_session(
        &mut self,
        previous: Option<&str>,
        now: Instant,
    ) -> Result<(String, Access), Refusal> {
        let policy = self.policy;
        self.sessions
            .retain(|_, session| now < Self::expiry(policy, session));
        if let Some(previous) = previous {
            self.sessions.remove(previous);
        }
        if self.sessions.len() >= policy.sessions {
            return Err(Refusal::Capacity);
        }
        let token = loop {
            let candidate = random_secret();
            if !self.sessions.contains_key(&candidate) {
                break candidate;
            }
        };
        self.sessions.insert(
            token.clone(),
            Session {
                csrf: random_secret(),
                created: now,
                activity: now,
            },
        );
        let access = self.access(&token, now).expect("new session");
        Ok((token, access))
    }
    pub fn access(&mut self, token: &str, now: Instant) -> Option<Access> {
        let session = self.sessions.get(token)?;
        let expires = Self::expiry(self.policy, session);
        if now >= expires {
            self.sessions.remove(token);
            return None;
        }
        Some(Access {
            csrf: session.csrf.clone(),
            expires,
            absolute: session.created + self.policy.absolute,
        })
    }
    pub fn activity(&mut self, token: &str, now: Instant) -> bool {
        if self.access(token, now).is_none() {
            return false;
        }
        self.sessions.get_mut(token).unwrap().activity = now;
        true
    }
    pub fn logout(&mut self, token: &str) {
        self.sessions.remove(token);
    }
}
