//! AccessKit bridge: exposes the runtime's toolkit-neutral accessibility
//! snapshot to platform screen readers (VoiceOver, Narrator/NVDA, Orca) and
//! turns their action requests back into runtime commands.
use crate::declarative::{AccessNode, AccessRole, AccessValue};
use accesskit::{
    Action, ActionData, ActionRequest, Node, NodeId, Rect, Role, Toggled, TreeId, TreeInfo,
    TreeUpdate,
};
use std::collections::HashMap;

pub(super) const ROOT: NodeId = NodeId(0);

/// Stable 64-bit id for a runtime key (FNV-1a); 0 is reserved for the window.
pub(super) fn node_id(key: &str) -> NodeId {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    NodeId(hash.max(1))
}

/// A screen-reader request, addressed by runtime key.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum AccessCommand {
    Focus(String),
    Click(String),
    SetValue(String, AccessValue),
    Step(String, bool),
    Reveal(String),
    Scroll(String, [i8; 2]),
}

/// Build a full tree update and the id map used to route later actions.
pub(super) fn tree_update(
    nodes: &[AccessNode],
    title: &str,
    size: [f32; 2],
) -> (TreeUpdate, HashMap<NodeId, String>) {
    let ids: Vec<NodeId> = nodes.iter().map(|n| node_id(&n.id)).collect();
    let mut children: Vec<Vec<NodeId>> = vec![Vec::new(); nodes.len()];
    let mut roots = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        match n.parent {
            Some(p) => children[p].push(ids[i]),
            None => roots.push(ids[i]),
        }
    }
    let mut window = Node::new(Role::Window);
    window.set_label(title);
    window.set_bounds(Rect::new(0., 0., f64::from(size[0]), f64::from(size[1])));
    window.set_children(roots);
    let mut out = vec![(ROOT, window)];
    let mut focus = ROOT;
    let mut map = HashMap::with_capacity(nodes.len());
    for ((n, id), kids) in nodes.iter().zip(&ids).zip(children) {
        let role = match n.role {
            AccessRole::Group => Role::Group,
            AccessRole::ScrollView => Role::ScrollView,
            AccessRole::Label => Role::Label,
            AccessRole::Button => Role::Button,
            AccessRole::CheckBox => Role::CheckBox,
            AccessRole::Switch => Role::Switch,
            AccessRole::TextInput if n.password => Role::PasswordInput,
            AccessRole::TextInput => Role::TextInput,
            AccessRole::MultilineTextInput => Role::MultilineTextInput,
            AccessRole::Slider => Role::Slider,
            AccessRole::TabList => Role::TabList,
            AccessRole::Tab => Role::Tab,
            AccessRole::RadioGroup => Role::RadioGroup,
            AccessRole::RadioButton => Role::RadioButton,
            AccessRole::ListBox => Role::ListBox,
            AccessRole::ListBoxOption => Role::ListBoxOption,
            AccessRole::Table => Role::Table,
            AccessRole::Row => Role::Row,
            AccessRole::ColumnHeader => Role::ColumnHeader,
            AccessRole::Cell => Role::Cell,
            AccessRole::Tree => Role::Tree,
            AccessRole::TreeItem => Role::TreeItem,
        };
        let mut node = Node::new(role);
        let [x, y, w, h] = n.bounds.map(f64::from);
        node.set_bounds(Rect::new(x, y, x + w, y + h));
        let input = matches!(
            n.role,
            AccessRole::TextInput | AccessRole::MultilineTextInput
        );
        if input {
            if let Some(placeholder) = n.placeholder.as_deref().filter(|s| !s.is_empty()) {
                node.set_placeholder(placeholder);
            }
        }
        if !n.label.is_empty() {
            node.set_label(n.label.as_str());
        }
        if let Some(value) = &n.value {
            node.set_value(value.as_str());
        }
        if let Some([value, min, max, step]) = n.numeric {
            node.set_numeric_value(f64::from(value));
            node.set_min_numeric_value(f64::from(min));
            node.set_max_numeric_value(f64::from(max));
            node.set_numeric_value_step(f64::from(step));
        }
        if let Some(checked) = n.checked {
            node.set_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        if let Some(table) = n.table {
            if let Some(v) = table.rows {
                node.set_row_count(v);
            }
            if let Some(v) = table.columns {
                node.set_column_count(v);
            }
            if let Some(v) = table.row {
                node.set_row_index(v);
            }
            if let Some(v) = table.column {
                node.set_column_index(v);
            }
            if let Some(ascending) = table.sort_ascending {
                node.set_sort_direction(if ascending {
                    accesskit::SortDirection::Ascending
                } else {
                    accesskit::SortDirection::Descending
                });
            }
        }
        if let Some(selected) = n.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = n.expanded {
            node.set_expanded(expanded);
        }
        if let Some(level) = n.level {
            node.set_level(level);
        }
        if let Some(position) = n.position_in_set {
            node.set_position_in_set(position);
        }
        if let Some(size) = n.size_of_set {
            node.set_size_of_set(size);
        }
        if let Some([x, y, max_x, max_y]) = n.scroll {
            node.set_scroll_x(f64::from(x));
            node.set_scroll_y(f64::from(y));
            node.set_scroll_x_min(0.);
            node.set_scroll_y_min(0.);
            node.set_scroll_x_max(f64::from(max_x));
            node.set_scroll_y_max(f64::from(max_y));
            if n.enabled {
                if max_x > 0. {
                    node.add_action(Action::ScrollLeft);
                    node.add_action(Action::ScrollRight);
                }
                if max_y > 0. {
                    node.add_action(Action::ScrollUp);
                    node.add_action(Action::ScrollDown);
                }
            }
        }
        if n.read_only {
            node.set_read_only();
        }
        if !n.enabled {
            node.set_disabled();
        } else {
            match n.role {
                AccessRole::Button
                | AccessRole::CheckBox
                | AccessRole::Switch
                | AccessRole::Tab
                | AccessRole::RadioButton
                | AccessRole::ListBoxOption
                | AccessRole::Row
                | AccessRole::TreeItem
                | AccessRole::ColumnHeader
                    if n.clickable =>
                {
                    node.add_action(Action::Focus);
                    node.add_action(Action::Click);
                }
                AccessRole::TextInput | AccessRole::MultilineTextInput => {
                    node.add_action(Action::Focus);
                    if !n.read_only {
                        node.add_action(Action::SetValue);
                    }
                }
                AccessRole::Slider => {
                    for action in [
                        Action::Focus,
                        Action::SetValue,
                        Action::Increment,
                        Action::Decrement,
                    ] {
                        node.add_action(action);
                    }
                }
                _ => {}
            }
            node.add_action(Action::ScrollIntoView);
        }
        if !kids.is_empty() {
            node.set_children(kids);
        }
        if n.focused {
            focus = *id;
        }
        map.insert(*id, n.id.clone());
        out.push((*id, node));
    }
    let mut info = TreeInfo::new(ROOT);
    info.toolkit_name = Some("Volt UI Kit".into());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
    let update = TreeUpdate {
        nodes: out,
        tree: Some(info),
        tree_id: TreeId::ROOT,
        focus,
    };
    (update, map)
}

