//! The parts of design part 6.1 that need no Windows call, compiled and tested on every system (design part 4.2): the
//! Credential Manager target a secret is stored under (Ruling W13). `win::creds` makes the Credential Manager calls.

/// The prefix of lazymkv's Credential Manager entries (design part 6.1; Ruling W13): the secret
/// `share:\\server\share` is the entry `lazymkv:share:\\server\share`, and `tmdb_token` the entry
/// `lazymkv:tmdb_token`. Only `CredentialStore::new` reaches Credential Manager with it; vault tests give a prefix of
/// their own (Global Constraints). Def: T3-13. Use: T3-13.
pub const CREDENTIAL_PREFIX: &str = "lazymkv:";

/// The Credential Manager target of the secret `name` under `prefix`: the two joined (design part 6.1; Ruling W13).
/// Def: T3-13. Use: T3-13.
pub fn credential_target(prefix: &str, name: &str) -> String {
    format!("{prefix}{name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_targets_are_the_prefix_and_the_secret_name() {
        assert_eq!(credential_target(CREDENTIAL_PREFIX, r"share:\\server\share"), r"lazymkv:share:\\server\share");
        assert_eq!(credential_target(CREDENTIAL_PREFIX, "tmdb_token"), "lazymkv:tmdb_token");
    }
}
