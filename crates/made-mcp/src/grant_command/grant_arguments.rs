use std::path::PathBuf;

use super::GrantSelection;

/// `made-mcp grant <store> ...` as the person typed it.
///
/// Parsed before anything is opened, so a usage error is answered
/// without a store having been touched. `--show` reads the policy and
/// asks nothing; everything else needs a selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantArguments {
    pub store: PathBuf,
    pub grantee: Option<String>,
    pub selection: Option<GrantSelection>,
    pub scope: Option<String>,
    pub valid_until: Option<String>,
    pub grant_id: Option<String>,
    pub show: bool,
}

impl GrantArguments {
    /// Parse the arguments after the command word.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let Some((store, flags)) = args.split_first() else {
            return Err("the store path is required".to_owned());
        };
        let mut arguments = Self {
            store: PathBuf::from(store),
            grantee: None,
            selection: None,
            scope: None,
            valid_until: None,
            grant_id: None,
            show: false,
        };
        let mut remaining = flags.iter();
        while let Some(flag) = remaining.next() {
            if flag == "--show" {
                arguments.show = true;
                continue;
            }
            let Some(value) = remaining.next() else {
                return Err(format!("`{flag}` needs a value"));
            };
            match flag.as_str() {
                "--grantee" => arguments.grantee = Some(value.clone()),
                "--profile" | "--actions" => {
                    if arguments.selection.is_some() {
                        return Err(
                            "choose one of --profile and --actions, not both or twice".to_owned()
                        );
                    }
                    arguments.selection = Some(if flag == "--profile" {
                        GrantSelection::Profile(value.clone())
                    } else {
                        GrantSelection::parse_actions(value)?
                    });
                }
                "--scope" => arguments.scope = Some(value.clone()),
                "--valid-until" => arguments.valid_until = Some(value.clone()),
                "--grant-id" => arguments.grant_id = Some(value.clone()),
                other => return Err(format!("unknown flag `{other}`")),
            }
        }
        if arguments.show {
            if arguments.selection.is_some()
                || arguments.scope.is_some()
                || arguments.valid_until.is_some()
                || arguments.grant_id.is_some()
            {
                return Err("--show reads the policy and takes only --grantee".to_owned());
            }
        } else if arguments.selection.is_none() {
            return Err("choose what to grant: --profile core, or --actions <a,b,...>".to_owned());
        }
        Ok(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<GrantArguments, String> {
        GrantArguments::parse(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn a_profile_grant_with_every_option_is_parsed() {
        let arguments = parse(&[
            "/tmp/store.sqlite3",
            "--profile",
            "core",
            "--grantee",
            "other-host",
            "--scope",
            "ceremony:pr-1",
            "--valid-until",
            "2030-01-01T00:00:00Z",
            "--grant-id",
            "g-1",
        ])
        .unwrap();
        assert_eq!(arguments.store, PathBuf::from("/tmp/store.sqlite3"));
        assert_eq!(
            arguments.selection,
            Some(GrantSelection::Profile("core".to_owned()))
        );
        assert_eq!(arguments.grantee.as_deref(), Some("other-host"));
        assert_eq!(arguments.scope.as_deref(), Some("ceremony:pr-1"));
        assert_eq!(
            arguments.valid_until.as_deref(),
            Some("2030-01-01T00:00:00Z")
        );
        assert_eq!(arguments.grant_id.as_deref(), Some("g-1"));
        assert!(!arguments.show);
    }

    #[test]
    fn show_takes_only_the_grantee_and_a_grant_needs_a_selection() {
        let shown = parse(&["store", "--show", "--grantee", "x"]).unwrap();
        assert!(shown.show);
        assert_eq!(shown.grantee.as_deref(), Some("x"));
        assert!(parse(&["store", "--show", "--profile", "core"]).is_err());
        let error = parse(&["store"]).unwrap_err();
        assert!(error.contains("--profile core"), "{error}");
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn conflicting_or_unknown_flags_are_usage_errors() {
        assert!(parse(&["store", "--profile", "core", "--actions", "x"]).is_err());
        assert!(parse(&["store", "--profile"]).is_err());
        assert!(parse(&["store", "--yes", "--profile", "core"]).is_err());
        let actions = parse(&[
            "store",
            "--actions",
            "design_ceremony,publish_ceremony_definition",
        ])
        .unwrap();
        assert_eq!(
            actions.selection,
            Some(GrantSelection::Actions(vec![
                "design_ceremony".to_owned(),
                "publish_ceremony_definition".to_owned(),
            ]))
        );
    }
}