/// Translate an action request into a runtime command, if it targets a node
/// in the current tree and is one we support.
pub(super) fn command(
    request: &ActionRequest,
    ids: &HashMap<NodeId, String>,
) -> Option<AccessCommand> {
    if request.target_tree != TreeId::ROOT {
        return None;
    }
    let key = ids.get(&request.target_node)?.clone();
    Some(match (request.action, &request.data) {
        (Action::Focus, _) => AccessCommand::Focus(key),
        (Action::Click, _) => AccessCommand::Click(key),
        (Action::Increment, _) => AccessCommand::Step(key, true),
        (Action::Decrement, _) => AccessCommand::Step(key, false),
        (Action::ScrollIntoView, _) => AccessCommand::Reveal(key),
        (Action::ScrollUp, _) => AccessCommand::Scroll(key, [0, -1]),
        (Action::ScrollDown, _) => AccessCommand::Scroll(key, [0, 1]),
        (Action::ScrollLeft, _) => AccessCommand::Scroll(key, [-1, 0]),
        (Action::ScrollRight, _) => AccessCommand::Scroll(key, [1, 0]),
        (Action::SetValue, Some(ActionData::Value(text))) if text.len() <= 8192 => {
            AccessCommand::SetValue(key, AccessValue::Text(text.to_string()))
        }
        (Action::SetValue, Some(ActionData::NumericValue(n))) if (*n as f32).is_finite() => {
            AccessCommand::SetValue(key, AccessValue::Number(*n as f32))
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requests_reject_foreign_trees_and_invalid_payloads() {
        let ids = HashMap::from([(node_id("name"), "name".into())]);
        let mut r = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node_id("name"),
            data: Some(ActionData::Value("x".repeat(8193).into())),
        };
        assert_eq!(command(&r, &ids), None);
        r.data = Some(ActionData::NumericValue(f64::INFINITY));
        assert_eq!(command(&r, &ids), None);
        r.data = Some(ActionData::NumericValue(f64::MAX));
        assert_eq!(command(&r, &ids), None);
        r.action = Action::Click;
        r.data = None;
        r.target_tree = TreeId("11111111-1111-1111-1111-111111111111".parse().unwrap());
        assert_eq!(command(&r, &ids), None);
    }
    fn node(id: &str, role: AccessRole, parent: Option<usize>) -> AccessNode {
        AccessNode {
            id: id.into(),
            parent,
            role,
            table: None,
            clickable: matches!(role, AccessRole::Button | AccessRole::Tab),
            label: id.into(),
            placeholder: None,
            value: None,
            numeric: None,
            checked: None,
            selected: None,
            expanded: None,
            level: None,
            position_in_set: None,
            size_of_set: None,
            scroll: None,
            bounds: [1., 2., 30., 40.],
            enabled: true,
            focused: false,
            password: false,
            read_only: false,
        }
    }
    #[test]
    fn tree_links_children_focus_and_routes_actions() {
        let mut slider = node("volume", AccessRole::Slider, Some(0));
        slider.numeric = Some([3., 0., 10., 1.]);
        slider.focused = true;
        let nodes = vec![node("list", AccessRole::ScrollView, None), slider];
        let (update, ids) = tree_update(&nodes, "App", [800., 600.]);
        assert_eq!(update.focus, node_id("volume"));
        let root = &update.nodes[0].1;
        assert_eq!(root.children(), &[node_id("list")]);
        let list = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id("list"))
            .unwrap();
        assert_eq!(list.1.children(), &[node_id("volume")]);
        let volume = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id("volume"))
            .unwrap();
        assert_eq!(volume.1.numeric_value(), Some(3.));
        assert!(volume.1.supports_action(Action::Increment));
        let bounds = volume.1.bounds().unwrap();
        assert_eq!((bounds.x0, bounds.y1), (1., 42.));
        let request = ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: node_id("volume"),
            data: Some(ActionData::NumericValue(6.)),
        };
        assert_eq!(
            command(&request, &ids),
            Some(AccessCommand::SetValue(
                "volume".into(),
                AccessValue::Number(6.)
            ))
        );
        let unknown = ActionRequest {
            target_node: NodeId(42),
            ..request
        };
        assert_eq!(command(&unknown, &ids), None);
    }
    #[test]
    fn disabled_controls_offer_no_actions_and_ids_are_stable() {
        let mut save = node("save", AccessRole::Button, None);
        save.enabled = false;
        let (update, _) = tree_update(&[save], "App", [10., 10.]);
        assert!(update.nodes[1].1.is_disabled());
        assert!(!update.nodes[1].1.supports_action(Action::Click));
        assert_eq!(node_id("save"), node_id("save"));
        assert_ne!(node_id("save"), node_id("cancel"));
        assert_ne!(node_id(""), ROOT);
    }
    #[test]
    fn composite_names_states_and_scroll_actions_map_to_accesskit() {
        let mut scroll = node("actions", AccessRole::ScrollView, None);
        scroll.scroll = Some([0., 32., 0., 400.]);
        let mut tab = node("details", AccessRole::Tab, Some(0));
        tab.selected = Some(true);
        let mut input = node("name", AccessRole::TextInput, None);
        input.label = "Profile name".into();
        input.placeholder = Some("Your full name".into());
        let mut branch = node("src", AccessRole::TreeItem, None);
        branch.expanded = Some(false);
        let (tree, ids) = tree_update(&[scroll, tab, input, branch], "Catalog", [800., 600.]);
        let find = |key: &str| {
            &tree
                .nodes
                .iter()
                .find(|(id, _)| *id == node_id(key))
                .unwrap()
                .1
        };
        assert_eq!(find("details").role(), Role::Tab);
        assert_eq!(find("details").is_selected(), Some(true));
        assert_eq!(find("src").is_expanded(), Some(false));
        assert_eq!(find("name").label(), Some("Profile name"));
        assert_eq!(find("name").placeholder(), Some("Your full name"));
        assert_eq!(find("actions").scroll_y_max(), Some(400.));
        assert!(find("actions").supports_action(Action::ScrollDown));
        assert!(!find("actions").supports_action(Action::ScrollRight));
        for (action, expected) in [
            (
                Action::ScrollDown,
                AccessCommand::Scroll("actions".into(), [0, 1]),
            ),
            (
                Action::ScrollUp,
                AccessCommand::Scroll("actions".into(), [0, -1]),
            ),
            (
                Action::ScrollLeft,
                AccessCommand::Scroll("actions".into(), [-1, 0]),
            ),
            (
                Action::ScrollRight,
                AccessCommand::Scroll("actions".into(), [1, 0]),
            ),
            (
                Action::ScrollIntoView,
                AccessCommand::Reveal("actions".into()),
            ),
        ] {
            assert_eq!(
                command(
                    &ActionRequest {
                        action,
                        target_tree: TreeId::ROOT,
                        target_node: node_id("actions"),
                        data: None
                    },
                    &ids
                ),
                Some(expected)
            );
        }
    }
    #[test]
    fn tables_expose_cells_sort_and_dimensions_without_inert_click_targets() {
        use crate::declarative::AccessTableInfo;
        let mut table = node("ledger", AccessRole::Table, None);
        table.table = Some(AccessTableInfo {
            rows: Some(3),
            columns: Some(2),
            ..AccessTableInfo::default()
        });
        let mut header = node("amount", AccessRole::ColumnHeader, Some(0));
        header.clickable = true;
        header.table = Some(AccessTableInfo {
            row: Some(0),
            column: Some(1),
            sort_ascending: Some(false),
            ..AccessTableInfo::default()
        });
        let row = node("headers", AccessRole::Row, Some(0));
        let mut cell = node("value", AccessRole::Cell, Some(2));
        cell.table = Some(AccessTableInfo {
            row: Some(1),
            column: Some(1),
            ..AccessTableInfo::default()
        });
        let (tree, _) = tree_update(&[table, header, row, cell], "Catalog", [800., 600.]);
        let get = |key: &str| {
            &tree
                .nodes
                .iter()
                .find(|(id, _)| *id == node_id(key))
                .unwrap()
                .1
        };
        assert_eq!(get("ledger").row_count(), Some(3));
        assert_eq!(get("ledger").column_count(), Some(2));
        assert_eq!(
            get("amount").sort_direction(),
            Some(accesskit::SortDirection::Descending)
        );
        assert!(get("amount").supports_action(Action::Click));
        assert!(!get("headers").supports_action(Action::Click));
        assert_eq!(get("value").role(), Role::Cell);
        assert_eq!(get("value").column_index(), Some(1));
        assert!(!get("value").supports_action(Action::Focus));
    }
}
