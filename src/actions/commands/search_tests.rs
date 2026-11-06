use super::SEARCH_COMMANDS;
use crate::actions::model::READ_SCOPE;
use crate::capability::Capability;

#[test]
fn newznab_search_is_a_read_scoped_search_command() {
    assert_eq!(SEARCH_COMMANDS.len(), 1);
    let cmd = &SEARCH_COMMANDS[0];
    assert_eq!(cmd.name, "newznab_search");
    assert_eq!(cmd.capability, Capability::Search);
    assert_eq!(cmd.required_scope, READ_SCOPE);
    assert_eq!(cmd.required_params, &["service", "query"]);
    assert!(!cmd.mutates && !cmd.destructive);
}
